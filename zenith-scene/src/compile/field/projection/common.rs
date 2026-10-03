//! Shared node helpers for the projection submodules: node-id extraction and
//! imported-component resolution used by the node-box and connector-target
//! walks alike.

use std::collections::BTreeMap;

use zenith_core::{ComponentDef, Node, ResolvedToken};

use crate::compile::imports::{ImportScopes, ImportSource, ImportedScope, parse_import_source};

/// The accumulated offset of the children of `node`: `(dx, dy)` plus the
/// node's [`Node::child_space`], unchanged when it has none.
pub(super) fn child_offset(
    node: &Node,
    dx: f64,
    dy: f64,
    resolved: &BTreeMap<String, ResolvedToken>,
) -> (f64, f64) {
    match node.child_space(resolved) {
        Some((sx, sy)) => (dx + sx, dy + sy),
        None => (dx, dy),
    }
}

pub(super) fn resolve_imported_component<'a>(
    source: &str,
    imports: &'a ImportScopes<'a>,
) -> Option<(&'a ImportedScope<'a>, &'a ComponentDef)> {
    if !imports.is_enabled() {
        return None;
    }
    let ImportSource::Component {
        import_id,
        component_id,
    } = parse_import_source(source)
    else {
        return None;
    };
    let imported = imports.get(import_id)?;
    let component = imported.components.get(component_id)?;
    Some((imported, component))
}
