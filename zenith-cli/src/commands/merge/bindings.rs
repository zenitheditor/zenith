//! `role="data.<column>"` bindings of a merge template.

use super::run::MergeError;

// ── Internal binding types ────────────────────────────────────────────────────

/// Maps a node id to the CSV column that supplies its replacement text.
pub(super) struct DataBinding {
    pub(super) node_id: String,
    pub(super) column: String,
}

/// Maps an image node id to the CSV column that supplies the per-row image path.
pub(super) struct AssetBinding {
    pub(super) node_id: String,
    pub(super) column: String,
}

// ── collect_data_nodes ────────────────────────────────────────────────────────

/// Return an error if `role` starts with `"data."` on a non-text, non-image node.
///
/// The error message and format live here exactly once.
fn reject_data_role_on_non_text(role: Option<&str>, id: &str) -> Result<(), MergeError> {
    if let Some(role) = role
        && role.starts_with("data.")
    {
        return Err(MergeError::new(format!(
            "role=\"{}\" on non-text node {}: replace_text supports text nodes only",
            role, id
        )));
    }
    Ok(())
}

/// Walk `nodes` recursively and collect every node that carries a
/// `role="data.<column>"` attribute.
///
/// `Node::Text` bindings are collected into `out`; `Node::Image` bindings are
/// collected into `asset_out`.  Any other variant with a `data.*` role is a
/// hard [`MergeError`].
///
/// Recurses into `Node::Frame`, `Node::Group`, and `Node::Table` cell children.
pub(super) fn collect_data_nodes(
    nodes: &[zenith_core::Node],
    out: &mut Vec<DataBinding>,
    asset_out: &mut Vec<AssetBinding>,
) -> Result<(), MergeError> {
    for node in nodes {
        match node {
            zenith_core::Node::Text(n) => {
                if let Some(role) = n.role.as_deref()
                    && let Some(col) = role.strip_prefix("data.")
                {
                    out.push(DataBinding {
                        node_id: n.id.clone(),
                        column: col.to_owned(),
                    });
                }
            }
            zenith_core::Node::Image(n) => {
                if let Some(role) = n.role.as_deref()
                    && let Some(col) = role.strip_prefix("data.")
                {
                    asset_out.push(AssetBinding {
                        node_id: n.id.clone(),
                        column: col.to_owned(),
                    });
                }
            }
            zenith_core::Node::Rect(n) => {
                reject_data_role_on_non_text(n.role.as_deref(), &n.id)?;
            }
            zenith_core::Node::Ellipse(n) => {
                reject_data_role_on_non_text(n.role.as_deref(), &n.id)?;
            }
            zenith_core::Node::Line(n) => {
                reject_data_role_on_non_text(n.role.as_deref(), &n.id)?;
            }
            zenith_core::Node::Code(n) => {
                reject_data_role_on_non_text(n.role.as_deref(), &n.id)?;
            }
            zenith_core::Node::Frame(n) => {
                reject_data_role_on_non_text(n.role.as_deref(), &n.id)?;
                collect_data_nodes(&n.children, out, asset_out)?;
            }
            zenith_core::Node::Group(n) => {
                reject_data_role_on_non_text(n.role.as_deref(), &n.id)?;
                collect_data_nodes(&n.children, out, asset_out)?;
            }
            zenith_core::Node::Polygon(n) => {
                reject_data_role_on_non_text(n.role.as_deref(), &n.id)?;
            }
            zenith_core::Node::Polyline(n) => {
                reject_data_role_on_non_text(n.role.as_deref(), &n.id)?;
            }
            zenith_core::Node::Path(n) => {
                reject_data_role_on_non_text(n.role.as_deref(), &n.id)?;
            }
            zenith_core::Node::Instance(n) => {
                reject_data_role_on_non_text(n.role.as_deref(), &n.id)?;
            }
            zenith_core::Node::Field(n) => {
                reject_data_role_on_non_text(n.role.as_deref(), &n.id)?;
            }
            zenith_core::Node::Toc(n) => {
                reject_data_role_on_non_text(n.role.as_deref(), &n.id)?;
            }
            zenith_core::Node::Footnote(n) => {
                reject_data_role_on_non_text(n.role.as_deref(), &n.id)?;
            }
            zenith_core::Node::Table(n) => {
                reject_data_role_on_non_text(n.role.as_deref(), &n.id)?;
                for row in &n.rows {
                    for cell in &row.cells {
                        collect_data_nodes(&cell.children, out, asset_out)?;
                    }
                }
            }
            zenith_core::Node::Shape(n) => {
                reject_data_role_on_non_text(n.role.as_deref(), &n.id)?;
            }
            zenith_core::Node::Connector(n) => {
                reject_data_role_on_non_text(n.role.as_deref(), &n.id)?;
            }
            zenith_core::Node::Pattern(n) => {
                reject_data_role_on_non_text(n.role.as_deref(), &n.id)?;
            }
            zenith_core::Node::Chart(n) => {
                reject_data_role_on_non_text(n.role.as_deref(), &n.id)?;
            }
            zenith_core::Node::Light(n) => {
                reject_data_role_on_non_text(n.role.as_deref(), &n.id)?;
            }
            zenith_core::Node::Mesh(n) => {
                reject_data_role_on_non_text(n.role.as_deref(), &n.id)?;
            }
            zenith_core::Node::Unknown(_n) => {
                // UnknownNode has no id or role field; data.* roles cannot be
                // placed on unknown nodes (the parser would not parse them).
            }
        }
    }
    Ok(())
}
