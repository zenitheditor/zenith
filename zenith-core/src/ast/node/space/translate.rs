//! Move a node by a px offset, writing its position properties.

use std::collections::BTreeMap;

use crate::ast::value::{Dimension, PropertyValue, Unit, dim_to_px};
use crate::tokens::ResolvedToken;

use super::super::common::{Node, Point};
use super::super::leaf::PathAnchor;
use super::resolve::resolve_geometry_px;

fn px_dim(v: f64) -> Dimension {
    Dimension {
        value: v,
        unit: Unit::Px,
    }
}

fn px_prop(v: f64) -> PropertyValue {
    PropertyValue::Dimension(px_dim(v))
}

/// Add `d` to a resolvable geometry value. An absent or unresolvable value
/// (anchor-placed) stays as authored.
fn shift_pv(pv: &mut Option<PropertyValue>, d: f64, resolved: &BTreeMap<String, ResolvedToken>) {
    if let Some(v) = resolve_geometry_px(pv.as_ref(), resolved) {
        *pv = Some(px_prop(v + d));
    }
}

/// Add `d` to a group origin. An absent origin counts as 0.
fn shift_origin(
    pv: &mut Option<PropertyValue>,
    d: f64,
    resolved: &BTreeMap<String, ResolvedToken>,
) {
    match pv {
        None => *pv = Some(px_prop(d)),
        Some(_) => shift_pv(pv, d, resolved),
    }
}

/// Add `d` to a raw dimension with a px value.
fn shift_dim(dim: &mut Option<Dimension>, d: f64) {
    if let Some(v) = dim.as_ref().and_then(|x| dim_to_px(x.value, &x.unit)) {
        *dim = Some(px_dim(v + d));
    }
}

/// Add `d` to an instance origin. An absent origin counts as 0.
fn shift_dim_origin(dim: &mut Option<Dimension>, d: f64) {
    match dim {
        None => *dim = Some(px_dim(d)),
        Some(_) => shift_dim(dim, d),
    }
}

fn shift_points(points: &mut [Point], dx: f64, dy: f64) {
    for p in points {
        shift_dim(&mut p.x, dx);
        shift_dim(&mut p.y, dy);
    }
}

fn shift_anchors(anchors: &mut [PathAnchor], dx: f64, dy: f64) {
    for a in anchors {
        shift_dim(&mut a.x, dx);
        shift_dim(&mut a.y, dy);
        shift_dim(&mut a.in_x, dx);
        shift_dim(&mut a.in_y, dy);
        shift_dim(&mut a.out_x, dx);
        shift_dim(&mut a.out_y, dy);
    }
}

/// Move `node` by `(dx, dy)` px. Shallow: only the node's own position
/// properties change, never its children.
///
/// - Box kinds and `light`: `x` / `y` move when they resolve. An absent value
///   (anchor-placed) stays absent.
/// - `frame`: `x` / `y` move when they resolve, and children follow through
///   [`Node::child_space`]. An absent value (anchor- or layout-placed) stays
///   absent.
/// - `group`, `instance`: the origin moves, and an absent origin counts as 0.
///   Children follow through [`Node::child_space`].
/// - `line`: both endpoints. `polygon` / `polyline`: every point.
/// - `path`: every anchor and handle, in every subpath.
/// - `connector` (follows its targets), `footnote`, unknown: no change.
///
/// A moved value is written as a `(px)` dimension.
pub fn translate_node(
    node: &mut Node,
    dx: f64,
    dy: f64,
    resolved: &BTreeMap<String, ResolvedToken>,
) {
    let xy = |x: &mut Option<PropertyValue>, y: &mut Option<PropertyValue>| {
        shift_pv(x, dx, resolved);
        shift_pv(y, dy, resolved);
    };
    match node {
        Node::Rect(n) => xy(&mut n.x, &mut n.y),
        Node::Ellipse(n) => xy(&mut n.x, &mut n.y),
        Node::Text(n) => xy(&mut n.x, &mut n.y),
        Node::Code(n) => xy(&mut n.x, &mut n.y),
        Node::Image(n) => xy(&mut n.x, &mut n.y),
        Node::Field(n) => xy(&mut n.x, &mut n.y),
        Node::Toc(n) => xy(&mut n.x, &mut n.y),
        Node::Table(n) => xy(&mut n.x, &mut n.y),
        Node::Shape(n) => xy(&mut n.x, &mut n.y),
        Node::Pattern(n) => xy(&mut n.x, &mut n.y),
        Node::Chart(n) => xy(&mut n.x, &mut n.y),
        Node::Mesh(n) => xy(&mut n.x, &mut n.y),
        Node::Light(n) => xy(&mut n.x, &mut n.y),
        Node::Frame(n) => xy(&mut n.x, &mut n.y),
        Node::Group(n) => {
            shift_origin(&mut n.x, dx, resolved);
            shift_origin(&mut n.y, dy, resolved);
        }
        Node::Instance(n) => {
            shift_dim_origin(&mut n.x, dx);
            shift_dim_origin(&mut n.y, dy);
        }
        Node::Line(n) => {
            shift_dim(&mut n.x1, dx);
            shift_dim(&mut n.y1, dy);
            shift_dim(&mut n.x2, dx);
            shift_dim(&mut n.y2, dy);
        }
        Node::Polygon(n) => shift_points(&mut n.points, dx, dy),
        Node::Polyline(n) => shift_points(&mut n.points, dx, dy),
        Node::Path(n) => {
            shift_anchors(&mut n.anchors, dx, dy);
            for sub in &mut n.subpaths {
                shift_anchors(&mut sub.anchors, dx, dy);
            }
        }
        Node::Connector(_) | Node::Footnote(_) | Node::Unknown(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::Document;
    use crate::parse::{KdlAdapter, KdlSource};

    use super::*;

    fn nodes(body: &str) -> Vec<Node> {
        let src = format!(
            r##"zenith version=1 {{
  project id="p" name="P"
  tokens format="zenith-token-v1" {{}}
  styles {{}}
  document id="d" title="D" {{
    page id="pg" w=(px)400 h=(px)300 {{
      {body}
    }}
  }}
}}"##
        );
        let doc: Document = KdlAdapter.parse(src.as_bytes()).expect("parse");
        doc.body.pages.into_iter().next().expect("page").children
    }

    fn moved(body: &str) -> Node {
        let mut n = nodes(body).into_iter().next().expect("node");
        translate_node(&mut n, 10.0, 20.0, &BTreeMap::new());
        n
    }

    fn px(v: f64) -> Option<PropertyValue> {
        Some(px_prop(v))
    }

    fn dpx(v: f64) -> Option<Dimension> {
        Some(px_dim(v))
    }

    /// The `x` / `y` of a box-like node.
    fn xy(n: &Node) -> (Option<PropertyValue>, Option<PropertyValue>) {
        match n {
            Node::Rect(n) => (n.x.clone(), n.y.clone()),
            Node::Ellipse(n) => (n.x.clone(), n.y.clone()),
            Node::Text(n) => (n.x.clone(), n.y.clone()),
            Node::Code(n) => (n.x.clone(), n.y.clone()),
            Node::Image(n) => (n.x.clone(), n.y.clone()),
            Node::Field(n) => (n.x.clone(), n.y.clone()),
            Node::Toc(n) => (n.x.clone(), n.y.clone()),
            Node::Table(n) => (n.x.clone(), n.y.clone()),
            Node::Shape(n) => (n.x.clone(), n.y.clone()),
            Node::Pattern(n) => (n.x.clone(), n.y.clone()),
            Node::Chart(n) => (n.x.clone(), n.y.clone()),
            Node::Mesh(n) => (n.x.clone(), n.y.clone()),
            Node::Light(n) => (n.x.clone(), n.y.clone()),
            Node::Frame(n) => (n.x.clone(), n.y.clone()),
            Node::Group(n) => (n.x.clone(), n.y.clone()),
            Node::Line(_)
            | Node::Polygon(_)
            | Node::Polyline(_)
            | Node::Path(_)
            | Node::Instance(_)
            | Node::Footnote(_)
            | Node::Connector(_)
            | Node::Unknown(_) => panic!("not a box-like node: {n:?}"),
        }
    }

    #[test]
    fn box_kinds_move_x_and_y() {
        let cases = [
            r##"rect id="a" x=(px)1 y=(pt)3 w=(px)5 h=(px)5 fill="#000000""##,
            r#"ellipse id="a" x=(px)1 y=(pt)3 w=(px)5 h=(px)5"#,
            r#"text id="a" x=(px)1 y=(pt)3 w=(px)5 h=(px)5 content="t""#,
            r#"code id="a" x=(px)1 y=(pt)3 w=(px)5 h=(px)5 content="t""#,
            r#"image id="a" x=(px)1 y=(pt)3 w=(px)5 h=(px)5 asset="img""#,
            r#"field id="a" type="page-number" x=(px)1 y=(pt)3 w=(px)5 h=(px)5"#,
            r#"toc id="a" x=(px)1 y=(pt)3 w=(px)5 h=(px)5"#,
            r#"shape id="a" x=(px)1 y=(pt)3 w=(px)5 h=(px)5"#,
            r#"chart id="a" kind="bar" x=(px)1 y=(pt)3 w=(px)5 h=(px)5"#,
            r#"mesh id="a" x=(px)1 y=(pt)3 w=(px)5 h=(px)5"#,
            r#"light id="a" x=(px)1 y=(pt)3"#,
        ];
        for case in cases {
            let n = moved(case);
            assert_eq!(xy(&n), (px(11.0), px(24.0)), "{case}");
        }
    }

    #[test]
    fn table_and_pattern_move_x_and_y() {
        let table = moved(
            r#"table id="a" x=(px)1 y=(pt)3 w=(px)5 h=(px)5 {
        row { cell { } }
      }"#,
        );
        assert_eq!(xy(&table), (px(11.0), px(24.0)));
        let pattern = moved(
            r##"pattern id="a" kind="grid" spacing=(px)10 x=(px)1 y=(pt)3 w=(px)5 h=(px)5 {
        rect id="m" x=(px)0 y=(px)0 w=(px)1 h=(px)1 fill="#000000"
      }"##,
        );
        assert_eq!(xy(&pattern), (px(11.0), px(24.0)));
    }

    #[test]
    fn absent_box_position_stays_absent() {
        let n = moved(r##"rect id="a" anchor="center" w=(px)5 h=(px)5 fill="#000000""##);
        assert_eq!(xy(&n), (None, None));
    }

    #[test]
    fn frame_moves_only_itself() {
        let n = moved(
            r##"frame id="f" x=(px)1 y=(px)2 w=(px)50 h=(px)50 {
        rect id="r" x=(px)3 y=(px)4 w=(px)5 h=(px)5 fill="#000000"
      }"##,
        );
        assert_eq!(xy(&n), (px(11.0), px(22.0)));
        let Node::Frame(f) = &n else {
            panic!("frame expected");
        };
        assert_eq!(xy(&f.children[0]), (px(3.0), px(4.0)));
    }

    #[test]
    fn group_moves_origin_and_absent_counts_as_zero() {
        let n = moved(
            r##"group id="g" {
        rect id="r" x=(px)3 y=(px)4 w=(px)5 h=(px)5 fill="#000000"
      }"##,
        );
        assert_eq!(xy(&n), (px(10.0), px(20.0)));
        let Node::Group(g) = &n else {
            panic!("group expected");
        };
        assert_eq!(xy(&g.children[0]), (px(3.0), px(4.0)));
        let n = moved(r#"group id="g" x=(px)1 y=(px)2 { }"#);
        assert_eq!(xy(&n), (px(11.0), px(22.0)));
    }

    #[test]
    fn instance_moves_origin() {
        let Node::Instance(i) = moved(r#"instance id="i" component="c""#) else {
            panic!("instance expected");
        };
        assert_eq!((i.x, i.y), (dpx(10.0), dpx(20.0)));
        let Node::Instance(i) = moved(r#"instance id="i" component="c" x=(px)1 y=(px)2"#) else {
            panic!("instance expected");
        };
        assert_eq!((i.x, i.y), (dpx(11.0), dpx(22.0)));
    }

    #[test]
    fn line_moves_both_endpoints() {
        let Node::Line(l) =
            moved(r##"line id="l" x1=(px)1 y1=(px)2 x2=(px)3 y2=(px)4 stroke="#000000""##)
        else {
            panic!("line expected");
        };
        assert_eq!(
            (l.x1, l.y1, l.x2, l.y2),
            (dpx(11.0), dpx(22.0), dpx(13.0), dpx(24.0))
        );
    }

    #[test]
    fn polygon_and_polyline_move_every_point() {
        let Node::Polygon(p) = moved(
            r##"polygon id="p" fill="#000000" {
        point x=(px)1 y=(px)2
        point x=(px)3 y=(px)4
      }"##,
        ) else {
            panic!("polygon expected");
        };
        let got: Vec<_> = p
            .points
            .iter()
            .map(|q| (q.x.clone(), q.y.clone()))
            .collect();
        assert_eq!(got, vec![(dpx(11.0), dpx(22.0)), (dpx(13.0), dpx(24.0))]);
        let Node::Polyline(p) = moved(
            r##"polyline id="p" stroke="#000000" {
        point x=(px)1 y=(px)2
      }"##,
        ) else {
            panic!("polyline expected");
        };
        assert_eq!(p.points[0].x, dpx(11.0));
        assert_eq!(p.points[0].y, dpx(22.0));
    }

    #[test]
    fn path_moves_anchors_handles_and_subpaths() {
        let Node::Path(p) = moved(
            r##"path id="p" fill="#000000" {
        anchor x=(px)1 y=(px)2 in-x=(px)3 in-y=(px)4 out-x=(px)5 out-y=(px)6
        subpath {
          anchor x=(px)7 y=(px)8
        }
      }"##,
        ) else {
            panic!("path expected");
        };
        let a = &p.anchors[0];
        assert_eq!(
            (&a.x, &a.y, &a.in_x, &a.in_y, &a.out_x, &a.out_y),
            (
                &dpx(11.0),
                &dpx(22.0),
                &dpx(13.0),
                &dpx(24.0),
                &dpx(15.0),
                &dpx(26.0)
            )
        );
        let s = &p.subpaths[0].anchors[0];
        assert_eq!((&s.x, &s.y), (&dpx(17.0), &dpx(28.0)));
    }

    #[test]
    fn connector_and_footnote_do_not_move() {
        let before = nodes(r#"connector id="c" from="a" to="b""#);
        let mut after = before.clone();
        translate_node(&mut after[0], 10.0, 20.0, &BTreeMap::new());
        assert_eq!(before, after);
        let before = nodes(r#"footnote id="n" { span "x" }"#);
        let mut after = before.clone();
        translate_node(&mut after[0], 10.0, 20.0, &BTreeMap::new());
        assert_eq!(before, after);
    }
}
