//! Image leaf-node compilation.

use std::collections::BTreeMap;

use zenith_core::{Diagnostic, ImageNode, ObjectPosition, ResolvedToken, dim_to_px};

use crate::ir::{FitMode, ImageClip, SceneCommand, SrcRect};

use super::RenderCtx;
use super::paint::resolve_property_shadow;
use super::util::{
    blend_mode_ir, resolve_property_dimension_px, rotation_degrees, unsupported_unit_diag,
};

/// Compile an `image` leaf node.
///
/// Mirrors the frame box-clip pattern: resolve geometry first (so early
/// returns stay push/pop balanced), then emit `PushClip(box)` → `DrawImage` →
/// `PopClip`. The box-clip is the normative image box-clip (doc 09 G-22): the
/// raster is ALWAYS clipped to its declared `[x, y, w, h]` box. `compile_node`
/// needs no asset provider here — the asset id string is enough; bytes are
/// resolved at render time.
pub(super) fn compile_image(
    image: &ImageNode,
    resolved: &BTreeMap<String, ResolvedToken>,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: RenderCtx,
) {
    // Skip invisible images.
    if image.visible == Some(false) {
        return;
    }

    // All four geometry dimensions are required. Resolve BEFORE PushClip so
    // any early return keeps push/pop balanced.
    let (Some(x_dim), Some(y_dim), Some(w_dim), Some(h_dim)) =
        (&image.x, &image.y, &image.w, &image.h)
    else {
        diagnostics.push(Diagnostic::advisory(
            "scene.missing_geometry",
            format!(
                "image '{}' is missing one or more geometry properties (x, y, w, h); skipped",
                image.id
            ),
            image.source_span,
            Some(image.id.clone()),
        ));
        return;
    };

    let Some(x_raw) = dim_to_px(x_dim.value, &x_dim.unit) else {
        diagnostics.push(unsupported_unit_diag(
            "image",
            &image.id,
            "x",
            image.source_span,
        ));
        return;
    };
    let Some(y_raw) = dim_to_px(y_dim.value, &y_dim.unit) else {
        diagnostics.push(unsupported_unit_diag(
            "image",
            &image.id,
            "y",
            image.source_span,
        ));
        return;
    };
    let Some(w) = dim_to_px(w_dim.value, &w_dim.unit) else {
        diagnostics.push(unsupported_unit_diag(
            "image",
            &image.id,
            "w",
            image.source_span,
        ));
        return;
    };
    let Some(h) = dim_to_px(h_dim.value, &h_dim.unit) else {
        diagnostics.push(unsupported_unit_diag(
            "image",
            &image.id,
            "h",
            image.source_span,
        ));
        return;
    };

    // Apply group translation offset.
    let x = x_raw + ctx.dx;
    let y = y_raw + ctx.dy;

    // Effective opacity: node opacity × cascaded ctx opacity.
    let full_opacity = image.opacity.unwrap_or(1.0).clamp(0.0, 1.0) * ctx.opacity;

    // Blend-mode layer (see compile_rect for the opacity-split rationale). With
    // a blend the layer carries `full_opacity` and the DrawImage paints at full
    // alpha; with no blend `opacity == full_opacity` → byte-identical.
    let blend = blend_mode_ir(image.blend_mode.as_deref());
    let (layer_op, opacity) = match blend {
        Some(_) => (full_opacity, 1.0),
        None => (1.0, full_opacity),
    };

    // Map fit string → FitMode. Default (absent or unknown) = Stretch.
    let fit = match image.fit.as_deref() {
        Some("contain") => FitMode::Contain,
        Some("cover") => FitMode::Cover,
        Some("none") => FitMode::None,
        _ => FitMode::Stretch,
    };

    let pos_x = object_pos_to_f64(&image.object_position_x);
    let pos_y = object_pos_to_f64(&image.object_position_y);

    // Resolve the clip-to-shape mode. `"ellipse"`/`"circle"` → the inscribed
    // ellipse; `"rounded"` → a rounded rect using `clip-radius` (resolved to px
    // exactly like rect `radius`, default 0.0). `"rect"`/absent/unknown → None,
    // i.e. the default rectangular box-clip (byte-identical to before).
    let clip_shape = match image.clip.as_deref() {
        Some("ellipse") | Some("circle") => Some(ImageClip::Ellipse),
        Some("rounded") => {
            let radius = resolve_property_dimension_px(&image.clip_radius, resolved, 0.0);
            Some(ImageClip::RoundedRect { radius })
        }
        _ => None,
    };

    // Rotation bracket (outermost — wraps the box-clip). Unrotated images
    // emit no PushTransform → byte-identical to before.
    let rot = rotation_degrees(image.rotate.as_ref());
    if let Some(angle) = rot {
        let cx = x + w / 2.0;
        let cy = y + h / 2.0;
        commands.push(SceneCommand::PushTransform {
            angle_deg: angle,
            cx,
            cy,
        });
    }

    // BLEND-MODE layer bracket (inside the rotation, outside the shadow).
    if let Some(blend_mode) = blend {
        commands.push(SceneCommand::PushLayer {
            opacity: layer_op,
            blend_mode: Some(blend_mode),
        });
    }

    // BLUR / SHADOW bracket (behind the image ink). Blur wins over shadow.
    let blur_sigma = image
        .blur
        .as_ref()
        .and_then(|d| dim_to_px(d.value, &d.unit))
        .filter(|&s| s > 0.0);
    let has_blur = blur_sigma.is_some();
    if let Some(sigma) = blur_sigma {
        commands.push(SceneCommand::BeginBlur { radius: sigma });
    }
    let has_shadow = !has_blur
        && match image
            .shadow
            .as_ref()
            .and_then(|p| resolve_property_shadow(p, resolved, &image.id))
        {
            Some(shadows) => {
                commands.push(SceneCommand::BeginShadow { shadows });
                true
            }
            None => false,
        };

    // Resolve the optional source sub-rectangle. All four dimensions must
    // resolve to px; if any present-but-unresolvable unit is encountered, push
    // a diagnostic and produce None (fall back to full-image draw).
    let src_rect: Option<SrcRect> = match (&image.src_x, &image.src_y, &image.src_w, &image.src_h) {
        (Some(sx), Some(sy), Some(sw), Some(sh)) => {
            let rx = dim_to_px(sx.value, &sx.unit);
            let ry = dim_to_px(sy.value, &sy.unit);
            let rw = dim_to_px(sw.value, &sw.unit);
            let rh = dim_to_px(sh.value, &sh.unit);
            match (rx, ry, rw, rh) {
                (Some(x0), Some(y0), Some(w0), Some(h0)) => Some(SrcRect {
                    x: x0,
                    y: y0,
                    w: w0,
                    h: h0,
                }),
                _ => {
                    // At least one dimension has an unresolvable unit.
                    diagnostics.push(unsupported_unit_diag(
                        "image",
                        &image.id,
                        "src-x/src-y/src-w/src-h",
                        image.source_span,
                    ));
                    None
                }
            }
        }
        // Partial presence is a validation error already; here we just produce None.
        _ => None,
    };

    // Box-clip (G-22): push the box, draw the image, pop. The image is always
    // clipped to its declared box ∩ enclosing clips.
    commands.push(SceneCommand::PushClip { x, y, w, h });
    commands.push(SceneCommand::DrawImage {
        x,
        y,
        w,
        h,
        asset_id: image.asset.clone(),
        fit,
        pos_x,
        pos_y,
        opacity,
        clip_shape,
        src_rect,
    });
    commands.push(SceneCommand::PopClip);

    if has_shadow {
        commands.push(SceneCommand::EndShadow);
    }
    if has_blur {
        commands.push(SceneCommand::EndBlur);
    }

    if blend.is_some() {
        commands.push(SceneCommand::PopLayer);
    }

    if rot.is_some() {
        commands.push(SceneCommand::PopTransform);
    }
}

/// Resolve an object-position anchor to `0.0..=100.0`.
///
/// `None` defaults to `50.0` (centered); `Start`→0, `Center`→50, `End`→100,
/// `Pct(n)`→`n` clamped to `0..=100`.
fn object_pos_to_f64(pos: &Option<ObjectPosition>) -> f64 {
    match pos {
        None => 50.0,
        Some(ObjectPosition::Start) => 0.0,
        Some(ObjectPosition::Center) => 50.0,
        Some(ObjectPosition::End) => 100.0,
        Some(ObjectPosition::Pct(n)) => n.clamp(0.0, 100.0),
    }
}
