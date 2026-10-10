//! The transform stack of a command stream and the painted extent of a
//! command range under it.

use crate::ir::{SceneCommand, path_segments_bbox};
use crate::layout::LayoutBox;

use super::super::text::{ShapeEnv, ink_bounds};
use super::affine::Affine2;

/// The local transform a push command opens, or `None` for any other
/// command.
fn of_push(cmd: &SceneCommand) -> Option<Affine2> {
    match cmd {
        SceneCommand::PushTransform { angle_deg, cx, cy } => {
            Some(Affine2::rotate_at(*angle_deg, *cx, *cy))
        }
        SceneCommand::PushScaleTranslate { sx, sy, tx, ty } => Some(Affine2 {
            a: *sx,
            b: 0.0,
            c: 0.0,
            d: *sy,
            e: *tx,
            f: *ty,
        }),
        SceneCommand::PushTransformMatrix { a, b, c, d, e, f } => Some(Affine2 {
            a: *a,
            b: *b,
            c: *c,
            d: *d,
            e: *e,
            f: *f,
        }),
        SceneCommand::FillRect { .. }
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
        | SceneCommand::DrawGlyphRun { .. }
        | SceneCommand::PushClip { .. }
        | SceneCommand::PushClipRoundedRect { .. }
        | SceneCommand::PopClip
        | SceneCommand::PushLayer { .. }
        | SceneCommand::PopLayer
        | SceneCommand::PopTransform
        | SceneCommand::BeginShadow { .. }
        | SceneCommand::EndShadow
        | SceneCommand::BeginBlur { .. }
        | SceneCommand::EndBlur
        | SceneCommand::BeginFilter { .. }
        | SceneCommand::EndFilter
        | SceneCommand::BeginMask { .. }
        | SceneCommand::EndMask => None,
    }
}

/// A transform stack driven by push / pop commands.
#[derive(Clone, Debug)]
pub(super) struct Stack {
    open: Vec<Affine2>,
}

impl Stack {
    pub(super) fn new(base: Affine2) -> Self {
        Self { open: vec![base] }
    }

    pub(super) fn top(&self) -> Affine2 {
        self.open.last().copied().unwrap_or(Affine2::IDENTITY)
    }

    /// Open a level that keeps the current transform: a push the caller
    /// leaves out.
    pub(super) fn hold(&mut self) {
        self.open.push(self.top());
    }

    /// Track `cmd`. With `skip_rotation`, a rotation opens as identity.
    pub(super) fn step(&mut self, cmd: &SceneCommand, skip_rotation: bool) {
        if matches!(cmd, SceneCommand::PopTransform) {
            if self.open.len() > 1 {
                self.open.pop();
            }
            return;
        }
        if let Some(local) = of_push(cmd) {
            let rotation = matches!(cmd, SceneCommand::PushTransform { .. });
            let next = if rotation && skip_rotation {
                self.top()
            } else {
                self.top().then(local)
            };
            self.open.push(next);
        }
    }
}

/// The points of the first `StrokePolyline` in `commands`, every transform
/// open under `base` applied. A connector draws its routed path as one.
pub(super) fn first_polyline(commands: &[SceneCommand], base: Affine2) -> Option<Vec<(f64, f64)>> {
    let mut stack = Stack::new(base);
    for cmd in commands {
        if let SceneCommand::StrokePolyline { points, .. } = cmd {
            let m = stack.top();
            return Some(
                points
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|[x, y]| m.apply(*x, *y))
                    .collect(),
            );
        }
        stack.step(cmd, false);
    }
    None
}

/// Axis-aligned extent accumulator.
#[derive(Clone, Copy)]
struct Extent {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
}

#[derive(Default)]
struct Acc(Option<Extent>);

impl Acc {
    fn point(&mut self, x: f64, y: f64, grow: f64) {
        if !(x.is_finite() && y.is_finite() && grow.is_finite()) {
            return;
        }
        let e = Extent {
            left: x - grow,
            top: y - grow,
            right: x + grow,
            bottom: y + grow,
        };
        self.0 = Some(match self.0 {
            Some(a) => Extent {
                left: a.left.min(e.left),
                top: a.top.min(e.top),
                right: a.right.max(e.right),
                bottom: a.bottom.max(e.bottom),
            },
            None => e,
        });
    }

    /// The four corners of `[x0, x1] × [y0, y1]` under `m`.
    fn rect(&mut self, m: Affine2, (x0, y0, x1, y1): (f64, f64, f64, f64)) {
        for (x, y) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
            let (px, py) = m.apply(x, y);
            self.point(px, py, 0.0);
        }
    }

    /// A flat `[x0, y0, x1, y1, …]` point list under `m`. Each point grows
    /// by `grow` in local space (a stroke half-width), so the growth scales
    /// with `m` like the stroke the backends draw.
    fn points(&mut self, m: Affine2, points: &[f64], grow: f64) {
        for [x, y] in points.as_chunks::<2>().0 {
            self.rect(m, (x - grow, y - grow, x + grow, y + grow));
        }
    }

    fn into_box(self) -> Option<LayoutBox> {
        let e = self.0?;
        Some(LayoutBox {
            x: e.left,
            y: e.top,
            w: e.right - e.left,
            h: e.bottom - e.top,
        })
    }
}

/// The axis-aligned box of `b` under `m`.
pub(super) fn map_box(m: Affine2, b: LayoutBox) -> LayoutBox {
    let mut acc = Acc::default();
    acc.rect(m, (b.x, b.y, b.x + b.w, b.y + b.h));
    acc.into_box().unwrap_or(b)
}

/// Where one [`measure`] walk starts.
#[derive(Clone, Copy)]
pub(super) struct Bases {
    /// The transform open where the commands start.
    pub(super) open: Affine2,
    /// The base of the `local` extent.
    pub(super) local: Affine2,
    /// The first command opens the node's own rotation, which `local` leaves
    /// out.
    pub(super) skip_spin: bool,
}

/// The painted extents of one command range: shapes, strokes (grown by half
/// their width), images, and glyph ink. Clips and effect brackets add
/// nothing. Each is `None` when nothing paints.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Extents {
    /// Under `Bases::open`, every rotation opened inside the range left out.
    pub(super) unrotated: Option<LayoutBox>,
    /// Under `Bases::local`, only the node's own rotation left out.
    pub(super) local: Option<LayoutBox>,
    /// Under `Bases::open`, every transform applied.
    pub(super) visual: Option<LayoutBox>,
}

/// One extent being measured: its transform stack and its accumulator.
struct Lane {
    stack: Stack,
    acc: Acc,
}

impl Lane {
    fn new(base: Affine2) -> Self {
        Self {
            stack: Stack::new(base),
            acc: Acc::default(),
        }
    }
}

/// The three lanes of [`measure`]: unrotated, local, visual.
struct Lanes([Lane; 3]);

impl Lanes {
    fn rect(&mut self, r: (f64, f64, f64, f64)) {
        for lane in &mut self.0 {
            let m = lane.stack.top();
            lane.acc.rect(m, r);
        }
    }

    fn points(&mut self, points: &[f64], grow: f64) {
        for lane in &mut self.0 {
            let m = lane.stack.top();
            lane.acc.points(m, points, grow);
        }
    }
}

/// The painted extents of `commands` from `bases`, in one walk. Each glyph
/// run's ink is read once.
pub(super) fn measure(commands: &[SceneCommand], bases: Bases, shape: ShapeEnv<'_>) -> Extents {
    let mut lanes = Lanes([
        Lane::new(bases.open),
        Lane::new(bases.local),
        Lane::new(bases.open),
    ]);
    for (index, cmd) in commands.iter().enumerate() {
        match cmd {
            SceneCommand::FillRect { x, y, w, h, .. }
            | SceneCommand::FillRoundedRect { x, y, w, h, .. }
            | SceneCommand::FillEllipse { x, y, w, h, .. }
            | SceneCommand::DrawImage { x, y, w, h, .. }
            | SceneCommand::DrawSvgAsset { x, y, w, h, .. } => {
                lanes.rect((*x, *y, x + w, y + h));
            }
            SceneCommand::StrokeRect {
                x,
                y,
                w,
                h,
                stroke_width,
                ..
            }
            | SceneCommand::StrokeRoundedRect {
                x,
                y,
                w,
                h,
                stroke_width,
                ..
            }
            | SceneCommand::StrokeEllipse {
                x,
                y,
                w,
                h,
                stroke_width,
                ..
            } => {
                let g = stroke_width / 2.0;
                lanes.rect((x - g, y - g, x + w + g, y + h + g));
            }
            SceneCommand::StrokeLine {
                x1,
                y1,
                x2,
                y2,
                stroke_width,
                ..
            } => lanes.points(&[*x1, *y1, *x2, *y2], stroke_width / 2.0),
            SceneCommand::FillPolygon { points, .. } => lanes.points(points, 0.0),
            SceneCommand::StrokePolyline {
                points,
                stroke_width,
                ..
            } => lanes.points(points, stroke_width / 2.0),
            SceneCommand::FillPath { segments, .. } => {
                if let Some((x, y, w, h)) = path_segments_bbox(segments) {
                    lanes.rect((x, y, x + w, y + h));
                }
            }
            SceneCommand::StrokePath {
                segments,
                stroke_width,
                ..
            } => {
                if let Some((x, y, w, h)) = path_segments_bbox(segments) {
                    let g = stroke_width / 2.0;
                    lanes.rect((x - g, y - g, x + w + g, y + h + g));
                }
            }
            SceneCommand::DrawGlyphRun { .. } => {
                if let Some(ink) = ink_bounds(std::slice::from_ref(cmd), shape) {
                    lanes.rect((ink.left, ink.top, ink.right, ink.bottom));
                }
            }
            SceneCommand::PushTransform { .. }
            | SceneCommand::PushScaleTranslate { .. }
            | SceneCommand::PushTransformMatrix { .. }
            | SceneCommand::PopTransform => {
                let [unrotated, local, visual] = &mut lanes.0;
                unrotated.stack.step(cmd, true);
                if index == 0 && bases.skip_spin {
                    local.stack.hold();
                } else {
                    local.stack.step(cmd, false);
                }
                visual.stack.step(cmd, false);
            }
            SceneCommand::PushClip { .. }
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
    let [unrotated, local, visual] = lanes.0;
    Extents {
        unrotated: unrotated.acc.into_box(),
        local: local.acc.into_box(),
        visual: visual.acc.into_box(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stroke_growth_scales_with_the_enclosing_transform() {
        let fonts = zenith_core::default_provider();
        let store = zenith_layout::FontFaceStore::new(&fonts);
        let engine = zenith_layout::RustybuzzEngine::new(&store);
        let shape = ShapeEnv {
            engine: &engine,
            fonts: &fonts,
        };
        let stroke = |cmd: SceneCommand| {
            measure(
                &[
                    SceneCommand::PushScaleTranslate {
                        sx: 2.0,
                        sy: 2.0,
                        tx: 0.0,
                        ty: 0.0,
                    },
                    cmd,
                    SceneCommand::PopTransform,
                ],
                Bases {
                    open: Affine2::IDENTITY,
                    local: Affine2::IDENTITY,
                    skip_spin: false,
                },
                shape,
            )
            .visual
        };
        let color = crate::ir::Color::srgb(0, 0, 0, 255);
        // A 2px line under scale 2 draws 4px wide: 2px each side.
        let line = stroke(SceneCommand::StrokeLine {
            x1: 10.0,
            y1: 10.0,
            x2: 20.0,
            y2: 10.0,
            color,
            stroke_width: 2.0,
            stroke_dash: None,
            stroke_gap: None,
            stroke_linecap: None,
        });
        assert_eq!(
            line,
            Some(LayoutBox {
                x: 18.0,
                y: 18.0,
                w: 24.0,
                h: 4.0
            })
        );
        let rect = stroke(SceneCommand::StrokeRect {
            x: 10.0,
            y: 10.0,
            w: 10.0,
            h: 10.0,
            color,
            stroke_width: 2.0,
            stroke_dash: None,
            stroke_gap: None,
            stroke_linecap: None,
        });
        assert_eq!(
            rect,
            Some(LayoutBox {
                x: 18.0,
                y: 18.0,
                w: 24.0,
                h: 24.0
            })
        );
    }

    #[test]
    fn map_box_of_a_quarter_turn_swaps_the_extent() {
        let b = LayoutBox {
            x: 0.0,
            y: 0.0,
            w: 40.0,
            h: 20.0,
        };
        let m = Affine2::rotate_at(90.0, 20.0, 10.0);
        let r = map_box(m, b);
        assert!(
            (r.w - 20.0).abs() < 1e-9 && (r.h - 40.0).abs() < 1e-9,
            "{r:?}"
        );
        assert!(
            (r.x - 10.0).abs() < 1e-9 && (r.y + 10.0).abs() < 1e-9,
            "{r:?}"
        );
    }
}
