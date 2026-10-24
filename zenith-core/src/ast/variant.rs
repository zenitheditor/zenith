//! Variant-set block declaration AST types.
//!
//! The top-level `variants` block declares named size/override variants derived
//! from a source page. Each `variant` entry specifies a target page `id`, the
//! `source` page it derives from, the target dimensions `w`/`h`, and an
//! optional list of per-node property overrides. It is a sibling of the
//! `provenance`/`document` blocks. Core round-trips and validates these records.
//! The CLI engine (`zenith variant`) generates the variants.

use std::collections::BTreeMap;

use super::Span;
use super::node::{LayoutContainer, LayoutKind, SizeKeyword, UnknownProperty};
use super::value::{Dimension, PropertyValue};

/// A single variant declaration within a `variants` block.
#[derive(Debug, Clone, PartialEq)]
pub struct VariantDef {
    /// The variant's own stable id. Required.
    pub id: String,
    /// The canonical page id this variant derives from. Required; existence is
    /// validated later, not by the parser.
    pub source: String,
    /// Target page width for this variant. Required.
    pub w: Dimension,
    /// Target page height for this variant. Required.
    pub h: Dimension,
    /// Per-node property overrides; empty when no `override` children are
    /// present.
    pub overrides: Vec<VariantOverride>,
    /// Source declaration span, when available.
    pub source_span: Option<Span>,
    /// Forward-compat: unrecognized attributes preserved with typed values +
    /// annotations.
    pub unknown_props: BTreeMap<String, UnknownProperty>,
}

/// A single per-node property override within a [`VariantDef`].
///
/// Every field is optional except `node`. An absent field keeps the source
/// page value. The layout fields re-run auto-layout at the variant size, so a
/// retarget changes frame modes and spacing instead of moving every node.
#[derive(Debug, Clone, PartialEq)]
pub struct VariantOverride {
    /// The target node id within the source page. Required.
    pub node: String,
    /// Override for the node's `visible` property.
    pub visible: Option<bool>,
    /// Override for the node's left edge (`x`), in document pixels or points.
    pub x: Option<Dimension>,
    /// Override for the node's top edge (`y`), in document pixels or points.
    pub y: Option<Dimension>,
    /// Override for the node's width (`w`), in document pixels or points.
    /// `None` when `w_keyword` is set.
    pub w: Option<Dimension>,
    /// Override for the node's height (`h`), in document pixels or points.
    /// `None` when `h_keyword` is set.
    pub h: Option<Dimension>,
    /// `w="hug"` / `w="fill"`: the auto-layout size keyword for the width.
    pub w_keyword: Option<SizeKeyword>,
    /// `h="hug"` / `h="fill"`: the auto-layout size keyword for the height.
    pub h_keyword: Option<SizeKeyword>,
    /// Override for the node's `fill` property (token ref or literal).
    pub fill: Option<PropertyValue>,
    /// Override for the node's `text` content.
    pub text: Option<String>,
    /// Override for the node's `font-size` (a dimension token ref).
    pub font_size: Option<PropertyValue>,
    /// Override for a frame's `layout` mode.
    pub layout: Option<LayoutKind>,
    /// Overrides for a frame's container attributes (`gap`, `wrap-gap`,
    /// `padding*`, `justify`, `align`, `wrap`). Empty when none is set.
    pub container: LayoutContainer,
    /// Source declaration span, when available.
    pub source_span: Option<Span>,
    /// Forward-compat: unrecognized attributes preserved with typed values +
    /// annotations.
    pub unknown_props: BTreeMap<String, UnknownProperty>,
}

impl VariantOverride {
    /// `true` when the override sets a frame-only property: `layout` or a
    /// container attribute.
    pub fn sets_frame_layout(&self) -> bool {
        self.layout.is_some() || !self.container.is_empty()
    }
}
