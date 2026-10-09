//! [`HitShape`]: the painted fill and stroke geometry of a vector node, in
//! page px, for exact hit tests.

use zenith_geometry::{CompoundFillRule, CubicBezier, Point2, fill_contains_point};

use crate::ir::{FillRule, PathSegment, SceneCommand, StrokeAlign};

use super::affine::Affine2;
use super::bounds::Stack;
use super::region::{edges, segment_polygon_distance, usable};

/// Flatten tolerance for curves, in page px.
const FLATTEN: f64 = 0.05;

/// Slack for edge tests, in px.
const EPS: f64 = 1e-9;

/// The region a vector node paints: the union of its fills and strokes, in
/// page px. Built for `line`, `polygon`, `polyline`, `path`, and
/// `connector` nodes whose paint is only lines, polygons, polylines, and
/// paths.
///
/// Rules:
/// - A fill covers its rings under its own fill rule (`nonzero` or
///   `evenodd`), every subpath of a path taken together, so holes miss.
///   Paint alpha does not cut a fill.
/// - A stroke covers the points within half its width of its centerline,
///   measured in the stroke's own space, so rotation and scale apply
///   exactly. Joins and ends count as round: a miter tip is short and a
///   butt end long by up to half the width. Dashes count as solid.
/// - An `inside` / `outside` aligned closed stroke covers the points within
///   its full width of the outline, on that side of it.
/// - A stroke-only node holds no interior point.
#[derive(Clone, Debug, PartialEq)]
pub struct HitShape {
    fills: Vec<Fill>,
    strokes: Vec<Stroke>,
}

/// One fill: closed rings in page px under one rule.
#[derive(Clone, Debug, PartialEq)]
struct Fill {
    rings: Vec<Vec<Point2>>,
    rule: CompoundFillRule,
}

/// Which side of a closed outline a stroke paints.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Side {
    Both,
    Inside(CompoundFillRule),
    Outside(CompoundFillRule),
}

/// One stroke: centerlines in page px, a closed one ending on its first
/// point.
#[derive(Clone, Debug, PartialEq)]
struct Stroke {
    lines: Vec<Vec<Point2>>,
    /// The band reach either side of a centerline, in stroke space px.
    reach: f64,
    /// Stroke space to page px.
    map: Affine2,
    side: Side,
}

/// The distance from a point to a painted region, and the region's nearest
/// point, in page px.
type Nearest = (f64, (f64, f64));

impl HitShape {
    /// `true` when the page point `(x, y)` lies in a fill (edges included) or
    /// a stroke band. Clips are not applied.
    #[must_use]
    pub fn contains(&self, x: f64, y: f64) -> bool {
        if !(x.is_finite() && y.is_finite()) {
            return false;
        }
        let p = Point2::new_unchecked(x, y);
        self.fills.iter().any(|f| f.contains(p)) || self.strokes.iter().any(|s| s.contains(p))
    }

    /// `true` when a fill or a stroke band meets the convex polygon
    /// `region` (page px, either winding). Clips are not applied.
    ///
    /// A fill meets the region when an edge or a vertex of either lies in
    /// the other, so a region inside a hole misses. A stroke meets it when
    /// a centerline comes within the band reach (scaled by the square root
    /// of the stroke map's area scale); an `inside` / `outside` aligned
    /// stroke counts on both sides of its outline.
    #[must_use]
    pub fn touches(&self, region: &[(f64, f64)]) -> bool {
        if !usable(region) {
            return false;
        }
        let at = |p: Point2| (p.x, p.y);
        let fills = self.fills.iter().any(|f| {
            f.rings.iter().any(|ring| {
                let pts: Vec<(f64, f64)> = ring.iter().copied().map(at).collect();
                edges(&pts).any(|(a, b)| segment_polygon_distance(a, b, region) <= EPS)
            }) || region
                .iter()
                .any(|&(x, y)| f.contains(Point2::new_unchecked(x, y)))
        });
        fills
            || self.strokes.iter().any(|s| {
                let m = s.map;
                let reach = s.reach * (m.a * m.d - m.b * m.c).abs().sqrt();
                reach.is_finite()
                    && s.lines.iter().any(|line| match line.as_slice() {
                        [only] => {
                            segment_polygon_distance(at(*only), at(*only), region) <= reach + EPS
                        }
                        _ => line.windows(2).any(|w| match w {
                            [a, b] => {
                                segment_polygon_distance(at(*a), at(*b), region) <= reach + EPS
                            }
                            _ => false,
                        }),
                    })
            })
    }

    /// The page px distance from `(x, y)` to the nearest stroke band, and
    /// the band's nearest point. The band reach scales by the square root of
    /// the stroke map's area scale, exact under rotation and uniform scale.
    /// `None` without a stroke or for a non-finite point.
    #[must_use]
    pub fn nearest_stroke(&self, x: f64, y: f64) -> Option<(f64, (f64, f64))> {
        if !(x.is_finite() && y.is_finite()) {
            return None;
        }
        let p = Point2::new_unchecked(x, y);
        closest(self.strokes.iter().filter_map(|s| s.nearest(p)))
    }

    /// The page px distance from `(x, y)` to the nearest fill edge, and that
    /// edge point. `None` without a fill or for a non-finite point.
    #[must_use]
    pub fn nearest_fill_edge(&self, x: f64, y: f64) -> Option<(f64, (f64, f64))> {
        if !(x.is_finite() && y.is_finite()) {
            return None;
        }
        let p = Point2::new_unchecked(x, y);
        closest(
            self.fills
                .iter()
                .flat_map(|f| f.rings.iter())
                .filter_map(|ring| nearest_on_line(p, ring, true)),
        )
    }
}

impl Fill {
    fn contains(&self, p: Point2) -> bool {
        fill_contains_point(&self.rings, p, self.rule)
            || self
                .rings
                .iter()
                .filter_map(|ring| nearest_on_line(p, ring, true))
                .any(|(d, _)| d <= EPS)
    }
}

impl Stroke {
    fn contains(&self, p: Point2) -> bool {
        let Some(inverse) = self.map.inverse() else {
            return false;
        };
        let local = |q: Point2| {
            let (x, y) = inverse.apply(q.x, q.y);
            Point2::new_unchecked(x, y)
        };
        let lp = local(p);
        let reach2 = self.reach * self.reach + EPS;
        let in_band = self.lines.iter().any(|line| {
            line.windows(2).any(|w| match w {
                [a, b] => lp.distance_squared_to_segment(local(*a), local(*b)) <= reach2,
                _ => false,
            })
        });
        in_band
            && match self.side {
                Side::Both => true,
                Side::Inside(rule) => fill_contains_point(&self.lines, p, rule),
                Side::Outside(rule) => !fill_contains_point(&self.lines, p, rule),
            }
    }

    fn nearest(&self, p: Point2) -> Option<Nearest> {
        let m = self.map;
        let scale = (m.a * m.d - m.b * m.c).abs().sqrt();
        let reach = self.reach * scale;
        if !reach.is_finite() {
            return None;
        }
        let (d, (qx, qy)) = closest(
            self.lines
                .iter()
                .filter_map(|line| nearest_on_line(p, line, false)),
        )?;
        if d <= reach {
            return Some((0.0, (p.x, p.y)));
        }
        let t = reach / d;
        Some((d - reach, (qx + (p.x - qx) * t, qy + (p.y - qy) * t)))
    }
}

/// The smallest-distance candidate; the first one wins a tie.
fn closest(candidates: impl Iterator<Item = Nearest>) -> Option<Nearest> {
    candidates.fold(None, |best, c| match best {
        Some(b) if b.0 <= c.0 => Some(b),
        Some(_) | None => Some(c),
    })
}

/// The distance from `p` to the polyline `line` (closed back to its first
/// point when `close`), and its nearest point. `None` for an empty line.
fn nearest_on_line(p: Point2, line: &[Point2], close: bool) -> Option<Nearest> {
    let first = *line.first()?;
    let closing = close.then(|| (line.last().copied().unwrap_or(first), first));
    let edges = line
        .windows(2)
        .filter_map(|w| match w {
            [a, b] => Some((*a, *b)),
            _ => None,
        })
        .chain(closing);
    let mut best: Option<Nearest> = None;
    for (a, b) in edges {
        let proj = p.project_onto_segment(a, b);
        let d = proj.distance_squared.sqrt();
        if best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, (proj.point.x, proj.point.y)));
        }
    }
    // A single point is its own nearest point.
    best.or_else(|| Some((p.distance_squared(first).sqrt(), (first.x, first.y))))
}

fn rule_of(rule: FillRule) -> CompoundFillRule {
    match rule {
        FillRule::NonZero => CompoundFillRule::NonZero,
        FillRule::EvenOdd => CompoundFillRule::EvenOdd,
    }
}

/// The side an aligned stroke paints: only a closed outline has sides.
fn side_of(align: StrokeAlign, closed: bool, rule: FillRule) -> Side {
    match (align, closed) {
        (StrokeAlign::Inside, true) => Side::Inside(rule_of(rule)),
        (StrokeAlign::Outside, true) => Side::Outside(rule_of(rule)),
        (StrokeAlign::Center | StrokeAlign::Inside | StrokeAlign::Outside, _) => Side::Both,
    }
}

/// The band reach of a stroke `width` wide: half of it centered, all of it
/// on one side.
fn reach_of(width: f64, side: Side) -> f64 {
    let w = if width.is_finite() {
        width.max(0.0)
    } else {
        0.0
    };
    match side {
        Side::Both => w / 2.0,
        Side::Inside(_) | Side::Outside(_) => w,
    }
}

/// The flat `[x0, y0, x1, y1, …]` list `points` under `m`.
fn mapped_points(m: Affine2, points: &[f64]) -> Option<Vec<Point2>> {
    points
        .as_chunks::<2>()
        .0
        .iter()
        .map(|[x, y]| {
            let (px, py) = m.apply(*x, *y);
            Point2::new(px, py).ok()
        })
        .collect()
}

/// One flattened subpath of a path, in page px.
struct Subpath {
    points: Vec<Point2>,
    closed: bool,
}

/// The subpaths of `segments` under `m`, curves flattened in page px.
/// `None` when a coordinate is not finite.
fn flatten_path(m: Affine2, segments: &[PathSegment]) -> Option<Vec<Subpath>> {
    let at = |x: f64, y: f64| {
        let (px, py) = m.apply(x, y);
        Point2::new(px, py).ok()
    };
    let mut out: Vec<Subpath> = Vec::new();
    let mut current: Option<Subpath> = None;
    for segment in segments {
        match segment {
            PathSegment::MoveTo { x, y } => {
                out.extend(current.take().filter(|sub| sub.points.len() > 1));
                current = Some(Subpath {
                    points: vec![at(*x, *y)?],
                    closed: false,
                });
            }
            PathSegment::LineTo { x, y } => {
                let p = at(*x, *y)?;
                match current.as_mut() {
                    Some(sub) => sub.points.push(p),
                    None => {
                        current = Some(Subpath {
                            points: vec![p],
                            closed: false,
                        });
                    }
                }
            }
            PathSegment::CubicTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => {
                let (p1, p2, p3) = (at(*x1, *y1)?, at(*x2, *y2)?, at(*x, *y)?);
                let sub = current.get_or_insert_with(|| Subpath {
                    points: vec![p1],
                    closed: false,
                });
                let p0 = sub.points.last().copied().unwrap_or(p1);
                let flat = CubicBezier::new(p0, p1, p2, p3)
                    .and_then(|c| c.flatten(FLATTEN))
                    .ok()?;
                sub.points.extend(flat.into_iter().skip(1));
            }
            PathSegment::Close => {
                if let Some(mut sub) = current.take() {
                    sub.closed = true;
                    let start = sub.points.first().copied();
                    if sub.points.len() > 1 {
                        out.push(sub);
                    }
                    // Drawing on after a close starts at the closed
                    // subpath's first point.
                    current = start.map(|p| Subpath {
                        points: vec![p],
                        closed: false,
                    });
                }
            }
        }
    }
    out.extend(current.filter(|sub| sub.points.len() > 1));
    Some(out)
}

/// `line` closed back to its first point.
fn closed_line(mut line: Vec<Point2>) -> Vec<Point2> {
    if let Some(&first) = line.first() {
        line.push(first);
    }
    line
}

/// The exact painted shape of a node's own commands `own`, which start
/// under `base` (render space to page px). `None` when nothing paints or
/// something other than a line, polygon, polyline, or path paints (the
/// box stands in), or a coordinate is not finite.
pub(super) fn hit_shape(own: &[SceneCommand], base: Affine2) -> Option<HitShape> {
    let mut stack = Stack::new(base);
    let mut shape = HitShape {
        fills: Vec::new(),
        strokes: Vec::new(),
    };
    for cmd in own {
        let m = stack.top();
        match cmd {
            SceneCommand::StrokeLine {
                x1,
                y1,
                x2,
                y2,
                stroke_width,
                ..
            } => shape.strokes.push(Stroke {
                lines: vec![mapped_points(m, &[*x1, *y1, *x2, *y2])?],
                reach: reach_of(*stroke_width, Side::Both),
                map: m,
                side: Side::Both,
            }),
            SceneCommand::FillPolygon {
                points, fill_rule, ..
            } => shape.fills.push(Fill {
                rings: vec![mapped_points(m, points)?],
                rule: rule_of(*fill_rule),
            }),
            SceneCommand::StrokePolyline {
                points,
                stroke_width,
                closed,
                align,
                clip_fill_rule,
                ..
            } => {
                let line = mapped_points(m, points)?;
                let side = side_of(*align, *closed, *clip_fill_rule);
                shape.strokes.push(Stroke {
                    lines: vec![if *closed { closed_line(line) } else { line }],
                    reach: reach_of(*stroke_width, side),
                    map: m,
                    side,
                });
            }
            SceneCommand::FillPath {
                segments,
                fill_rule,
                ..
            } => shape.fills.push(Fill {
                rings: flatten_path(m, segments)?
                    .into_iter()
                    .map(|sub| sub.points)
                    .collect(),
                rule: rule_of(*fill_rule),
            }),
            SceneCommand::StrokePath {
                segments,
                stroke_width,
                closed,
                align,
                clip_fill_rule,
                ..
            } => {
                let side = side_of(*align, *closed, *clip_fill_rule);
                shape.strokes.push(Stroke {
                    lines: flatten_path(m, segments)?
                        .into_iter()
                        .map(|sub| {
                            if sub.closed {
                                closed_line(sub.points)
                            } else {
                                sub.points
                            }
                        })
                        .collect(),
                    reach: reach_of(*stroke_width, side),
                    map: m,
                    side,
                });
            }
            SceneCommand::FillRect { .. }
            | SceneCommand::StrokeRect { .. }
            | SceneCommand::FillRoundedRect { .. }
            | SceneCommand::StrokeRoundedRect { .. }
            | SceneCommand::FillEllipse { .. }
            | SceneCommand::StrokeEllipse { .. }
            | SceneCommand::DrawImage { .. }
            | SceneCommand::DrawSvgAsset { .. }
            | SceneCommand::DrawGlyphRun { .. } => return None,
            SceneCommand::PushTransform { .. }
            | SceneCommand::PushScaleTranslate { .. }
            | SceneCommand::PushTransformMatrix { .. }
            | SceneCommand::PopTransform => stack.step(cmd, false),
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
    (!(shape.fills.is_empty() && shape.strokes.is_empty())).then_some(shape)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Color, Paint};

    fn black() -> Color {
        Color::srgb(0, 0, 0, 255)
    }

    fn polyline(points: Vec<f64>, width: f64, closed: bool, align: StrokeAlign) -> SceneCommand {
        SceneCommand::StrokePolyline {
            points,
            color: black(),
            stroke_width: width,
            closed,
            align,
            clip_fill_rule: FillRule::NonZero,
        }
    }

    #[test]
    fn aligned_strokes_paint_one_side_of_the_outline() {
        let square = vec![0.0, 0.0, 100.0, 0.0, 100.0, 100.0, 0.0, 100.0];
        let inside = hit_shape(
            &[polyline(square.clone(), 10.0, true, StrokeAlign::Inside)],
            Affine2::IDENTITY,
        )
        .expect("shape");
        assert!(inside.contains(8.0, 50.0));
        assert!(!inside.contains(-3.0, 50.0));
        assert!(!inside.contains(50.0, 50.0));
        let outside = hit_shape(
            &[polyline(square, 10.0, true, StrokeAlign::Outside)],
            Affine2::IDENTITY,
        )
        .expect("shape");
        assert!(outside.contains(-8.0, 50.0));
        assert!(!outside.contains(3.0, 50.0));
    }

    #[test]
    fn stroke_band_scales_with_its_transform() {
        let cmds = [
            SceneCommand::PushScaleTranslate {
                sx: 1.0,
                sy: 4.0,
                tx: 0.0,
                ty: 0.0,
            },
            polyline(
                vec![0.0, 10.0, 100.0, 10.0],
                2.0,
                false,
                StrokeAlign::Center,
            ),
            SceneCommand::PopTransform,
        ];
        let shape = hit_shape(&cmds, Affine2::IDENTITY).expect("shape");
        // Drawn along y = 40 with a 2 px stroke stretched to 8 px.
        assert!(shape.contains(50.0, 43.5));
        assert!(!shape.contains(50.0, 44.5));
    }

    #[test]
    fn curves_flatten_and_glyphs_fall_back_to_the_box() {
        let curve = SceneCommand::FillPath {
            segments: vec![
                PathSegment::MoveTo { x: 0.0, y: 0.0 },
                PathSegment::CubicTo {
                    x1: 0.0,
                    y1: 100.0,
                    x2: 100.0,
                    y2: 100.0,
                    x: 100.0,
                    y: 0.0,
                },
                PathSegment::Close,
            ],
            paint: Paint::solid(black()),
            fill_rule: FillRule::NonZero,
        };
        let shape = hit_shape(std::slice::from_ref(&curve), Affine2::IDENTITY).expect("shape");
        // The curve peaks at y = 75.
        assert!(shape.contains(50.0, 74.0));
        assert!(!shape.contains(50.0, 76.0));
        assert!(!shape.contains(2.0, 70.0));
        let rect = SceneCommand::FillRect {
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 1.0,
            paint: Paint::solid(black()),
        };
        assert_eq!(hit_shape(&[curve, rect], Affine2::IDENTITY), None);
        assert_eq!(hit_shape(&[], Affine2::IDENTITY), None);
    }

    #[test]
    fn nearest_stroke_measures_from_the_band_edge() {
        let shape = hit_shape(
            &[polyline(
                vec![0.0, 0.0, 100.0, 0.0],
                4.0,
                false,
                StrokeAlign::Center,
            )],
            Affine2::IDENTITY,
        )
        .expect("shape");
        assert_eq!(shape.nearest_stroke(50.0, 1.0), Some((0.0, (50.0, 1.0))));
        assert_eq!(shape.nearest_stroke(50.0, 7.0), Some((5.0, (50.0, 2.0))));
        assert_eq!(shape.nearest_fill_edge(50.0, 7.0), None);
    }
}
