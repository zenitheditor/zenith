//! Scene compilation: `Document` → `CompileResult`.
//!
//! Entry point: [`compile`].
//!
//! Rect, ellipse, line, text, code, and group nodes are compiled; the page
//! background is emitted first; unknown nodes produce an advisory diagnostic
//! and are skipped.
//!
//! [`compile`] renders page 0; [`compile_page`] renders a chosen page by index.
//!
//! The compiler is split across submodules: `leaf` (rect/ellipse/line/
//! polygon/polyline/path), `text` (text + code shaping), `container` (group +
//! frame), `image`, `paint` (color/gradient/shadow resolvers), and
//! `util` (small geometry/diagnostic helpers). This module is wiring only:
//! submodule declarations, type aliases, and re-exports.

mod anchor;
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
mod leaf;
mod line_jumps;
mod markdown_resolve;
mod page_source;
mod paint;
mod pattern;
mod pipeline;
mod table;
mod table_flow;
mod text;
mod toc;
mod util;

use std::collections::BTreeMap;

use zenith_core::{ComponentDef, MasterDef};

pub(super) type ComponentMap<'a> = BTreeMap<&'a str, &'a ComponentDef>;
pub(super) type MasterMap<'a> = BTreeMap<&'a str, &'a MasterDef>;

pub(in crate::compile) use ctx::NodeCtx;
pub(in crate::compile) use dispatch::compile_node;
pub use entry::{compile, compile_page, compile_page_with_imports};
pub use imports::{ImportGraph, ImportedDocument};
pub use pipeline::CompileResult;
pub(in crate::compile) use pipeline::{RenderCtx, compile_page_inner, style_prop};
