//! Canonical field accessors on [`Node`].
//!
//! Every node kind spreads common fields (`id`, `role`, `visible`, …) across
//! its struct. Call sites must not re-match the enum for those fields — use
//! these methods so a new variant is a compile error in one place only.

use super::{LayoutItem, Node};
use crate::ast::Span;
use crate::ast::value::PropertyValue;

/// Borrowed geometry of a box node, returned by [`Node::box_view`].
#[derive(Debug, Clone, Copy)]
pub struct BoxView<'a> {
    pub x: Option<&'a PropertyValue>,
    pub y: Option<&'a PropertyValue>,
    pub w: Option<&'a PropertyValue>,
    pub h: Option<&'a PropertyValue>,
    /// `true` when any `anchor*` attribute is authored.
    pub anchored: bool,
    pub layout_item: &'a LayoutItem,
}

impl Node {
    /// Stable authored id, if any. `Unknown` nodes may omit `id`.
    pub fn id(&self) -> Option<&str> {
        match self {
            Node::Rect(n) => Some(n.id.as_str()),
            Node::Ellipse(n) => Some(n.id.as_str()),
            Node::Line(n) => Some(n.id.as_str()),
            Node::Text(n) => Some(n.id.as_str()),
            Node::Code(n) => Some(n.id.as_str()),
            Node::Frame(n) => Some(n.id.as_str()),
            Node::Group(n) => Some(n.id.as_str()),
            Node::Image(n) => Some(n.id.as_str()),
            Node::Polygon(n) => Some(n.id.as_str()),
            Node::Polyline(n) => Some(n.id.as_str()),
            Node::Path(n) => Some(n.id.as_str()),
            Node::Instance(n) => Some(n.id.as_str()),
            Node::Field(n) => Some(n.id.as_str()),
            Node::Toc(n) => Some(n.id.as_str()),
            Node::Footnote(n) => Some(n.id.as_str()),
            Node::Table(n) => Some(n.id.as_str()),
            Node::Shape(n) => Some(n.id.as_str()),
            Node::Connector(n) => Some(n.id.as_str()),
            Node::Pattern(n) => Some(n.id.as_str()),
            Node::Chart(n) => Some(n.id.as_str()),
            Node::Light(n) => Some(n.id.as_str()),
            Node::Mesh(n) => Some(n.id.as_str()),
            Node::Unknown(n) => n.id.as_deref(),
        }
    }

    /// Mutable handle to the authored id string, when the variant has one.
    ///
    /// `Unknown` yields `None` even when an optional id is present (callers
    /// that need to mutate unknown ids must match `Node::Unknown` directly).
    pub fn id_mut(&mut self) -> Option<&mut String> {
        match self {
            Node::Rect(n) => Some(&mut n.id),
            Node::Ellipse(n) => Some(&mut n.id),
            Node::Line(n) => Some(&mut n.id),
            Node::Text(n) => Some(&mut n.id),
            Node::Code(n) => Some(&mut n.id),
            Node::Frame(n) => Some(&mut n.id),
            Node::Group(n) => Some(&mut n.id),
            Node::Image(n) => Some(&mut n.id),
            Node::Polygon(n) => Some(&mut n.id),
            Node::Polyline(n) => Some(&mut n.id),
            Node::Path(n) => Some(&mut n.id),
            Node::Instance(n) => Some(&mut n.id),
            Node::Field(n) => Some(&mut n.id),
            Node::Toc(n) => Some(&mut n.id),
            Node::Footnote(n) => Some(&mut n.id),
            Node::Table(n) => Some(&mut n.id),
            Node::Shape(n) => Some(&mut n.id),
            Node::Connector(n) => Some(&mut n.id),
            Node::Pattern(n) => Some(&mut n.id),
            Node::Chart(n) => Some(&mut n.id),
            Node::Light(n) => Some(&mut n.id),
            Node::Mesh(n) => Some(&mut n.id),
            Node::Unknown(_) => None,
        }
    }

    /// Id string for diagnostics: authored id when present, otherwise the
    /// unknown node's kind name (never empty for a well-formed node).
    pub fn id_or_kind(&self) -> &str {
        match self {
            Node::Rect(n) => n.id.as_str(),
            Node::Ellipse(n) => n.id.as_str(),
            Node::Line(n) => n.id.as_str(),
            Node::Text(n) => n.id.as_str(),
            Node::Code(n) => n.id.as_str(),
            Node::Frame(n) => n.id.as_str(),
            Node::Group(n) => n.id.as_str(),
            Node::Image(n) => n.id.as_str(),
            Node::Polygon(n) => n.id.as_str(),
            Node::Polyline(n) => n.id.as_str(),
            Node::Path(n) => n.id.as_str(),
            Node::Instance(n) => n.id.as_str(),
            Node::Field(n) => n.id.as_str(),
            Node::Toc(n) => n.id.as_str(),
            Node::Footnote(n) => n.id.as_str(),
            Node::Table(n) => n.id.as_str(),
            Node::Shape(n) => n.id.as_str(),
            Node::Connector(n) => n.id.as_str(),
            Node::Pattern(n) => n.id.as_str(),
            Node::Chart(n) => n.id.as_str(),
            Node::Light(n) => n.id.as_str(),
            Node::Mesh(n) => n.id.as_str(),
            Node::Unknown(n) => n.id.as_deref().unwrap_or(n.kind.as_str()),
        }
    }

    /// `true` when this node's own `role` is `decoration` or `background`.
    ///
    /// Callers fold it with the inherited flag so the exemption covers the
    /// whole subtree. Lint and placement advisories skip exempt nodes.
    pub fn is_decorative(&self) -> bool {
        matches!(self.role(), Some("decoration" | "background"))
    }

    /// Optional `role` attribute (`guide`, block roles, …). `Unknown` has none.
    pub fn role(&self) -> Option<&str> {
        match self {
            Node::Rect(n) => n.role.as_deref(),
            Node::Ellipse(n) => n.role.as_deref(),
            Node::Line(n) => n.role.as_deref(),
            Node::Text(n) => n.role.as_deref(),
            Node::Code(n) => n.role.as_deref(),
            Node::Frame(n) => n.role.as_deref(),
            Node::Group(n) => n.role.as_deref(),
            Node::Image(n) => n.role.as_deref(),
            Node::Polygon(n) => n.role.as_deref(),
            Node::Polyline(n) => n.role.as_deref(),
            Node::Path(n) => n.role.as_deref(),
            Node::Instance(n) => n.role.as_deref(),
            Node::Field(n) => n.role.as_deref(),
            Node::Toc(n) => n.role.as_deref(),
            Node::Footnote(n) => n.role.as_deref(),
            Node::Table(n) => n.role.as_deref(),
            Node::Shape(n) => n.role.as_deref(),
            Node::Connector(n) => n.role.as_deref(),
            Node::Pattern(n) => n.role.as_deref(),
            Node::Chart(n) => n.role.as_deref(),
            Node::Light(n) => n.role.as_deref(),
            Node::Mesh(n) => n.role.as_deref(),
            Node::Unknown(_) => None,
        }
    }

    /// Authored `visible` flag when the variant carries one.
    ///
    /// `None` means the attribute was omitted (default visible) or the variant
    /// has no such field (`Footnote`, `Unknown`).
    pub fn visible(&self) -> Option<bool> {
        match self {
            Node::Rect(n) => n.visible,
            Node::Ellipse(n) => n.visible,
            Node::Line(n) => n.visible,
            Node::Text(n) => n.visible,
            Node::Code(n) => n.visible,
            Node::Frame(n) => n.visible,
            Node::Group(n) => n.visible,
            Node::Image(n) => n.visible,
            Node::Polygon(n) => n.visible,
            Node::Polyline(n) => n.visible,
            Node::Path(n) => n.visible,
            Node::Instance(n) => n.visible,
            Node::Field(n) => n.visible,
            Node::Toc(n) => n.visible,
            Node::Table(n) => n.visible,
            Node::Shape(n) => n.visible,
            Node::Connector(n) => n.visible,
            Node::Pattern(n) => n.visible,
            Node::Chart(n) => n.visible,
            Node::Light(n) => n.visible,
            Node::Mesh(n) => n.visible,
            Node::Footnote(_) | Node::Unknown(_) => None,
        }
    }

    /// Effective visibility: omitted / unsupported → visible (`true`).
    pub fn is_visible(&self) -> bool {
        self.visible().unwrap_or(true)
    }

    /// Authored opacity multiplier when present.
    pub fn opacity(&self) -> Option<f64> {
        match self {
            Node::Rect(n) => n.opacity,
            Node::Ellipse(n) => n.opacity,
            Node::Line(n) => n.opacity,
            Node::Text(n) => n.opacity,
            Node::Code(n) => n.opacity,
            Node::Frame(n) => n.opacity,
            Node::Group(n) => n.opacity,
            Node::Image(n) => n.opacity,
            Node::Polygon(n) => n.opacity,
            Node::Polyline(n) => n.opacity,
            Node::Path(n) => n.opacity,
            Node::Instance(n) => n.opacity,
            Node::Field(n) => n.opacity,
            Node::Toc(n) => n.opacity,
            Node::Table(n) => n.opacity,
            Node::Shape(n) => n.opacity,
            Node::Connector(n) => n.opacity,
            Node::Pattern(n) => n.opacity,
            Node::Chart(n) => n.opacity,
            Node::Light(n) => n.opacity,
            Node::Mesh(n) => n.opacity,
            Node::Footnote(_) | Node::Unknown(_) => None,
        }
    }

    /// Style id reference when the variant carries `style`.
    pub fn style_ref(&self) -> Option<&str> {
        match self {
            Node::Rect(n) => n.style.as_deref(),
            Node::Ellipse(n) => n.style.as_deref(),
            Node::Line(n) => n.style.as_deref(),
            Node::Text(n) => n.style.as_deref(),
            Node::Code(n) => n.style.as_deref(),
            Node::Frame(n) => n.style.as_deref(),
            Node::Group(n) => n.style.as_deref(),
            Node::Image(n) => n.style.as_deref(),
            Node::Polygon(n) => n.style.as_deref(),
            Node::Polyline(n) => n.style.as_deref(),
            Node::Path(n) => n.style.as_deref(),
            Node::Field(n) => n.style.as_deref(),
            Node::Toc(n) => n.style.as_deref(),
            Node::Footnote(n) => n.style.as_deref(),
            Node::Table(n) => n.style.as_deref(),
            Node::Shape(n) => n.style.as_deref(),
            Node::Connector(n) => n.style.as_deref(),
            Node::Pattern(n) => n.style.as_deref(),
            Node::Chart(n) => n.style.as_deref(),
            Node::Instance(_) | Node::Light(_) | Node::Mesh(_) | Node::Unknown(_) => None,
        }
    }

    /// Source span from the parse, when retained.
    pub fn source_span(&self) -> Option<Span> {
        match self {
            Node::Rect(n) => n.source_span,
            Node::Ellipse(n) => n.source_span,
            Node::Line(n) => n.source_span,
            Node::Text(n) => n.source_span,
            Node::Code(n) => n.source_span,
            Node::Frame(n) => n.source_span,
            Node::Group(n) => n.source_span,
            Node::Image(n) => n.source_span,
            Node::Polygon(n) => n.source_span,
            Node::Polyline(n) => n.source_span,
            Node::Path(n) => n.source_span,
            Node::Instance(n) => n.source_span,
            Node::Field(n) => n.source_span,
            Node::Toc(n) => n.source_span,
            Node::Footnote(n) => n.source_span,
            Node::Table(n) => n.source_span,
            Node::Shape(n) => n.source_span,
            Node::Connector(n) => n.source_span,
            Node::Pattern(n) => n.source_span,
            Node::Chart(n) => n.source_span,
            Node::Light(n) => n.source_span,
            Node::Mesh(n) => n.source_span,
            Node::Unknown(n) => n.source_span,
        }
    }

    /// Id and source span together — the diagnostic address of a node.
    pub fn id_and_span(&self) -> (&str, Option<Span>) {
        (self.id_or_kind(), self.source_span())
    }

    /// Auto-layout item attributes for the box-node kinds and `instance`;
    /// `None` for kinds without an `x`/`y`/`w`/`h` box.
    pub fn layout_item(&self) -> Option<&LayoutItem> {
        if let Node::Instance(n) = self {
            return Some(&n.layout_item);
        }
        self.box_view().map(|b| b.layout_item)
    }

    /// Mutable auto-layout item attributes; the same kinds as
    /// [`Node::layout_item`] return `Some`.
    pub fn layout_item_mut(&mut self) -> Option<&mut LayoutItem> {
        match self {
            Node::Rect(n) => Some(&mut n.layout_item),
            Node::Ellipse(n) => Some(&mut n.layout_item),
            Node::Text(n) => Some(&mut n.layout_item),
            Node::Code(n) => Some(&mut n.layout_item),
            Node::Frame(n) => Some(&mut n.layout_item),
            Node::Group(n) => Some(&mut n.layout_item),
            Node::Image(n) => Some(&mut n.layout_item),
            Node::Instance(n) => Some(&mut n.layout_item),
            Node::Field(n) => Some(&mut n.layout_item),
            Node::Toc(n) => Some(&mut n.layout_item),
            Node::Table(n) => Some(&mut n.layout_item),
            Node::Shape(n) => Some(&mut n.layout_item),
            Node::Pattern(n) => Some(&mut n.layout_item),
            Node::Chart(n) => Some(&mut n.layout_item),
            Node::Mesh(n) => Some(&mut n.layout_item),
            Node::Line(_)
            | Node::Polygon(_)
            | Node::Polyline(_)
            | Node::Path(_)
            | Node::Footnote(_)
            | Node::Connector(_)
            | Node::Light(_)
            | Node::Unknown(_) => None,
        }
    }

    /// Borrowed box geometry for the box-node kinds; `None` for kinds without
    /// an `x`/`y`/`w`/`h` box (line, polygon, polyline, path, instance,
    /// footnote, connector, light, unknown).
    pub fn box_view(&self) -> Option<BoxView<'_>> {
        match self {
            Node::Rect(n) => Some(BoxView {
                x: n.x.as_ref(),
                y: n.y.as_ref(),
                w: n.w.as_ref(),
                h: n.h.as_ref(),
                anchored: n.anchor.is_some()
                    || n.anchor_zone.is_some()
                    || n.anchor_sibling.is_some()
                    || n.anchor_edge.is_some()
                    || n.anchor_gap.is_some()
                    || n.anchor_parent.is_some(),
                layout_item: &n.layout_item,
            }),
            Node::Ellipse(n) => Some(BoxView {
                x: n.x.as_ref(),
                y: n.y.as_ref(),
                w: n.w.as_ref(),
                h: n.h.as_ref(),
                anchored: n.anchor.is_some()
                    || n.anchor_zone.is_some()
                    || n.anchor_sibling.is_some()
                    || n.anchor_edge.is_some()
                    || n.anchor_gap.is_some()
                    || n.anchor_parent.is_some(),
                layout_item: &n.layout_item,
            }),
            Node::Text(n) => Some(BoxView {
                x: n.x.as_ref(),
                y: n.y.as_ref(),
                w: n.w.as_ref(),
                h: n.h.as_ref(),
                anchored: n.anchor.is_some()
                    || n.anchor_zone.is_some()
                    || n.anchor_sibling.is_some()
                    || n.anchor_edge.is_some()
                    || n.anchor_gap.is_some()
                    || n.anchor_parent.is_some(),
                layout_item: &n.layout_item,
            }),
            Node::Code(n) => Some(BoxView {
                x: n.x.as_ref(),
                y: n.y.as_ref(),
                w: n.w.as_ref(),
                h: n.h.as_ref(),
                anchored: n.anchor.is_some()
                    || n.anchor_zone.is_some()
                    || n.anchor_sibling.is_some()
                    || n.anchor_edge.is_some()
                    || n.anchor_gap.is_some()
                    || n.anchor_parent.is_some(),
                layout_item: &n.layout_item,
            }),
            Node::Frame(n) => Some(BoxView {
                x: n.x.as_ref(),
                y: n.y.as_ref(),
                w: n.w.as_ref(),
                h: n.h.as_ref(),
                anchored: n.anchor.is_some()
                    || n.anchor_zone.is_some()
                    || n.anchor_sibling.is_some()
                    || n.anchor_edge.is_some()
                    || n.anchor_gap.is_some()
                    || n.anchor_parent.is_some(),
                layout_item: &n.layout_item,
            }),
            Node::Group(n) => Some(BoxView {
                x: n.x.as_ref(),
                y: n.y.as_ref(),
                w: n.w.as_ref(),
                h: n.h.as_ref(),
                anchored: n.anchor.is_some()
                    || n.anchor_zone.is_some()
                    || n.anchor_sibling.is_some()
                    || n.anchor_edge.is_some()
                    || n.anchor_gap.is_some()
                    || n.anchor_parent.is_some(),
                layout_item: &n.layout_item,
            }),
            Node::Image(n) => Some(BoxView {
                x: n.x.as_ref(),
                y: n.y.as_ref(),
                w: n.w.as_ref(),
                h: n.h.as_ref(),
                anchored: n.anchor.is_some()
                    || n.anchor_zone.is_some()
                    || n.anchor_sibling.is_some()
                    || n.anchor_edge.is_some()
                    || n.anchor_gap.is_some()
                    || n.anchor_parent.is_some(),
                layout_item: &n.layout_item,
            }),
            Node::Field(n) => Some(BoxView {
                x: n.x.as_ref(),
                y: n.y.as_ref(),
                w: n.w.as_ref(),
                h: n.h.as_ref(),
                anchored: n.anchor.is_some()
                    || n.anchor_zone.is_some()
                    || n.anchor_sibling.is_some()
                    || n.anchor_edge.is_some()
                    || n.anchor_gap.is_some()
                    || n.anchor_parent.is_some(),
                layout_item: &n.layout_item,
            }),
            Node::Toc(n) => Some(BoxView {
                x: n.x.as_ref(),
                y: n.y.as_ref(),
                w: n.w.as_ref(),
                h: n.h.as_ref(),
                anchored: n.anchor.is_some()
                    || n.anchor_zone.is_some()
                    || n.anchor_sibling.is_some()
                    || n.anchor_edge.is_some()
                    || n.anchor_gap.is_some()
                    || n.anchor_parent.is_some(),
                layout_item: &n.layout_item,
            }),
            Node::Table(n) => Some(BoxView {
                x: n.x.as_ref(),
                y: n.y.as_ref(),
                w: n.w.as_ref(),
                h: n.h.as_ref(),
                anchored: n.anchor.is_some()
                    || n.anchor_zone.is_some()
                    || n.anchor_sibling.is_some()
                    || n.anchor_edge.is_some()
                    || n.anchor_gap.is_some()
                    || n.anchor_parent.is_some(),
                layout_item: &n.layout_item,
            }),
            Node::Shape(n) => Some(BoxView {
                x: n.x.as_ref(),
                y: n.y.as_ref(),
                w: n.w.as_ref(),
                h: n.h.as_ref(),
                anchored: n.anchor.is_some()
                    || n.anchor_zone.is_some()
                    || n.anchor_sibling.is_some()
                    || n.anchor_edge.is_some()
                    || n.anchor_gap.is_some()
                    || n.anchor_parent.is_some(),
                layout_item: &n.layout_item,
            }),
            Node::Pattern(n) => Some(BoxView {
                x: n.x.as_ref(),
                y: n.y.as_ref(),
                w: n.w.as_ref(),
                h: n.h.as_ref(),
                anchored: n.anchor.is_some()
                    || n.anchor_zone.is_some()
                    || n.anchor_sibling.is_some()
                    || n.anchor_edge.is_some()
                    || n.anchor_gap.is_some()
                    || n.anchor_parent.is_some(),
                layout_item: &n.layout_item,
            }),
            Node::Chart(n) => Some(BoxView {
                x: n.x.as_ref(),
                y: n.y.as_ref(),
                w: n.w.as_ref(),
                h: n.h.as_ref(),
                anchored: n.anchor.is_some()
                    || n.anchor_zone.is_some()
                    || n.anchor_sibling.is_some()
                    || n.anchor_edge.is_some()
                    || n.anchor_gap.is_some()
                    || n.anchor_parent.is_some(),
                layout_item: &n.layout_item,
            }),
            Node::Mesh(n) => Some(BoxView {
                x: n.x.as_ref(),
                y: n.y.as_ref(),
                w: n.w.as_ref(),
                h: n.h.as_ref(),
                anchored: false,
                layout_item: &n.layout_item,
            }),
            Node::Line(_)
            | Node::Polygon(_)
            | Node::Polyline(_)
            | Node::Path(_)
            | Node::Instance(_)
            | Node::Footnote(_)
            | Node::Connector(_)
            | Node::Light(_)
            | Node::Unknown(_) => None,
        }
    }

    /// Static KDL kind name for this variant (`"rect"`, `"unknown"`, …).
    pub fn kind_str(&self) -> &'static str {
        match self {
            Node::Rect(_) => "rect",
            Node::Ellipse(_) => "ellipse",
            Node::Line(_) => "line",
            Node::Text(_) => "text",
            Node::Code(_) => "code",
            Node::Frame(_) => "frame",
            Node::Group(_) => "group",
            Node::Image(_) => "image",
            Node::Polygon(_) => "polygon",
            Node::Polyline(_) => "polyline",
            Node::Path(_) => "path",
            Node::Instance(_) => "instance",
            Node::Field(_) => "field",
            Node::Toc(_) => "toc",
            Node::Footnote(_) => "footnote",
            Node::Table(_) => "table",
            Node::Shape(_) => "shape",
            Node::Connector(_) => "connector",
            Node::Pattern(_) => "pattern",
            Node::Chart(_) => "chart",
            Node::Light(_) => "light",
            Node::Mesh(_) => "mesh",
            Node::Unknown(_) => "unknown",
        }
    }

    /// Direct child list for container variants (`frame`, `group`, `unknown`).
    pub fn children(&self) -> Option<&[Node]> {
        match self {
            Node::Frame(n) => Some(n.children.as_slice()),
            Node::Group(n) => Some(n.children.as_slice()),
            Node::Unknown(n) => Some(n.children.as_slice()),
            Node::Rect(_)
            | Node::Ellipse(_)
            | Node::Line(_)
            | Node::Text(_)
            | Node::Code(_)
            | Node::Image(_)
            | Node::Polygon(_)
            | Node::Polyline(_)
            | Node::Path(_)
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
            | Node::Mesh(_) => None,
        }
    }

    /// Mutable child list for container variants.
    pub fn children_mut(&mut self) -> Option<&mut Vec<Node>> {
        match self {
            Node::Frame(n) => Some(&mut n.children),
            Node::Group(n) => Some(&mut n.children),
            Node::Unknown(n) => Some(&mut n.children),
            Node::Rect(_)
            | Node::Ellipse(_)
            | Node::Line(_)
            | Node::Text(_)
            | Node::Code(_)
            | Node::Image(_)
            | Node::Polygon(_)
            | Node::Polyline(_)
            | Node::Path(_)
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
            | Node::Mesh(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{KdlAdapter, KdlSource};

    fn first_child(src: &str) -> Node {
        let doc = KdlAdapter
            .parse(src.as_bytes())
            .expect("test document must parse");
        doc.body
            .pages
            .into_iter()
            .next()
            .expect("page")
            .children
            .into_iter()
            .next()
            .expect("child")
    }

    #[test]
    fn id_role_visible_style_kind() {
        let n = first_child(
            r#"
zenith version=1 {
  document id="d" {
    page id="pg" w=(px)100 h=(px)100 {
      rect id="r1" role="guide" visible=#false style="s.card" opacity=0.5
    }
  }
}
"#,
        );
        assert_eq!(n.id(), Some("r1"));
        assert_eq!(n.role(), Some("guide"));
        assert_eq!(n.visible(), Some(false));
        assert!(!n.is_visible());
        assert_eq!(n.style_ref(), Some("s.card"));
        assert_eq!(n.kind_str(), "rect");
        assert_eq!(n.opacity(), Some(0.5));
        assert_eq!(n.id_or_kind(), "r1");
    }

    #[test]
    fn unknown_id_falls_back_to_kind() {
        let n = first_child(
            r#"
zenith version=1 {
  document id="d" {
    page id="pg" w=(px)100 h=(px)100 {
      sparkle
    }
  }
}
"#,
        );
        assert!(matches!(n, Node::Unknown(_)));
        assert_eq!(n.id(), None);
        assert_eq!(n.id_or_kind(), "sparkle");
        assert_eq!(n.kind_str(), "unknown");
        assert!(n.is_visible());
    }

    #[test]
    fn children_on_group() {
        let mut n = first_child(
            r#"
zenith version=1 {
  document id="d" {
    page id="pg" w=(px)100 h=(px)100 {
      group id="g" {
        rect id="c"
      }
    }
  }
}
"#,
        );
        assert_eq!(n.children().map(|c| c.len()), Some(1));
        n.children_mut().expect("group").clear();
        assert_eq!(n.children().map(|c| c.len()), Some(0));
    }

    #[test]
    fn ellipse_defaults_visible() {
        let n = first_child(
            r#"
zenith version=1 {
  document id="d" {
    page id="pg" w=(px)100 h=(px)100 {
      ellipse id="e"
    }
  }
}
"#,
        );
        assert_eq!(n.visible(), None);
        assert!(n.is_visible());
        assert_eq!(n.kind_str(), "ellipse");
    }
}
