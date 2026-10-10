//! Raster/SVG image draws: decode (or rasterize an SVG) the asset, apply any
//! source-rect crop, compute the fit transform, build the box / shape clip
//! mask, then composite onto `target`. Pulls fields from
//! [`SceneCommand::DrawImage`] under the shared [`DrawCtx`]; byte-identical to
//! the prior arm. The SVG path lazily builds `svg_fontdb` at most once.

use resvg::usvg;
use resvg::usvg::TreeParsing;
use resvg::usvg::TreeTextToPath;
use tiny_skia::{FillRule, FilterQuality, Mask, PathBuilder, Pixmap, PixmapPaint, Rect, Transform};
use zenith_core::{AssetKind, AssetProvider};
use zenith_scene::{FitMode, ImageClip, SceneCommand};

use super::super::commands::DrawCtx;
use super::super::paths::build_rounded_rect_path;
use super::super::pixels::MAX_DIMENSION;
use super::super::raster::{crop_raster_image, decode_raster_image};
use super::super::surface::{draw_pixmap, mask_fill_path};

pub(in crate::tiny_skia) fn draw_image(
    target: &mut Pixmap,
    ctx: DrawCtx,
    cmd: &SceneCommand,
    assets: &dyn AssetProvider,
    fonts: &dyn zenith_core::FontProvider,
    svg_fontdb: &mut Option<resvg::usvg::fontdb::Database>,
) {
    let SceneCommand::DrawImage {
        x,
        y,
        w,
        h,
        asset_id,
        fit,
        pos_x,
        pos_y,
        opacity,
        clip_shape,
        src_rect,
        svg_style,
    } = cmd
    else {
        return;
    };
    let current_ts = ctx.current_ts;

    // ── a. Resolve bytes; only raster images are drawn ────────
    let Some(asset) = assets.by_id(asset_id) else {
        return; // unknown/missing asset: skip (no panic)
    };
    // ── b. Produce a raster Pixmap from Image (PNG) or Svg ────
    let src: Pixmap = match asset.kind {
        AssetKind::Image => {
            let Some(decoded) = decode_raster_image(&asset.bytes) else {
                return; // unsupported/malformed raster image: skip
            };
            let Some(cropped) = crop_raster_image(decoded, src_rect.as_ref()) else {
                return;
            };
            cropped
        }
        AssetKind::Svg => {
            // Build the fontdb at most once per render, only when
            // an SVG is drawn. Loaded from the registered faces in
            // deterministic BTreeMap (by_id) order — no system fonts.
            let fontdb: &resvg::usvg::fontdb::Database = svg_fontdb.get_or_insert_with(|| {
                let mut db = resvg::usvg::fontdb::Database::new();
                db.set_sans_serif_family("Noto Sans");
                db.set_serif_family("Noto Sans");
                db.set_monospace_family("Noto Sans Mono");
                for face in fonts.all_faces() {
                    db.load_font_data(face.bytes.to_vec());
                }
                db
            });
            // Set default font-family so unstyled SVG <text> resolves
            // to "Noto Sans" instead of the usvg default "Times New Roman".
            let opts = usvg::Options {
                font_family: "Noto Sans".to_owned(),
                ..Default::default()
            };
            let svg_bytes = crate::svg_style::styled_svg_bytes(&asset.bytes, *svg_style);
            let Ok(mut usvg_tree) = usvg::Tree::from_data(&svg_bytes, &opts) else {
                return; // malformed SVG: skip
            };
            usvg_tree.convert_text(fontdb);
            let sz = usvg_tree.size;
            let (svw, svh) = (f64::from(sz.width()), f64::from(sz.height()));
            if !(svw > 0.0 && svh > 0.0) {
                return;
            }
            // Rasterize at destination resolution so the
            // downstream bilinear scale is near 1:1 (crisp),
            // preserving the SVG's own aspect ratio.
            // A scaled render multiplies by the root output scale so the
            // intermediate matches device pixels (exact at scale 1).
            let raster_scale = svg_raster_scale((*w, *h), (svw, svh), ctx.device_scale);
            let pw = ((svw * raster_scale).ceil() as u32).max(1);
            let ph = ((svh * raster_scale).ceil() as u32).max(1);
            let Some(mut pm) = Pixmap::new(pw, ph) else {
                return;
            };
            let resvg_tree = resvg::Tree::from_usvg(&usvg_tree);
            resvg_tree.render(
                Transform::from_scale(raster_scale as f32, raster_scale as f32),
                &mut pm.as_mut(),
            );
            pm
        }
        // Font or Unknown: not a drawable image; skip.
        AssetKind::Font | AssetKind::Unknown(_) => return,
    };
    let (sw, sh) = (f64::from(src.width()), f64::from(src.height()));
    if !(sw > 0.0 && sh > 0.0) {
        return;
    }

    // ── c. Compute the fit transform (sx, sy, tx, ty) ─────────
    // pos_x / pos_y are 0..=100 object-position anchors.
    let (sx, sy, tx, ty) = match fit {
        FitMode::Stretch => (w / sw, h / sh, *x, *y),
        FitMode::Contain => {
            let s = (w / sw).min(h / sh);
            let (rw, rh) = (sw * s, sh * s);
            let tx = x + (w - rw) * pos_x / 100.0;
            let ty = y + (h - rh) * pos_y / 100.0;
            (s, s, tx, ty)
        }
        FitMode::Cover => {
            let s = (w / sw).max(h / sh);
            let (rw, rh) = (sw * s, sh * s);
            let tx = x - (rw - w) * pos_x / 100.0;
            let ty = y - (rh - h) * pos_y / 100.0;
            (s, s, tx, ty)
        }
        FitMode::None => {
            let tx = x - (sw - w) * pos_x / 100.0;
            let ty = y - (sh - h) * pos_y / 100.0;
            (1.0, 1.0, tx, ty)
        }
    };
    if !sx.is_finite()
        || !sy.is_finite()
        || !tx.is_finite()
        || !ty.is_finite()
        || sx <= 0.0
        || sy <= 0.0
    {
        return;
    }

    // ── d. Build the clip Mask from the effective clip ────────
    // The compiler emits PushClip(box) before DrawImage, so
    // clip_stack.last() already equals the image box ∩ enclosing
    // clips (box-clip).  clip_mask() handles the full-pixmap
    // fast path (returns Some(None) → no mask allocation) and the
    // sub-page case (returns Some(Some(mask))).
    let mask = match ctx.clip_mask() {
        None => return, // clip fully off-canvas
        Some(m) => m,
    };

    // ── d2. Clip-to-shape (ellipse / rounded rect) ────────────
    // When the image carries a non-rectangular clip shape, build
    // a path Mask from the shape INSCRIBED in the device box and
    // use it in place of the box mask. The shape is a subset of
    // the box, so the shape mask alone enforces both the
    // box clip and the shape clip. AA-on path fill is
    // deterministic same-machine, consistent with FillEllipse.
    // `current_ts` is applied so a rotated image clips to the
    // rotated shape (identity case → unchanged geometry).
    // None / unset clip_shape leaves `mask` untouched → the
    // non-clipped path is byte-identical to before.
    let shape_mask: Option<Mask> = match clip_shape {
        None => None,
        Some(shape) => {
            let Some(rect) = Rect::from_xywh(*x as f32, *y as f32, *w as f32, *h as f32) else {
                return; // degenerate box: nothing to draw
            };
            let path = match shape {
                ImageClip::Ellipse => PathBuilder::from_oval(rect),
                ImageClip::RoundedRect { radius } => build_rounded_rect_path(
                    *x as f32,
                    *y as f32,
                    *w as f32,
                    *h as f32,
                    [*radius as f32; 4],
                ),
            };
            let Some(path) = path else {
                return; // degenerate path: nothing to draw
            };
            let Some(mut m) = Mask::new(ctx.surface.w, ctx.surface.h) else {
                return;
            };
            mask_fill_path(
                &mut m,
                ctx.surface,
                &path,
                FillRule::Winding,
                true,
                current_ts,
            );
            Some(ctx.restrict(m))
        }
    };
    // Prefer the shape mask when present; else the box mask.
    let mask: Option<&Mask> = match &shape_mask {
        Some(m) => Some(m),
        None => mask.as_ref(),
    };

    // ── e. Paint: opacity + bilinear filtering ────────────────
    let paint = PixmapPaint {
        opacity: (*opacity as f32).clamp(0.0, 1.0),
        quality: FilterQuality::Bilinear,
        ..Default::default()
    };

    // ── f. Scale + translate transform ────────────────────────
    // Compose the rotation transform stack on top of the fit
    // transform. For the identity case `current_ts.pre_concat(fit)`
    // == `fit`, so the unrotated output is byte-identical.
    let fit = Transform::from_row(sx as f32, 0.0, 0.0, sy as f32, tx as f32, ty as f32);
    let transform = current_ts.pre_concat(fit);

    // ── g. Composite. Box-clip is enforced by the Mask;
    // deterministic same-machine (pure-software bilinear). ─────
    draw_pixmap(target, ctx.surface, src.as_ref(), &paint, transform, mask);
}

/// The scale an SVG of intrinsic size `(svw, svh)` rasterizes at for a
/// destination box `(w, h)` at `device_scale`: near 1:1 with the device,
/// within 0.01..16, and never past the pixmap dimension limit (a larger
/// SVG rasterizes smaller and scales up downstream).
fn svg_raster_scale((w, h): (f64, f64), (svw, svh): (f64, f64), device_scale: f64) -> f64 {
    ((w / svw).max(h / svh) * device_scale)
        .clamp(0.01, 16.0)
        .min(f64::from(MAX_DIMENSION) / svw)
        .min(f64::from(MAX_DIMENSION) / svh)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_huge_svg_rasterizes_within_the_pixmap_limit() {
        let limit = f64::from(MAX_DIMENSION);
        let s = svg_raster_scale((100.0, 100.0), (1e7, 1e7), 1.0);
        assert!((1e7 * s).ceil() <= limit + 1.0, "{s}");
        // An ordinary SVG keeps the 1:1 device scale.
        assert_eq!(svg_raster_scale((200.0, 100.0), (100.0, 50.0), 1.0), 2.0);
    }
}
