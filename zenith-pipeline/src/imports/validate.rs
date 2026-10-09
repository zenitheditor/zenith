//! Root-document target validation and expanded-id collision detection over the
//! loaded import graph.

use std::collections::{BTreeMap, BTreeSet};

use zenith_core::{Diagnostic, Document, ImportDecl, InstanceNode, Node, Page};

use super::loader::ImportGraphLoader;
use super::source::{ImportSource, parse_import_source};
use super::walk::{collect_all_node_ids, collect_instances, same_page_size};

impl ImportGraphLoader<'_> {
    pub(super) fn validate_root_targets(&mut self, root: &Document) {
        let declared_imports: BTreeMap<&str, &ImportDecl> = root
            .imports
            .iter()
            .map(|import| (import.id.as_str(), import))
            .collect();
        for page in &root.body.pages {
            self.validate_page_source(page, &declared_imports);
            self.validate_node_sources(&page.children, &declared_imports);
        }
        for component in &root.components {
            self.validate_node_sources(&component.children, &declared_imports);
        }
        for master in &root.masters {
            self.validate_node_sources(&master.children, &declared_imports);
        }
    }

    /// Detect page-level imported-instance id collisions.
    ///
    /// An imported component instance expands its descendant ids as
    /// `<instance-id>/<local-id>`. Because a host node may be authored with a
    /// literal slash-bearing id, an expansion can silently duplicate an existing
    /// host node id. This guard forms every expanded id for each page-level
    /// imported-component instance and emits `import.id_collision` (a hard Error)
    /// when it clashes with an authored host page-node id.
    ///
    /// (Distinct instance-id prefixes make cross-instance and within-instance
    /// collisions structurally impossible under this scheme, so only the
    /// host-clash case is checked.)
    pub(super) fn detect_id_collisions(&mut self, root: &Document) {
        let mut host_ids: BTreeSet<String> = BTreeSet::new();
        let mut instances: Vec<&InstanceNode> = Vec::new();
        for page in &root.body.pages {
            collect_all_node_ids(&page.children, &mut host_ids);
            collect_instances(&page.children, &mut instances);
        }

        // Collect owned collisions first so the immutable borrow of `self.documents`
        // ends before pushing into `self.diagnostics`.
        let mut collisions: Vec<(String, Option<zenith_core::Span>, String)> = Vec::new();
        for instance in instances {
            let Some(source) = instance.source.as_deref() else {
                continue;
            };
            let ImportSource::Component {
                import_id,
                component_id,
            } = parse_import_source(source)
            else {
                continue;
            };
            let Some(imported) = self.documents.get(import_id) else {
                continue;
            };
            let Some(component) = imported
                .components
                .iter()
                .find(|component| component.id == component_id)
            else {
                continue;
            };
            let mut local_ids: BTreeSet<String> = BTreeSet::new();
            collect_all_node_ids(&component.children, &mut local_ids);
            for local in &local_ids {
                let expanded = format!("{}/{}", instance.id, local);
                if host_ids.contains(&expanded) {
                    collisions.push((instance.id.clone(), instance.source_span, expanded));
                }
            }
        }

        for (instance_id, span, expanded) in collisions {
            self.diagnostics.push(Diagnostic::error(
                "import.id_collision",
                format!(
                    "instance '{instance_id}' expands to node id '{expanded}' which collides with an existing host node id"
                ),
                span,
                Some(instance_id),
            ));
        }
    }

    fn validate_page_source(
        &mut self,
        page: &Page,
        declared_imports: &BTreeMap<&str, &ImportDecl>,
    ) {
        let Some(source) = page.source.as_deref() else {
            return;
        };
        match parse_import_source(source) {
            ImportSource::Page { import_id, page_id } => {
                let Some(imported) = self.imported_document_for_reference(
                    import_id,
                    declared_imports,
                    page.source_span,
                ) else {
                    return;
                };
                let Some(imported_page) = imported
                    .body
                    .pages
                    .iter()
                    .find(|candidate| candidate.id == page_id)
                else {
                    self.push_unknown_reference(
                        format!(
                            "page '{}' source references unknown page '{}' in import '{}'",
                            page.id, page_id, import_id
                        ),
                        page.source_span,
                        Some(page.id.clone()),
                    );
                    return;
                };
                if page.fit.is_none() && !same_page_size(page, imported_page) {
                    self.diagnostics.push(Diagnostic::error(
                        "import.page_size_mismatch",
                        format!(
                            "page '{}' source '{}' has different dimensions and no explicit fit",
                            page.id, source
                        ),
                        page.source_span,
                        Some(page.id.clone()),
                    ));
                }
            }
            ImportSource::Component { .. }
            | ImportSource::UnsupportedTarget
            | ImportSource::Invalid => self.push_unsupported_target(
                format!(
                    "page '{}' source '{}' is not a supported page target",
                    page.id, source
                ),
                page.source_span,
                Some(page.id.clone()),
            ),
        }
    }

    fn validate_node_sources(
        &mut self,
        nodes: &[Node],
        declared_imports: &BTreeMap<&str, &ImportDecl>,
    ) {
        for node in nodes {
            match node {
                Node::Frame(frame) => self.validate_node_sources(&frame.children, declared_imports),
                Node::Group(group) => self.validate_node_sources(&group.children, declared_imports),
                Node::Table(table) => {
                    for row in &table.rows {
                        for cell in &row.cells {
                            self.validate_node_sources(&cell.children, declared_imports);
                        }
                    }
                }
                Node::Instance(instance) => {
                    if let Some(source) = instance.source.as_deref() {
                        match parse_import_source(source) {
                            ImportSource::Component {
                                import_id,
                                component_id,
                            } => {
                                let Some(imported) = self.imported_document_for_reference(
                                    import_id,
                                    declared_imports,
                                    instance.source_span,
                                ) else {
                                    continue;
                                };
                                if !imported
                                    .components
                                    .iter()
                                    .any(|component| component.id == component_id)
                                {
                                    self.push_unknown_reference(
                                        format!(
                                            "instance '{}' source references unknown component '{}' in import '{}'",
                                            instance.id, component_id, import_id
                                        ),
                                        instance.source_span,
                                        Some(instance.id.clone()),
                                    );
                                }
                            }
                            ImportSource::Page { .. }
                            | ImportSource::UnsupportedTarget
                            | ImportSource::Invalid => self.push_unsupported_target(
                                format!(
                                    "instance '{}' source '{}' is not a supported component target",
                                    instance.id, source
                                ),
                                instance.source_span,
                                Some(instance.id.clone()),
                            ),
                        }
                    }
                }
                Node::Unknown(unknown) => {
                    self.validate_node_sources(&unknown.children, declared_imports);
                }
                Node::Rect(_)
                | Node::Ellipse(_)
                | Node::Line(_)
                | Node::Text(_)
                | Node::Code(_)
                | Node::Image(_)
                | Node::Polygon(_)
                | Node::Polyline(_)
                | Node::Path(_)
                | Node::Field(_)
                | Node::Footnote(_)
                | Node::Toc(_)
                | Node::Shape(_)
                | Node::Connector(_)
                | Node::Pattern(_)
                | Node::Chart(_)
                | Node::Light(_)
                | Node::Mesh(_) => {}
            }
        }
    }

    fn imported_document_for_reference(
        &mut self,
        import_id: &str,
        declared_imports: &BTreeMap<&str, &ImportDecl>,
        span: Option<zenith_core::Span>,
    ) -> Option<&Document> {
        if self.documents.contains_key(import_id) {
            return self.documents.get(import_id);
        }
        if declared_imports.contains_key(import_id) {
            return None;
        }
        self.push_unknown_reference(
            format!("source references undeclared import '{}'", import_id),
            span,
            Some(import_id.to_owned()),
        );
        None
    }
}
