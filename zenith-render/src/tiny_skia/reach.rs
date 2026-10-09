//! Effect reach: how far, in device pixels, an effect moves ink.
//!
//! A region render draws only its surface. An effect that reads ink outside
//! the surface (a blur, a shadow offset, a mask feather) is wrong near the
//! surface edge unless the surface extends past the region by that reach.
//! Nested effects add their reaches. The pad of a scene is the largest sum
//! along any nesting chain.

use zenith_scene::{Scene, SceneCommand};

use super::blur::blur_reach;

/// Reach of a blur with sigma `sigma` device pixels: the box-blur reach plus
/// the one-pixel margin the crop helpers add.
fn sigma_reach(sigma: f64) -> u32 {
    if sigma.is_finite() && sigma > 0.0 {
        blur_reach(sigma).saturating_add(1)
    } else {
        0
    }
}

/// Whole device pixels covered by an offset of `v` page pixels at `scale`.
fn offset_reach(v: f64, scale: f64) -> u32 {
    let px = (v * scale).abs().ceil();
    if px.is_finite() && px < f64::from(u32::MAX) {
        px as u32
    } else {
        u32::MAX
    }
}

/// Device-pixel pad a region render of `scene` at `scale` needs so every
/// region pixel sees the same effect input as the full render.
pub(super) fn effect_pad(scene: &Scene, scale: f64) -> u32 {
    // Running reach of each open effect bracket, innermost last.
    let mut open: Vec<u32> = Vec::new();
    let mut total: u32 = 0;
    let mut pad: u32 = 0;
    for cmd in &scene.commands {
        let opened = match cmd {
            SceneCommand::BeginBlur { radius } => Some(sigma_reach(radius * scale)),
            SceneCommand::BeginShadow { shadows } => Some(
                shadows
                    .iter()
                    .map(|s| {
                        offset_reach(s.dx, scale)
                            .max(offset_reach(s.dy, scale))
                            .saturating_add(sigma_reach(s.blur * scale))
                    })
                    .max()
                    .unwrap_or(0),
            ),
            SceneCommand::BeginMask { mask } => Some(sigma_reach(mask.feather * scale)),
            SceneCommand::BeginFilter { .. } => Some(0),
            SceneCommand::EndBlur
            | SceneCommand::EndShadow
            | SceneCommand::EndMask
            | SceneCommand::EndFilter => {
                if let Some(r) = open.pop() {
                    total = total.saturating_sub(r);
                }
                None
            }
            SceneCommand::PushClip { .. }
            | SceneCommand::PushClipRoundedRect { .. }
            | SceneCommand::PopClip
            | SceneCommand::PushLayer { .. }
            | SceneCommand::PopLayer
            | SceneCommand::PushTransform { .. }
            | SceneCommand::PushScaleTranslate { .. }
            | SceneCommand::PushTransformMatrix { .. }
            | SceneCommand::PopTransform
            | SceneCommand::FillRect { .. }
            | SceneCommand::StrokeRect { .. }
            | SceneCommand::FillRoundedRect { .. }
            | SceneCommand::StrokeRoundedRect { .. }
            | SceneCommand::FillEllipse { .. }
            | SceneCommand::StrokeEllipse { .. }
            | SceneCommand::StrokeLine { .. }
            | SceneCommand::FillPolygon { .. }
            | SceneCommand::StrokePolyline { .. }
            | SceneCommand::FillPath { .. }
            | SceneCommand::StrokePath { .. }
            | SceneCommand::DrawImage { .. }
            | SceneCommand::DrawSvgAsset { .. }
            | SceneCommand::DrawGlyphRun { .. } => None,
        };
        if let Some(r) = opened {
            open.push(r);
            total = total.saturating_add(r);
            pad = pad.max(total);
        }
    }
    pad
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_scene::{Color, ShadowSpec};

    fn scene(commands: Vec<SceneCommand>) -> Scene {
        let mut s = Scene::new(100.0, 100.0);
        s.commands = commands;
        s
    }

    #[test]
    fn plain_scene_needs_no_pad() {
        assert_eq!(effect_pad(&scene(vec![]), 2.0), 0);
    }

    #[test]
    fn nested_effects_add_and_siblings_take_the_max() {
        let shadow = SceneCommand::BeginShadow {
            shadows: vec![ShadowSpec {
                dx: 3.0,
                dy: -7.5,
                blur: 0.0,
                color: Color {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 255,
                    cmyk: None,
                },
            }],
        };
        let blur = SceneCommand::BeginBlur { radius: 2.0 };
        let nested = scene(vec![
            shadow.clone(),
            blur.clone(),
            SceneCommand::EndBlur,
            SceneCommand::EndShadow,
        ]);
        let siblings = scene(vec![
            shadow,
            SceneCommand::EndShadow,
            blur,
            SceneCommand::EndBlur,
        ]);
        let blur_alone = sigma_reach(4.0);
        assert_eq!(effect_pad(&nested, 2.0), 15 + blur_alone);
        assert_eq!(effect_pad(&siblings, 2.0), 15.max(blur_alone));
    }
}
