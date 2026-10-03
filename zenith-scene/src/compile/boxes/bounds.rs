//! The transform stack of a command stream and the painted extent of a
//! command range under it.

use crate::ir::{SceneCommand, path_segments_bbox};
use crate::layout::LayoutBox;

use super::super::text::{ShapeEnv, ink_bounds};

/// A 2-D affine map: `x' = a·x + c·y + e`, `y' = b·x + d·y + f` (the
/// render backends' `PushTransformMatrix` convention).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Affine {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}

impl Affine {
    pub(super) const IDENTITY: Affine = Affine {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    /// Rotation by `deg` about `(cx, cy)` (`PushTransform`).
    pub(super) fn rotate_at(deg: f64, cx: f64, cy: f64) -> Affine {
        let (sin, cos) = deg.to_radians().sin_cos();
        Affine {
            a: cos,
            b: sin,
            c: -sin,
            d: cos,
            e: cx - cos * cx + sin * cy,
            f: cy - sin * cx - cos * cy,
        }
    }

    /// `self ∘ local`: `local` maps a point first, then `self` (the
    /// backends' `pre_concat`).
    pub(super) fn then(self, l: Affine) -> Affine {
        Affine {
            a: self.a * l.a + self.c * l.b,
            b: self.b * l.a + self.d * l.b,
            c: self.a * l.c + self.c * l.d,
            d: self.b * l.c + self.d * l.d,
            e: self.a * l.e + self.c * l.f + self.e,
            f: self.b * l.e + self.d * l.f + self.f,
        }
    }

    pub(super) fn apply(self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }

    /// `true` when the map keeps axis-aligned boxes axis-aligned: no
    /// rotation off a quarter turn, no shear.
    pub(super) fn is_axis_aligned(self) -> bool {
        const EPS: f64 = 1e-9;
        (self.b.abs() < EPS && self.c.abs() < EPS) || (self.a.abs() < EPS && self.d.abs() < EPS)
    }

    /// The local transform a push command opens, or `None` for any other
    /// command.
    fn of_push(cmd: &SceneCommand) -> Option<Affine> {
        match cmd {
            SceneCommand::PushTransform { angle_deg, cx, cy } => {
                Some(Affine::rotate_at(*angle_deg, *cx, *cy))
            }
            SceneCommand::PushScaleTranslate { sx, sy, tx, ty } => Some(Affine {
                a: *sx,
                b: 0.0,
                c: 0.0,
                d: *sy,
                e: *tx,
                f: *ty,
            }),
            SceneCommand::PushTransformMatrix { a, b, c, d, e, f } => Some(Affine {
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
}

/// A transform stack driven by push / pop commands.
pub(super) struct Stack {
    open: Vec<Affine>,
}

impl Stack {
    pub(super) fn new(base: Affine) -> Self {
        Self { open: vec![base] }
    }

    pub(super) fn top(&self) -> Affine {
        self.open.last().copied().unwrap_or(Affine::IDENTITY)
    }

    /// Track `cmd`. With `skip_rotation`, a rotation opens as identity.
    pub(super) fn step(&mut self, cmd: &SceneCommand, skip_rotation: bool) {
        if matches!(cmd, SceneCommand::PopTransform) {
            if self.open.len() > 1 {
                self.open.pop();
            }
            return;
        }
        if let Some(local) = Affine::of_push(cmd) {
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

/// The transform open at the end of `commands`, under `base`.
pub(super) fn open_transform(base: Affine, commands: &[SceneCommand]) -> Affine {
    let mut stack = Stack::new(base);
    for cmd in commands {
        stack.step(cmd, false);
    }
    stack.top()
}

/// The points of the first `StrokePolyline` in `commands`, every transform
/// open under `base` applied. A connector draws its routed path as one.
pub(super) fn first_polyline(commands: &[SceneCommand], base: Affine) -> Option<Vec<(f64, f64)>> {
    let mut stack = Stack::new(base);
    for cmd in commands {
        if let SceneCommand::StrokePolyline { points, .. } = cmd {
            let m = stack.top();
            return Some(
                points
                    .chunks_exact(2)
                    .filter_map(|pair| match pair {
                        [x, y] => Some(m.apply(*x, *y)),
                        _ => None,
                    })
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
    fn rect(&mut self, m: Affine, (x0, y0, x1, y1): (f64, f64, f64, f64)) {
        for (x, y) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
            let (px, py) = m.apply(x, y);
            self.point(px, py, 0.0);
        }
    }

    /// A flat `[x0, y0, x1, y1, …]` point list under `m`. Each point grows
    /// by `grow` in local space (a stroke half-width), so the growth scales
    /// with `m` like the stroke the backends draw.
    fn points(&mut self, m: Affine, points: &[f64], grow: f64) {
        for pair in points.chunks_exact(2) {
            if let [x, y] = pair {
                self.rect(m, (x - grow, y - grow, x + grow, y + grow));
            }
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
pub(super) fn map_box(m: Affine, b: LayoutBox) -> LayoutBox {
    let mut acc = Acc::default();
    acc.rect(m, (b.x, b.y, b.x + b.w, b.y + b.h));
    acc.into_box().unwrap_or(b)
}

/// The painted extent of `commands` under the open transform `base`: shapes,
/// strokes (grown by half their width), images, and glyph ink. With
/// `skip_rotation`, rotations opened inside `commands` are left out. Clips
/// and effect brackets add nothing. `None` when nothing paints.
pub(super) fn painted(
    commands: &[SceneCommand],
    base: Affine,
    skip_rotation: bool,
    shape: ShapeEnv<'_>,
) -> Option<LayoutBox> {
    let mut stack = Stack::new(base);
    let mut acc = Acc::default();
    for cmd in commands {
        let m = stack.top();
        match cmd {
            SceneCommand::FillRect { x, y, w, h, .. }
            | SceneCommand::FillRoundedRect { x, y, w, h, .. }
            | SceneCommand::FillEllipse { x, y, w, h, .. }
            | SceneCommand::DrawImage { x, y, w, h, .. }
            | SceneCommand::DrawSvgAsset { x, y, w, h, .. } => {
                acc.rect(m, (*x, *y, x + w, y + h));
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
                acc.rect(m, (x - g, y - g, x + w + g, y + h + g));
            }
            SceneCommand::StrokeLine {
                x1,
                y1,
                x2,
                y2,
                stroke_width,
                ..
            } => acc.points(m, &[*x1, *y1, *x2, *y2], stroke_width / 2.0),
            SceneCommand::FillPolygon { points, .. } => acc.points(m, points, 0.0),
            SceneCommand::StrokePolyline {
                points,
                stroke_width,
                ..
            } => acc.points(m, points, stroke_width / 2.0),
            SceneCommand::FillPath { segments, .. } => {
                if let Some((x, y, w, h)) = path_segments_bbox(segments) {
                    acc.rect(m, (x, y, x + w, y + h));
                }
            }
            SceneCommand::StrokePath {
                segments,
                stroke_width,
                ..
            } => {
                if let Some((x, y, w, h)) = path_segments_bbox(segments) {
                    let g = stroke_width / 2.0;
                    acc.rect(m, (x - g, y - g, x + w + g, y + h + g));
                }
            }
            SceneCommand::DrawGlyphRun { .. } => {
                if let Some(ink) = ink_bounds(std::slice::from_ref(cmd), shape) {
                    acc.rect(m, (ink.left, ink.top, ink.right, ink.bottom));
                }
            }
            SceneCommand::PushTransform { .. }
            | SceneCommand::PushScaleTranslate { .. }
            | SceneCommand::PushTransformMatrix { .. }
            | SceneCommand::PopTransform => stack.step(cmd, skip_rotation),
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
    acc.into_box()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_about_a_center_keeps_the_center() {
        let m = Affine::rotate_at(90.0, 10.0, 20.0);
        let (x, y) = m.apply(10.0, 20.0);
        assert!((x - 10.0).abs() < 1e-9 && (y - 20.0).abs() < 1e-9);
        // y-down: +90° turns +x into +y.
        let (x, y) = m.apply(11.0, 20.0);
        assert!((x - 10.0).abs() < 1e-9 && (y - 21.0).abs() < 1e-9);
    }

    #[test]
    fn then_applies_local_first() {
        let scale = Affine {
            a: 2.0,
            d: 2.0,
            ..Affine::IDENTITY
        };
        let shift = Affine {
            e: 5.0,
            ..Affine::IDENTITY
        };
        // shift ∘ scale: scale first, then shift.
        assert_eq!(shift.then(scale).apply(1.0, 1.0), (7.0, 2.0));
    }

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
            painted(
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
                Affine::IDENTITY,
                false,
                shape,
            )
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
        let m = Affine::rotate_at(90.0, 20.0, 10.0);
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
