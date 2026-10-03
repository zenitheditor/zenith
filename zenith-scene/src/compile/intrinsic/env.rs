//! [`IntrinsicEnv`]: the read-only lookups intrinsic measurement needs.

use std::collections::BTreeMap;

use zenith_core::{BlockStyle, FontProvider, ResolvedToken, Style};
use zenith_layout::RustybuzzEngine;

use super::super::NodeCtx;
use super::super::field::FieldCtx;
use super::super::markdown_resolve::MdBlockMap;

/// Borrowed environment for intrinsic-size measurement.
///
/// Build it from a page compile's [`NodeCtx`] with
/// [`IntrinsicEnv::from_node_ctx`], or field by field in the page pre-pass.
#[derive(Clone, Copy)]
pub(crate) struct IntrinsicEnv<'a> {
    pub(in crate::compile) resolved: &'a BTreeMap<String, ResolvedToken>,
    pub(in crate::compile) style_map: &'a BTreeMap<&'a str, &'a Style>,
    pub(in crate::compile) fonts: &'a dyn FontProvider,
    pub(in crate::compile) engine: &'a RustybuzzEngine<'a>,
    pub(in crate::compile) md_blocks: &'a MdBlockMap,
    /// Page context for `field` and `toc` resolution.
    pub(in crate::compile) field_ctx: &'a FieldCtx<'a>,
    pub(in crate::compile) page_block_styles: &'a [BlockStyle],
    pub(in crate::compile) doc_block_styles: &'a [BlockStyle],
}

impl<'a> IntrinsicEnv<'a> {
    /// The measure environment of a page compile.
    pub(in crate::compile) fn from_node_ctx(cx: NodeCtx<'a>) -> Self {
        Self {
            resolved: cx.resolved,
            style_map: cx.style_map,
            fonts: cx.fonts,
            engine: cx.engine,
            md_blocks: cx.md_blocks,
            field_ctx: cx.field_ctx,
            page_block_styles: cx.page_block_styles,
            doc_block_styles: cx.doc_block_styles,
        }
    }

    /// Resolved token table.
    pub(crate) fn resolved(&self) -> &'a BTreeMap<String, ResolvedToken> {
        self.resolved
    }

    /// Style id → style lookup.
    pub(crate) fn style_map(&self) -> &'a BTreeMap<&'a str, &'a Style> {
        self.style_map
    }
}
