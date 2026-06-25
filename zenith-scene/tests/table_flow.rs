//! Integration tests for multi-page `table` flow.
//!
//! Covers: a `table flows="t"` splitting its body across two member boxes while
//! repeating the header on both pages, and a rowspan body group being pushed
//! WHOLE to the continuation page rather than split across the boundary.

mod common;

use common::{SceneCommand, compile_page, default_provider, parse};

/// Count the `DrawGlyphRun` text strings on a compiled page, by collecting the
/// first span text of each run is not exposed; instead count runs and, where the
/// span text matters, count `DrawGlyphRun` commands (one per shaped run).
fn glyph_run_count(result: &common::CompileResult) -> usize {
    result
        .scene
        .commands
        .iter()
        .filter(|c| matches!(c, SceneCommand::DrawGlyphRun { .. }))
        .count()
}

/// A 2-page document: each page hosts a `table flows="t"` box sharing the same
/// single-column layout + header-rows=1. The SOURCE (page 1) carries the header
/// row plus 6 body rows; the page-1 box is short enough that only some body rows
/// fit, so the rest flow onto the page-2 continuation box (which declares the
/// same flow id with EMPTY rows). The header repeats on both pages.
fn flow_src() -> &'static str {
    r##"zenith version=1 {
  project id="proj.fl" name="FL"
  tokens format="zenith-token-v1" {
    token id="color.ink" type="color" value="#000000"
  }
  styles {}
  document id="doc.fl" title="FL" {
    page id="page.fl1" w=(px)400 h=(px)400 {
      table id="src" flows="t" x=(px)20 y=(px)20 w=(px)360 h=(px)120 header-rows=1 cell-padding=(px)0 gap=(px)0 {
        column
        row { cell { text id="h" x=(px)0 y=(px)0 fill=(token)"color.ink" { span "HEAD" } } }
        row { cell { text id="b1" x=(px)0 y=(px)0 fill=(token)"color.ink" { span "row-1" } } }
        row { cell { text id="b2" x=(px)0 y=(px)0 fill=(token)"color.ink" { span "row-2" } } }
        row { cell { text id="b3" x=(px)0 y=(px)0 fill=(token)"color.ink" { span "row-3" } } }
        row { cell { text id="b4" x=(px)0 y=(px)0 fill=(token)"color.ink" { span "row-4" } } }
        row { cell { text id="b5" x=(px)0 y=(px)0 fill=(token)"color.ink" { span "row-5" } } }
        row { cell { text id="b6" x=(px)0 y=(px)0 fill=(token)"color.ink" { span "row-6" } } }
      }
    }
    page id="page.fl2" w=(px)400 h=(px)400 {
      table id="cont" flows="t" x=(px)20 y=(px)20 w=(px)360 h=(px)400 header-rows=1 cell-padding=(px)0 gap=(px)0 {
        column
      }
    }
  }
}
"##
}

/// The flow splits the body across the two member boxes: page 1 shows the header
/// plus a leading slice; page 2 shows the header AGAIN plus the remaining body
/// rows. Every body row appears exactly once, and the header appears on BOTH
/// pages. We count glyph runs (one per non-empty cell text) per page: the total
/// across both pages = 6 body rows + 2 header repeats = 8 runs.
#[test]
fn flow_splits_body_and_repeats_header_across_pages() {
    let doc = parse(flow_src());
    let fonts = default_provider();
    let p1 = compile_page(&doc, &fonts, 0, None);
    let p2 = compile_page(&doc, &fonts, 1, None);

    let c1 = glyph_run_count(&p1);
    let c2 = glyph_run_count(&p2);

    // Both pages render the repeated header → each page has ≥1 run.
    assert!(
        c1 >= 1,
        "page 1 must render header + a body slice; got {c1}"
    );
    assert!(
        c2 >= 1,
        "page 2 must render header + remaining body; got {c2}"
    );
    // Header repeats on both pages: 6 distinct body rows + 2 header copies.
    assert_eq!(
        c1 + c2,
        8,
        "total runs = 6 body + 2 header copies; page1={c1} page2={c2}"
    );
    // The split is real: page 1 did NOT take all 7 source rows (would be 7 runs).
    assert!(
        c1 < 7,
        "page 1 must not fit the whole source table; got {c1} runs"
    );
    // Page 2 must carry more than just its header (it received overflow body).
    assert!(c2 >= 2, "page 2 must carry overflow body rows; got {c2}");
}

/// A rowspan body group that would straddle the page-1/page-2 boundary is pushed
/// WHOLE to the continuation: the spanning cell renders on page 2, not split. We
/// place a tall rowspan=2 cell late in the body so it cannot fit page 1's
/// remaining capacity and must move entirely to page 2.
fn flow_rowspan_src() -> &'static str {
    r##"zenith version=1 {
  project id="proj.fr" name="FR"
  tokens format="zenith-token-v1" {
    token id="color.ink" type="color" value="#000000"
  }
  styles {}
  document id="doc.fr" title="FR" {
    page id="page.fr1" w=(px)400 h=(px)400 {
      table id="rsrc" flows="t" x=(px)20 y=(px)20 w=(px)360 h=(px)55 header-rows=1 cell-padding=(px)0 gap=(px)0 {
        column
        column
        row { cell { text id="rh1" x=(px)0 y=(px)0 fill=(token)"color.ink" { span "H1" } }; cell { text id="rh2" x=(px)0 y=(px)0 fill=(token)"color.ink" { span "H2" } } }
        row { cell { text id="ra1" x=(px)0 y=(px)0 fill=(token)"color.ink" { span "A1" } }; cell { text id="ra2" x=(px)0 y=(px)0 fill=(token)"color.ink" { span "A2" } } }
        row { cell rowspan=2 { text id="span" x=(px)0 y=(px)0 fill=(token)"color.ink" { span "SPAN" } }; cell { text id="rb2" x=(px)0 y=(px)0 fill=(token)"color.ink" { span "B2" } } }
        row { cell { text id="rc2" x=(px)0 y=(px)0 fill=(token)"color.ink" { span "C2" } } }
      }
    }
    page id="page.fr2" w=(px)400 h=(px)400 {
      table id="rcont" flows="t" x=(px)20 y=(px)20 w=(px)360 h=(px)400 header-rows=1 cell-padding=(px)0 gap=(px)0 {
        column
        column
      }
    }
  }
}
"##
}

#[test]
fn flow_rowspan_group_not_split_across_pages() {
    let doc = parse(flow_rowspan_src());
    let fonts = default_provider();
    let p1 = compile_page(&doc, &fonts, 0, None);
    let p2 = compile_page(&doc, &fonts, 1, None);

    // The rowspan group (rows containing SPAN/B2 + C2) must land WHOLE on page 2.
    // Page 1 fits the header (H1,H2) + the first body row (A1,A2): 4 runs, and
    // must NOT contain the spanning group. Total runs = header(2)×2 + body cells.
    let c1 = glyph_run_count(&p1);
    let c2 = glyph_run_count(&p2);
    // Page 1 should carry the header + first body row only (no SPAN yet).
    assert!(
        (2..=4).contains(&c1),
        "page 1 = header + first body row, no rowspan group; got {c1}"
    );
    // Page 2 carries the repeated header plus the rowspan group (SPAN,B2,C2).
    assert!(
        c2 >= 4,
        "page 2 must carry the repeated header + whole rowspan group; got {c2}"
    );
}
