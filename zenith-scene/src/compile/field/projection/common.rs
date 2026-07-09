//! Shared node helpers for the projection submodules: node-id extraction and
//! imported-component resolution used by the node-box and connector-target
//! walks alike.

use zenith_core::ComponentDef;

use crate::compile::imports::{ImportScopes, ImportSource, ImportedScope, parse_import_source};

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
