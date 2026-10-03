//! Auto-layout types: the `frame` container modes and the per-child item
//! attributes shared by every box-node kind.
//!
//! Every enum keeps an `Unknown(String)` variant (where the attribute is a free
//! string in KDL) so parse → format stays lossless; validation reports the
//! unknown value as `node.invalid_value`.

use crate::ast::value::PropertyValue;

/// `frame layout=…` — how a frame places its children.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutKind {
    /// Children keep their authored coordinates (the default).
    Absolute,
    /// Children stack left to right along the x axis.
    Row,
    /// Children stack top to bottom along the y axis.
    Column,
    /// Children tile row-major into a `columns × rows` grid.
    Grid,
    /// Unrecognized value, preserved verbatim for round-trip.
    Unknown(String),
}

impl LayoutKind {
    /// Parse a KDL `layout` value, preserving unknown values.
    pub fn from_attr(value: &str) -> Self {
        match value {
            "absolute" => Self::Absolute,
            "row" => Self::Row,
            "column" => Self::Column,
            "grid" => Self::Grid,
            other => Self::Unknown(other.to_owned()),
        }
    }

    /// Canonical KDL spelling.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Absolute => "absolute",
            Self::Row => "row",
            Self::Column => "column",
            Self::Grid => "grid",
            Self::Unknown(v) => v.as_str(),
        }
    }

    /// `true` for the stacking modes (`row`, `column`).
    pub fn is_stack(&self) -> bool {
        match self {
            Self::Row | Self::Column => true,
            Self::Absolute | Self::Grid | Self::Unknown(_) => false,
        }
    }

    /// `true` for every mode that positions children (`row`, `column`, `grid`).
    pub fn positions_children(&self) -> bool {
        match self {
            Self::Row | Self::Column | Self::Grid => true,
            Self::Absolute | Self::Unknown(_) => false,
        }
    }
}

/// `frame justify=…` — main-axis distribution of a stack frame's children.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutJustify {
    Start,
    Center,
    End,
    SpaceBetween,
    Unknown(String),
}

impl LayoutJustify {
    /// Parse a KDL `justify` value, preserving unknown values.
    pub fn from_attr(value: &str) -> Self {
        match value {
            "start" => Self::Start,
            "center" => Self::Center,
            "end" => Self::End,
            "space-between" => Self::SpaceBetween,
            other => Self::Unknown(other.to_owned()),
        }
    }

    /// Canonical KDL spelling.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Start => "start",
            Self::Center => "center",
            Self::End => "end",
            Self::SpaceBetween => "space-between",
            Self::Unknown(v) => v.as_str(),
        }
    }
}

/// `frame align=…` — cross-axis placement of a stack frame's children.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutAlign {
    Start,
    Center,
    End,
    Stretch,
    Unknown(String),
}

impl LayoutAlign {
    /// Parse a KDL `align` value, preserving unknown values.
    pub fn from_attr(value: &str) -> Self {
        match value {
            "start" => Self::Start,
            "center" => Self::Center,
            "end" => Self::End,
            "stretch" => Self::Stretch,
            other => Self::Unknown(other.to_owned()),
        }
    }

    /// Canonical KDL spelling.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Start => "start",
            Self::Center => "center",
            Self::End => "end",
            Self::Stretch => "stretch",
            Self::Unknown(v) => v.as_str(),
        }
    }
}

/// Item `position=…` — whether a child of a stack frame joins the flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutPosition {
    /// The child is placed by the parent's layout (the default).
    Auto,
    /// The child keeps its authored `x`/`y` (or anchor) and takes no space.
    Absolute,
    Unknown(String),
}

impl LayoutPosition {
    /// Parse a KDL `position` value, preserving unknown values.
    pub fn from_attr(value: &str) -> Self {
        match value {
            "auto" => Self::Auto,
            "absolute" => Self::Absolute,
            other => Self::Unknown(other.to_owned()),
        }
    }

    /// Canonical KDL spelling.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Auto => "auto",
            Self::Absolute => "absolute",
            Self::Unknown(v) => v.as_str(),
        }
    }
}

/// Keyword form of an item `w`/`h` (`w="hug"`, `h="fill"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeKeyword {
    /// Size to the content (intrinsic size, or the children's extent).
    Hug,
    /// Take the remaining space of the parent along that axis.
    Fill,
}

impl SizeKeyword {
    /// Parse a size keyword. Any other string is not a keyword.
    pub fn from_attr(value: &str) -> Option<Self> {
        match value {
            "hug" => Some(Self::Hug),
            "fill" => Some(Self::Fill),
            _ => None,
        }
    }

    /// Canonical KDL spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hug => "hug",
            Self::Fill => "fill",
        }
    }
}

/// Per-child auto-layout attributes, embedded in every box-node kind.
///
/// A keyword `w`/`h` lives here and the node's own `w`/`h` stays `None`, so
/// [`PropertyValue`] geometry is unchanged. All fields `None` (the
/// [`Default`]) means the node uses none of these attributes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LayoutItem {
    /// `w="hug"` / `w="fill"`.
    pub w_keyword: Option<SizeKeyword>,
    /// `h="hug"` / `h="fill"`.
    pub h_keyword: Option<SizeKeyword>,
    /// `min-w` (px literal or dimension token).
    pub min_w: Option<PropertyValue>,
    /// `max-w` (px literal or dimension token).
    pub max_w: Option<PropertyValue>,
    /// `min-h` (px literal or dimension token).
    pub min_h: Option<PropertyValue>,
    /// `max-h` (px literal or dimension token).
    pub max_h: Option<PropertyValue>,
    /// `position="auto"|"absolute"`.
    pub position: Option<LayoutPosition>,
}

impl LayoutItem {
    /// `true` when no item attribute is set.
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

/// Container attributes of a `frame` that lays out its children.
///
/// All fields `None` (the [`Default`]) means the frame sets none of them; the
/// scene then falls back to the frame style's `gap` / `padding`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LayoutContainer {
    /// Main-axis gap between children (px literal or dimension token).
    pub gap: Option<PropertyValue>,
    /// Gap between wrapped lines; defaults to `gap`.
    pub wrap_gap: Option<PropertyValue>,
    /// Uniform inset on all four sides.
    pub padding: Option<PropertyValue>,
    /// Left + right inset; overrides `padding`.
    pub padding_x: Option<PropertyValue>,
    /// Top + bottom inset; overrides `padding`.
    pub padding_y: Option<PropertyValue>,
    /// Top inset; overrides `padding-y` and `padding`.
    pub padding_top: Option<PropertyValue>,
    /// Right inset; overrides `padding-x` and `padding`.
    pub padding_right: Option<PropertyValue>,
    /// Bottom inset; overrides `padding-y` and `padding`.
    pub padding_bottom: Option<PropertyValue>,
    /// Left inset; overrides `padding-x` and `padding`.
    pub padding_left: Option<PropertyValue>,
    /// Main-axis distribution (default `start`).
    pub justify: Option<LayoutJustify>,
    /// Cross-axis placement (default `stretch`).
    pub align: Option<LayoutAlign>,
    /// Wrap children onto new lines when the main axis is full (default off).
    pub wrap: Option<bool>,
}

impl LayoutContainer {
    /// `true` when no container attribute is set.
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_kind_round_trips_every_value() {
        for v in ["absolute", "row", "column", "grid", "flow"] {
            assert_eq!(LayoutKind::from_attr(v).as_str(), v);
        }
        assert_eq!(
            LayoutKind::from_attr("flow"),
            LayoutKind::Unknown("flow".to_owned())
        );
        assert!(LayoutKind::Row.is_stack());
        assert!(!LayoutKind::Grid.is_stack());
        assert!(LayoutKind::Grid.positions_children());
        assert!(!LayoutKind::Absolute.positions_children());
    }

    #[test]
    fn enums_round_trip() {
        for v in ["start", "center", "end", "space-between", "x"] {
            assert_eq!(LayoutJustify::from_attr(v).as_str(), v);
        }
        for v in ["start", "center", "end", "stretch", "x"] {
            assert_eq!(LayoutAlign::from_attr(v).as_str(), v);
        }
        for v in ["auto", "absolute", "x"] {
            assert_eq!(LayoutPosition::from_attr(v).as_str(), v);
        }
        assert_eq!(SizeKeyword::from_attr("hug"), Some(SizeKeyword::Hug));
        assert_eq!(
            SizeKeyword::from_attr("fill").map(SizeKeyword::as_str),
            Some("fill")
        );
        assert_eq!(SizeKeyword::from_attr("huge"), None);
    }

    #[test]
    fn defaults_are_empty() {
        assert!(LayoutItem::default().is_empty());
        assert!(LayoutContainer::default().is_empty());
        let c = LayoutContainer {
            padding_left: Some(PropertyValue::Literal("x".to_owned())),
            ..LayoutContainer::default()
        };
        assert!(!c.is_empty());
    }
}
