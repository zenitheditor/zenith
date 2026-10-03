//! Geometry checks for pages that use auto-layout.
//!
//! A page that holds a `row` / `column` / `grid` frame has no final geometry
//! until the scene engine lays it out (text sizes need shaping). [`validate`]
//! skips the geometry checks on such a page. The scene engine lowers the page
//! to absolute geometry and calls [`layout_geometry_checks`] on the lowered
//! document, so each check runs once, on final geometry.
//!
//! The checks: `frame.child_overflow`, `layout.off_canvas`, `contrast.*`,
//! `safe_zone.violation`, `fold.content_crossing`, and `margin.violation`.
//!
//! [`validate`]: super::validate

use std::collections::BTreeMap;

use crate::ast::document::{Document, Page};
use crate::ast::node::{Node, subtree_uses_layout};
use crate::ast::style::Style;
use crate::ast::value::{PropertyValue, dim_to_px};
use crate::color::parse_rgb;
use crate::diagnostics::Diagnostic;
use crate::tokens::{ResolvedToken, ResolvedValue, resolve_tokens};

use super::contrast::{ContentScopes, check_page_text_contrast, check_scoped_text_contrast};
use super::nodes::placement_walk;
use super::{fold, margin, safezone};

/// `true` when validation leaves `page`'s geometry checks to the scene engine.
pub(super) fn page_is_layout_managed(page: &Page) -> bool {
    subtree_uses_layout(&page.children)
}

/// The page background as an RGB triple, when it is a color token.
pub(super) fn page_background_rgb(
    page: &Page,
    resolved: &BTreeMap<String, ResolvedToken>,
) -> Option<(u8, u8, u8)> {
    let PropertyValue::TokenRef(id) = page.background.as_ref()? else {
        return None;
    };
    match &resolved.get(id.as_str())?.value {
        ResolvedValue::Color(hex) => parse_rgb(hex),
        _ => None,
    }
}

/// Text contrast of `children` drawn on `page`: the compile-stage pass over
/// expanded content.
///
/// The scene engine passes the page content it compiled, with each
/// `instance` replaced by the subtree it expanded to and the master
/// projection first. Validation skips that content, because its ids and
/// positions exist only after expansion. The result judges every `text`
/// node in `children`; the caller keeps the diagnostics of expanded nodes.
/// Each keeps the span of its authored component or master node. A group
/// in `scopes` stands in for an instance drawn in its own token scope (an
/// imported component) or under a `w` / `h` fit transform.
pub fn expanded_text_contrast_checks<'a>(
    page: &Page,
    children: &[Node],
    resolved: &'a BTreeMap<String, ResolvedToken>,
    style_map: &'a BTreeMap<&'a str, &'a Style>,
    scopes: &'a ContentScopes<'a>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let Some(page_size) = dim_to_px(page.width.value, &page.width.unit)
        .zip(dim_to_px(page.height.value, &page.height.unit))
    else {
        return diagnostics;
    };
    check_scoped_text_contrast(
        children,
        page_background_rgb(page, resolved),
        page_size,
        resolved,
        style_map,
        Some(scopes),
        &mut diagnostics,
    );
    diagnostics
}

/// Run the geometry checks on every page of `doc` that uses auto-layout.
///
/// Call it with a document whose layout frames are lowered to absolute
/// geometry. The result holds one entry per page, in page order. A page
/// without a layout frame gets an empty entry, because [`super::validate`]
/// already checked it. Diagnostics name the authored nodes and keep their
/// source spans.
pub fn layout_geometry_checks(doc: &Document) -> Vec<Vec<Diagnostic>> {
    let resolution = resolve_tokens(&doc.tokens);
    let resolved = &resolution.resolved;
    let style_map: BTreeMap<&str, &Style> = doc
        .styles
        .styles
        .iter()
        .map(|s| (s.id.as_str(), s))
        .collect();
    let mirror_margins = doc.mirror_margins.unwrap_or(false);
    let rtl = doc.page_progression.as_deref() == Some("rtl");

    doc.body
        .pages
        .iter()
        .enumerate()
        .map(|(index, page)| {
            let mut diagnostics = Vec::new();
            if !page_is_layout_managed(page) {
                return diagnostics;
            }
            let Some((page_w, page_h)) = dim_to_px(page.width.value, &page.width.unit)
                .zip(dim_to_px(page.height.value, &page.height.unit))
            else {
                return diagnostics;
            };
            placement_walk(&page.children, None, (page_w, page_h), &mut diagnostics);
            check_page_text_contrast(
                &page.children,
                page_background_rgb(page, resolved),
                (page_w, page_h),
                resolved,
                &style_map,
                &mut diagnostics,
            );
            safezone::check_safe_zones(page, page_w, page_h, &mut diagnostics);
            fold::check_folds(page, page_w, page_h, &mut diagnostics);
            margin::check_margins(
                doc,
                page,
                margin::PageMarginCtx {
                    page_w,
                    page_h,
                    is_recto: doc.page_is_recto(page, index + 1),
                    mirror_margins,
                    rtl,
                },
                &mut diagnostics,
            );
            diagnostics
        })
        .collect()
}
