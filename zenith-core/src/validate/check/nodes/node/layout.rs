//! Auto-layout checks: per-child item attributes (any box node) and `frame`
//! container attributes.
//!
//! Structural rules only: enum values, `min-*` / `max-*` / `gap` / `padding*`
//! dimensions, inert attributes, ignored or missing placement, and statically
//! contradictory sizes. Rules that need measured sizes (`layout.unsized_child`,
//! `layout.child_overflow`, `layout.fill_in_hug_parent`) run in the scene
//! layout engine. An `instance` carries the item attributes like a box node.

use crate::ast::Span;
use crate::ast::node::{
    FrameNode, LayoutAlign, LayoutItem, LayoutJustify, LayoutKind, LayoutPosition, Node,
    SizeKeyword,
};
use crate::ast::value::{PropertyValue, Unit, dim_to_px};
use crate::diagnostics::{Diagnostic, FixHint};
use crate::schema::enums::{LAYOUT_ALIGNS, LAYOUT_JUSTIFIES, LAYOUT_KINDS, LAYOUT_POSITIONS};

use super::shared::{TokenEnv, check_optional_dim};
use super::suggest::push_invalid_value;

/// The layout mode of a node's direct parent frame, when that frame positions
/// its children.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::validate::check) enum FlowParent {
    Row,
    Column,
    Grid,
}

impl FlowParent {
    /// The parent mode a frame gives its direct children.
    pub(in crate::validate::check) fn of_frame(f: &FrameNode) -> Option<Self> {
        match f.layout.as_ref()? {
            LayoutKind::Row => Some(Self::Row),
            LayoutKind::Column => Some(Self::Column),
            LayoutKind::Grid => Some(Self::Grid),
            LayoutKind::Absolute | LayoutKind::Unknown(_) => None,
        }
    }

    fn is_stack(self) -> bool {
        match self {
            Self::Row | Self::Column => true,
            Self::Grid => false,
        }
    }
}

/// Where a node sits relative to auto-layout, bundled for the checks.
#[derive(Clone, Copy)]
pub(in crate::validate::check) struct LayoutSite {
    /// Layout mode of the direct parent frame, if it positions children.
    pub(in crate::validate::check) parent: Option<FlowParent>,
    /// `false` when a parent (layout frame, table cell, unknown node) supplies
    /// this node's geometry.
    pub(in crate::validate::check) geom_required: bool,
}

/// The layout-relevant facts of a node that carries item attributes.
struct ItemFacts<'a> {
    item: &'a LayoutItem,
    x: bool,
    y: bool,
    w: bool,
    h: bool,
    anchored: bool,
}

/// The item facts of a box node or an `instance`; `None` for other kinds.
fn item_facts(node: &Node) -> Option<ItemFacts<'_>> {
    if let Node::Instance(i) = node {
        return Some(ItemFacts {
            item: &i.layout_item,
            x: i.x.is_some(),
            y: i.y.is_some(),
            w: i.w.is_some(),
            h: i.h.is_some(),
            anchored: false,
        });
    }
    let view = node.box_view()?;
    Some(ItemFacts {
        item: view.layout_item,
        x: view.x.is_some(),
        y: view.y.is_some(),
        w: view.w.is_some(),
        h: view.h.is_some(),
        anchored: view.anchored,
    })
}

/// One attribute a layout check flags for removal, and whether `zenith fix`
/// can remove it unambiguously.
#[derive(Clone, Copy)]
struct Removable {
    name: &'static str,
    /// `true` attaches a [`FixHint::RemoveProperty`].
    fixable: bool,
}

impl Removable {
    fn fixable(name: &'static str) -> Self {
        Self {
            name,
            fixable: true,
        }
    }

    /// The `RemoveProperty` hint for this attribute, when it is fixable.
    fn hint(self) -> Option<FixHint> {
        self.fixable.then(|| FixHint::RemoveProperty {
            property: self.name.to_owned(),
        })
    }
}

/// Push one `layout.inert_attribute` Advisory per inert attribute, each with
/// a `RemoveProperty` fix when the removal is unambiguous.
fn push_inert(
    subject: &str,
    id: &str,
    attrs: &[Removable],
    reason: &str,
    span: Option<Span>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for attr in attrs {
        diagnostics.push(
            Diagnostic::advisory(
                "layout.inert_attribute",
                format!(
                    "{subject} '{id}': {} has no effect {reason}; remove it",
                    attr.name
                ),
                span,
                Some(id.to_owned()),
            )
            .with_fix(attr.hint()),
        );
    }
}

/// The placement attributes `node` sets: `x`, `y`, and each anchor
/// attribute.
fn placement_attrs(node: &Node, view: &ItemFacts<'_>) -> Vec<Removable> {
    let mut out = Vec::new();
    for (name, set) in [("x", view.x), ("y", view.y)] {
        if set {
            out.push(Removable::fixable(name));
        }
    }
    if let Some(a) = node.anchor_view() {
        for (name, set) in [
            ("anchor", a.anchor.is_some()),
            ("anchor-zone", a.anchor_zone.is_some()),
            ("anchor-sibling", a.anchor_sibling.is_some()),
            ("anchor-parent", a.anchor_parent.is_some()),
            ("anchor-edge", a.anchor_edge.is_some()),
            ("anchor-gap", a.anchor_gap.is_some()),
        ] {
            if set {
                out.push(Removable::fixable(name));
            }
        }
    }
    out
}

/// Push a `layout.conflicting_size` Error.
fn push_conflict(id: &str, detail: String, span: Option<Span>, diagnostics: &mut Vec<Diagnostic>) {
    diagnostics.push(Diagnostic::error(
        "layout.conflicting_size",
        format!("node '{id}': {detail}"),
        span,
        Some(id.to_owned()),
    ));
}

/// Static px value of a dimension literal; `None` for tokens, `pct`, or bad units.
fn static_px(pv: Option<&PropertyValue>) -> Option<f64> {
    match pv? {
        PropertyValue::Dimension(d) => match d.unit {
            Unit::Pct => None,
            Unit::Px | Unit::Pt | Unit::Deg | Unit::Unknown(_) => dim_to_px(d.value, &d.unit),
        },
        PropertyValue::TokenRef(_) | PropertyValue::Literal(_) | PropertyValue::DataRef(_) => None,
    }
}

/// Validate the item attributes of any box node at `site`.
pub(in crate::validate::check) fn check_layout_item(
    node: &Node,
    site: LayoutSite,
    tokens: &mut TokenEnv<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(view) = item_facts(node) else {
        return;
    };
    let item = view.item;
    let (id, span) = node.id_and_span();
    let subject = node.kind_str();

    for (prop, value) in [
        ("min-w", item.min_w.as_ref()),
        ("max-w", item.max_w.as_ref()),
        ("min-h", item.min_h.as_ref()),
        ("max-h", item.max_h.as_ref()),
    ] {
        check_optional_dim(id, prop, value, false, span, tokens, diagnostics);
    }
    if let Some(LayoutPosition::Unknown(v)) = &item.position {
        push_invalid_value(
            &format!("{subject} '{id}'"),
            Some(id.to_owned()),
            "position",
            v,
            LAYOUT_POSITIONS,
            span,
            diagnostics,
        );
    }

    check_min_max(id, item, span, diagnostics);
    for (axis, keyword, value) in [("w", item.w_keyword, view.w), ("h", item.h_keyword, view.h)] {
        if let (Some(k), true) = (keyword, value) {
            push_conflict(
                id,
                format!(
                    "{axis} is both \"{}\" and a dimension; keep one",
                    k.as_str()
                ),
                span,
                diagnostics,
            );
        }
    }

    let in_stack = site.parent.is_some_and(FlowParent::is_stack);
    if !in_stack {
        // `hug` sizes a layout frame from its own children, so it stays live
        // on a layout frame at any depth. Everything else needs a stack parent.
        let own_layout = match node {
            Node::Frame(f) => f
                .layout
                .as_ref()
                .is_some_and(LayoutKind::positions_children),
            Node::Rect(_)
            | Node::Ellipse(_)
            | Node::Line(_)
            | Node::Text(_)
            | Node::Code(_)
            | Node::Group(_)
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
            | Node::Mesh(_)
            | Node::Unknown(_) => false,
        };
        let mut inert: Vec<Removable> = Vec::new();
        for (name, keyword, has_px) in
            [("w", item.w_keyword, view.w), ("h", item.h_keyword, view.h)]
        {
            match keyword {
                Some(SizeKeyword::Hug) if own_layout => {}
                // With a px size on the same axis, removing `w` / `h` is
                // ambiguous (`layout.conflicting_size` reports it).
                Some(SizeKeyword::Hug | SizeKeyword::Fill) => inert.push(Removable {
                    name,
                    fixable: !has_px,
                }),
                None => {}
            }
        }
        for (name, set) in [
            ("min-w", item.min_w.is_some()),
            ("max-w", item.max_w.is_some()),
            ("min-h", item.min_h.is_some()),
            ("max-h", item.max_h.is_some()),
            ("position", item.position.is_some()),
        ] {
            if set {
                inert.push(Removable::fixable(name));
            }
        }
        push_inert(
            subject,
            id,
            &inert,
            "outside a row/column frame",
            span,
            diagnostics,
        );
        return;
    }

    let absolute = matches!(item.position, Some(LayoutPosition::Absolute));
    if absolute {
        if !(view.anchored || (view.x && view.y)) {
            diagnostics.push(Diagnostic::error(
                "layout.absolute_unplaced",
                format!(
                    "{subject} '{id}': position=\"absolute\" needs x and y or an anchor; \
                     add them or remove position"
                ),
                span,
                Some(id.to_owned()),
            ));
        }
    } else {
        // One advisory per ignored attribute, each with its own removal fix.
        for attr in placement_attrs(node, &view) {
            diagnostics.push(
                Diagnostic::advisory(
                    "layout.position_ignored",
                    format!(
                        "{subject} '{id}': {} is ignored inside a row/column frame; \
                         remove it or set position=\"absolute\"",
                        attr.name
                    ),
                    span,
                    Some(id.to_owned()),
                )
                .with_fix(attr.hint()),
            );
        }
    }

    if let (Node::Text(t), Some(parent)) = (node, site.parent)
        && t.overflow.as_deref() == Some("fit")
        && !absolute
    {
        let (axis, keyword, value) = match parent {
            FlowParent::Column => ("h", item.h_keyword, view.h),
            FlowParent::Row | FlowParent::Grid => ("w", item.w_keyword, view.w),
        };
        let hugs = match keyword {
            Some(SizeKeyword::Hug) => true,
            Some(SizeKeyword::Fill) => false,
            None => !value,
        };
        if hugs {
            push_conflict(
                id,
                format!(
                    "overflow=\"fit\" needs a fixed {axis}, but {axis} hugs the text; \
                     set {axis} or remove overflow=\"fit\""
                ),
                span,
                diagnostics,
            );
        }
    }
}

/// `layout.conflicting_size` for a statically known `min-*` above `max-*`.
fn check_min_max(
    id: &str,
    item: &LayoutItem,
    span: Option<Span>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (axis, min, max) in [
        ("w", item.min_w.as_ref(), item.max_w.as_ref()),
        ("h", item.min_h.as_ref(), item.max_h.as_ref()),
    ] {
        if let (Some(lo), Some(hi)) = (static_px(min), static_px(max))
            && lo > hi
        {
            push_conflict(
                id,
                format!(
                    "min-{axis} {lo}px is above max-{axis} {hi}px; lower min-{axis} or raise max-{axis}"
                ),
                span,
                diagnostics,
            );
        }
    }
}

/// Validate a frame's container attributes at `site`.
pub(in crate::validate::check) fn check_frame_layout(
    f: &FrameNode,
    site: LayoutSite,
    tokens: &mut TokenEnv<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let c = &f.container;
    let id = f.id.as_str();
    let span = f.source_span;
    let subject = format!("frame '{id}'");

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
        check_optional_dim(id, prop, value, false, span, tokens, diagnostics);
    }

    if let Some(LayoutKind::Unknown(v)) = &f.layout {
        push_invalid_value(
            &subject,
            Some(id.to_owned()),
            "layout",
            v,
            LAYOUT_KINDS,
            span,
            diagnostics,
        );
    }
    if let Some(LayoutJustify::Unknown(v)) = &c.justify {
        push_invalid_value(
            &subject,
            Some(id.to_owned()),
            "justify",
            v,
            LAYOUT_JUSTIFIES,
            span,
            diagnostics,
        );
    }
    if let Some(LayoutAlign::Unknown(v)) = &c.align {
        push_invalid_value(
            &subject,
            Some(id.to_owned()),
            "align",
            v,
            LAYOUT_ALIGNS,
            span,
            diagnostics,
        );
    }

    let kind = f.layout.clone().unwrap_or(LayoutKind::Absolute);
    let stack_only = [
        ("wrap-gap", c.wrap_gap.is_some()),
        ("justify", c.justify.is_some()),
        ("align", c.align.is_some()),
        ("wrap", c.wrap.is_some()),
    ];
    let spacing = [
        ("gap", c.gap.is_some()),
        ("padding", c.padding.is_some()),
        ("padding-x", c.padding_x.is_some()),
        ("padding-y", c.padding_y.is_some()),
        ("padding-top", c.padding_top.is_some()),
        ("padding-right", c.padding_right.is_some()),
        ("padding-bottom", c.padding_bottom.is_some()),
        ("padding-left", c.padding_left.is_some()),
    ];
    let set_names = |list: &[(&'static str, bool)]| -> Vec<Removable> {
        list.iter()
            .filter(|(_, set)| *set)
            .map(|(n, _)| Removable::fixable(n))
            .collect()
    };
    match kind {
        LayoutKind::Row | LayoutKind::Column => {}
        LayoutKind::Grid => {
            push_inert(
                "frame",
                id,
                &set_names(&stack_only),
                "on a grid frame",
                span,
                diagnostics,
            );
        }
        LayoutKind::Absolute | LayoutKind::Unknown(_) => {
            let mut inert = set_names(&spacing);
            inert.extend(set_names(&stack_only));
            push_inert(
                "frame",
                id,
                &inert,
                "without layout=\"row\", \"column\", or \"grid\"",
                span,
                diagnostics,
            );
        }
    }

    check_frame_hug(f, &kind, site, diagnostics);
}

/// Whether the frame's own `w` / `h` hugs its children (keyword `hug`, or
/// absent and not supplied by the parent layout).
fn frame_hugs(f: &FrameNode, site: LayoutSite) -> (bool, bool) {
    let supplied_by_parent = |axis_from_parent: bool| -> bool {
        match site.parent {
            Some(_) => axis_from_parent,
            None => !site.geom_required,
        }
    };
    let w_supplied = supplied_by_parent(matches!(
        site.parent,
        Some(FlowParent::Column | FlowParent::Grid)
    ));
    let h_supplied = supplied_by_parent(matches!(
        site.parent,
        Some(FlowParent::Row | FlowParent::Grid)
    ));
    let hugs =
        |keyword: Option<SizeKeyword>, value: Option<&PropertyValue>, supplied: bool| match keyword
        {
            Some(SizeKeyword::Hug) => true,
            Some(SizeKeyword::Fill) => false,
            None => value.is_none() && !supplied,
        };
    (
        hugs(f.layout_item.w_keyword, f.w.as_ref(), w_supplied),
        hugs(f.layout_item.h_keyword, f.h.as_ref(), h_supplied),
    )
}

/// `layout.conflicting_size` for `wrap=#true` on a hugging main axis.
fn check_frame_hug(
    f: &FrameNode,
    kind: &LayoutKind,
    site: LayoutSite,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if f.container.wrap != Some(true) {
        return;
    }
    let (w_hugs, h_hugs) = frame_hugs(f, site);
    let main = match kind {
        LayoutKind::Row => Some(("w", w_hugs)),
        LayoutKind::Column => Some(("h", h_hugs)),
        LayoutKind::Grid | LayoutKind::Absolute | LayoutKind::Unknown(_) => None,
    };
    if let Some((axis, true)) = main {
        push_conflict(
            &f.id,
            format!(
                "wrap=#true needs a fixed main-axis {axis}, but {axis} hugs the children; set {axis} or remove wrap"
            ),
            f.source_span,
            diagnostics,
        );
    }
}
