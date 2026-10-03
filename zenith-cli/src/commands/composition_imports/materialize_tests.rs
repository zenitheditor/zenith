use super::*;
use std::fs;

use zenith_core::{KdlAdapter, KdlSource, Node, validate};

fn write(path: &Path, src: &str) {
    fs::write(path, src).expect("write fixture");
}

fn import_doc_with_component(component_id: &str) -> String {
    // r## so token value="#…" does not terminate the raw string early.
    format!(
        r##"zenith version=1 {{
  project id="proj.brand" name="Brand"
  tokens format="zenith-token-v1" {{
    token id="color.brand" type="color" value="#00aa77"
  }}
  styles {{
    style id="style.mark" {{
      fill (token)"color.brand"
    }}
  }}
  document id="doc.brand" title="Brand" {{
    page id="page.brand" w=(px)100 h=(px)100
  }}
  components {{
    component id="{component_id}" {{
      rect id="mark" x=(px)0 y=(px)0 w=(px)20 h=(px)20 fill=(token)"color.brand"
    }}
  }}
}}
"##
    )
}

fn host_with_import(src: &str, page_id: &str) -> String {
    format!(
        r#"zenith version=1 {{
  project id="proj.host" name="Host"
  tokens format="zenith-token-v1" {{}}
  styles {{}}
  imports {{
    import id="brand" kind="zen" src="{src}"
  }}
  document id="doc.host" title="Host" {{
    page id="{page_id}" w=(px)800 h=(px)600
  }}
}}
"#
    )
}

#[test]
fn materialize_component_copies_deps_and_provenance() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(
        &dir.path().join("brand.zen"),
        &import_doc_with_component("logo"),
    );
    let host_path = dir.path().join("host.zen");
    let host_src = host_with_import("brand.zen", "page.1");
    write(&host_path, &host_src);

    let result = run(
        &host_src,
        &host_path,
        "brand#component.logo",
        "page.1",
        (12.0, 34.0),
        None,
    )
    .expect("materialize ok");

    assert_eq!(result.outcome.target_component_id, "import.brand.logo");
    assert_eq!(result.outcome.instance_id, "logo");
    assert_eq!(result.outcome.library_id, "import:brand");
    assert!(result.outcome.warnings.is_empty());

    let doc = KdlAdapter
        .parse(&result.formatted)
        .expect("formatted must parse");
    assert!(doc.components.iter().any(|c| c.id == "import.brand.logo"));
    let comp = doc
        .components
        .iter()
        .find(|c| c.id == "import.brand.logo")
        .expect("component");
    assert!(matches!(comp.children.first(), Some(Node::Rect(r)) if r.id == "mark"));
    assert!(doc.tokens.tokens.iter().any(|t| t.id == "color.brand"));
    assert!(doc.styles.styles.iter().any(|s| s.id == "style.mark"));
    assert!(doc.libraries.iter().any(|l| l.id == "import:brand"));
    let prov = doc
        .provenance
        .iter()
        .find(|p| p.node == "logo")
        .expect("provenance");
    assert_eq!(prov.library, "import:brand");
    assert_eq!(prov.item.as_deref(), Some("component.logo"));
    assert_eq!(prov.linked, Some(false));

    let inst = doc.body.pages[0]
        .children
        .iter()
        .find_map(|n| match n {
            Node::Instance(i) => Some(i),
            _ => None,
        })
        .expect("instance");
    assert_eq!(inst.component.as_deref(), Some("import.brand.logo"));
    assert!(inst.source.is_none());
    assert_eq!(inst.x, Some(px(12.0)));
    assert_eq!(inst.y, Some(px(34.0)));

    let errors: Vec<_> = validate(&doc)
        .diagnostics
        .into_iter()
        .filter(|d| d.is_error())
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn materialize_dedups_component_on_second_call() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(
        &dir.path().join("brand.zen"),
        &import_doc_with_component("logo"),
    );
    let host_path = dir.path().join("host.zen");
    let host_src = host_with_import("brand.zen", "page.1");

    let first = run(
        &host_src,
        &host_path,
        "brand#component.logo",
        "page.1",
        (0.0, 0.0),
        None,
    )
    .expect("first");
    let first_src = String::from_utf8(first.formatted).expect("utf8");
    let second = run(
        &first_src,
        &host_path,
        "brand#component.logo",
        "page.1",
        (10.0, 10.0),
        None,
    )
    .expect("second");

    let doc = KdlAdapter.parse(&second.formatted).expect("parse");
    let comps: Vec<_> = doc
        .components
        .iter()
        .filter(|c| c.id == "import.brand.logo")
        .collect();
    assert_eq!(comps.len(), 1, "component must be deduped");
    let instances: Vec<_> = doc.body.pages[0]
        .children
        .iter()
        .filter(|n| matches!(n, Node::Instance(_)))
        .collect();
    assert_eq!(instances.len(), 2, "second instance still placed");
    assert_eq!(second.outcome.instance_id, "logo.1");
}

#[test]
fn materialize_rewrites_asset_paths_relative_to_host() {
    let dir = tempfile::tempdir().expect("tempdir");
    let brand_dir = dir.path().join("brand");
    fs::create_dir(&brand_dir).expect("mkdir brand");
    let import_src = r#"zenith version=1 {
  project id="proj.brand" name="Brand"
  assets {
    asset id="asset.mark" kind="image" src="assets/mark.png"
  }
  document id="doc.brand" title="Brand" {
    page id="page.brand" w=(px)100 h=(px)100
  }
  components {
    component id="logo" {
      image id="img" asset="asset.mark" x=(px)0 y=(px)0 w=(px)10 h=(px)10
    }
  }
}
"#;
    write(&brand_dir.join("logo.zen"), import_src);
    let host_path = dir.path().join("host.zen");
    let host_src = host_with_import("brand/logo.zen", "page.1");
    write(&host_path, &host_src);

    let result = run(
        &host_src,
        &host_path,
        "brand#component.logo",
        "page.1",
        (0.0, 0.0),
        None,
    )
    .expect("materialize ok");

    let doc = KdlAdapter.parse(&result.formatted).expect("parse");
    let asset = doc
        .assets
        .assets
        .iter()
        .find(|a| a.id == "asset.mark")
        .expect("asset copied");
    assert_eq!(asset.src, "brand/assets/mark.png");
}

#[test]
fn materialize_token_conflict_keeps_host_and_warns() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(
        &dir.path().join("brand.zen"),
        &import_doc_with_component("logo"),
    );
    let host_path = dir.path().join("host.zen");
    let host_src = r##"zenith version=1 {
  project id="proj.host" name="Host"
  tokens format="zenith-token-v1" {
    token id="color.brand" type="color" value="#ffffff"
  }
  styles {}
  imports {
    import id="brand" kind="zen" src="brand.zen"
  }
  document id="doc.host" title="Host" {
    page id="page.1" w=(px)800 h=(px)600
  }
}
"##;
    write(&host_path, host_src);

    let result = run(
        host_src,
        &host_path,
        "brand#component.logo",
        "page.1",
        (0.0, 0.0),
        None,
    )
    .expect("materialize ok");

    assert!(
        result
            .outcome
            .warnings
            .iter()
            .any(|w| w.contains("import.dependency_conflict") && w.contains("color.brand")),
        "warnings: {:?}",
        result.outcome.warnings
    );
    let doc = KdlAdapter.parse(&result.formatted).expect("parse");
    let tok = doc
        .tokens
        .tokens
        .iter()
        .find(|t| t.id == "color.brand")
        .expect("token");
    // Host value must be kept over the import's `#00aa77`.
    assert_eq!(
        tok.value,
        zenith_core::TokenValue::Literal(zenith_core::TokenLiteral::String("#ffffff".to_owned()))
    );
}

#[test]
fn materialize_missing_import_fails() {
    let dir = tempfile::tempdir().expect("tempdir");
    let host_path = dir.path().join("host.zen");
    let host_src = host_with_import("gone.zen", "page.1");
    write(&host_path, &host_src);

    let err = run(
        &host_src,
        &host_path,
        "brand#component.logo",
        "page.1",
        (0.0, 0.0),
        None,
    )
    .expect_err("missing import");
    assert_eq!(err.exit_code, 2);
    assert!(err.message.contains("missing"), "{}", err.message);
}

#[test]
fn materialize_unknown_component_fails() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(
        &dir.path().join("brand.zen"),
        &import_doc_with_component("logo"),
    );
    let host_path = dir.path().join("host.zen");
    let host_src = host_with_import("brand.zen", "page.1");
    write(&host_path, &host_src);

    let err = run(
        &host_src,
        &host_path,
        "brand#component.nope",
        "page.1",
        (0.0, 0.0),
        None,
    )
    .expect_err("unknown component");
    assert_eq!(err.exit_code, 2);
    assert!(err.message.contains("unknown component"), "{}", err.message);
    assert!(err.message.contains("logo"), "{}", err.message);
}

#[test]
fn materialize_missing_page_fails() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(
        &dir.path().join("brand.zen"),
        &import_doc_with_component("logo"),
    );
    let host_path = dir.path().join("host.zen");
    let host_src = host_with_import("brand.zen", "page.1");
    write(&host_path, &host_src);

    let err = run(
        &host_src,
        &host_path,
        "brand#component.logo",
        "page.nope",
        (0.0, 0.0),
        None,
    )
    .expect_err("missing page");
    assert_eq!(err.exit_code, 2);
    assert!(err.message.contains("page 'page.nope'"), "{}", err.message);
}

#[test]
fn materialize_page_target_rejected() {
    let dir = tempfile::tempdir().expect("tempdir");
    let host_path = dir.path().join("host.zen");
    let host_src = host_with_import("brand.zen", "page.1");
    let err = run(
        &host_src,
        &host_path,
        "brand#page.cover",
        "page.1",
        (0.0, 0.0),
        None,
    )
    .expect_err("page target");
    assert_eq!(err.exit_code, 2);
    assert!(err.message.contains("page targets"), "{}", err.message);
}

#[test]
fn materialize_hash_mismatch_fails() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(
        &dir.path().join("brand.zen"),
        &import_doc_with_component("logo"),
    );
    let host_path = dir.path().join("host.zen");
    let host_src = r#"zenith version=1 {
  project id="proj.host" name="Host"
  tokens format="zenith-token-v1" {}
  styles {}
  imports {
    import id="brand" kind="zen" src="brand.zen" sha256="0000000000000000000000000000000000000000000000000000000000000000"
  }
  document id="doc.host" title="Host" {
    page id="page.1" w=(px)800 h=(px)600
  }
}
"#;
    write(&host_path, host_src);

    let err = run(
        host_src,
        &host_path,
        "brand#component.logo",
        "page.1",
        (0.0, 0.0),
        None,
    )
    .expect_err("hash mismatch");
    assert_eq!(err.exit_code, 2);
    assert!(err.message.contains("sha256 mismatch"), "{}", err.message);
}

#[test]
fn materialize_undeclared_import_fails() {
    let dir = tempfile::tempdir().expect("tempdir");
    let host_path = dir.path().join("host.zen");
    let host_src = r#"zenith version=1 {
  project id="proj.host" name="Host"
  document id="doc.host" title="Host" {
    page id="page.1" w=(px)800 h=(px)600
  }
}
"#;
    let err = run(
        host_src,
        &host_path,
        "brand#component.logo",
        "page.1",
        (0.0, 0.0),
        None,
    )
    .expect_err("undeclared");
    assert_eq!(err.exit_code, 2);
    assert!(err.message.contains("not declared"), "{}", err.message);
}

#[test]
fn materialize_id_override_used() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(
        &dir.path().join("brand.zen"),
        &import_doc_with_component("logo"),
    );
    let host_path = dir.path().join("host.zen");
    let host_src = host_with_import("brand.zen", "page.1");
    write(&host_path, &host_src);

    let result = run(
        &host_src,
        &host_path,
        "brand#component.logo",
        "page.1",
        (0.0, 0.0),
        Some("hero.logo"),
    )
    .expect("ok");
    assert_eq!(result.outcome.instance_id, "hero.logo");
}

#[test]
fn materialize_json_schema() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(
        &dir.path().join("brand.zen"),
        &import_doc_with_component("logo"),
    );
    let host_path = dir.path().join("host.zen");
    let host_src = host_with_import("brand.zen", "page.1");
    write(&host_path, &host_src);

    let result = run(
        &host_src,
        &host_path,
        "brand#component.logo",
        "page.1",
        (0.0, 0.0),
        None,
    )
    .expect("ok");
    let json = format_json(&host_path, &result, true);
    let v: serde_json::Value = serde_json::from_str(&json).expect("json");
    assert_eq!(v["schema"], SCHEMA);
    assert_eq!(v["import_id"], "brand");
    assert_eq!(v["component_id"], "logo");
    assert_eq!(v["target_component_id"], "import.brand.logo");
    assert!(v["source"].as_str().is_some());
}

#[test]
fn materialize_records_library_hash_when_known() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(
        &dir.path().join("brand.zen"),
        &import_doc_with_component("logo"),
    );
    let host_path = dir.path().join("host.zen");
    let host_src = host_with_import("brand.zen", "page.1");
    write(&host_path, &host_src);

    let result = run(
        &host_src,
        &host_path,
        "brand#component.logo",
        "page.1",
        (0.0, 0.0),
        None,
    )
    .expect("ok");
    let doc = KdlAdapter.parse(&result.formatted).expect("parse");
    let lib = doc
        .libraries
        .iter()
        .find(|l| l.id == "import:brand")
        .expect("library");
    assert!(
        lib.hash.as_ref().is_some_and(|h| h.len() == 64),
        "expected sha256 hex, got {:?}",
        lib.hash
    );
}
