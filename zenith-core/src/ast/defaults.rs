//! The `defaults { … }` block: per-node-kind default style references.
//!
//! A `defaults` block sits at document scope (after `styles`) or at the start
//! of a page body. Each entry names a node kind and the style id that kind
//! takes when a node of that kind sets no `style` of its own:
//!
//! ```kdl
//! defaults {
//!   text style="body"
//!   shape style="box" text-style="box.label"
//! }
//! ```
//!
//! Entries are keyed by [`DefaultsKind`], so the canonical order is the kind
//! order and a duplicate kind cannot overwrite an earlier entry. Rows the
//! parser cannot accept (an unknown kind, an unsupported kind, a repeated
//! kind) are kept in [`DefaultsBlock::rejected`] so the validator reports them
//! and the formatter writes them back.

use std::collections::BTreeMap;

use crate::ast::node::UnknownProperty;
use crate::ast::span::Span;

/// A node kind that accepts a `defaults` entry: every kind with a `style` field.
///
/// Variants are declared in alphabetical order of their source name, so the
/// derived `Ord` is the canonical (alphabetical) entry order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DefaultsKind {
    Chart,
    Code,
    Connector,
    Ellipse,
    Field,
    Footnote,
    Frame,
    Group,
    Image,
    Line,
    Path,
    Pattern,
    Polygon,
    Polyline,
    Rect,
    Shape,
    Table,
    Text,
    Toc,
}

impl DefaultsKind {
    /// Every kind, in canonical order.
    pub const ALL: &'static [DefaultsKind] = &[
        Self::Chart,
        Self::Code,
        Self::Connector,
        Self::Ellipse,
        Self::Field,
        Self::Footnote,
        Self::Frame,
        Self::Group,
        Self::Image,
        Self::Line,
        Self::Path,
        Self::Pattern,
        Self::Polygon,
        Self::Polyline,
        Self::Rect,
        Self::Shape,
        Self::Table,
        Self::Text,
        Self::Toc,
    ];

    /// The source spelling of the kind (the node name).
    pub const fn name(self) -> &'static str {
        match self {
            Self::Chart => "chart",
            Self::Code => "code",
            Self::Connector => "connector",
            Self::Ellipse => "ellipse",
            Self::Field => "field",
            Self::Footnote => "footnote",
            Self::Frame => "frame",
            Self::Group => "group",
            Self::Image => "image",
            Self::Line => "line",
            Self::Path => "path",
            Self::Pattern => "pattern",
            Self::Polygon => "polygon",
            Self::Polyline => "polyline",
            Self::Rect => "rect",
            Self::Shape => "shape",
            Self::Table => "table",
            Self::Text => "text",
            Self::Toc => "toc",
        }
    }

    /// The kind whose source spelling is `name`, or `None`.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|k| k.name() == name)
    }

    /// The source spellings of every kind, in canonical order.
    pub fn names() -> Vec<&'static str> {
        Self::ALL.iter().map(|k| k.name()).collect()
    }

    /// Whether the kind takes a `text-style` entry (it owns a label).
    pub const fn accepts_text_style(self) -> bool {
        match self {
            Self::Shape | Self::Connector => true,
            Self::Chart
            | Self::Code
            | Self::Ellipse
            | Self::Field
            | Self::Footnote
            | Self::Frame
            | Self::Group
            | Self::Image
            | Self::Line
            | Self::Path
            | Self::Pattern
            | Self::Polygon
            | Self::Polyline
            | Self::Rect
            | Self::Table
            | Self::Text
            | Self::Toc => false,
        }
    }
}

/// Node kinds that exist but take no `defaults` entry (they carry no `style`).
pub const DEFAULTS_UNSUPPORTED_KINDS: &[&str] = &["instance", "light", "mesh"];

/// Entry attribute names a `defaults` row accepts.
pub const DEFAULTS_ENTRY_PROPS: &[&str] = &["style", "text-style"];

/// One `<kind> style="…" [text-style="…"]` row.
#[derive(Debug, Clone, PartialEq)]
pub struct DefaultsEntry {
    /// The default style id for the kind.
    pub style: String,
    /// The default label style id (`shape` and `connector` only).
    pub text_style: Option<String>,
    /// Attributes other than `style` / `text-style`, kept for the validator.
    pub unknown_props: BTreeMap<String, UnknownProperty>,
    /// Byte-range of the row in the source (for diagnostics).
    pub source_span: Option<Span>,
}

/// Why the parser did not accept a `defaults` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultsRejection {
    /// The row names no node kind.
    UnknownKind,
    /// The row names a node kind that carries no `style` (see
    /// [`DEFAULTS_UNSUPPORTED_KINDS`]).
    UnsupportedKind,
    /// The row repeats a kind an earlier row of the same block declared.
    DuplicateKind(DefaultsKind),
}

/// A `defaults` row the parser did not accept, kept in source order.
#[derive(Debug, Clone, PartialEq)]
pub struct RejectedDefaultsEntry {
    /// The row's node name as written.
    pub name: String,
    /// Why the row was not accepted.
    pub reason: DefaultsRejection,
    /// The row's attributes.
    pub entry: DefaultsEntry,
}

/// A document-level or page-level `defaults { … }` block.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DefaultsBlock {
    /// Accepted rows, keyed by kind (canonical order).
    pub entries: BTreeMap<DefaultsKind, DefaultsEntry>,
    /// Rejected rows, in source order.
    pub rejected: Vec<RejectedDefaultsEntry>,
    /// Byte-range of the `defaults` block in the source (for diagnostics).
    pub source_span: Option<Span>,
}

impl DefaultsBlock {
    /// Whether the block has no rows (absent or written as `defaults {}`).
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty() && self.rejected.is_empty()
    }

    /// The accepted row for `kind`, if any.
    pub fn get(&self, kind: DefaultsKind) -> Option<&DefaultsEntry> {
        self.entries.get(&kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_is_sorted_by_name_and_matches_ord() {
        let names = DefaultsKind::names();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted);
        let mut kinds = DefaultsKind::ALL.to_vec();
        kinds.sort();
        assert_eq!(kinds, DefaultsKind::ALL);
    }

    #[test]
    fn from_name_round_trips() {
        for k in DefaultsKind::ALL {
            assert_eq!(DefaultsKind::from_name(k.name()), Some(*k));
        }
        assert_eq!(DefaultsKind::from_name("instance"), None);
    }

    #[test]
    fn text_style_kinds() {
        let with: Vec<_> = DefaultsKind::ALL
            .iter()
            .filter(|k| k.accepts_text_style())
            .map(|k| k.name())
            .collect();
        assert_eq!(with, ["connector", "shape"]);
    }

    #[test]
    fn kinds_cover_schema_node_kinds() {
        for kind in crate::schema::node_kinds() {
            assert!(
                DefaultsKind::from_name(kind).is_some()
                    || DEFAULTS_UNSUPPORTED_KINDS.contains(kind),
                "{kind} is neither a defaults kind nor unsupported"
            );
        }
    }
}
