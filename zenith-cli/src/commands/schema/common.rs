//! Shared formatting helper for the `zenith schema` command surfaces.

use crate::json_types::SchemaAttr;

/// Render an attribute list as an aligned `name — type` table.
pub(super) fn format_attr_table(attrs: &[SchemaAttr]) -> String {
    let col_width = attrs.iter().map(|a| a.name.len()).max().unwrap_or(0);

    let mut out = String::new();
    for attr in attrs {
        let default = attr
            .default
            .as_deref()
            .map(|d| format!(" (default: {d})"))
            .unwrap_or_default();
        out.push_str(&format!(
            "  {:<col_width$}  —  {}{default}\n",
            attr.name,
            attr.ty,
            col_width = col_width,
        ));
    }
    out
}
