//! Entry points of the contrast check: the page text pass of validation and
//! the label pass of the compile stage.

use std::collections::BTreeMap;

use crate::ast::document::Page;
use crate::ast::node::Node;
use crate::ast::style::Style;
use crate::ast::value::dim_to_px;
use crate::diagnostics::Diagnostic;
use crate::tokens::ResolvedToken;
use crate::validate::check::geometry::page_background_rgb;

use super::label::LabelInk;
use super::types::{ContrastEnv, PaintCtx};
use super::walk::walk_paint;

pub(in crate::validate::check) fn check_page_text_contrast(
    children: &[Node],
    page_bg_rgb: Option<(u8, u8, u8)>,
    page_size: (f64, f64),
    resolved_tokens: &BTreeMap<String, ResolvedToken>,
    style_map: &BTreeMap<&str, &Style>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut candidates = Vec::new();
    let ctx = PaintCtx {
        dx: 0.0,
        dy: 0.0,
        clip: None,
        opacity: 1.0,
        unmodeled: false,
        page_bg_rgb,
        page_size,
        header_style: None,
    };
    let env = ContrastEnv {
        resolved_tokens,
        style_map,
        labels: None,
    };
    walk_paint(children, ctx, &mut candidates, env, diagnostics);
}

/// Judge the drawn `shape` / `connector` labels of `page` against their
/// backdrops: the label pass of the compile stage.
///
/// `labels` maps an owner node id to its label's measured ink (see
/// [`LabelInk`]). Text nodes are not judged here: validation and the layout
/// geometry checks judge them.
pub fn label_contrast_checks(
    page: &Page,
    resolved_tokens: &BTreeMap<String, ResolvedToken>,
    style_map: &BTreeMap<&str, &Style>,
    labels: &BTreeMap<String, LabelInk>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let Some(page_size) = dim_to_px(page.width.value, &page.width.unit)
        .zip(dim_to_px(page.height.value, &page.height.unit))
    else {
        return diagnostics;
    };
    if labels.is_empty() {
        return diagnostics;
    }
    let ctx = PaintCtx {
        dx: 0.0,
        dy: 0.0,
        clip: None,
        opacity: 1.0,
        unmodeled: false,
        page_bg_rgb: page_background_rgb(page, resolved_tokens),
        page_size,
        header_style: None,
    };
    let env = ContrastEnv {
        resolved_tokens,
        style_map,
        labels: Some(labels),
    };
    let mut candidates = Vec::new();
    walk_paint(&page.children, ctx, &mut candidates, env, &mut diagnostics);
    diagnostics
}
