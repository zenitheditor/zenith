//! Raster image placement and SVG image dispatch.

use pdf_writer::Content;
use zenith_core::{AssetProvider, FontProvider};
use zenith_scene::{FitMode, ImageClip, SrcRect};

use super::draw::{finite, rect_ok};
use super::resources::{ALPHA_PREFIX, IMAGE_PREFIX, PageResources, name};
use crate::pdf::geometry::{ellipse_path, rounded_rect_path};
use crate::pdf::image::decode_for_pdf;

/// Borrowed fields and scalar values for one [`zenith_scene::SceneCommand::DrawImage`] emission.
#[derive(Clone, Copy)]
pub(super) struct ImageDraw<'a> {
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) w: f64,
    pub(super) h: f64,
    /// Stable asset id; resolved via `AssetProvider::by_id`.
    pub(super) asset_id: &'a str,
    /// How the image scales to fill the box.
    pub(super) fit: FitMode,
    /// Horizontal object-position anchor in `0.0..=100.0`.
    pub(super) pos_x: f64,
    /// Vertical object-position anchor in `0.0..=100.0`.
    pub(super) pos_y: f64,
    /// Effective opacity, `0.0..=1.0`.
    pub(super) opacity: f64,
    /// Optional non-rectangular clip shape inscribed in the box.
    pub(super) clip_shape: &'a Option<ImageClip>,
    /// Raster-only source crop. SVG assets ignore this field.
    pub(super) src_rect: Option<&'a SrcRect>,
    /// SVG-only style overrides.
    pub(super) svg_style: Option<zenith_scene::SvgStyle>,
}

pub(super) fn emit_image(
    content: &mut Content,
    res: &mut PageResources,
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
    draw: ImageDraw<'_>,
) {
    let ImageDraw {
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
    } = draw;
    if !rect_ok(x, y, w, h) {
        return;
    }
    let Some(asset) = assets.by_id(asset_id) else {
        return;
    };
    // Dispatch on asset kind. Raster images embed as a Flate XObject below; SVG
    // assets translate to native PDF vector operators (paths + shadings) via the
    // `svg` module — true vector output, not a rasterized embed. Font/Unknown
    // kinds are not drawable images.
    match asset.kind {
        zenith_core::AssetKind::Image => {}
        zenith_core::AssetKind::Svg => {
            crate::pdf::svg::emit_svg(
                content,
                res,
                fonts,
                &asset.bytes,
                crate::pdf::svg::SvgPlacement {
                    x,
                    y,
                    w,
                    h,
                    fit,
                    pos_x,
                    pos_y,
                    opacity,
                    clip_shape,
                    svg_style,
                },
            );
            return;
        }
        zenith_core::AssetKind::Font | zenith_core::AssetKind::Unknown(_) => return,
    }
    let Some(decoded) = decode_for_pdf(&asset.bytes, src_rect) else {
        return;
    };
    let (sw, sh) = (f64::from(decoded.width), f64::from(decoded.height));
    if !(sw > 0.0 && sh > 0.0) {
        return;
    }

    // Fit transform (sx, sy, tx, ty) in scene space — identical math to the
    // raster backend's DrawImage arm.
    let (sx, sy, tx, ty) = match fit {
        FitMode::Stretch => (w / sw, h / sh, x, y),
        FitMode::Contain => {
            let s = (w / sw).min(h / sh);
            let (rw, rh) = (sw * s, sh * s);
            (
                s,
                s,
                x + (w - rw) * pos_x / 100.0,
                y + (h - rh) * pos_y / 100.0,
            )
        }
        FitMode::Cover => {
            let s = (w / sw).max(h / sh);
            let (rw, rh) = (sw * s, sh * s);
            (
                s,
                s,
                x - (rw - w) * pos_x / 100.0,
                y - (rh - h) * pos_y / 100.0,
            )
        }
        FitMode::None => (
            1.0,
            1.0,
            x - (sw - w) * pos_x / 100.0,
            y - (sh - h) * pos_y / 100.0,
        ),
    };
    if !finite(sx) || !finite(sy) || !finite(tx) || !finite(ty) || sx <= 0.0 || sy <= 0.0 {
        return;
    }

    let id = res.images.len();
    res.images.push(decoded);

    content.save_state();

    // Opacity via an ExtGState (image opacity is a separate factor from any
    // color alpha). 1.0 needs no state.
    let op = (opacity as f32).clamp(0.0, 1.0);
    if op < 1.0 {
        let a = (op * 255.0).round().clamp(0.0, 255.0) as u8;
        let aidx = res.intern_alpha(a);
        content.set_parameters(name(ALPHA_PREFIX, aidx).as_name());
    }

    // Box clip (rect or inscribed shape). The compiler also pushes a PushClip
    // box around images, but re-asserting the box here is harmless and makes
    // the non-rectangular shape clip self-contained.
    match clip_shape {
        None => {
            content.rect(x as f32, y as f32, w as f32, h as f32);
            content.clip_nonzero();
            content.end_path();
        }
        Some(ImageClip::Ellipse) => {
            ellipse_path(content, x, y, w, h, None, None);
            content.clip_nonzero();
            content.end_path();
        }
        Some(ImageClip::RoundedRect { radius }) => {
            rounded_rect_path(content, x, y, w, h, [*radius; 4]);
            content.clip_nonzero();
            content.end_path();
        }
    }

    // An image XObject is a 1×1 unit square in its own space; place it by
    // mapping that unit square onto the fitted box. PDF images are y-up, so we
    // flip within the placement matrix: image row 0 (top) must land at the box
    // top (smaller scene-y). The CTM below maps unit (u, v) → scene point
    // (tx + u*sw*sx, ty + (1-v)*sh*sy), i.e. scale_y is negative with a +height
    // translate, all composed with the page's outer flip.
    let iw = (sw * sx) as f32;
    let ih = (sh * sy) as f32;
    content.transform([iw, 0.0, 0.0, -ih, tx as f32, ty as f32 + ih]);
    content.x_object(name(IMAGE_PREFIX, id).as_name());

    content.restore_state();
}
