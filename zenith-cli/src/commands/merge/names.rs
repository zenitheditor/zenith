//! Output file names of merge rows.

use crate::commands::render::{BatchFormat, BatchLayout};

/// Map filesystem-unsafe characters and NUL to `_`, trim leading/trailing
/// dots and whitespace, and return `"_"` for the empty result.
pub fn sanitize_filename(s: &str) -> String {
    let mapped: String = s
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => '_',
            other => other,
        })
        .collect();
    let trimmed = mapped.trim_matches(|c: char| c == '.' || c.is_whitespace());
    if trimmed.is_empty() {
        "_".to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// Output file names of one row, in page order.
///
/// A per-page format gives one name per page. A single-page template keeps
/// the bare stem. A multi-page template adds the 1-based `-page-N` suffix,
/// as `zenith render --all-pages` does. A per-row format gives one
/// `<stem>.<ext>` name for every page. A row with no pages has no names.
pub(super) fn row_filenames(stem: &str, page_count: usize, format: BatchFormat) -> Vec<String> {
    if page_count == 0 {
        return Vec::new();
    }
    let extension = format.extension();
    match format.layout() {
        BatchLayout::PerRow => vec![format!("{stem}.{extension}")],
        BatchLayout::PerPage if page_count == 1 => vec![format!("{stem}.{extension}")],
        BatchLayout::PerPage => (1..=page_count)
            .map(|page| format!("{stem}-page-{page}.{extension}"))
            .collect(),
    }
}
