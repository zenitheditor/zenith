//! Authored geometry of the kinds without a box: line endpoints,
//! polygon / polyline vertices, and path anchors, in authored px.

use zenith_core::{Dimension, Node, PathNode, Point, dim_to_px};

/// One path anchor in authored px, with its complete handles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct AnchorPx {
    pub(crate) at: (f64, f64),
    pub(crate) handle_in: Option<(f64, f64)>,
    pub(crate) handle_out: Option<(f64, f64)>,
}

/// The editable geometry of a node without a box.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Shape {
    /// A line from `start` to `end`.
    Line { start: (f64, f64), end: (f64, f64) },
    /// Polygon or polyline vertices.
    Points(Vec<(f64, f64)>),
    /// Path contours. `compound` is `true` when the path uses `subpath`
    /// children, so ops address a contour by index.
    Path {
        contours: Vec<Vec<AnchorPx>>,
        compound: bool,
    },
}

impl Shape {
    /// The points that set the rotation pivot and the resize frame: line
    /// endpoints, vertices, or path anchors (handles left out, as the scene
    /// computes a path's pivot from its anchors).
    pub(crate) fn frame_points(&self) -> Vec<(f64, f64)> {
        match self {
            Shape::Line { start, end } => vec![*start, *end],
            Shape::Points(points) => points.clone(),
            Shape::Path { contours, .. } => contours.iter().flatten().map(|a| a.at).collect(),
        }
    }
}

/// Why a shape has no px geometry: the named value has no px conversion.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Unresolved(pub(crate) String);

/// The shape of `node`. `None` for a kind with a box (or no geometry);
/// `Some(Err)` names a coordinate that is absent or not px / pt.
pub(crate) fn shape_of(node: &Node) -> Option<Result<Shape, Unresolved>> {
    match node {
        Node::Line(l) => Some((|| {
            Ok(Shape::Line {
                start: (coord(l.x1.as_ref(), "x1")?, coord(l.y1.as_ref(), "y1")?),
                end: (coord(l.x2.as_ref(), "x2")?, coord(l.y2.as_ref(), "y2")?),
            })
        })()),
        Node::Polygon(p) => Some(points(&p.points).map(Shape::Points)),
        Node::Polyline(p) => Some(points(&p.points).map(Shape::Points)),
        Node::Path(p) => Some(path(p)),
        Node::Rect(_)
        | Node::Ellipse(_)
        | Node::Text(_)
        | Node::Code(_)
        | Node::Frame(_)
        | Node::Group(_)
        | Node::Image(_)
        | Node::Instance(_)
        | Node::Field(_)
        | Node::Toc(_)
        | Node::Footnote(_)
        | Node::Table(_)
        | Node::Shape(_)
        | Node::Connector(_)
        | Node::Pattern(_)
        | Node::Chart(_)
        | Node::Light(_)
        | Node::Mesh(_)
        | Node::Unknown(_) => None,
    }
}

fn coord(d: Option<&Dimension>, name: &str) -> Result<f64, Unresolved> {
    let d = d.ok_or_else(|| Unresolved(format!("{name} is absent")))?;
    dim_to_px(d.value, &d.unit).ok_or_else(|| Unresolved(format!("{name} {}", d.to_kdl_string())))
}

fn points(list: &[Point]) -> Result<Vec<(f64, f64)>, Unresolved> {
    list.iter()
        .enumerate()
        .map(|(i, p)| {
            Ok((
                coord(p.x.as_ref(), &format!("point {i} x"))?,
                coord(p.y.as_ref(), &format!("point {i} y"))?,
            ))
        })
        .collect()
}

fn path(p: &PathNode) -> Result<Shape, Unresolved> {
    let compound = p.anchors.is_empty() && !p.subpaths.is_empty();
    let contours = p
        .effective_subpaths()
        .enumerate()
        .map(|(s, contour)| {
            contour
                .anchors
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    let at = |x: Option<&Dimension>, y: Option<&Dimension>, what: &str| {
                        Ok::<_, Unresolved>((
                            coord(x, &format!("anchor {s}.{i} {what}x"))?,
                            coord(y, &format!("anchor {s}.{i} {what}y"))?,
                        ))
                    };
                    let handle =
                        |x: Option<&Dimension>, y: Option<&Dimension>, what: &str| match (x, y) {
                            (Some(_), Some(_)) => at(x, y, what).map(Some),
                            (Some(_), None) | (None, Some(_)) | (None, None) => Ok(None),
                        };
                    Ok(AnchorPx {
                        at: at(a.x.as_ref(), a.y.as_ref(), "")?,
                        handle_in: handle(a.in_x.as_ref(), a.in_y.as_ref(), "in_")?,
                        handle_out: handle(a.out_x.as_ref(), a.out_y.as_ref(), "out_")?,
                    })
                })
                .collect::<Result<Vec<_>, Unresolved>>()
        })
        .collect::<Result<Vec<_>, Unresolved>>()?;
    Ok(Shape::Path { contours, compound })
}

/// The axis-aligned bounds `(x, y, w, h)` of `points`. `None` when empty.
pub(crate) fn bounds(points: &[(f64, f64)]) -> Option<(f64, f64, f64, f64)> {
    let first = points.first()?;
    let (mut x0, mut y0, mut x1, mut y1) = (first.0, first.1, first.0, first.1);
    for &(x, y) in points {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    Some((x0, y0, x1 - x0, y1 - y0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::{KdlAdapter, KdlSource};

    fn first(src: &str) -> Node {
        let doc = KdlAdapter.parse(src.as_bytes()).expect("parse");
        doc.body
            .pages
            .into_iter()
            .next()
            .and_then(|p| p.children.into_iter().next())
            .expect("node")
    }

    #[test]
    fn line_and_points_read_in_px() {
        let line = first(
            r#"zenith version=1 { document id="d" { page id="p" w=(px)10 h=(px)10 {
              line id="l" x1=(px)1 y1=(pt)3 x2=(px)5 y2=(px)6
            } } }"#,
        );
        let Some(Ok(Shape::Line { start, end })) = shape_of(&line) else {
            panic!("line shape");
        };
        assert_eq!(start, (1.0, 4.0));
        assert_eq!(end, (5.0, 6.0));
        assert_eq!(
            bounds(&[(1.0, 4.0), (5.0, 2.0)]),
            Some((1.0, 2.0, 4.0, 2.0))
        );
        assert_eq!(bounds(&[]), None);
    }
}
