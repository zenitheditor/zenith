use std::path::Path;

use zenith_core::{Document, KdlAdapter, KdlSource};

use super::load_import_graph;
use super::loaded::ImportEdgeStatus;
use crate::MemFs;

/// The project directory every test resolves imports against.
const DIR: &str = "proj";

const EMPTY_DOC: &str = r#"zenith version=1 {
  project id="proj.empty" name="Empty"
  document id="doc.empty" title="Empty" {
    page id="page.empty" w=(px)100 h=(px)100
  }
}
"#;

fn parse(src: &str) -> Document {
    KdlAdapter
        .parse(src.as_bytes())
        .expect("test document must parse")
}

fn root_with_import(src: &str, extra: &str) -> Document {
    parse(&format!(
        r#"zenith version=1 {{
  project id="proj.root" name="Root"
  imports {{
    import id="child" kind="zen" src="{src}"{extra}
  }}
  document id="doc.root" title="Root" {{
    page id="page.root" w=(px)100 h=(px)100
  }}
}}
"#
    ))
}

fn root_with_imports(imports: &str) -> Document {
    parse(&format!(
        r#"zenith version=1 {{
  project id="proj.root" name="Root"
  imports {{
{imports}
  }}
  document id="doc.root" title="Root" {{
    page id="page.root" w=(px)100 h=(px)100
  }}
}}
"#
    ))
}

fn root_with_import_and_body(src: &str, body: &str) -> Document {
    parse(&format!(
        r#"zenith version=1 {{
  project id="proj.root" name="Root"
  imports {{
    import id="child" kind="zen" src="{src}"
  }}
  document id="doc.root" title="Root" {{
{body}
  }}
}}
"#
    ))
}

fn imported_with_component_and_page(component_id: &str, page_id: &str, w: f64, h: f64) -> String {
    format!(
        r#"zenith version=1 {{
  project id="proj.child" name="Child"
  document id="doc.child" title="Child" {{
    page id="{page_id}" w=(px){w} h=(px){h}
  }}
  components {{
    component id="{component_id}" {{
      rect id="mark" x=(px)0 y=(px)0 w=(px)10 h=(px)10
    }}
  }}
}}
"#
    )
}

/// Load the import graph of `root` from `fs` with the project in [`DIR`].
fn load(fs: &MemFs, root: &Document) -> super::LoadedImportGraph {
    load_import_graph(fs, root, Some(Path::new(DIR)))
}

/// `fs` with `contents` stored at `DIR/name`.
fn with_file(fs: MemFs, name: &str, contents: impl Into<Vec<u8>>) -> MemFs {
    fs.with(Path::new(DIR).join(name), contents)
}

#[test]
fn load_import_graph_resolves_relative_imports() {
    let fs = with_file(MemFs::new(), "modules/child.zen", EMPTY_DOC);
    let root = root_with_import("modules/child.zen", "");

    let graph = load(&fs, &root);

    assert!(graph.diagnostics.is_empty(), "{:?}", graph.diagnostics);
    assert_eq!(graph.edges().len(), 1);
    let edge = &graph.edges()[0];
    assert_eq!(edge.id, "child");
    assert_eq!(edge.status, ImportEdgeStatus::Ok);
    assert_eq!(edge.depth, 0);
    assert!(edge.importer.is_none());
    assert_eq!(
        edge.resolved_path.as_deref(),
        Some(Path::new("proj/modules/child.zen"))
    );
    assert!(edge.sha256_actual.is_some());
}

#[test]
fn load_import_graph_reports_missing_import() {
    let root = root_with_import("missing.zen", "");

    let graph = load(&MemFs::new(), &root);
    assert_eq!(graph.edges().len(), 1);
    assert_eq!(graph.edges()[0].status, ImportEdgeStatus::Missing);
    assert!(graph.edges()[0].resolved_path.is_some());

    let diagnostics = graph.into_diagnostics();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "import.missing");
    assert_eq!(diagnostics[0].subject_id.as_deref(), Some("child"));
    assert!(
        diagnostics[0]
            .message
            .contains("'proj/missing.zen': no such file in the in-memory file set"),
        "{}",
        diagnostics[0].message
    );
}

#[test]
fn load_import_graph_keeps_same_file_import_aliases() {
    let fs = with_file(MemFs::new(), "shared.zen", EMPTY_DOC);
    let root = root_with_imports(
        r#"    import id="first" kind="zen" src="shared.zen"
    import id="second" kind="zen" src="./shared.zen""#,
    );

    let graph = load(&fs, &root);

    assert!(graph.diagnostics.is_empty(), "{:?}", graph.diagnostics);
    assert!(graph.documents.contains_key("first"));
    assert!(graph.documents.contains_key("second"));
}

#[test]
fn load_import_graph_reports_parse_error() {
    let fs = with_file(MemFs::new(), "bad.zen", "not zenith");
    let root = root_with_import("bad.zen", "");

    let graph = load(&fs, &root);
    assert_eq!(graph.edges()[0].status, ImportEdgeStatus::ParseError);
    assert!(graph.edges()[0].sha256_actual.is_some());

    let diagnostics = graph.into_diagnostics();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "import.parse_error");
}

#[test]
fn load_import_graph_reports_hash_mismatch() {
    let fs = with_file(MemFs::new(), "child.zen", EMPTY_DOC);
    let root = root_with_import("child.zen", r#" sha256="0000""#);

    let graph = load(&fs, &root);
    assert_eq!(graph.edges()[0].status, ImportEdgeStatus::HashMismatch);
    assert_eq!(graph.edges()[0].sha256_declared.as_deref(), Some("0000"));
    assert!(graph.edges()[0].sha256_actual.is_some());
    // Hash mismatch still loads the document for render soft-fail paths.
    assert!(graph.documents.contains_key("child"));

    let diagnostics = graph.into_diagnostics();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "import.hash_mismatch");
}

#[test]
fn load_import_graph_reports_hash_mismatch_for_cached_alias() {
    let fs = with_file(MemFs::new(), "shared.zen", EMPTY_DOC);
    let root = root_with_imports(
        r#"    import id="first" kind="zen" src="shared.zen"
    import id="second" kind="zen" src="./shared.zen" sha256="0000""#,
    );

    let graph = load(&fs, &root);
    assert_eq!(graph.edges().len(), 2);
    assert_eq!(graph.edges()[0].status, ImportEdgeStatus::Ok);
    assert_eq!(graph.edges()[1].status, ImportEdgeStatus::HashMismatch);
    assert_eq!(graph.edges()[1].id, "second");

    let diagnostics = graph.into_diagnostics();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "import.hash_mismatch");
    assert_eq!(diagnostics[0].subject_id.as_deref(), Some("second"));
}

#[test]
fn load_import_graph_reports_cycles() {
    let fs = with_file(
        MemFs::new(),
        "a.zen",
        r#"zenith version=1 {
  project id="proj.a" name="A"
  imports {
    import id="b" kind="zen" src="b.zen"
  }
  document id="doc.a" title="A" {
    page id="page.a" w=(px)100 h=(px)100
  }
}
"#,
    );
    let fs = with_file(
        fs,
        "b.zen",
        r#"zenith version=1 {
  project id="proj.b" name="B"
  imports {
    import id="a" kind="zen" src="a.zen"
  }
  document id="doc.b" title="B" {
    page id="page.b" w=(px)100 h=(px)100
  }
}
"#,
    );
    let root = root_with_import("a.zen", "");

    let graph = load(&fs, &root);
    let statuses: Vec<_> = graph.edges().iter().map(|e| e.status).collect();
    assert!(
        statuses.contains(&ImportEdgeStatus::Cycle),
        "edges: {:?}",
        graph.edges()
    );
    let cycle = graph
        .edges()
        .iter()
        .find(|e| e.status == ImportEdgeStatus::Cycle)
        .expect("cycle edge");
    assert_eq!(cycle.depth, 2);
    assert!(cycle.importer.is_some());

    let diagnostics = graph.into_diagnostics();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "import.cycle");
    assert_eq!(
        diagnostics[0].message,
        "import 'a' forms a cycle: proj/a.zen -> proj/b.zen -> proj/a.zen"
    );
}

#[test]
fn load_import_graph_records_skipped_kind_and_unresolvable() {
    let root = root_with_imports(
        r#"    import id="pic" kind="image" src="x.png"
    import id="child" kind="zen" src="child.zen""#,
    );

    let graph = load_import_graph(&MemFs::new(), &root, None);
    assert_eq!(graph.edges().len(), 2);
    assert_eq!(graph.edges()[0].status, ImportEdgeStatus::SkippedKind);
    assert_eq!(graph.edges()[0].id, "pic");
    assert_eq!(graph.edges()[1].status, ImportEdgeStatus::Unresolvable);
    assert_eq!(graph.edges()[1].id, "child");
    assert!(graph.edges()[1].resolved_path.is_none());
}

#[test]
fn load_import_graph_records_nested_importer_and_depth() {
    let fs = with_file(
        MemFs::new(),
        "mid.zen",
        r#"zenith version=1 {
  project id="proj.mid" name="Mid"
  imports {
    import id="leaf" kind="zen" src="leaf.zen"
  }
  document id="doc.mid" title="Mid" {
    page id="page.mid" w=(px)100 h=(px)100
  }
}
"#,
    );
    let fs = with_file(fs, "leaf.zen", EMPTY_DOC);
    let root = root_with_import("mid.zen", "");

    let graph = load(&fs, &root);
    assert!(graph.diagnostics.is_empty(), "{:?}", graph.diagnostics);
    assert_eq!(graph.edges().len(), 2);
    assert_eq!(graph.edges()[0].id, "child");
    assert_eq!(graph.edges()[0].depth, 0);
    assert!(graph.edges()[0].importer.is_none());
    assert_eq!(graph.edges()[1].id, "leaf");
    assert_eq!(graph.edges()[1].depth, 1);
    assert!(
        graph.edges()[1]
            .importer
            .as_ref()
            .is_some_and(|p| p.ends_with("mid.zen")),
        "importer: {:?}",
        graph.edges()[1].importer
    );
}

/// A file set holding `child.zen`: component `component.card` and a `cover`
/// page of `w` × `h`.
fn child_fixture(w: f64, h: f64) -> MemFs {
    with_file(
        MemFs::new(),
        "child.zen",
        imported_with_component_and_page("component.card", "cover", w, h),
    )
}

#[test]
fn load_import_graph_reports_unknown_component_target() {
    let fs = child_fixture(100.0, 100.0);
    let root = root_with_import_and_body(
        "child.zen",
        r#"    page id="page.root" w=(px)100 h=(px)100 {
      instance id="inst.missing" source="child#component.missing" x=(px)0 y=(px)0
    }"#,
    );

    let diagnostics = load(&fs, &root).into_diagnostics();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "import.unknown_reference");
    assert_eq!(diagnostics[0].subject_id.as_deref(), Some("inst.missing"));
}

#[test]
fn load_import_graph_reports_unsupported_instance_page_target() {
    let fs = child_fixture(100.0, 100.0);
    let root = root_with_import_and_body(
        "child.zen",
        r#"    page id="page.root" w=(px)100 h=(px)100 {
      instance id="inst.page" source="child#page.cover" x=(px)0 y=(px)0
    }"#,
    );

    let diagnostics = load(&fs, &root).into_diagnostics();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "import.unsupported_target");
    assert_eq!(diagnostics[0].subject_id.as_deref(), Some("inst.page"));
}

#[test]
fn load_import_graph_reports_unknown_page_target() {
    let fs = child_fixture(100.0, 100.0);
    let root = root_with_import_and_body(
        "child.zen",
        r#"    page id="page.root" source="child#page.missing" w=(px)100 h=(px)100"#,
    );

    let diagnostics = load(&fs, &root).into_diagnostics();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "import.unknown_reference");
    assert_eq!(diagnostics[0].subject_id.as_deref(), Some("page.root"));
}

#[test]
fn load_import_graph_reports_expanded_id_collision() {
    let fs = child_fixture(100.0, 100.0);
    // The host authors a node whose id equals what the instance expansion
    // (`<instance-id>/<local-id>`) would produce: `card/mark`.
    let root = root_with_import_and_body(
        "child.zen",
        r#"    page id="page.root" w=(px)100 h=(px)100 {
      rect id="card/mark" x=(px)0 y=(px)0 w=(px)10 h=(px)10
      instance id="card" source="child#component.component.card" x=(px)0 y=(px)0
    }"#,
    );

    let diagnostics = load(&fs, &root).into_diagnostics();

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "import.id_collision");
    assert_eq!(diagnostics[0].subject_id.as_deref(), Some("card"));
}

#[test]
fn load_import_graph_reports_page_size_mismatch() {
    let fs = child_fixture(200.0, 100.0);
    let root = root_with_import_and_body(
        "child.zen",
        r#"    page id="page.root" source="child#page.cover" w=(px)100 h=(px)100"#,
    );

    let diagnostics = load(&fs, &root).into_diagnostics();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "import.page_size_mismatch");
    assert_eq!(diagnostics[0].subject_id.as_deref(), Some("page.root"));
}
