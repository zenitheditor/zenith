//! Integration tests for the canonical writer: nodes.
//!
//! Leaf and decorative nodes — images, ellipses, assets, safe-zones, folds, and
//! unknown properties — parse, serialize, and round-trip.
//!
//! Moved verbatim from the former in-`src` `format/writer/tests.rs`; the body of
//! every test is unchanged — only import paths were rewritten to the public
//! `zenith_core` surface. Span-stripping uses `zenith_core::strip_spans`.

#[path = "common/px.rs"]
mod px;

use px::px;
use zenith_core::format::format_document;
use zenith_core::{AnchorKind, KdlAdapter, KdlSource, Node, PropertyValue, strip_spans};

#[path = "format_nodes/anchor.rs"]
mod anchor;
#[path = "format_nodes/connector_label.rs"]
mod connector_label;
#[path = "format_nodes/ellipse.rs"]
mod ellipse;
#[path = "format_nodes/image.rs"]
mod image;
#[path = "format_nodes/round_trips.rs"]
mod round_trips;
#[path = "format_nodes/safe_zone_fold.rs"]
mod safe_zone_fold;
#[path = "format_nodes/unknown_property.rs"]
mod unknown_property;
