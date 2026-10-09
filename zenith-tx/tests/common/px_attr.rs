//! `extract_px_attr`, shared by the `align_distribute`, `frame_space`,
//! `layout_ops`, and `structure` binaries.

/// Parse a node's attribute value from `source` by locating the node line via
/// `id="<node_id>"` and then reading `<attr>=(px)<value>` on that line.
/// Intentionally naive — sufficient for the deterministic test documents used
/// throughout this suite.
pub fn extract_px_attr(source: &str, node_id: &str, attr: &str) -> Option<f64> {
    source
        .lines()
        .find(|line| line.contains(&format!("id=\"{node_id}\"")))
        .and_then(|line| {
            let needle = format!("{attr}=(px)");
            let start = line.find(&needle)? + needle.len();
            let rest = &line[start..];
            let end = rest
                .find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')
                .unwrap_or(rest.len());
            rest[..end].parse::<f64>().ok()
        })
}
