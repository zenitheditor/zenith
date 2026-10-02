//! `DocumentPrep` + `PageCompiler` equal the one-page `compile_page` wrapper.
//!
//! Every page of every `examples/*.zen` document, plus cross-page chain,
//! table-flow, and data-binding fixtures, compiles to the same scene JSON and
//! the same diagnostics in the same order through both paths.

mod common;

use std::path::PathBuf;

use common::{Document, compile_page, default_provider, parse};
use zenith_core::{BytesFontProvider, DataContext};
use zenith_scene::{DocumentPrep, PageCompiler};

/// Compile every page plus one out-of-range index both ways and compare.
fn assert_paths_match(name: &str, doc: &Document, data: Option<&DataContext>) {
    let fonts = default_provider();
    let prep = DocumentPrep::new(doc, data, None);
    let compiler = PageCompiler::new(&prep, &fonts);
    let page_count = doc.body.pages.len();
    assert_eq!(compiler.page_count(), page_count, "{name}: page count");
    for page_index in 0..=page_count {
        let expected = compile_page(doc, &fonts, page_index, data);
        let actual = compiler.compile_page(page_index);
        assert_eq!(
            actual.scene.to_json().expect("scene serialises"),
            expected.scene.to_json().expect("scene serialises"),
            "{name}: scene JSON differs on page {page_index}"
        );
        assert_eq!(
            actual.diagnostics, expected.diagnostics,
            "{name}: diagnostics differ on page {page_index}"
        );
    }
}

#[test]
fn page_compiler_matches_wrapper_for_every_example() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../examples");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("examples directory reads")
        .map(|entry| entry.expect("directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "zen"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "examples/*.zen must exist");
    for path in &paths {
        let src = std::fs::read_to_string(path).expect("example reads");
        let doc = parse(&src);
        assert_paths_match(&path.display().to_string(), &doc, None);
    }
}

/// Two-page chain with an unresolvable family, so the chain pre-pass emits
/// diagnostics that only page 0 reports.
const CHAIN_SRC: &str = r##"zenith version=1 {
  project id="proj.pc" name="PC"
  tokens format="zenith-token-v1" {
    token id="color.ink" type="color" value="#111827"
    token id="font.missing" type="fontFamily" value="No Such Family"
    token id="size.body" type="dimension" value=(px)20
  }
  styles {}
  document id="doc.pc" title="PC" {
    page id="page.pc1" w=(px)400 h=(px)200 {
      text id="a1" x=(px)10 y=(px)10 w=(px)200 h=(px)60 chain="art" fill=(token)"color.ink" font-family=(token)"font.missing" font-size=(token)"size.body" {
        span "Alpha bravo charlie delta echo foxtrot golf hotel india juliet kilo lima mike november oscar papa quebec romeo sierra tango"
      }
    }
    page id="page.pc2" w=(px)400 h=(px)200 {
      text id="a2" x=(px)10 y=(px)10 w=(px)200 h=(px)180 chain="art" fill=(token)"color.ink" font-family=(token)"font.missing" font-size=(token)"size.body" {
      }
    }
  }
}
"##;

const FLOW_SRC: &str = r##"zenith version=1 {
  project id="proj.fl" name="FL"
  tokens format="zenith-token-v1" {
    token id="color.ink" type="color" value="#000000"
  }
  styles {}
  document id="doc.fl" title="FL" {
    page id="page.fl1" w=(px)400 h=(px)400 {
      table id="src" flows="t" x=(px)20 y=(px)20 w=(px)360 h=(px)60 header-rows=1 cell-padding=(px)0 gap=(px)0 {
        column
        row { cell { text id="h" x=(px)0 y=(px)0 w=(px)100 h=(px)20 fill=(token)"color.ink" { span "HEAD" } } }
        row { cell { text id="b1" x=(px)0 y=(px)0 w=(px)100 h=(px)20 fill=(token)"color.ink" { span "row-1" } } }
        row { cell { text id="b2" x=(px)0 y=(px)0 w=(px)100 h=(px)20 fill=(token)"color.ink" { span "row-2" } } }
        row { cell { text id="b3" x=(px)0 y=(px)0 w=(px)100 h=(px)20 fill=(token)"color.ink" { span "row-3" } } }
        row { cell { text id="b4" x=(px)0 y=(px)0 w=(px)100 h=(px)20 fill=(token)"color.ink" { span "row-4" } } }
      }
    }
    page id="page.fl2" w=(px)400 h=(px)400 {
      table id="cont" flows="t" x=(px)20 y=(px)20 w=(px)360 h=(px)400 header-rows=1 cell-padding=(px)0 gap=(px)0 {
        column
      }
    }
  }
}
"##;

const DATA_SRC: &str = r##"zenith version=1 {
  project id="proj.dt" name="DT"
  tokens format="zenith-token-v1" {
    token id="color.ink" type="color" value="#000000"
  }
  styles {}
  document id="doc.dt" title="DT" {
    page id="page.dt1" w=(px)300 h=(px)100 {
      text id="t1" x=(px)0 y=(px)0 w=(px)200 h=(px)40 fill=(token)"color.ink" { span "x" data-ref="name" }
    }
    page id="page.dt2" w=(px)300 h=(px)100 {
      text id="t2" x=(px)0 y=(px)0 w=(px)200 h=(px)40 fill=(token)"color.ink" { span "y" data-ref="missing" }
    }
  }
}
"##;

#[test]
fn page_compiler_matches_wrapper_for_cross_page_chain() {
    let doc = parse(CHAIN_SRC);
    let fonts = default_provider();
    let page0 = compile_page(&doc, &fonts, 0, None);
    let page1 = compile_page(&doc, &fonts, 1, None);
    assert!(
        page0.diagnostics.len() > page1.diagnostics.len(),
        "the fixture must emit page-0-only chain diagnostics"
    );
    assert_paths_match("chain", &doc, None);
}

#[test]
fn page_compiler_matches_wrapper_for_table_flow() {
    assert_paths_match("table-flow", &parse(FLOW_SRC), None);
}

#[test]
fn page_compiler_matches_wrapper_with_and_without_data() {
    let doc = parse(DATA_SRC);
    let mut data = DataContext::default();
    data.fields.insert("name".to_owned(), "Ada".to_owned());
    assert_paths_match("data", &doc, Some(&data));
    assert_paths_match("no-data", &doc, None);
}

#[test]
fn page_compiler_matches_wrapper_for_empty_document() {
    let src = r##"zenith version=1 {
  project id="proj.e" name="E"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.e" title="E" {}
}
"##;
    assert_paths_match("empty", &parse(src), None);
}

#[test]
fn page_compiler_is_sync_for_a_sync_provider() {
    fn assert_sync<T: Sync>() {}
    assert_sync::<PageCompiler<'static, BytesFontProvider>>();
    assert_sync::<DocumentPrep<'static>>();
}
