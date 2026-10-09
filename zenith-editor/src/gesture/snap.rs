//! Snapping: a move or resize lands its edges and centres on the edges and
//! centres of other nodes, their containers, and the page, when they come
//! within a threshold. The result is a page delta like the pointer's, so
//! the gesture still maps to the ordinary ops; the guides tell the page what
//! lined up.

use std::collections::BTreeSet;

use serde::Serialize;
use zenith_core::{Document, dim_to_px};
use zenith_scene::CompiledBox;

use super::target::Target;
use crate::doc::tree::child_lists;
use crate::geom::{Drag, Grip, Pt, Rect, resize};

/// Two lines closer than this (page px) are aligned.
const ALIGNED: f64 = 1e-6;

/// An axis-aligned page box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Aabb {
    pub(crate) x0: f64,
    pub(crate) y0: f64,
    pub(crate) x1: f64,
    pub(crate) y1: f64,
}

impl Aabb {
    /// The bounds of `points`; `None` for none or a non-finite point.
    pub(crate) fn of(points: impl IntoIterator<Item = Pt>) -> Option<Aabb> {
        let mut out: Option<Aabb> = None;
        for (x, y) in points {
            if !(x.is_finite() && y.is_finite()) {
                return None;
            }
            out = Some(match out {
                None => Aabb {
                    x0: x,
                    y0: y,
                    x1: x,
                    y1: y,
                },
                Some(b) => Aabb {
                    x0: b.x0.min(x),
                    y0: b.y0.min(y),
                    x1: b.x1.max(x),
                    y1: b.y1.max(y),
                },
            });
        }
        out
    }

    /// The box as a [`Rect`].
    pub(crate) fn rect(self) -> Rect {
        Rect {
            x: self.x0,
            y: self.y0,
            w: self.x1 - self.x0,
            h: self.y1 - self.y0,
        }
    }

    fn of_rect(r: Rect) -> Aabb {
        Aabb {
            x0: r.x,
            y0: r.y,
            x1: r.x + r.w,
            y1: r.y + r.h,
        }
    }

    /// Its corners, clockwise from the top-left.
    pub(crate) fn corners(self) -> [Pt; 4] {
        [
            (self.x0, self.y0),
            (self.x1, self.y0),
            (self.x1, self.y1),
            (self.x0, self.y1),
        ]
    }

    fn lines(self, axis: Axis) -> [f64; 3] {
        match axis {
            Axis::X => [self.x0, (self.x0 + self.x1) / 2.0, self.x1],
            Axis::Y => [self.y0, (self.y0 + self.y1) / 2.0, self.y1],
        }
    }

    /// The extent across `axis`: the y span of an x line.
    fn span(self, axis: Axis) -> (f64, f64) {
        match axis {
            Axis::X => (self.y0, self.y1),
            Axis::Y => (self.x0, self.x1),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    X,
    Y,
}

/// One alignment the page draws: a vertical line at `x = at` (axis `x`) or
/// a horizontal one at `y = at`, from `from` to `to` across it, page px.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct Guide {
    pub(crate) axis: &'static str,
    pub(crate) at: f64,
    pub(crate) from: f64,
    pub(crate) to: f64,
}

/// A snapped gesture: the page delta to use instead of the pointer's, and
/// the guides of what lined up.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct Snapped {
    pub(crate) dx: f64,
    pub(crate) dy: f64,
    pub(crate) guides: Vec<Guide>,
}

/// What a moving box can snap to: the boxes of every node not in the
/// gesture, and the page.
pub(crate) struct Scene {
    boxes: Vec<Aabb>,
}

impl Scene {
    /// The snap targets of a gesture on `targets` (one page): every drawn
    /// box of the page except the targets, their subtrees, and their
    /// expanded content, then the page box. Order is the raw id order, the
    /// page last, so ties resolve the same on every run.
    pub(crate) fn of(doc: &Document, targets: &[Target<'_>]) -> Scene {
        let Some(first) = targets.first() else {
            return Scene { boxes: Vec::new() };
        };
        let mut skip: BTreeSet<String> = BTreeSet::new();
        let mut expanded: Vec<String> = Vec::new();
        for t in targets {
            let prefix = t
                .page
                .raw_id
                .strip_suffix(t.id.as_str())
                .unwrap_or_default();
            let mut ids = Vec::new();
            subtree_ids(t.located.node, &mut ids);
            for id in ids {
                skip.insert(format!("{prefix}{id}"));
                expanded.push(format!("{prefix}{id}/"));
            }
        }
        let keep = |raw: &str, b: &CompiledBox| {
            !b.hidden && !skip.contains(raw) && !expanded.iter().any(|p| raw.starts_with(p))
        };
        let mut boxes: Vec<Aabb> = first
            .view
            .boxes
            .iter()
            .filter(|(raw, b)| keep(raw, b))
            .filter_map(|(_, b)| Aabb::of(b.corners()))
            .collect();
        if let Some(page) = doc.body.pages.get(first.page.index)
            && let (Some(w), Some(h)) = (
                dim_to_px(page.width.value, &page.width.unit),
                dim_to_px(page.height.value, &page.height.unit),
            )
        {
            boxes.push(Aabb {
                x0: 0.0,
                y0: 0.0,
                x1: w,
                y1: h,
            });
        }
        Scene { boxes }
    }
}

fn subtree_ids<'d>(node: &'d zenith_core::Node, out: &mut Vec<&'d str>) {
    if let Some(id) = node.id() {
        out.push(id);
    }
    for list in child_lists(node) {
        for child in list {
            subtree_ids(child, out);
        }
    }
}

/// A line of the moving box: where it starts and how far it moves per unit
/// of pointer delta (1 follows the pointer, -1 mirrors it).
#[derive(Debug, Clone, Copy)]
struct Mover {
    base: f64,
    rate: f64,
}

/// Snap a move of `moving` by `delta` within `threshold` page px: its left,
/// centre, and right lines to any target's, and the same for top, centre,
/// and bottom.
pub(crate) fn snap_move(scene: &Scene, moving: Aabb, delta: Pt, threshold: f64) -> Snapped {
    let movers = |axis: Axis| moving.lines(axis).map(|base| Mover { base, rate: 1.0 });
    let dx = snap_axis(scene, &movers(Axis::X), Axis::X, delta.0, threshold);
    let dy = snap_axis(scene, &movers(Axis::Y), Axis::Y, delta.1, threshold);
    let moved = Aabb {
        x0: moving.x0 + dx,
        y0: moving.y0 + dy,
        x1: moving.x1 + dx,
        y1: moving.y1 + dy,
    };
    Snapped {
        dx,
        dy,
        guides: guides(scene, moved),
    }
}

/// The grip drag that a snap of `moving` resizes: `grip`, about the
/// centre with `from_center`, aspect kept with `constrain`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct GripDrag {
    pub(crate) grip: Grip,
    pub(crate) constrain: bool,
    pub(crate) from_center: bool,
}

/// Snap a resize of `moving` by grip `g` with pointer `delta`: the edges
/// the grip moves snap to target lines. The guides come from the box the
/// resize really makes (with `constrain`, the aspect ratio can carry an
/// edge off its line).
pub(crate) fn snap_grip(
    scene: &Scene,
    moving: Aabb,
    g: GripDrag,
    delta: Pt,
    threshold: f64,
) -> Snapped {
    let (hx, hy) = g.grip.fractions();
    let edge_movers = |f: f64, lo: f64, hi: f64| -> Vec<Mover> {
        let (moving_edge, other) = if f == 1.0 {
            (hi, lo)
        } else if f == 0.0 {
            (lo, hi)
        } else {
            return Vec::new();
        };
        let mut out = vec![Mover {
            base: moving_edge,
            rate: 1.0,
        }];
        if g.from_center {
            out.push(Mover {
                base: other,
                rate: -1.0,
            });
        }
        out
    };
    let dx = snap_axis(
        scene,
        &edge_movers(hx, moving.x0, moving.x1),
        Axis::X,
        delta.0,
        threshold,
    );
    let dy = snap_axis(
        scene,
        &edge_movers(hy, moving.y0, moving.y1),
        Axis::Y,
        delta.1,
        threshold,
    );
    let after = resize(
        moving.rect(),
        zenith_scene::Affine2::IDENTITY,
        Drag {
            grip: g.grip,
            delta: (dx, dy),
            constrain: g.constrain,
            from_center: g.from_center,
        },
    )
    .apply(moving.rect());
    let resized = Aabb::of_rect(after);
    let mut found = guides(scene, resized);
    // Only the edges the grip moved count as snapped.
    found.retain(|guide| {
        let moved = |a: f64, b: f64| (a - b).abs() > ALIGNED;
        match guide.axis {
            "x" => {
                ((guide.at - resized.x0).abs() <= ALIGNED && moved(resized.x0, moving.x0))
                    || ((guide.at - resized.x1).abs() <= ALIGNED && moved(resized.x1, moving.x1))
            }
            _ => {
                ((guide.at - resized.y0).abs() <= ALIGNED && moved(resized.y0, moving.y0))
                    || ((guide.at - resized.y1).abs() <= ALIGNED && moved(resized.y1, moving.y1))
            }
        }
    });
    Snapped {
        dx,
        dy,
        guides: found,
    }
}

/// The delta on `axis` that lands the closest mover on the closest target
/// line within `threshold` of where `delta` puts it; `delta` when none is
/// that close. A tie keeps the first mover, then the first target.
fn snap_axis(scene: &Scene, movers: &[Mover], axis: Axis, delta: f64, threshold: f64) -> f64 {
    let mut best: Option<(f64, f64)> = None;
    for m in movers {
        let at = m.base + m.rate * delta;
        for b in &scene.boxes {
            for line in b.lines(axis) {
                let gap = (line - at).abs();
                if gap <= threshold && best.is_none_or(|(g, _)| gap < g) {
                    best = Some((gap, (line - m.base) / m.rate));
                }
            }
        }
    }
    best.map_or(delta, |(_, d)| d)
}

/// The guides of `moved`: each of its lines that lies on a target line,
/// spanning both boxes. One guide per line position, sorted.
fn guides(scene: &Scene, moved: Aabb) -> Vec<Guide> {
    let mut out: Vec<Guide> = Vec::new();
    for (axis, name) in [(Axis::X, "x"), (Axis::Y, "y")] {
        for at in moved.lines(axis) {
            for b in &scene.boxes {
                if b.lines(axis).iter().any(|l| (l - at).abs() <= ALIGNED) {
                    let (m0, m1) = moved.span(axis);
                    let (b0, b1) = b.span(axis);
                    let (from, to) = (m0.min(b0), m1.max(b1));
                    match out
                        .iter_mut()
                        .find(|g| g.axis == name && (g.at - at).abs() <= ALIGNED)
                    {
                        Some(g) => {
                            g.from = g.from.min(from);
                            g.to = g.to.max(to);
                        }
                        None => out.push(Guide {
                            axis: name,
                            at,
                            from,
                            to,
                        }),
                    }
                }
            }
        }
    }
    out.sort_by(|a, b| a.axis.cmp(b.axis).then(a.at.total_cmp(&b.at)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene(boxes: &[[f64; 4]]) -> Scene {
        Scene {
            boxes: boxes
                .iter()
                .map(|&[x0, y0, x1, y1]| Aabb { x0, y0, x1, y1 })
                .collect(),
        }
    }

    const MOVING: Aabb = Aabb {
        x0: 0.0,
        y0: 0.0,
        x1: 10.0,
        y1: 10.0,
    };

    #[test]
    fn move_snaps_the_nearest_line_within_the_threshold() {
        let s = scene(&[[30.0, 50.0, 40.0, 60.0]]);
        // The right edge (10 + 18 = 28) lands on the target's left edge 30.
        let out = snap_move(&s, MOVING, (18.0, 0.0), 3.0);
        assert_eq!((out.dx, out.dy), (20.0, 0.0));
        assert_eq!(
            out.guides,
            vec![Guide {
                axis: "x",
                at: 30.0,
                from: 0.0,
                to: 60.0,
            }]
        );
        // Out of reach: the pointer delta stays, no guide.
        let far = snap_move(&s, MOVING, (10.0, 0.0), 3.0);
        assert_eq!((far.dx, far.dy), (10.0, 0.0));
        assert!(far.guides.is_empty());
        // Centres snap too: centre 5 + 29.5 = 34.5 is 0.5 from 35.
        let centred = snap_move(&s, MOVING, (29.5, 49.0), 1.0);
        assert_eq!((centred.dx, centred.dy), (30.0, 50.0));
        assert_eq!(centred.guides.len(), 6);
    }

    #[test]
    fn grip_snaps_only_the_edges_it_moves() {
        let s = scene(&[[30.0, 50.0, 40.0, 60.0]]);
        let east = GripDrag {
            grip: Grip::E,
            constrain: false,
            from_center: false,
        };
        let out = snap_grip(&s, MOVING, east, (18.5, 3.0), 3.0);
        assert_eq!((out.dx, out.dy), (20.0, 3.0));
        assert_eq!(out.guides.len(), 1);
        assert_eq!(out.guides.first().map(|g| g.at), Some(30.0));
        // About the centre the left edge mirrors the right: -12 + 2 = -10
        // lands nowhere, but the right edge 10 + 20 = 30 does.
        let centred = GripDrag {
            from_center: true,
            ..east
        };
        let c = snap_grip(&s, MOVING, centred, (19.0, 0.0), 2.0);
        assert_eq!(c.dx, 20.0);
    }
}
