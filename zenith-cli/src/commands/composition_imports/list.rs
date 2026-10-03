//! Read-only `zenith imports list` — dump the composition import graph.
//!
//! Reuses [`super::load_import_graph`]; does not re-walk the filesystem itself.
//! Host parse failure is exit 2; graph edge failures stay exit 0 and appear in
//! the edge/status (and optional diagnostics) data.

use std::path::Path;

use zenith_core::{Diagnostic, KdlAdapter, KdlSource as _};

use super::load_import_graph;
use super::loaded::ImportEdge;
use crate::commands::{format_diagnostic_line, serialize_pretty};
use crate::json_types::DiagnosticJson;

const SCHEMA: &str = "zenith-imports-list-v1";

/// Error returned when the host document cannot be listed (parse failure).
#[derive(Debug)]
pub(crate) struct ListCmdErr {
    pub message: String,
    pub exit_code: u8,
}

/// Machine-readable envelope for `zenith imports list --json`.
#[derive(Debug, serde::Serialize)]
struct ImportsListOutput {
    schema: &'static str,
    document: String,
    edges: Vec<EdgeJson>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    diagnostics: Vec<DiagnosticJson>,
}

#[derive(Debug, serde::Serialize)]
struct EdgeJson {
    #[serde(skip_serializing_if = "Option::is_none")]
    importer: Option<String>,
    id: String,
    kind: String,
    src: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    resolved_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sha256_declared: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sha256_actual: Option<String>,
    status: &'static str,
    depth: u32,
}

impl From<&ImportEdge> for EdgeJson {
    fn from(edge: &ImportEdge) -> Self {
        Self {
            importer: edge.importer.as_ref().map(|p| p.display().to_string()),
            id: edge.id.clone(),
            kind: edge.kind.clone(),
            src: edge.src.clone(),
            resolved_path: edge.resolved_path.as_ref().map(|p| p.display().to_string()),
            sha256_declared: edge.sha256_declared.clone(),
            sha256_actual: edge.sha256_actual.clone(),
            status: edge.status.as_str(),
            depth: edge.depth,
        }
    }
}

/// List composition import edges for in-memory host source at `document_path`.
///
/// `document_path` is used as the schema `document` label and to derive the
/// project directory (`parent`) for path resolution.
///
/// - Host parse failure → [`ListCmdErr`] with exit code 2.
/// - Graph edge failures (missing/hash/cycle/…) → success with status on edges.
pub(crate) fn run(src: &str, document_path: &Path, json: bool) -> Result<String, ListCmdErr> {
    let doc = KdlAdapter.parse(src.as_bytes()).map_err(|e| ListCmdErr {
        message: crate::report::parse_error_line(src, &e),
        exit_code: 2,
    })?;

    let project_dir = document_path.parent();
    let graph = load_import_graph(&doc, project_dir);
    let document_label = document_path.display().to_string();

    if json {
        let out = ImportsListOutput {
            schema: SCHEMA,
            document: document_label,
            edges: graph.edges().iter().map(EdgeJson::from).collect(),
            diagnostics: graph
                .diagnostics()
                .iter()
                .map(DiagnosticJson::from)
                .collect(),
        };
        Ok(serialize_pretty(&out))
    } else {
        Ok(format_human(
            &document_label,
            graph.edges(),
            graph.diagnostics(),
        ))
    }
}

fn format_human(document: &str, edges: &[ImportEdge], diagnostics: &[Diagnostic]) -> String {
    let mut lines = Vec::new();
    lines.push(format!("document: {document}"));
    if edges.is_empty() {
        lines.push("  (no imports)".to_owned());
    } else {
        for edge in edges {
            lines.push(format_edge_line(edge));
        }
    }
    if !diagnostics.is_empty() {
        lines.push(String::new());
        lines.push("diagnostics:".to_owned());
        for d in diagnostics {
            lines.push(format!("  {}", format_diagnostic_line(d)));
        }
    }
    lines.join("\n")
}

fn format_edge_line(edge: &ImportEdge) -> String {
    let indent = "  ".repeat(edge.depth as usize + 1);
    let resolved = edge
        .resolved_path
        .as_ref()
        .map_or_else(|| "-".to_owned(), |p| p.display().to_string());
    let mut line = format!(
        "{indent}[{status}] {id}  kind={kind}  src={src}  → {resolved}",
        status = edge.status.as_str(),
        id = edge.id,
        kind = edge.kind,
        src = edge.src,
    );
    if let Some(importer) = edge.importer.as_ref() {
        line.push_str(&format!("  importer={}", importer.display()));
    }
    if let Some(declared) = edge.sha256_declared.as_deref() {
        line.push_str(&format!("  sha256_declared={declared}"));
    }
    if let Some(actual) = edge.sha256_actual.as_deref() {
        line.push_str(&format!("  sha256_actual={actual}"));
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const EMPTY_DOC: &str = r#"zenith version=1 {
  project id="proj.empty" name="Empty"
  document id="doc.empty" title="Empty" {
    page id="page.empty" w=(px)100 h=(px)100
  }
}
"#;

    fn host_with_import(src: &str) -> String {
        format!(
            r#"zenith version=1 {{
  project id="proj.root" name="Root"
  imports {{
    import id="child" kind="zen" src="{src}"
  }}
  document id="doc.root" title="Root" {{
    page id="page.root" w=(px)100 h=(px)100
  }}
}}
"#
        )
    }

    #[test]
    fn list_json_schema_and_ok_edge() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::write(dir.path().join("child.zen"), EMPTY_DOC).expect("write child");
        let host_path = dir.path().join("host.zen");
        let host_src = host_with_import("child.zen");
        fs::write(&host_path, &host_src).expect("write host");

        let out = run(&host_src, &host_path, true).expect("list must succeed");
        let v: serde_json::Value = serde_json::from_str(&out).expect("json");
        assert_eq!(v["schema"], SCHEMA);
        assert_eq!(v["document"], host_path.display().to_string());
        let edges = v["edges"].as_array().expect("edges array");
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0]["id"], "child");
        assert_eq!(edges[0]["status"], "ok");
        assert_eq!(edges[0]["depth"], 0);
        assert!(edges[0]["sha256_actual"].as_str().is_some());
        assert!(v.get("diagnostics").is_none() || v["diagnostics"].as_array().unwrap().is_empty());
    }

    #[test]
    fn list_reports_missing_as_edge_status_exit_ok() {
        let dir = tempfile::tempdir().expect("tempdir");
        let host_path = dir.path().join("host.zen");
        let host_src = host_with_import("gone.zen");
        fs::write(&host_path, &host_src).expect("write host");

        let out = run(&host_src, &host_path, true).expect("list must succeed on graph failures");
        let v: serde_json::Value = serde_json::from_str(&out).expect("json");
        assert_eq!(v["edges"][0]["status"], "missing");
        let diags = v["diagnostics"].as_array().expect("diagnostics present");
        assert_eq!(diags[0]["code"], "import.missing");
    }

    #[test]
    fn list_parse_failure_is_exit_2() {
        let path = Path::new("broken.zen");
        let err = run("not zenith", path, false).expect_err("parse must fail");
        assert_eq!(err.exit_code, 2);
        assert!(err.message.contains("parse.error"), "{}", err.message);
    }

    #[test]
    fn list_human_mentions_status() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::write(dir.path().join("child.zen"), EMPTY_DOC).expect("write child");
        let host_path = dir.path().join("host.zen");
        let host_src = host_with_import("child.zen");
        let out = run(&host_src, &host_path, false).expect("list");
        assert!(out.contains("[ok]"), "{out}");
        assert!(out.contains("child"), "{out}");
        assert!(out.contains("document:"), "{out}");
    }

    #[test]
    fn list_skipped_kind_edge() {
        let dir = tempfile::tempdir().expect("tempdir");
        let host_path = dir.path().join("host.zen");
        let host_src = r#"zenith version=1 {
  project id="proj.root" name="Root"
  imports {
    import id="pic" kind="image" src="x.png"
  }
  document id="doc.root" title="Root" {
    page id="page.root" w=(px)100 h=(px)100
  }
}
"#;
        let out = run(host_src, &host_path, true).expect("list");
        let v: serde_json::Value = serde_json::from_str(&out).expect("json");
        assert_eq!(v["edges"][0]["status"], "skipped_kind");
        assert_eq!(v["edges"][0]["kind"], "image");
    }
}
