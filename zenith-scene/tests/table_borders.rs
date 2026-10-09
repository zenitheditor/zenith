//! Integration tests for `table` border-collapse behavior.
//!
//! Covers: `border-collapse="collapse"` deduplicating shared interior edges
//! (fewer StrokeLines than `separate`), and the tie-break rule that an explicit
//! cell border wins over the inherited table default on a shared edge.

mod common;

use common::parse;
use zenith_core::default_provider;
use zenith_scene::compile;
use zenith_scene::ir::SceneCommand;

// ── border-collapse="collapse" tests ─────────────────────────────────────────

/// A 2×2 table source used by the collapse tests. Two explicit columns of 100px
/// each, gap=0, pad=0, so adjacent cell edges are coincident.
fn collapse_2x2_src() -> &'static str {
    r##"zenith version=1 {
  project id="proj.col" name="COL"
  tokens format="zenith-token-v1" {
    token id="color.border" type="color" value="#aaaaaa"
    token id="color.bg"     type="color" value="#ffffff"
  }
  styles {}
  document id="doc.col" title="COL" {
    page id="page.col" w=(px)400 h=(px)300 {
      table id="tc" x=(px)0 y=(px)0 w=(px)200 h=(px)200 border=(token)"color.border" border-width=(px)1 fill=(token)"color.bg" cell-padding=(px)0 gap=(px)0 border-collapse="collapse" {
        column width=(px)100
        column width=(px)100
        row {
          cell { text id="r0c0" x=(px)0 y=(px)0 { span "A" } }
          cell { text id="r0c1" x=(px)0 y=(px)0 { span "B" } }
        }
        row {
          cell { text id="r1c0" x=(px)0 y=(px)0 { span "C" } }
          cell { text id="r1c1" x=(px)0 y=(px)0 { span "D" } }
        }
      }
    }
  }
}
"##
}

/// The same table but in `separate` mode (the default).
fn separate_2x2_src() -> &'static str {
    r##"zenith version=1 {
  project id="proj.sep" name="SEP"
  tokens format="zenith-token-v1" {
    token id="color.border" type="color" value="#aaaaaa"
    token id="color.bg"     type="color" value="#ffffff"
  }
  styles {}
  document id="doc.sep" title="SEP" {
    page id="page.sep" w=(px)400 h=(px)300 {
      table id="ts" x=(px)0 y=(px)0 w=(px)200 h=(px)200 border=(token)"color.border" border-width=(px)1 fill=(token)"color.bg" cell-padding=(px)0 gap=(px)0 {
        column width=(px)100
        column width=(px)100
        row {
          cell { text id="r0c0" x=(px)0 y=(px)0 { span "A" } }
          cell { text id="r0c1" x=(px)0 y=(px)0 { span "B" } }
        }
        row {
          cell { text id="r1c0" x=(px)0 y=(px)0 { span "C" } }
          cell { text id="r1c1" x=(px)0 y=(px)0 { span "D" } }
        }
      }
    }
  }
}
"##
}

/// `border-collapse="collapse"` on a 2×2 table emits FEWER `StrokeLine`s than
/// `separate` mode because the shared interior vertical and horizontal edges are
/// deduplicated. In separate mode 4 cells × 4 edges = 16; in collapse mode the
/// same table has 6 unique edges (4 perimeter + 1 interior vertical + 1 interior
/// horizontal), so collapse_count < separate_count.
#[test]
fn collapse_emits_fewer_stroke_lines_than_separate() {
    let col_result = compile(&parse(collapse_2x2_src()), &default_provider());
    let sep_result = compile(&parse(separate_2x2_src()), &default_provider());

    let col_strokes = col_result
        .scene
        .commands
        .iter()
        .filter(|c| matches!(c, SceneCommand::StrokeLine { .. }))
        .count();
    let sep_strokes = sep_result
        .scene
        .commands
        .iter()
        .filter(|c| matches!(c, SceneCommand::StrokeLine { .. }))
        .count();

    // Separate: 4 cells × 4 edges = 16.
    assert_eq!(
        sep_strokes, 16,
        "separate mode must emit 4 edges per cell (4×4=16); got {sep_strokes}"
    );
    // Collapse dedups the 4 SHARED interior segments (the x=interior vertical seam
    // and y=interior horizontal seam, each split per row/col and shared by two
    // cells). 16 − 4 deduplicated = 12. (Collapse removes doubled edges; it does
    // NOT merge collinear segments into single grid lines.)
    assert_eq!(
        col_strokes, 12,
        "collapse mode must dedup the 4 shared interior segments (16→12); got {col_strokes}"
    );
    assert!(
        col_strokes < sep_strokes,
        "collapse ({col_strokes}) must be strictly fewer than separate ({sep_strokes})"
    );
}

/// When one cell in a collapse table has an OWN explicit `border` color that
/// differs from the table-level default, the shared edge between that cell and
/// its neighbour must take the explicit cell color (tie-break rule: explicit wins
/// over inherited).
#[test]
fn collapse_explicit_cell_border_wins_on_shared_edge() {
    // A 1×2 table: left cell has a red explicit border; right cell inherits the
    // table default (#aaaaaa grey). The shared right edge of the left cell (= the
    // left edge of the right cell) must be red, not grey.
    let src = r##"zenith version=1 {
  project id="proj.tb" name="TB"
  tokens format="zenith-token-v1" {
    token id="color.grey" type="color" value="#aaaaaa"
    token id="color.red"  type="color" value="#ff0000"
    token id="color.bg"   type="color" value="#ffffff"
  }
  styles {}
  document id="doc.tb" title="TB" {
    page id="page.tb" w=(px)400 h=(px)200 {
      table id="tt" x=(px)0 y=(px)0 w=(px)200 h=(px)100 border=(token)"color.grey" border-width=(px)1 fill=(token)"color.bg" cell-padding=(px)0 gap=(px)0 border-collapse="collapse" {
        column width=(px)100
        column width=(px)100
        row {
          cell border=(token)"color.red" {
            text id="lc" x=(px)0 y=(px)0 { span "Left" }
          }
          cell {
            text id="rc" x=(px)0 y=(px)0 { span "Right" }
          }
        }
      }
    }
  }
}
"##;
    let result = compile(&parse(src), &default_provider());

    // The shared vertical interior edge sits at x=100 (left cell right edge =
    // right cell left edge). Its y-extent is the content-based row height (the
    // table does not stretch to its declared h), so match on x only — x=0 and
    // x=200 are the perimeter verticals, leaving x=100 as the unique interior one.
    let shared_edge_color = result.scene.commands.iter().find_map(|c| match c {
        SceneCommand::StrokeLine { x1, x2, color, .. }
            if (x1 - 100.0).abs() < 0.1 && (x2 - 100.0).abs() < 0.1 =>
        {
            Some(*color)
        }
        _ => None,
    });

    let color = shared_edge_color.expect(
        "a StrokeLine at x=100 (the shared vertical interior edge) must exist in collapse output",
    );
    // The explicit cell border is red (#ff0000 → r=255, g=0, b=0).
    assert_eq!(
        color.r, 255,
        "shared edge must be red (explicit cell border wins); got r={} g={} b={}",
        color.r, color.g, color.b
    );
    assert_eq!(
        color.g, 0,
        "shared edge must be red; got r={} g={} b={}",
        color.r, color.g, color.b
    );
    assert_eq!(
        color.b, 0,
        "shared edge must be red; got r={} g={} b={}",
        color.r, color.g, color.b
    );
}
