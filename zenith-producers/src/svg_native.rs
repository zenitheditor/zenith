use std::collections::BTreeMap;

use usvg::tiny_skia_path::{PathSegment, Point};
use usvg::{NodeKind, Paint, TreeParsing, Visibility};
use zenith_core::{
    AnchorKind, Dimension, Node, PathAnchor, PathNode, PathSubpath, PropertyValue, Unit,
};

use crate::ProduceError;

/// Options for converting an SVG into editable Zenith path nodes.
#[derive(Debug, Clone, PartialEq)]
pub struct SvgNativeOptions {
    /// Prefix used to produce stable path ids, e.g. `icon`.
    pub id_prefix: String,
    /// Token/value used for stroked SVG paths.
    pub stroke: Option<PropertyValue>,
    /// Token/value used for filled SVG paths.
    pub fill: Option<PropertyValue>,
    /// Token/value used for SVG stroke width.
    pub stroke_width: Option<PropertyValue>,
}

/// Convert supported SVG vector paths into editable Zenith [`Node::Path`] nodes.
///
/// The converter uses `usvg`, so SVG primitives and arc commands are normalized
/// before conversion. Unsupported nested raster/text nodes are ignored. A path
/// with neither supported fill nor stroke is skipped.
pub fn svg_to_native_paths(
    bytes: &[u8],
    options: &SvgNativeOptions,
) -> Result<Vec<Node>, ProduceError> {
    let opts = usvg::Options::default();
    let tree =
        usvg::Tree::from_data(bytes, &opts).map_err(|e| ProduceError::SvgNative(e.to_string()))?;

    let mut nodes = Vec::new();
    let mut index = 0usize;
    collect_nodes(
        &tree.root,
        Affine::identity(),
        options,
        &mut nodes,
        &mut index,
    );
    Ok(nodes)
}

fn collect_nodes(
    node: &usvg::Node,
    parent_to_scene: Affine,
    options: &SvgNativeOptions,
    out: &mut Vec<Node>,
    index: &mut usize,
) {
    let kind = node.borrow();
    let local_to_scene = parent_to_scene.then(Affine::from_usvg(kind.transform()));
    match &*kind {
        NodeKind::Group(_) => {
            for child in node.children() {
                collect_nodes(&child, local_to_scene, options, out, index);
            }
        }
        NodeKind::Path(path) => {
            if let Some(node) = convert_path(path, local_to_scene, options, *index) {
                *index += 1;
                out.push(Node::Path(node));
            }
        }
        NodeKind::Image(_) | NodeKind::Text(_) => {}
    }
}

fn convert_path(
    path: &usvg::Path,
    transform: Affine,
    options: &SvgNativeOptions,
    index: usize,
) -> Option<PathNode> {
    if path.visibility != Visibility::Visible {
        return None;
    }

    let stroke = path
        .stroke
        .as_ref()
        .and_then(|stroke| supported_paint(&stroke.paint).then(|| options.stroke.clone()))
        .flatten();
    let fill = path
        .fill
        .as_ref()
        .and_then(|fill| supported_paint(&fill.paint).then(|| options.fill.clone()))
        .flatten();

    if stroke.is_none() && fill.is_none() {
        return None;
    }

    let subpaths = path_subpaths(path, transform);
    if subpaths.is_empty() {
        return None;
    }

    Some(PathNode {
        id: format!("{}.{index}", options.id_prefix),
        name: None,
        role: Some("icon.native".to_owned()),
        closed: None,
        fill,
        stroke,
        stroke_width: options.stroke_width.clone(),
        stroke_alignment: None,
        stroke_linejoin: path.stroke.as_ref().and_then(stroke_linejoin),
        stroke_linecap: path.stroke.as_ref().and_then(stroke_linecap),
        stroke_miter_limit: None,
        fill_rule: path.fill.as_ref().and_then(|fill| match fill.rule {
            usvg::FillRule::NonZero => None,
            usvg::FillRule::EvenOdd => Some("evenodd".to_owned()),
        }),
        opacity: None,
        visible: None,
        locked: None,
        rotate: None,
        style: None,
        anchors: Vec::new(),
        subpaths,
        source_span: None,
        unknown_props: BTreeMap::new(),
    })
}

fn supported_paint(paint: &Paint) -> bool {
    matches!(paint, Paint::Color(_))
}

fn stroke_linecap(stroke: &usvg::Stroke) -> Option<String> {
    match stroke.linecap {
        usvg::LineCap::Butt => None,
        usvg::LineCap::Round => Some("round".to_owned()),
        usvg::LineCap::Square => Some("square".to_owned()),
    }
}

fn stroke_linejoin(stroke: &usvg::Stroke) -> Option<String> {
    match stroke.linejoin {
        usvg::LineJoin::Miter | usvg::LineJoin::MiterClip => None,
        usvg::LineJoin::Round => Some("round".to_owned()),
        usvg::LineJoin::Bevel => Some("bevel".to_owned()),
    }
}

fn path_subpaths(path: &usvg::Path, transform: Affine) -> Vec<PathSubpath> {
    let mut subpaths = Vec::new();
    let mut current = Vec::new();
    let mut closed = false;
    let mut cur = (0.0_f64, 0.0_f64);

    for segment in path.data.segments() {
        match segment {
            PathSegment::MoveTo(p) => {
                push_subpath(&mut subpaths, &mut current, closed);
                closed = false;
                let (x, y) = transform.map_pt(p);
                current.push(anchor(x, y));
                cur = (x, y);
            }
            PathSegment::LineTo(p) => {
                let (x, y) = transform.map_pt(p);
                current.push(anchor(x, y));
                cur = (x, y);
            }
            PathSegment::QuadTo(p0, p1) => {
                let (qx, qy) = transform.map_pt(p0);
                let (ex, ey) = transform.map_pt(p1);
                let (sx, sy) = cur;
                let c1 = (sx + 2.0 / 3.0 * (qx - sx), sy + 2.0 / 3.0 * (qy - sy));
                let c2 = (ex + 2.0 / 3.0 * (qx - ex), ey + 2.0 / 3.0 * (qy - ey));
                add_cubic(&mut current, c1, c2, (ex, ey));
                cur = (ex, ey);
            }
            PathSegment::CubicTo(p0, p1, p2) => {
                let c1 = transform.map_pt(p0);
                let c2 = transform.map_pt(p1);
                let end = transform.map_pt(p2);
                add_cubic(&mut current, c1, c2, end);
                cur = end;
            }
            PathSegment::Close => {
                closed = true;
            }
        }
    }

    push_subpath(&mut subpaths, &mut current, closed);
    subpaths
}

fn push_subpath(subpaths: &mut Vec<PathSubpath>, current: &mut Vec<PathAnchor>, closed: bool) {
    if current.is_empty() {
        return;
    }
    subpaths.push(PathSubpath {
        closed: Some(closed),
        anchors: std::mem::take(current),
    });
}

fn add_cubic(anchors: &mut Vec<PathAnchor>, c1: (f64, f64), c2: (f64, f64), end: (f64, f64)) {
    let Some(last) = anchors.last_mut() else {
        return;
    };
    last.out_x = Some(px(c1.0));
    last.out_y = Some(px(c1.1));

    let mut next = anchor(end.0, end.1);
    next.in_x = Some(px(c2.0));
    next.in_y = Some(px(c2.1));
    next.kind = Some(AnchorKind::Smooth);
    anchors.push(next);
}

fn anchor(x: f64, y: f64) -> PathAnchor {
    PathAnchor {
        x: Some(px(x)),
        y: Some(px(y)),
        kind: None,
        in_x: None,
        in_y: None,
        out_x: None,
        out_y: None,
    }
}

fn px(value: f64) -> Dimension {
    Dimension {
        value,
        unit: Unit::Px,
    }
}

/// A 2-D affine map `(x, y) -> (a*x + c*y + e, b*x + d*y + f)`.
#[derive(Clone, Copy)]
struct Affine {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}

impl Affine {
    fn identity() -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 0.0,
            f: 0.0,
        }
    }

    fn from_usvg(t: usvg::Transform) -> Self {
        Self {
            a: f64::from(t.sx),
            b: f64::from(t.ky),
            c: f64::from(t.kx),
            d: f64::from(t.sy),
            e: f64::from(t.tx),
            f: f64::from(t.ty),
        }
    }

    fn then(self, next: Self) -> Self {
        Self {
            a: next.a * self.a + next.c * self.b,
            b: next.b * self.a + next.d * self.b,
            c: next.a * self.c + next.c * self.d,
            d: next.b * self.c + next.d * self.d,
            e: next.a * self.e + next.c * self.f + next.e,
            f: next.b * self.e + next.d * self.f + next.f,
        }
    }

    fn map_pt(self, p: Point) -> (f64, f64) {
        let x = f64::from(p.x);
        let y = f64::from(p.y);
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_lucide_primitives_to_native_paths() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="20" height="14" x="2" y="3" rx="2"/><line x1="8" x2="16" y1="21" y2="21"/></svg>"#;
        let nodes = svg_to_native_paths(
            svg,
            &SvgNativeOptions {
                id_prefix: "icon".to_owned(),
                stroke: Some(PropertyValue::TokenRef("lib.icons.stroke".to_owned())),
                fill: None,
                stroke_width: Some(PropertyValue::TokenRef("lib.icons.stroke_width".to_owned())),
            },
        )
        .expect("convert");

        assert!(nodes.len() >= 2, "nodes: {nodes:?}");
        let Node::Path(first) = &nodes[0] else {
            panic!("expected path");
        };
        assert_eq!(first.id, "icon.0");
        assert_eq!(
            first.stroke,
            Some(PropertyValue::TokenRef("lib.icons.stroke".to_owned()))
        );
        assert_eq!(first.fill, None);
        assert_eq!(first.stroke_linecap.as_deref(), Some("round"));
        assert_eq!(first.stroke_linejoin.as_deref(), Some("round"));
        assert!(
            first
                .subpaths
                .iter()
                .flat_map(|subpath| &subpath.anchors)
                .any(|anchor| anchor.in_x.is_some() || anchor.out_x.is_some()),
            "rounded rect should contain bezier handles: {first:?}"
        );
    }

    #[test]
    fn skips_unpainted_paths() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><path d="M0 0h10"/></svg>"#;
        let nodes = svg_to_native_paths(
            svg,
            &SvgNativeOptions {
                id_prefix: "empty".to_owned(),
                stroke: None,
                fill: None,
                stroke_width: None,
            },
        )
        .expect("convert");
        assert!(nodes.is_empty());
    }

    #[test]
    fn preserves_square_and_bevel_stroke_style() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10" fill="none" stroke="black" stroke-width="1" stroke-linecap="square" stroke-linejoin="bevel"><path d="M1 9L5 1L9 9"/></svg>"#;
        let nodes = svg_to_native_paths(
            svg,
            &SvgNativeOptions {
                id_prefix: "style".to_owned(),
                stroke: Some(PropertyValue::Literal("ink".to_owned())),
                fill: None,
                stroke_width: Some(PropertyValue::Literal("1px".to_owned())),
            },
        )
        .expect("convert");

        let Node::Path(path) = &nodes[0] else {
            panic!("expected path");
        };
        assert_eq!(path.stroke_linecap.as_deref(), Some("square"));
        assert_eq!(path.stroke_linejoin.as_deref(), Some("bevel"));
    }
}
