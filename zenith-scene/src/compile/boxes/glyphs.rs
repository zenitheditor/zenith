//! Per-glyph ink of every attributed glyph run in a page command stream.

use std::collections::BTreeMap;

use zenith_layout::{GlyphInkBox, TextLayoutEngine};

use crate::ir::SceneCommand;
use crate::layout::LayoutBox;

use super::super::text::ShapeEnv;
use super::bounds::{Affine, Stack, map_box};

/// The drawn glyph ink of one source node, in page px.
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::compile) struct TextInk {
    /// Each glyph's ink box with every transform applied (axis-aligned
    /// bounds of the transformed box).
    pub(in crate::compile) glyphs: Vec<LayoutBox>,
    /// Each glyph's ink box with rotations left out (scale and translation
    /// kept), in the same order as `glyphs`.
    pub(in crate::compile) local: Vec<LayoutBox>,
    /// Largest fill alpha of the node's runs (node and cascaded opacity
    /// included).
    pub(in crate::compile) alpha: u8,
    /// Largest run font size, in px.
    pub(in crate::compile) font_size: f64,
    /// Every run draws under a transform that keeps boxes axis-aligned.
    pub(in crate::compile) axis_aligned: bool,
    /// The baseline y of the first run, in page px.
    pub(in crate::compile) baseline: Option<f64>,
    /// The ink of each drawn line: the glyph boxes of the runs that share a
    /// transform and a baseline, in run space, with the map to page px.
    pub(in crate::compile) lines: Vec<RunLine>,
}

/// The run-space ink box of one drawn line and the map to page px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::compile) struct RunLine {
    /// Run-space ink box (glyph boxes unioned).
    pub(in crate::compile) rect: LayoutBox,
    /// The pen baseline y of the line's runs, in run space.
    pub(in crate::compile) baseline: f64,
    /// Run space to page px: `[a, b, c, d, e, f]`.
    pub(in crate::compile) matrix: [f64; 6],
}

impl RunLine {
    /// Grow the line box to hold `rect`.
    fn add(&mut self, rect: LayoutBox) {
        let left = self.rect.x.min(rect.x);
        let top = self.rect.y.min(rect.y);
        let right = (self.rect.x + self.rect.w).max(rect.x + rect.w);
        let bottom = (self.rect.y + self.rect.h).max(rect.y + rect.h);
        self.rect = LayoutBox {
            x: left,
            y: top,
            w: right - left,
            h: bottom - top,
        };
    }
}

/// The glyph ink of every glyph run in `commands` that names its source
/// node, grouped by `source_node_id`. `origin` is the scene position of page
/// `(0, 0)` (the bleed offset). Glyphs with no ink (spaces) are left out.
pub(in crate::compile) fn glyph_inks(
    commands: &[SceneCommand],
    origin: (f64, f64),
    shape: ShapeEnv<'_>,
) -> BTreeMap<String, TextInk> {
    let mut full = Stack::new(Affine::IDENTITY);
    let mut local = Stack::new(Affine::IDENTITY);
    let mut out: BTreeMap<String, TextInk> = BTreeMap::new();
    // Glyph boxes repeat across runs: look each one up once.
    let mut memo: BTreeMap<(&str, u16, u32), Option<GlyphInkBox>> = BTreeMap::new();
    let page = |b: LayoutBox| LayoutBox {
        x: b.x - origin.0,
        y: b.y - origin.1,
        ..b
    };
    for cmd in commands {
        match cmd {
            SceneCommand::PushTransform { .. }
            | SceneCommand::PushScaleTranslate { .. }
            | SceneCommand::PushTransformMatrix { .. }
            | SceneCommand::PopTransform => {
                full.step(cmd, false);
                local.step(cmd, true);
            }
            SceneCommand::DrawGlyphRun {
                x,
                y,
                font_id,
                font_size,
                color,
                stroke_color,
                stroke_width,
                source_node_id: Some(source),
                glyphs,
                ..
            } => {
                let m = full.top();
                let ml = local.top();
                let grow = match (stroke_color, stroke_width) {
                    (Some(_), Some(w)) if *w > 0.0 => w / 2.0,
                    _ => 0.0,
                };
                let ink = out.entry(source.clone()).or_insert_with(|| TextInk {
                    axis_aligned: true,
                    ..TextInk::default()
                });
                ink.alpha = ink.alpha.max(color.a);
                ink.font_size = ink.font_size.max(f64::from(*font_size));
                ink.axis_aligned &= m.is_axis_aligned();
                if ink.baseline.is_none() {
                    ink.baseline = Some(m.apply(*x, *y).1 - origin.1);
                }
                let [a, b, c, d, e, f] = m.coefficients();
                let matrix = [a, b, c, d, e - origin.0, f - origin.1];
                // Runs of one line share the transform and the baseline.
                let mut line = ink
                    .lines
                    .iter()
                    .position(|l| l.matrix == matrix && l.baseline == *y);
                for g in glyphs {
                    let key = (font_id.as_str(), g.glyph_id, font_size.to_bits());
                    let Some(b) = *memo.entry(key).or_insert_with(|| {
                        shape
                            .engine
                            .glyph_ink_box(font_id, g.glyph_id, *font_size, shape.fonts)
                    }) else {
                        continue;
                    };
                    let (gx, gy) = (x + f64::from(g.dx), y + f64::from(g.dy));
                    let rect = LayoutBox {
                        x: gx + f64::from(b.x_min) - grow,
                        y: gy + f64::from(b.y_min) - grow,
                        w: f64::from(b.x_max - b.x_min) + 2.0 * grow,
                        h: f64::from(b.y_max - b.y_min) + 2.0 * grow,
                    };
                    if !(rect.w > 0.0 && rect.h > 0.0) {
                        continue;
                    }
                    ink.glyphs.push(page(map_box(m, rect)));
                    ink.local.push(page(map_box(ml, rect)));
                    match line.and_then(|i| ink.lines.get_mut(i)) {
                        Some(l) => l.add(rect),
                        None => {
                            line = Some(ink.lines.len());
                            ink.lines.push(RunLine {
                                rect,
                                baseline: *y,
                                matrix,
                            });
                        }
                    }
                }
            }
            SceneCommand::DrawGlyphRun {
                source_node_id: None,
                ..
            }
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
            | SceneCommand::PushClip { .. }
            | SceneCommand::PushClipRoundedRect { .. }
            | SceneCommand::PopClip
            | SceneCommand::PushLayer { .. }
            | SceneCommand::PopLayer
            | SceneCommand::BeginShadow { .. }
            | SceneCommand::EndShadow
            | SceneCommand::BeginBlur { .. }
            | SceneCommand::EndBlur
            | SceneCommand::BeginFilter { .. }
            | SceneCommand::EndFilter
            | SceneCommand::BeginMask { .. }
            | SceneCommand::EndMask => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Color, SceneGlyph};

    fn run(source: Option<&str>) -> SceneCommand {
        SceneCommand::DrawGlyphRun {
            x: 10.0,
            y: 50.0,
            font_id: "noto-sans-400-normal".to_owned(),
            font_size: 20.0,
            color: Color::srgb(0, 0, 0, 255),
            stroke_color: None,
            stroke_width: None,
            link: None,
            selectable: true,
            source_node_id: source.map(str::to_owned),
            glyphs: vec![SceneGlyph {
                glyph_id: 36,
                dx: 0.0,
                dy: 0.0,
                text: String::new(),
            }],
        }
    }

    #[test]
    fn groups_by_source_and_applies_transforms() {
        let fonts = zenith_core::default_provider();
        let store = zenith_layout::FontFaceStore::new(&fonts);
        let engine = zenith_layout::RustybuzzEngine::new(&store);
        let shape = ShapeEnv {
            engine: &engine,
            fonts: &fonts,
        };
        let plain = glyph_inks(&[run(Some("t")), run(None)], (0.0, 0.0), shape);
        assert_eq!(plain.len(), 1);
        let ink = plain.get("t").expect("t ink");
        assert_eq!(ink.glyphs.len(), 1);
        assert!(ink.axis_aligned);
        assert_eq!(ink.glyphs, ink.local);

        let moved = glyph_inks(
            &[
                SceneCommand::PushScaleTranslate {
                    sx: 1.0,
                    sy: 1.0,
                    tx: 5.0,
                    ty: 7.0,
                },
                run(Some("t")),
                SceneCommand::PopTransform,
            ],
            (2.0, 2.0),
            shape,
        );
        let a = plain
            .get("t")
            .expect("t")
            .glyphs
            .first()
            .copied()
            .expect("glyph");
        let b = moved
            .get("t")
            .expect("t")
            .glyphs
            .first()
            .copied()
            .expect("glyph");
        assert!((b.x - (a.x + 3.0)).abs() < 1e-9 && (b.y - (a.y + 5.0)).abs() < 1e-9);

        let spun = glyph_inks(
            &[
                SceneCommand::PushTransform {
                    angle_deg: 30.0,
                    cx: 0.0,
                    cy: 0.0,
                },
                run(Some("t")),
                SceneCommand::PopTransform,
            ],
            (0.0, 0.0),
            shape,
        );
        let ink = spun.get("t").expect("t");
        assert!(!ink.axis_aligned);
        assert_eq!(ink.local, plain.get("t").expect("t").local);
    }
}
