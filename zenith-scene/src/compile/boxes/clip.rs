//! The transform and clips open at a point of a command stream, and the
//! page-space [`ClipShape`] a box record keeps.

use crate::ir::SceneCommand;
use crate::layout::LayoutBox;

use super::affine::Affine2;
use super::bounds::{Stack, map_box};

/// Slack for the point-in-shape tests, in px.
const EPS: f64 = 1e-9;

/// One clip open over a node: the (rounded) rectangle `rect` under the map
/// `world` to page px. A point is inside when `world`'s inverse takes it into
/// `rect` (edges included) and inside the rounded corners.
///
/// A `rect` with a negative `w` or `h` is empty: two disjoint clips met.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipShape {
    /// Clip space to page px.
    pub world: Affine2,
    /// The clip rectangle in clip space.
    pub rect: LayoutBox,
    /// The corner radius in clip space. `0` is a sharp rectangle. Wider
    /// than half the short side draws as half the short side, as the
    /// backends clamp it.
    pub radius: f64,
}

impl ClipShape {
    /// `true` when the page point `(x, y)` lies inside the clip.
    #[must_use]
    pub fn contains(&self, x: f64, y: f64) -> bool {
        let Some(inverse) = self.world.inverse() else {
            return false;
        };
        let (lx, ly) = inverse.apply(x, y);
        in_rounded_rect(self.rect, self.radius, lx, ly)
    }
}

/// `true` when `(x, y)` lies in `rect` with corners rounded by `radius`
/// (clamped to half the short side), edges included.
pub(super) fn in_rounded_rect(rect: LayoutBox, radius: f64, x: f64, y: f64) -> bool {
    let (left, top) = (rect.x, rect.y);
    let (right, bottom) = (rect.x + rect.w, rect.y + rect.h);
    let inside = x >= left - EPS && x <= right + EPS && y >= top - EPS && y <= bottom + EPS;
    if !inside {
        return false;
    }
    let r = radius.min(rect.w / 2.0).min(rect.h / 2.0);
    if r.is_nan() || r <= 0.0 {
        return true;
    }
    let cx = x.clamp(left + r, right - r);
    let cy = y.clamp(top + r, bottom - r);
    let (dx, dy) = (x - cx, y - cy);
    dx * dx + dy * dy <= r * r + EPS
}

/// One clip pushed in a render-space command stream.
#[derive(Clone, Copy, Debug)]
struct Pushed {
    /// The transform open at the push.
    transform: Affine2,
    rect: LayoutBox,
    radius: f64,
}

/// The transform and the clips open at one point of a command stream.
#[derive(Clone, Debug)]
pub(super) struct Open {
    pub(super) transform: Affine2,
    clips: Vec<Pushed>,
}

impl Default for Open {
    fn default() -> Self {
        Self {
            transform: Affine2::IDENTITY,
            clips: Vec::new(),
        }
    }
}

impl Open {
    /// The state open at the end of `commands`, which run under `self`.
    /// A pop with nothing opened in `commands` closes nothing.
    pub(super) fn after(&self, commands: &[SceneCommand]) -> Open {
        let mut stack = Stack::new(self.transform);
        let mut clips = self.clips.clone();
        let floor = clips.len();
        for cmd in commands {
            let pushed = match cmd {
                SceneCommand::PushClip { x, y, w, h } => Some((*x, *y, *w, *h, 0.0)),
                SceneCommand::PushClipRoundedRect { x, y, w, h, radius } => {
                    Some((*x, *y, *w, *h, *radius))
                }
                SceneCommand::PopClip => {
                    if clips.len() > floor {
                        clips.pop();
                    }
                    None
                }
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
                | SceneCommand::PushLayer { .. }
                | SceneCommand::PopLayer
                | SceneCommand::PushTransform { .. }
                | SceneCommand::PushScaleTranslate { .. }
                | SceneCommand::PushTransformMatrix { .. }
                | SceneCommand::PopTransform
                | SceneCommand::BeginShadow { .. }
                | SceneCommand::EndShadow
                | SceneCommand::BeginBlur { .. }
                | SceneCommand::EndBlur
                | SceneCommand::BeginFilter { .. }
                | SceneCommand::EndFilter
                | SceneCommand::BeginMask { .. }
                | SceneCommand::EndMask => {
                    stack.step(cmd, false);
                    None
                }
            };
            if let Some((x, y, w, h, radius)) = pushed {
                clips.push(Pushed {
                    transform: stack.top(),
                    rect: LayoutBox { x, y, w, h },
                    radius,
                });
            }
        }
        Open {
            transform: stack.top(),
            clips,
        }
    }

    /// The open clips in page px, where render `(0, 0)` + `origin` is page
    /// `(0, 0)`. Every sharp clip whose map keeps boxes axis-aligned folds
    /// into one page-space rectangle (their exact intersection), listed
    /// first. Rotated, sheared, and rounded clips follow, one each. Empty
    /// when nothing clips.
    pub(super) fn page_clips(&self, origin: (f64, f64)) -> Vec<ClipShape> {
        let to_page = Affine2::translate(-origin.0, -origin.1);
        let mut merged: Option<LayoutBox> = None;
        let mut rest: Vec<ClipShape> = Vec::new();
        for clip in &self.clips {
            let world = to_page.then(clip.transform);
            if clip.radius > 0.0 || !world.is_axis_aligned() {
                rest.push(ClipShape {
                    world,
                    rect: clip.rect,
                    radius: clip.radius,
                });
                continue;
            }
            let page = map_box(world, clip.rect);
            merged = Some(match merged {
                Some(m) => intersect(m, page),
                None => page,
            });
        }
        let mut out: Vec<ClipShape> = Vec::with_capacity(rest.len() + 1);
        if let Some(rect) = merged {
            out.push(ClipShape {
                world: Affine2::IDENTITY,
                rect,
                radius: 0.0,
            });
        }
        out.extend(rest);
        out
    }
}

/// The overlap of `a` and `b`. A negative `w` or `h` means they do not meet.
fn intersect(a: LayoutBox, b: LayoutBox) -> LayoutBox {
    let left = a.x.max(b.x);
    let top = a.y.max(b.y);
    let right = (a.x + a.w).min(b.x + b.w);
    let bottom = (a.y + a.h).min(b.y + b.h);
    LayoutBox {
        x: left,
        y: top,
        w: right - left,
        h: bottom - top,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> LayoutBox {
        LayoutBox { x, y, w, h }
    }

    #[test]
    fn rounded_corners_cut_the_corner_points() {
        let r = rect(0.0, 0.0, 100.0, 50.0);
        assert!(in_rounded_rect(r, 10.0, 0.0, 25.0));
        assert!(in_rounded_rect(r, 10.0, 3.0, 3.0));
        assert!(!in_rounded_rect(r, 10.0, 0.5, 0.5));
        // A huge radius clamps to half the short side: a stadium.
        assert!(!in_rounded_rect(r, 1000.0, 2.0, 2.0));
        assert!(in_rounded_rect(r, 1000.0, 50.0, 0.0));
        assert!(!in_rounded_rect(r, 0.0, 100.5, 10.0));
    }

    #[test]
    fn sharp_axis_aligned_clips_fold_into_one_rectangle() {
        let commands = [
            SceneCommand::PushClip {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 100.0,
            },
            SceneCommand::PushScaleTranslate {
                sx: 2.0,
                sy: 2.0,
                tx: 10.0,
                ty: 0.0,
            },
            SceneCommand::PushClip {
                x: 0.0,
                y: 0.0,
                w: 20.0,
                h: 70.0,
            },
        ];
        let clips = Open::default().after(&commands).page_clips((0.0, 0.0));
        assert_eq!(
            clips,
            vec![ClipShape {
                world: Affine2::IDENTITY,
                rect: rect(10.0, 0.0, 40.0, 100.0),
                radius: 0.0,
            }]
        );
    }

    #[test]
    fn rotated_and_rounded_clips_stay_exact_and_pops_close_them() {
        let commands = [
            SceneCommand::PushTransform {
                angle_deg: 45.0,
                cx: 50.0,
                cy: 50.0,
            },
            SceneCommand::PushClipRoundedRect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 100.0,
                radius: 4.0,
            },
        ];
        let open = Open::default().after(&commands);
        let clips = open.page_clips((0.0, 0.0));
        assert_eq!(clips.len(), 1);
        // The square turned 45° about its centre reaches past its own box
        // along the axes and misses its old corners.
        assert!(clips[0].contains(50.0, -15.0));
        assert!(!clips[0].contains(2.0, 2.0));
        let mut all = commands.to_vec();
        all.extend([SceneCommand::PopClip, SceneCommand::PopTransform]);
        let closed = Open::default().after(&all);
        assert!(closed.page_clips((0.0, 0.0)).is_empty());
        assert_eq!(closed.transform, Affine2::IDENTITY);
        // A pop with nothing opened in the walk leaves the base clip open.
        assert_eq!(
            open.after(&[SceneCommand::PopClip])
                .page_clips((0.0, 0.0))
                .len(),
            1
        );
    }
}
