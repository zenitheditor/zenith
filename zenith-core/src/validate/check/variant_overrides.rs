//! Value and node-kind checks for the properties of one variant `override`.
//!
//! Checks performed on an override whose target node resolved:
//!
//! - **`variant.override_unsupported_property`** (Error): the override sets a
//!   property the target node kind does not have. `font-size` needs a text
//!   kind. `layout` and the container attributes need a `frame`. A `hug` /
//!   `fill` size keyword needs a box node.
//! - Value checks reuse the node codes: `node.invalid_value` for an unknown
//!   `layout` / `justify` / `align` word, `node.invalid_geometry` for a bad
//!   `gap` / `padding*` value, and the `token.*` codes for `fill` and
//!   `font-size`.
//!
//! Every token ref an override names counts as used, so a token that only a
//! variant references does not raise `token.unused`.

use crate::ast::node::{LayoutAlign, LayoutJustify, LayoutKind, Node};
use crate::ast::variant::{VariantDef, VariantOverride};
use crate::diagnostics::Diagnostic;
use crate::schema::enums::{LAYOUT_ALIGNS, LAYOUT_JUSTIFIES, LAYOUT_KINDS};

use super::nodes::{TokenEnv, check_optional_dim, push_invalid_value};
use super::visual::{VisualExpect, attach_visual_spans, check_visual_prop};

/// Check the property values of `ov`.
///
/// `target` is the node `ov.node` names, when it resolved in the source page.
/// Without it only the token refs are checked, so the kind checks never
/// report a node that `variant.override_unknown_node` already reports.
pub(in crate::validate::check) fn check_override_props(
    variant: &VariantDef,
    ov: &VariantOverride,
    target: Option<&Node>,
    tokens: &mut TokenEnv<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let from = diagnostics.len();
    check_visual_prop(
        &ov.node,
        "fill",
        ov.fill.as_ref(),
        VisualExpect::ColorOrGradient,
        &mut *tokens.referenced,
        tokens.resolved,
        diagnostics,
    );
    check_visual_prop(
        &ov.node,
        "font-size",
        ov.font_size.as_ref(),
        VisualExpect::Dimension,
        &mut *tokens.referenced,
        tokens.resolved,
        diagnostics,
    );
    attach_visual_spans(diagnostics, from, ov.source_span);
    check_layout_values(variant, ov, tokens, diagnostics);

    let Some(node) = target else {
        return;
    };
    let kind = node.kind_str();
    let mut unsupported: Vec<&str> = Vec::new();
    if ov.font_size.is_some() && !has_font_size(node) {
        unsupported.push("font-size");
    }
    if !matches!(node, Node::Frame(_)) {
        unsupported.extend(frame_props_set(ov));
    }
    if node.layout_item().is_none() {
        if ov.w_keyword.is_some() {
            unsupported.push("w");
        }
        if ov.h_keyword.is_some() {
            unsupported.push("h");
        }
    }
    for prop in unsupported {
        let fix = match prop {
            "font-size" => "target a text, code, field, footnote, or toc node",
            "w" | "h" => "use a dimension such as (px)800",
            _ => "target a frame",
        };
        diagnostics.push(Diagnostic::error(
            "variant.override_unsupported_property",
            format!(
                "variant '{}': override for node '{}' sets '{prop}', which a {kind} node \
                 does not support; {fix}, or remove '{prop}'",
                variant.id, ov.node
            ),
            ov.source_span,
            Some(variant.id.clone()),
        ));
    }
}

/// Check the `layout` word, the container words, and the container
/// dimensions of `ov`.
fn check_layout_values(
    variant: &VariantDef,
    ov: &VariantOverride,
    tokens: &mut TokenEnv<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let subject = format!("variant '{}': override for node '{}'", variant.id, ov.node);
    let id = Some(variant.id.clone());
    let span = ov.source_span;
    if let Some(LayoutKind::Unknown(v)) = &ov.layout {
        push_invalid_value(
            &subject,
            id.clone(),
            "layout",
            v,
            LAYOUT_KINDS,
            span,
            diagnostics,
        );
    }
    let c = &ov.container;
    if let Some(LayoutJustify::Unknown(v)) = &c.justify {
        push_invalid_value(
            &subject,
            id.clone(),
            "justify",
            v,
            LAYOUT_JUSTIFIES,
            span,
            diagnostics,
        );
    }
    if let Some(LayoutAlign::Unknown(v)) = &c.align {
        push_invalid_value(&subject, id, "align", v, LAYOUT_ALIGNS, span, diagnostics);
    }
    for (prop, value) in [
        ("gap", c.gap.as_ref()),
        ("wrap-gap", c.wrap_gap.as_ref()),
        ("padding", c.padding.as_ref()),
        ("padding-x", c.padding_x.as_ref()),
        ("padding-y", c.padding_y.as_ref()),
        ("padding-top", c.padding_top.as_ref()),
        ("padding-right", c.padding_right.as_ref()),
        ("padding-bottom", c.padding_bottom.as_ref()),
        ("padding-left", c.padding_left.as_ref()),
    ] {
        check_optional_dim(&ov.node, prop, value, false, span, tokens, diagnostics);
    }
}

/// The frame-only properties `ov` sets, in canonical order.
fn frame_props_set(ov: &VariantOverride) -> Vec<&'static str> {
    let c = &ov.container;
    [
        ("layout", ov.layout.is_some()),
        ("gap", c.gap.is_some()),
        ("wrap-gap", c.wrap_gap.is_some()),
        ("padding", c.padding.is_some()),
        ("padding-x", c.padding_x.is_some()),
        ("padding-y", c.padding_y.is_some()),
        ("padding-top", c.padding_top.is_some()),
        ("padding-right", c.padding_right.is_some()),
        ("padding-bottom", c.padding_bottom.is_some()),
        ("padding-left", c.padding_left.is_some()),
        ("justify", c.justify.is_some()),
        ("align", c.align.is_some()),
        ("wrap", c.wrap.is_some()),
    ]
    .into_iter()
    .filter_map(|(name, set)| set.then_some(name))
    .collect()
}

/// `true` for the node kinds that carry a `font-size` attribute.
fn has_font_size(node: &Node) -> bool {
    match node {
        Node::Text(_) | Node::Code(_) | Node::Field(_) | Node::Footnote(_) | Node::Toc(_) => true,
        Node::Rect(_)
        | Node::Ellipse(_)
        | Node::Line(_)
        | Node::Frame(_)
        | Node::Group(_)
        | Node::Image(_)
        | Node::Polygon(_)
        | Node::Polyline(_)
        | Node::Path(_)
        | Node::Instance(_)
        | Node::Table(_)
        | Node::Shape(_)
        | Node::Connector(_)
        | Node::Pattern(_)
        | Node::Chart(_)
        | Node::Light(_)
        | Node::Mesh(_)
        | Node::Unknown(_) => false,
    }
}

/// The node with id `id` in `nodes` or their descendants.
pub(in crate::validate::check) fn find_node<'a>(nodes: &'a [Node], id: &str) -> Option<&'a Node> {
    nodes.iter().find_map(|n| {
        if n.id() == Some(id) {
            return Some(n);
        }
        n.children().and_then(|children| find_node(children, id))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{KdlAdapter, KdlSource};

    fn doc(src: &str) -> crate::Document {
        KdlAdapter.parse(src.as_bytes()).expect("fixture parses")
    }

    const SRC: &str = r##"zenith version=1 {
  project id="p" name="P"
  tokens format="zenith-token-v1" {
    token id="size.t" type="dimension" value=(px)40
  }
  styles {}
  variants {
    variant id="v" source="pg" w=(px)200 h=(px)100 {
      override node="f" layout="row" gap=(px)8 font-size=(token)"size.t"
    }
  }
  document id="d" title="D" {
    page id="pg" w=(px)100 h=(px)100 {
      frame id="f" x=(px)0 y=(px)0 w=(px)100 h=(px)100 layout="column" {
        text id="t" w=(px)80 { span "a" }
      }
    }
  }
}
"##;

    #[test]
    fn find_node_walks_descendants() {
        let d = doc(SRC);
        let page = &d.body.pages[0];
        assert_eq!(
            find_node(&page.children, "t").map(Node::kind_str),
            Some("text")
        );
        assert_eq!(
            find_node(&page.children, "f").map(Node::kind_str),
            Some("frame")
        );
        assert!(find_node(&page.children, "missing").is_none());
    }

    #[test]
    fn frame_props_set_lists_layout_and_container_fields() {
        let d = doc(SRC);
        let ov = &d.variants[0].overrides[0];
        assert_eq!(frame_props_set(ov), ["layout", "gap"]);
        assert!(ov.sets_frame_layout());
    }

    #[test]
    fn font_size_on_frame_is_unsupported() {
        let d = doc(SRC);
        let variant = &d.variants[0];
        let ov = &variant.overrides[0];
        let target = find_node(&d.body.pages[0].children, "f");
        let resolved = std::collections::BTreeMap::new();
        let mut referenced = std::collections::BTreeSet::new();
        let mut tokens = TokenEnv {
            referenced: &mut referenced,
            resolved: &resolved,
        };
        let mut diagnostics = Vec::new();
        check_override_props(variant, ov, target, &mut tokens, &mut diagnostics);
        assert!(referenced.contains("size.t"));
        let unsupported: Vec<&str> = diagnostics
            .iter()
            .filter(|d| d.code == "variant.override_unsupported_property")
            .map(|d| d.message.as_str())
            .collect();
        assert_eq!(unsupported.len(), 1, "{diagnostics:?}");
        assert!(unsupported[0].contains("'font-size'"));
        assert!(unsupported[0].contains("frame node"));
    }
}
