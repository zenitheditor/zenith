//! Materialize an imported component into the host document.
//!
//! Escape hatch for live `source="import#component.id"` references: copy the
//! component subtree (plus tokens/styles/assets) into the host with a synthetic
//! `libraries` + `provenance` record (`linked=false`). Page targets, rewire,
//! and lockfiles are out of scope for this first cut.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use zenith_core::{
    AssetDecl, ComponentDef, Document, InstanceNode, KdlAdapter, KdlSource as _, LibraryDef, Node,
    ProvenanceDef, Style, Token, validate,
};

use super::load_import_graph;
use super::loaded::{ImportEdge, ImportEdgeStatus, LoadedImportGraph};
use super::path::normalize_import_path;
use super::source::{ImportSource, parse_import_source};
use crate::commands::{format_diagnostic_line, serialize_pretty};
use crate::library::{collect_all_ids, px, unique_id};

const SCHEMA: &str = "zenith-imports-materialize-v1";

/// Error returned by `zenith imports materialize`.
#[derive(Debug)]
pub(crate) struct MaterializeCmdErr {
    pub message: String,
    pub exit_code: u8,
}

impl MaterializeCmdErr {
    fn fail(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            exit_code: 2,
        }
    }

    fn validation(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            exit_code: 1,
        }
    }
}

/// Structured outcome of a successful materialize (human + JSON).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MaterializeOutcome {
    pub import_id: String,
    pub component_id: String,
    pub target_component_id: String,
    pub instance_id: String,
    pub provenance_id: String,
    pub library_id: String,
    pub page_id: String,
    pub warnings: Vec<String>,
}

/// Result of materialize: formatted host bytes + summaries.
#[derive(Debug)]
pub(crate) struct MaterializeResult {
    /// Canonical formatted host document after materialization.
    pub formatted: Vec<u8>,
    /// Human-readable multi-line summary.
    pub summary: String,
    /// Structured fields for JSON output.
    pub outcome: MaterializeOutcome,
}

#[derive(Debug, serde::Serialize)]
struct MaterializeJson {
    schema: &'static str,
    document: String,
    import_id: String,
    component_id: String,
    target_component_id: String,
    instance_id: String,
    provenance_id: String,
    library_id: String,
    page: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
}

/// Materialize `--target import#component.id` into host source at `host_path`.
///
/// Pure with respect to the host file (no write); the dispatcher owns I/O.
/// Loads the import graph from `host_path`'s parent directory.
///
/// # Errors
///
/// Exit code 2 for parse/target/import/page/component failures. Exit code 1
/// when the post-mutation host fails validation with hard errors.
pub(crate) fn run(
    host_src: &str,
    host_path: &Path,
    target: &str,
    page_id: &str,
    at: (f64, f64),
    id_override: Option<&str>,
) -> Result<MaterializeResult, MaterializeCmdErr> {
    let mut host = KdlAdapter
        .parse(host_src.as_bytes())
        .map_err(|e| MaterializeCmdErr::fail(format!("error[parse.error]: {}", e.message)))?;

    let (import_id, component_id) = parse_component_target(target)?;

    let host_dir = host_path.parent().filter(|p| !p.as_os_str().is_empty());
    let graph = load_import_graph(&host, host_dir);

    let edge = resolve_target_edge(&graph, &host, import_id)?;
    let import_doc = graph.documents.get(import_id).ok_or_else(|| {
        MaterializeCmdErr::fail(format!(
            "import '{import_id}' did not load a document (status={})",
            edge.status.as_str()
        ))
    })?;

    if !host.body.pages.iter().any(|p| p.id == page_id) {
        let available: Vec<&str> = host.body.pages.iter().map(|p| p.id.as_str()).collect();
        return Err(MaterializeCmdErr::fail(format!(
            "page '{page_id}' not found in host document (available: {})",
            format_id_list(&available)
        )));
    }

    let comp = import_doc
        .components
        .iter()
        .find(|c| c.id == component_id)
        .ok_or_else(|| {
            let available: Vec<&str> = import_doc
                .components
                .iter()
                .map(|c| c.id.as_str())
                .collect();
            MaterializeCmdErr::fail(format!(
                "unknown component '{component_id}' in import '{import_id}' (available: {})",
                format_id_list(&available)
            ))
        })?;

    let mut warnings: Vec<String> = Vec::new();

    // 1. Copy component under namespaced host id (dedup if already present).
    let target_comp_id = import_component_id(import_id, component_id);
    if !host.components.iter().any(|c| c.id == target_comp_id) {
        host.components.push(ComponentDef {
            id: target_comp_id.clone(),
            ports: comp.ports.clone(),
            children: comp.children.clone(),
            source_span: None,
        });
    }

    // 2. Copy tokens/styles (conflict → keep host + warning).
    if host.tokens.format.is_empty() {
        host.tokens.format = import_doc.tokens.format.clone();
    }
    copy_tokens(
        &import_doc.tokens.tokens,
        &mut host.tokens.tokens,
        &mut warnings,
    );
    copy_styles(
        &import_doc.styles.styles,
        &mut host.styles.styles,
        &mut warnings,
    );

    // 3. Copy assets; rewrite paths so host resolves relative to host dir.
    let import_dir = graph
        .document_dirs
        .get(import_id)
        .map(PathBuf::as_path)
        .or_else(|| edge.resolved_path.as_ref().and_then(|p| p.parent()));
    copy_assets_rewritten(
        &import_doc.assets.assets,
        &mut host.assets.assets,
        host_dir,
        import_dir,
        &mut warnings,
    );

    // 4. Place InstanceNode on the page.
    let id_base = id_override.unwrap_or(component_id);
    let mut all_ids = collect_all_ids(&host);
    // Import declaration ids share the global id namespace (validate registers
    // them) but are not part of collect_all_ids; reserve them so instance /
    // provenance ids cannot collide with `import id="…"`.
    for imp in &host.imports {
        all_ids.insert(imp.id.clone());
    }
    let instance_id = unique_id(id_base, &all_ids);
    all_ids.insert(instance_id.clone());

    let (at_x, at_y) = at;
    let instance = InstanceNode {
        id: instance_id.clone(),
        name: None,
        role: None,
        component: Some(target_comp_id.clone()),
        source: None,
        x: Some(px(at_x)),
        y: Some(px(at_y)),
        w: None,
        h: None,
        fit: None,
        opacity: None,
        visible: None,
        locked: None,
        overrides: Vec::new(),
        source_span: None,
        unknown_props: BTreeMap::new(),
    };

    if let Some(page) = host.body.pages.iter_mut().find(|p| p.id == page_id) {
        page.children.push(Node::Instance(instance));
    }

    // 5. Synthetic libraries entry + detached provenance.
    let library_id = format!("import:{import_id}");
    let provenance_id = unique_id(&format!("prov.{instance_id}"), &all_ids);
    let hash = edge
        .sha256_actual
        .clone()
        .or_else(|| edge.sha256_declared.clone());

    if !host.libraries.iter().any(|l| l.id == library_id) {
        host.libraries.push(LibraryDef {
            id: library_id.clone(),
            version: None,
            hash,
            source_span: None,
            unknown_props: BTreeMap::new(),
        });
    }

    let item = format!("component.{component_id}");
    host.provenance.push(ProvenanceDef {
        id: provenance_id.clone(),
        node: instance_id.clone(),
        library: library_id.clone(),
        item: Some(item),
        linked: Some(false),
        source_span: None,
        unknown_props: BTreeMap::new(),
    });

    let formatted = validate_and_format(&host)?;

    let outcome = MaterializeOutcome {
        import_id: import_id.to_owned(),
        component_id: component_id.to_owned(),
        target_component_id: target_comp_id,
        instance_id,
        provenance_id,
        library_id,
        page_id: page_id.to_owned(),
        warnings,
    };
    let summary = format_summary(&outcome);

    Ok(MaterializeResult {
        formatted,
        summary,
        outcome,
    })
}

/// Format JSON output for materialize (`zenith-imports-materialize-v1`).
///
/// When `include_source` is true (dry-run), the formatted host KDL is embedded.
pub(crate) fn format_json(
    host_path: &Path,
    result: &MaterializeResult,
    include_source: bool,
) -> String {
    let source = if include_source {
        Some(String::from_utf8_lossy(&result.formatted).into_owned())
    } else {
        None
    };
    let out = MaterializeJson {
        schema: SCHEMA,
        document: host_path.display().to_string(),
        import_id: result.outcome.import_id.clone(),
        component_id: result.outcome.component_id.clone(),
        target_component_id: result.outcome.target_component_id.clone(),
        instance_id: result.outcome.instance_id.clone(),
        provenance_id: result.outcome.provenance_id.clone(),
        library_id: result.outcome.library_id.clone(),
        page: result.outcome.page_id.clone(),
        warnings: result.outcome.warnings.clone(),
        source,
    };
    serialize_pretty(&out)
}

fn parse_component_target(target: &str) -> Result<(&str, &str), MaterializeCmdErr> {
    match parse_import_source(target) {
        ImportSource::Component {
            import_id,
            component_id,
        } => Ok((import_id, component_id)),
        ImportSource::Page { .. } => Err(MaterializeCmdErr::fail(format!(
            "page targets are not supported by imports materialize yet (got {target:?}); \
             only `#component.*` targets are accepted"
        ))),
        ImportSource::UnsupportedTarget => Err(MaterializeCmdErr::fail(format!(
            "unsupported materialize target {target:?} (expected `<import>#component.<id>`)"
        ))),
        ImportSource::Invalid => Err(MaterializeCmdErr::fail(format!(
            "malformed materialize target {target:?} (expected `<import>#component.<id>`, \
             e.g. `brand#component.logo`)"
        ))),
    }
}

/// Resolve the host-level edge for `import_id`, failing hard on load problems.
fn resolve_target_edge<'a>(
    graph: &'a LoadedImportGraph,
    host: &Document,
    import_id: &str,
) -> Result<&'a ImportEdge, MaterializeCmdErr> {
    if !host.imports.iter().any(|i| i.id == import_id) {
        let available: Vec<&str> = host.imports.iter().map(|i| i.id.as_str()).collect();
        return Err(MaterializeCmdErr::fail(format!(
            "import '{import_id}' is not declared on the host document (available: {})",
            format_id_list(&available)
        )));
    }

    let edge = graph
        .edges()
        .iter()
        .find(|e| e.id == import_id && e.depth == 0)
        .ok_or_else(|| {
            MaterializeCmdErr::fail(format!(
                "import '{import_id}' has no host-level load edge (internal loader error)"
            ))
        })?;

    match edge.status {
        ImportEdgeStatus::Ok => Ok(edge),
        ImportEdgeStatus::Missing => Err(MaterializeCmdErr::fail(format!(
            "import '{import_id}' file is missing{}",
            edge.resolved_path
                .as_ref()
                .map(|p| format!(" ({})", p.display()))
                .unwrap_or_default()
        ))),
        ImportEdgeStatus::HashMismatch => Err(MaterializeCmdErr::fail(format!(
            "import '{import_id}' sha256 mismatch (declared {}, actual {})",
            edge.sha256_declared.as_deref().unwrap_or("?"),
            edge.sha256_actual.as_deref().unwrap_or("?")
        ))),
        ImportEdgeStatus::ParseError => Err(MaterializeCmdErr::fail(format!(
            "import '{import_id}' could not be parsed{}",
            edge.resolved_path
                .as_ref()
                .map(|p| format!(" from '{}'", p.display()))
                .unwrap_or_default()
        ))),
        ImportEdgeStatus::Cycle => Err(MaterializeCmdErr::fail(format!(
            "import '{import_id}' participates in an import cycle"
        ))),
        ImportEdgeStatus::Unresolvable => Err(MaterializeCmdErr::fail(format!(
            "import '{import_id}' cannot be resolved without a project directory"
        ))),
        ImportEdgeStatus::SkippedKind => Err(MaterializeCmdErr::fail(format!(
            "import '{import_id}' has kind {:?} (only kind=\"zen\" can be materialized)",
            edge.kind
        ))),
    }
}

/// Namespaced host component id: `import.<sanitized_import_id>.<component_id>`.
fn import_component_id(import_id: &str, component_id: &str) -> String {
    format!(
        "import.{}.{}",
        sanitize_id_fragment(import_id),
        component_id
    )
}

/// Sanitize an import id fragment for use inside a component id.
fn sanitize_id_fragment(id: &str) -> String {
    let mut out = String::with_capacity(id.len());
    let mut prev_dot = false;
    for ch in id.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
            out.push(ch);
            prev_dot = ch == '.';
        } else if !prev_dot && !out.is_empty() {
            out.push('.');
            prev_dot = true;
        }
    }
    while out.ends_with('.') {
        out.pop();
    }
    if out.is_empty() {
        "import".to_owned()
    } else {
        out
    }
}

fn copy_tokens(from: &[Token], target: &mut Vec<Token>, warnings: &mut Vec<String>) {
    for tok in from {
        match target.iter().find(|t| t.id == tok.id) {
            Some(existing)
                if existing.token_type != tok.token_type || existing.value != tok.value =>
            {
                warnings.push(dependency_conflict("token", &tok.id));
            }
            Some(_) => {}
            None => target.push(tok.clone()),
        }
    }
}

fn copy_styles(from: &[Style], target: &mut Vec<Style>, warnings: &mut Vec<String>) {
    for st in from {
        match target.iter().find(|t| t.id == st.id) {
            Some(existing) if existing.properties != st.properties => {
                warnings.push(dependency_conflict("style", &st.id));
            }
            Some(_) => {}
            None => target.push(st.clone()),
        }
    }
}

/// Copy asset decls, rewriting `src` relative to the host directory when possible.
fn copy_assets_rewritten(
    from: &[AssetDecl],
    target: &mut Vec<AssetDecl>,
    host_dir: Option<&Path>,
    import_dir: Option<&Path>,
    warnings: &mut Vec<String>,
) {
    for asset in from {
        let mut rewritten = asset.clone();
        rewritten.src = rewrite_asset_src(host_dir, import_dir, &asset.src);
        match target.iter().find(|a| a.id == rewritten.id) {
            Some(existing)
                if existing.kind != rewritten.kind
                    || existing.src != rewritten.src
                    || existing.sha256 != rewritten.sha256 =>
            {
                warnings.push(dependency_conflict("asset", &rewritten.id));
            }
            Some(_) => {}
            None => target.push(rewritten),
        }
    }
}

fn rewrite_asset_src(host_dir: Option<&Path>, import_dir: Option<&Path>, src: &str) -> String {
    let Some(import_dir) = import_dir else {
        return src.to_owned();
    };
    let Some(host_dir) = host_dir else {
        return src.to_owned();
    };

    let absolute = if Path::new(src).is_absolute() {
        PathBuf::from(src)
    } else {
        normalize_import_path(import_dir, src)
    };

    match relative_path(host_dir, &absolute) {
        Some(rel) => path_to_unix(&rel),
        None => src.to_owned(),
    }
}

/// Lexical relative path from `base` to `target` (no filesystem access).
fn relative_path(base: &Path, target: &Path) -> Option<PathBuf> {
    let base_comps: Vec<Component<'_>> = base.components().collect();
    let target_comps: Vec<Component<'_>> = target.components().collect();

    let mut i = 0;
    while i < base_comps.len()
        && i < target_comps.len()
        && base_comps[i].as_os_str() == target_comps[i].as_os_str()
    {
        i += 1;
    }

    // Different roots (e.g. different drive prefixes) cannot be relativized.
    if i == 0
        && base_comps
            .first()
            .is_some_and(|c| matches!(c, Component::Prefix(_) | Component::RootDir))
        && target_comps
            .first()
            .is_some_and(|c| matches!(c, Component::Prefix(_) | Component::RootDir))
        && base_comps.first().map(|c| c.as_os_str()) != target_comps.first().map(|c| c.as_os_str())
    {
        return None;
    }

    let mut rel = PathBuf::new();
    for _ in i..base_comps.len() {
        rel.push("..");
    }
    for c in &target_comps[i..] {
        rel.push(c.as_os_str());
    }
    if rel.as_os_str().is_empty() {
        Some(PathBuf::from("."))
    } else {
        Some(rel)
    }
}

fn path_to_unix(path: &Path) -> String {
    path.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn dependency_conflict(kind: &str, id: &str) -> String {
    format!(
        "import.dependency_conflict: {kind} '{id}' already exists in the host with a \
         different definition; kept the existing one"
    )
}

/// Comma-join ids for error messages; `"none"` when the list is empty.
fn format_id_list(ids: &[&str]) -> String {
    if ids.is_empty() {
        "none".to_owned()
    } else {
        ids.join(", ")
    }
}

fn format_summary(outcome: &MaterializeOutcome) -> String {
    let mut summary = String::new();
    summary.push_str(&format!(
        "materialized {}#component.{} as instance '{}' on page '{}'\n",
        outcome.import_id, outcome.component_id, outcome.instance_id, outcome.page_id
    ));
    summary.push_str(&format!("  component: {}\n", outcome.target_component_id));
    summary.push_str(&format!("  library: {}\n", outcome.library_id));
    summary.push_str(&format!("  provenance: {}", outcome.provenance_id));
    for w in &outcome.warnings {
        summary.push_str(&format!("\n  warning: {w}"));
    }
    summary
}

fn validate_and_format(host: &Document) -> Result<Vec<u8>, MaterializeCmdErr> {
    let report = validate(host);
    let errors: Vec<String> = report
        .diagnostics
        .iter()
        .filter(|d| d.is_error())
        .map(format_diagnostic_line)
        .collect();
    if !errors.is_empty() {
        return Err(MaterializeCmdErr::validation(format!(
            "materialized document has {} validation error(s):\n{}",
            errors.len(),
            errors.join("\n")
        )));
    }
    KdlAdapter
        .format(host)
        .map_err(|e| MaterializeCmdErr::fail(format!("format error: {}", e.message)))
}

#[cfg(test)]
#[path = "materialize_tests.rs"]
mod tests;
