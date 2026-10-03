//! Layout value types: resolved boxes, per-axis sizing, and the resolved
//! settings of a layout frame.

use std::collections::BTreeMap;

use zenith_core::{
    FrameNode, LayoutAlign, LayoutJustify, LayoutKind, LayoutPosition, Node, PropertyValue,
    ResolvedToken, SizeKeyword, Style,
};

use crate::compile::{resolve_geometry_px, resolve_property_dimension_px, style_prop};

/// A resolved layout box in px: the top-left corner, then the size.
///
/// Coordinates are page-absolute: group translations are applied.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutBox {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// The space a frame offers along one axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Avail {
    /// The size is fixed.
    Definite(f64),
    /// The size hugs the content, optionally capped (the cap wraps text).
    Hug(Option<f64>),
}

impl Avail {
    pub(super) fn definite(self) -> Option<f64> {
        match self {
            Self::Definite(v) => Some(v),
            Self::Hug(_) => None,
        }
    }

    /// The largest size the content can take: the fixed size or the cap.
    pub(super) fn cap(self) -> Option<f64> {
        match self {
            Self::Definite(v) => Some(v),
            Self::Hug(cap) => cap,
        }
    }

    /// The space left after `inset` px of padding.
    pub(super) fn inset(self, inset: f64) -> Self {
        match self {
            Self::Definite(v) => Self::Definite((v - inset).max(0.0)),
            Self::Hug(cap) => Self::Hug(cap.map(|c| (c - inset).max(0.0))),
        }
    }
}

/// How a child sizes along one axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Sizing {
    Fixed(f64),
    Hug,
    Fill,
}

/// One axis of a child: its sizing and its `min-*` / `max-*` clamp.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct AxisSpec {
    pub(super) sizing: Sizing,
    pub(super) min: f64,
    pub(super) max: f64,
}

impl AxisSpec {
    /// Clamp `v` into `[min, max]`. `min` wins when the two conflict.
    pub(super) fn clamp(self, v: f64) -> f64 {
        v.min(self.max).max(self.min)
    }
}

/// The two axes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Axis {
    X,
    Y,
}

impl Axis {
    /// The size attribute of the axis (`w` or `h`).
    pub(super) fn size_attr(self) -> &'static str {
        match self {
            Self::X => "w",
            Self::Y => "h",
        }
    }
}

/// Main-axis distribution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Justify {
    Start,
    Center,
    End,
    SpaceBetween,
}

/// Cross-axis placement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Align {
    Start,
    Center,
    End,
    Stretch,
}

/// The layout mode of a frame that positions its children.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Mode {
    Row,
    Column,
    Grid,
}

impl Mode {
    /// The mode of `frame`, or `None` for an absolute frame.
    pub(super) fn of(frame: &FrameNode) -> Option<Self> {
        match frame.layout.as_ref()? {
            LayoutKind::Row => Some(Self::Row),
            LayoutKind::Column => Some(Self::Column),
            LayoutKind::Grid => Some(Self::Grid),
            LayoutKind::Absolute | LayoutKind::Unknown(_) => None,
        }
    }

    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Row => "row",
            Self::Column => "column",
            Self::Grid => "grid",
        }
    }
}

/// Resolved padding of a frame, in px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Insets {
    pub(super) top: f64,
    pub(super) right: f64,
    pub(super) bottom: f64,
    pub(super) left: f64,
}

/// The resolved container settings of a layout frame.
#[derive(Clone, Copy, Debug)]
pub(super) struct FrameSpec {
    pub(super) mode: Mode,
    pub(super) insets: Insets,
    pub(super) gap: f64,
    pub(super) wrap_gap: f64,
    pub(super) justify: Justify,
    pub(super) align: Align,
    pub(super) wrap: bool,
}

impl FrameSpec {
    /// Resolve `frame`'s container attributes.
    ///
    /// Padding per side: `padding-<side>` > `padding-x`/`padding-y` >
    /// `padding` > style `padding` > 0. `gap`: attribute > style `gap` > 0.
    /// `wrap-gap` defaults to `gap`. An unknown `justify` / `align` takes the
    /// default (`start` / `stretch`); validation reports it.
    pub(super) fn of(
        frame: &FrameNode,
        mode: Mode,
        resolved: &BTreeMap<String, ResolvedToken>,
        style_map: &BTreeMap<&str, &Style>,
    ) -> Self {
        let attr_or = |pv: &Option<PropertyValue>, fallback: f64| -> f64 {
            match pv {
                Some(p) => resolve_property_dimension_px(Some(p), resolved, fallback),
                None => fallback,
            }
        };
        let c = &frame.container;
        let style_pad = resolve_property_dimension_px(
            style_prop(&frame.style, style_map, "padding"),
            resolved,
            0.0,
        );
        let style_gap = resolve_property_dimension_px(
            style_prop(&frame.style, style_map, "gap"),
            resolved,
            0.0,
        );
        let pad = attr_or(&c.padding, style_pad);
        let pad_x = attr_or(&c.padding_x, pad);
        let pad_y = attr_or(&c.padding_y, pad);
        let gap = attr_or(&c.gap, style_gap);
        let justify = match &c.justify {
            Some(LayoutJustify::Center) => Justify::Center,
            Some(LayoutJustify::End) => Justify::End,
            Some(LayoutJustify::SpaceBetween) => Justify::SpaceBetween,
            Some(LayoutJustify::Start | LayoutJustify::Unknown(_)) | None => Justify::Start,
        };
        let align = match &c.align {
            Some(LayoutAlign::Start) => Align::Start,
            Some(LayoutAlign::Center) => Align::Center,
            Some(LayoutAlign::End) => Align::End,
            Some(LayoutAlign::Stretch | LayoutAlign::Unknown(_)) | None => Align::Stretch,
        };
        Self {
            mode,
            insets: Insets {
                top: attr_or(&c.padding_top, pad_y),
                right: attr_or(&c.padding_right, pad_x),
                bottom: attr_or(&c.padding_bottom, pad_y),
                left: attr_or(&c.padding_left, pad_x),
            },
            gap,
            wrap_gap: attr_or(&c.wrap_gap, gap),
            justify,
            align,
            wrap: c.wrap == Some(true) && mode != Mode::Grid,
        }
    }
}

/// How a layout frame treats one of its children.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ChildRole {
    /// Placed and sized by the frame.
    Flow,
    /// Out of flow: its x/y (or points) count from the frame's top-left.
    Absolute,
    /// Left as authored: guides, hidden nodes, instances, connectors,
    /// footnotes, and unknown kinds.
    Untouched,
}

/// The role `node` plays as a child of a layout frame.
pub(super) fn child_role(node: &Node) -> ChildRole {
    if node.role() == Some("guide") || node.visible() == Some(false) {
        return ChildRole::Untouched;
    }
    if let Some(view) = node.box_view() {
        return match view.layout_item.position {
            Some(LayoutPosition::Absolute) => ChildRole::Absolute,
            Some(LayoutPosition::Auto | LayoutPosition::Unknown(_)) | None => ChildRole::Flow,
        };
    }
    match node {
        Node::Line(_) | Node::Polygon(_) | Node::Polyline(_) | Node::Path(_) | Node::Light(_) => {
            ChildRole::Absolute
        }
        Node::Instance(_) | Node::Footnote(_) | Node::Connector(_) | Node::Unknown(_) => {
            ChildRole::Untouched
        }
        // Box kinds returned above.
        Node::Rect(_)
        | Node::Ellipse(_)
        | Node::Text(_)
        | Node::Code(_)
        | Node::Frame(_)
        | Node::Group(_)
        | Node::Image(_)
        | Node::Field(_)
        | Node::Toc(_)
        | Node::Table(_)
        | Node::Shape(_)
        | Node::Pattern(_)
        | Node::Chart(_)
        | Node::Mesh(_) => ChildRole::Flow,
    }
}

/// A px value of a geometry property, when it resolves.
pub(super) fn px_of(
    pv: Option<&PropertyValue>,
    resolved: &BTreeMap<String, ResolvedToken>,
) -> Option<f64> {
    resolve_geometry_px(pv, resolved)
}

/// The sizing of a flow child's axis.
///
/// A resolvable `w` / `h` is fixed. A `hug` / `fill` keyword wins next. An
/// omitted main-axis size hugs. An omitted cross-axis size fills under
/// `stretch` and hugs otherwise.
pub(super) fn axis_sizing(
    declared: Option<f64>,
    keyword: Option<SizeKeyword>,
    is_main: bool,
    stretch: bool,
) -> Sizing {
    match (declared, keyword) {
        (Some(v), _) => Sizing::Fixed(v),
        (None, Some(SizeKeyword::Hug)) => Sizing::Hug,
        (None, Some(SizeKeyword::Fill)) => Sizing::Fill,
        (None, None) if is_main => Sizing::Hug,
        (None, None) if stretch => Sizing::Fill,
        (None, None) => Sizing::Hug,
    }
}
