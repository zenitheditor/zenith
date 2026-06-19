//! Image leaf-node compilation.

use std::collections::BTreeMap;

use zenith_core::{Diagnostic, ImageNode, ObjectPosition, ResolvedToken, dim_to_px};

use crate::ir::{FitMode, SceneCommand};

use super::RenderCtx;
use super::paint::resolve_property_shadow;
use super::util::{rotation_degrees, unsupported_unit_diag};

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
    let opacity = image.opacity.unwrap_or(1.0).clamp(0.0, 1.0) * ctx.opacity;

    // Map fit string → FitMode. Default (absent or unknown) = Stretch.
    let fit = match image.fit.as_deref() {
        Some("contain") => FitMode::Contain,
        Some("cover") => FitMode::Cover,
        Some("none") => FitMode::None,
        _ => FitMode::Stretch,
    };

    let pos_x = object_pos_to_f64(&image.object_position_x);
    let pos_y = object_pos_to_f64(&image.object_position_y);

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

    // SHADOW bracket (behind the image ink, inside the rotation). Opened only
    // here, where the DrawImage below is guaranteed to follow.
    let has_shadow = match image
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
    });
    commands.push(SceneCommand::PopClip);

    if has_shadow {
        commands.push(SceneCommand::EndShadow);
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
