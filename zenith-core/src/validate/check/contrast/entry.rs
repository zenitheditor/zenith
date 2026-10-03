//! The entry point of the contrast check: the compile-stage pass over one
//! drawn page.

use std::collections::BTreeMap;

use crate::ast::document::Page;
use crate::ast::node::Node;
use crate::ast::style::Style;
use crate::ast::value::dim_to_px;
use crate::diagnostics::Diagnostic;
use crate::tokens::ResolvedToken;
use crate::validate::check::geometry::page_background_rgb;

use super::ink::ContrastInks;
use super::scope::ContentScopes;
use super::types::{ContrastEnv, PaintCtx};
use super::walk::walk_paint;

/// Judge the contrast of `children` drawn on `page`: every `text` node, the
/// text in each table cell, and every measured `shape` / `connector` label.
///
/// The scene engine calls it once per compiled page, with the page content
/// as it drew it (master projection first, each `instance` replaced by its
/// expansion) and the drawn ink in `inks`. A text node is judged where its
/// glyph ink lands. A text with no ink entry draws no glyph and is skipped.
/// With `inks` set to `None`, each text is judged by its box and labels are
/// skipped. The groups in `scopes` draw their children in their own token
/// scope or under a fit transform. Diagnostics keep each node's span.
pub fn page_contrast_checks<'a>(
    page: &Page,
    children: &[Node],
    resolved_tokens: &'a BTreeMap<String, ResolvedToken>,
    style_map: &'a BTreeMap<&'a str, &'a Style>,
    inks: Option<ContrastInks<'a>>,
    scopes: &'a ContentScopes<'a>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let Some(page_size) = dim_to_px(page.width.value, &page.width.unit)
        .zip(dim_to_px(page.height.value, &page.height.unit))
    else {
        return diagnostics;
    };
    let ctx = PaintCtx {
        dx: 0.0,
        dy: 0.0,
        sx: 1.0,
        sy: 1.0,
        clip: None,
        opacity: 1.0,
        unmodeled: false,
        page_bg_rgb: page_background_rgb(page, resolved_tokens),
        page_size,
        header_style: None,
        in_cell: false,
    };
    let env = ContrastEnv {
        resolved_tokens,
        style_map,
        inks,
        scopes: Some(scopes),
    };
    let mut candidates = Vec::new();
    walk_paint(children, ctx, &mut candidates, env, &mut diagnostics);
    diagnostics
}
