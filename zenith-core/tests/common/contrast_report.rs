//! `contrast_report`, shared by the `child_space`, `validate_contrast`, and
//! `validate_contrast_effects` binaries.

use std::collections::BTreeMap;
use zenith_core::{Document, Style, ValidationReport, validate};

/// Validate `doc` and judge the contrast of every page on its authored boxes
/// (no measured glyph ink): the core judgement the compile stage runs with
/// drawn ink. Pages read the defaults-lowered document, as the compile does.
pub fn contrast_report(doc: &Document) -> ValidationReport {
    let mut report = validate(doc);
    let resolution = zenith_core::resolve_tokens(&doc.tokens);
    let lowered = zenith_core::defaults::lower(doc, &resolution.resolved);
    let doc = lowered.as_ref().map_or(doc, |l| &l.document);
    let style_map: BTreeMap<&str, &Style> = doc
        .styles
        .styles
        .iter()
        .map(|s| (s.id.as_str(), s))
        .collect();
    let scopes = zenith_core::ContentScopes::new();
    for page in &doc.body.pages {
        report.diagnostics.extend(zenith_core::page_contrast_checks(
            page,
            &page.children,
            &resolution.resolved,
            &style_map,
            None,
            &scopes,
        ));
    }
    report
}
