//! Scene compilation: `Document` → `CompileResult`.
//!
//! Entry point: [`compile`].
//!
//! Rect, ellipse, line, text, code, and group nodes are compiled; the page
//! background is emitted first; unknown nodes produce an advisory diagnostic
//! and are skipped.
//!
//! [`compile`] renders page 0; [`compile_page`] renders a chosen page by index.
//! [`DocumentPrep`] and [`PageCompiler`] compile many pages of one document
//! and share the document-level work.
//!
//! The compiler is split across submodules: `leaf` (rect/ellipse/line/
//! polygon/polyline/path), `text` (text + code shaping), `container` (group +
//! frame), `image`, `paint` (color/gradient/shadow resolvers), and
//! `util` (small geometry/diagnostic helpers). This module is wiring only:
//! submodule declarations, type aliases, and re-exports.

mod anchor;
mod backdrop;
mod boxes;
mod chain;
mod chart;
mod container;
mod crop;
mod ctx;
mod data_resolve;
mod dispatch;
mod effect;
mod entry;
mod field;
mod font_ns;
mod footnote;
mod image;
mod imports;
mod intrinsic;
mod leaf;
mod line_jumps;
mod lint;
mod markdown_resolve;
mod page_source;
mod paint;
mod pattern;
mod pipeline;
mod session;
mod table;
mod table_flow;
mod text;
mod toc;
mod util;

use std::collections::BTreeMap;

use zenith_core::{ComponentDef, MasterDef};

pub(super) type ComponentMap<'a> = BTreeMap<&'a str, &'a ComponentDef>;
pub(super) type MasterMap<'a> = BTreeMap<&'a str, &'a MasterDef>;

pub(crate) use anchor::{AnchorMap, ParentCtx, PrePassEnv, anchor_origin, anchor_sibling_of};
pub use boxes::{
    Affine2, ClipShape, CompiledBox, HitShape, hit_region, hit_test, hit_test_within, selectable_id,
};
pub(in crate::compile) use ctx::NodeCtx;
pub(in crate::compile) use dispatch::compile_node;
pub use entry::{compile, compile_page, compile_page_with_imports, layout_boxes};
pub use imports::{ImportGraph, ImportedDocument};
pub(crate) use intrinsic::{ImageSizes, IntrinsicEnv, ProbeAt, ProbeHints};
pub use lint::{sizes_read_as_one, text_size_floor_px};
pub use pipeline::CompileResult;
pub(in crate::compile) use pipeline::{RenderCtx, compile_page_inner};
pub(crate) use pipeline::{style_enum, style_h_align, style_prop};
pub use session::{DocumentPrep, LayoutStats, PageCompiler};
pub(crate) use util::{px, px_prop, resolve_geometry_px, resolve_property_dimension_px};
