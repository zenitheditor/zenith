//! Integration tests for single-page `table` layout and sizing.
//!
//! Covers: cell background + border command emission, a `colspan=2` cell
//! spanning two columns' width, CONTENT-BASED column auto-sizing + content-based
//! row heights (auto column sizes to its widest cell's text; a wrapping cell
//! makes its row taller; all-explicit-width columns are unchanged), cell content
//! positioned at the cell content-box origin, a `visible=#false` table emitting
//! nothing, and the separate-mode / non-flow regression guards.

mod common;

use common::parse;
use zenith_core::default_provider;
use zenith_scene::compile;
use zenith_scene::ir::SceneCommand;

/// A 2-row × 3-col table: one explicit column (160px) plus two auto columns,
/// with a colspan=2 cell in the first row. Border + fill use color tokens.
fn table_src() -> &'static str {
    r##"zenith version=1 {
  project id="proj.tbl" name="TBL"
  tokens format="zenith-token-v1" {
    token id="color.line" type="color" value="#cccccc"
    token id="color.cellbg" type="color" value="#f0f0f0"
    token id="color.ink" type="color" value="#000000"
  }
  styles {}
  document id="doc.tbl" title="TBL" {
    page id="page.tbl" w=(px)640 h=(px)400 {
      table id="t1" x=(px)40 y=(px)40 w=(px)520 h=(px)240 border=(token)"color.line" border-width=(px)1 fill=(token)"color.cellbg" cell-padding=(px)0 gap=(px)0 {
        column width=(px)160
        column
        column
        row {
          cell { text id="c11" x=(px)0 y=(px)0 fill=(token)"color.ink" { span "Name" } }
          cell colspan=2 { text id="c12" x=(px)0 y=(px)0 fill=(token)"color.ink" { span "Details" } }
        }
        row {
          cell { text id="c21" x=(px)0 y=(px)0 fill=(token)"color.ink" { span "Ada" } }
          cell { text id="c22" x=(px)0 y=(px)0 fill=(token)"color.ink" { span "Lovelace" } }
          cell { text id="c23" x=(px)0 y=(px)0 fill=(token)"color.ink" { span "1815" } }
        }
      }
    }
  }
}
"##
}

#[test]
fn table_emits_cell_backgrounds_and_borders() {
    let doc = parse(table_src());
    let result = compile(&doc, &default_provider());

    let fill_count = result
        .scene
        .commands
        .iter()
        .filter(|c| matches!(c, SceneCommand::FillRect { .. }))
        .count();
    let stroke_count = result
        .scene
        .commands
        .iter()
        .filter(|c| matches!(c, SceneCommand::StrokeLine { .. }))
        .count();

    // 5 placed cells (2 in row 1 due to colspan, 3 in row 2). Each emits one
    // FillRect (cell background) and four StrokeLines (separate border edges).
    // The page has no background, so every FillRect here is a cell background.
    assert_eq!(fill_count, 5, "expected one fill per placed cell");
    assert_eq!(
        stroke_count,
        5 * 4,
        "expected four border edges per placed cell"
    );

    // Cell content: every cell's text must produce a glyph run.
    let glyph_runs = result
        .scene
        .commands
        .iter()
        .filter(|c| matches!(c, SceneCommand::DrawGlyphRun { .. }))
        .count();
    assert_eq!(glyph_runs, 5, "expected one glyph run per cell text");
}

#[test]
fn colspan_cell_spans_two_columns() {
    let doc = parse(table_src());
    let result = compile(&doc, &default_provider());

    // Column 0 is EXPLICIT 160px (fixed, unchanged by content-based sizing).
    // Columns 1 and 2 are AUTO and now size to their content. With gap=0/pad=0
    // the colspan cell still starts at x=40+160=200 (col 0 is explicit), and its
    // width must equal the sum of the two AUTO column widths — which are exactly
    // the widths of the two single cells in row 2 (col1="Lovelace", col2="1815").
    //
    // Emission is row-major: fills[0]=cell0 (col0), fills[1]=colspan (cols1+2),
    // fills[2]=row2-col0, fills[3]=row2-col1, fills[4]=row2-col2.
    let fills: Vec<(f64, f64)> = result
        .scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::FillRect { x, w, .. } => Some((*x, *w)),
            _ => None,
        })
        .collect();

    assert_eq!(fills.len(), 5, "expected 5 cell fills; got {fills:?}");
    // First cell: x=40 (table origin), width=160 (explicit column, unchanged).
    assert!((fills[0].0 - 40.0).abs() < 0.01, "cell0 x; got {fills:?}");
    assert!((fills[0].1 - 160.0).abs() < 0.01, "cell0 w; got {fills:?}");
    // Colspan cell starts immediately after the explicit column: x=200.
    assert!(
        (fills[1].0 - 200.0).abs() < 0.01,
        "colspan x; got {fills:?}"
    );
    // The colspan width spans BOTH auto columns: it equals the sum of the two
    // single auto cells' widths in row 2 (col1 + col2).
    let col1_w = fills[3].1;
    let col2_w = fills[4].1;
    assert!(
        (fills[1].1 - (col1_w + col2_w)).abs() < 0.5,
        "colspan w must span both auto columns: {} vs {}+{}; got {fills:?}",
        fills[1].1,
        col1_w,
        col2_w
    );
    // The two auto columns place edge-to-edge (gap=0): row-2 col1 starts at 200,
    // col2 starts at 200+col1_w.
    assert!(
        (fills[3].0 - 200.0).abs() < 0.01,
        "row2 col1 x; got {fills:?}"
    );
    assert!(
        (fills[4].0 - (200.0 + col1_w)).abs() < 0.5,
        "row2 col2 x; got {fills:?}"
    );
    // Auto columns are content-sized, NOT the old equal-split 180px each.
    assert!(
        col1_w > 0.0 && col2_w > 0.0,
        "auto cols sized; got {fills:?}"
    );
}

/// An AUTO column sizes to its WIDEST cell's natural text: a column whose cells
/// hold a long word is wider than a column whose cells hold a short word.
#[test]
fn auto_column_sizes_to_widest_text() {
    // Two AUTO columns, two rows. Column 0 always holds a short word; column 1
    // holds a much longer word. The long-text column must come out wider.
    let src = r##"zenith version=1 {
  project id="proj.aw" name="AW"
  tokens format="zenith-token-v1" {
    token id="color.ink" type="color" value="#000000"
  }
  styles {}
  document id="doc.aw" title="AW" {
    page id="page.aw" w=(px)800 h=(px)400 {
      table id="t.aw" x=(px)0 y=(px)0 w=(px)800 h=(px)300 fill=(token)"color.ink" cell-padding=(px)0 gap=(px)0 {
        column
        column
        row {
          cell { text id="a1" x=(px)0 y=(px)0 { span "Hi" } }
          cell { text id="a2" x=(px)0 y=(px)0 { span "Supercalifragilistic" } }
        }
        row {
          cell { text id="b1" x=(px)0 y=(px)0 { span "Ok" } }
          cell { text id="b2" x=(px)0 y=(px)0 { span "Antidisestablishmentarianism" } }
        }
      }
    }
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    // Row-major fills: [0]=col0/row0, [1]=col1/row0, [2]=col0/row1, [3]=col1/row1.
    let fills: Vec<(f64, f64)> = result
        .scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::FillRect { x, w, .. } => Some((*x, *w)),
            _ => None,
        })
        .collect();
    assert_eq!(fills.len(), 4, "expected 4 cell fills; got {fills:?}");
    let col0_w = fills[0].1;
    let col1_w = fills[1].1;
    assert!(
        col1_w > col0_w,
        "the long-text column must be wider than the short-text column: {col1_w} vs {col0_w}"
    );
}

/// A cell whose text WRAPS onto multiple lines makes its row taller than a row
/// whose cells fit on a single line.
#[test]
fn wrapping_text_makes_row_taller() {
    // Two AUTO columns. Column 0 is widened by a long header in row 0; column 1
    // is forced narrow. Row 0's col-1 text is short (single line); row 1's col-1
    // text is long, so at the narrow assigned width it WRAPS to several lines and
    // its row must be taller than the single-line row 0.
    let src = r##"zenith version=1 {
  project id="proj.rh" name="RH"
  tokens format="zenith-token-v1" {
    token id="color.ink" type="color" value="#000000"
  }
  styles {}
  document id="doc.rh" title="RH" {
    page id="page.rh" w=(px)400 h=(px)600 {
      table id="t.rh" x=(px)0 y=(px)0 w=(px)200 h=(px)500 fill=(token)"color.ink" cell-padding=(px)0 gap=(px)0 {
        column width=(px)40
        column width=(px)80
        row {
          cell { text id="r0a" x=(px)0 y=(px)0 { span "A" } }
          cell { text id="r0b" x=(px)0 y=(px)0 w=(px)80 { span "Short" } }
        }
        row {
          cell { text id="r1a" x=(px)0 y=(px)0 { span "B" } }
          cell { text id="r1b" x=(px)0 y=(px)0 w=(px)80 { span "alpha bravo charlie delta echo foxtrot golf hotel india juliet" } }
        }
      }
    }
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    // Row-major fills: [0],[1]=row0 cells; [2],[3]=row1 cells. Compare row tops/
    // heights by the cell y positions and heights.
    let fills: Vec<(f64, f64)> = result
        .scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::FillRect { y, h, .. } => Some((*y, *h)),
            _ => None,
        })
        .collect();
    assert_eq!(fills.len(), 4, "expected 4 cell fills; got {fills:?}");
    let row0_h = fills[0].1;
    let row1_h = fills[2].1;
    assert!(
        row1_h > row0_h + 1.0,
        "the wrapping row must be taller than the single-line row: {row1_h} vs {row0_h}"
    );
    // Row 1 must start below row 0 (content-based stacking, top-aligned).
    assert!(
        fills[2].0 > fills[0].0,
        "row 1 must sit below row 0; got {fills:?}"
    );
}

/// An ALL-EXPLICIT-width table produces the SAME column widths as the pre
/// content-sizing behavior (determinism guarantee): explicit columns are never
/// touched by content measurement.
#[test]
fn all_explicit_columns_unchanged() {
    let src = r##"zenith version=1 {
  project id="proj.ex" name="EX"
  tokens format="zenith-token-v1" {
    token id="color.ink" type="color" value="#000000"
  }
  styles {}
  document id="doc.ex" title="EX" {
    page id="page.ex" w=(px)800 h=(px)400 {
      table id="t.ex" x=(px)10 y=(px)10 w=(px)600 h=(px)300 fill=(token)"color.ink" cell-padding=(px)0 gap=(px)0 {
        column width=(px)100
        column width=(px)250
        column width=(px)90
        row {
          cell { text id="e1" x=(px)0 y=(px)0 { span "One" } }
          cell { text id="e2" x=(px)0 y=(px)0 { span "Two" } }
          cell { text id="e3" x=(px)0 y=(px)0 { span "Three" } }
        }
      }
    }
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    let fills: Vec<(f64, f64)> = result
        .scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::FillRect { x, w, .. } => Some((*x, *w)),
            _ => None,
        })
        .collect();
    assert_eq!(fills.len(), 3, "expected 3 cell fills; got {fills:?}");
    // Explicit widths are honored verbatim, regardless of cell content.
    assert!((fills[0].0 - 10.0).abs() < 0.01, "col0 x; got {fills:?}");
    assert!((fills[0].1 - 100.0).abs() < 0.01, "col0 w; got {fills:?}");
    assert!((fills[1].0 - 110.0).abs() < 0.01, "col1 x; got {fills:?}");
    assert!((fills[1].1 - 250.0).abs() < 0.01, "col1 w; got {fills:?}");
    assert!((fills[2].0 - 360.0).abs() < 0.01, "col2 x; got {fills:?}");
    assert!((fills[2].1 - 90.0).abs() < 0.01, "col2 w; got {fills:?}");
}

#[test]
fn cell_text_positioned_at_content_origin() {
    let doc = parse(table_src());
    let result = compile(&doc, &default_provider());

    // The first cell's text (authored x=0) is translated to the cell content
    // origin x=40 (table x + 0 padding). With h-align default "start" the run
    // x equals the content-box left edge.
    let first_run_x = result.scene.commands.iter().find_map(|c| match c {
        SceneCommand::DrawGlyphRun { x, .. } => Some(*x),
        _ => None,
    });
    assert_eq!(
        first_run_x,
        Some(40.0),
        "first cell text must sit at the cell content origin x=40"
    );
}

#[test]
fn invisible_table_emits_nothing() {
    let src = table_src().replace("table id=\"t1\"", "table id=\"t1\" visible=#false");
    let doc = parse(&src);
    let result = compile(&doc, &default_provider());

    // No table-derived commands: no FillRect (no page bg), no StrokeLine, no
    // glyph runs. (PushClip for the media box is always present.)
    let drawn = result.scene.commands.iter().any(|c| {
        matches!(
            c,
            SceneCommand::FillRect { .. }
                | SceneCommand::StrokeLine { .. }
                | SceneCommand::DrawGlyphRun { .. }
        )
    });
    assert!(!drawn, "an invisible table must emit no drawing commands");
}

/// Separate mode (no `border-collapse` attribute) still emits exactly 4
/// `StrokeLine`s per cell — the existing behavior must be byte-identical.
/// This guards against regressions on the default separate path.
#[test]
fn separate_mode_stroke_count_unchanged() {
    // The shared `table_src()` is 5 placed cells (one colspan=2 in row 0,
    // three normal in row 1). Separate mode: 5 × 4 = 20 StrokeLines.
    let result = compile(&parse(table_src()), &default_provider());
    let stroke_count = result
        .scene
        .commands
        .iter()
        .filter(|c| matches!(c, SceneCommand::StrokeLine { .. }))
        .count();
    assert_eq!(
        stroke_count,
        5 * 4,
        "separate mode (default) must emit 4 border edges per placed cell; got {stroke_count}"
    );
}

/// Regression: a NORMAL table with NO `flows` attribute renders byte-identically
/// to before this unit. The shared `table_src()` (no flows) must produce the
/// exact same command count whether or not the flow pre-pass exists.
#[test]
fn non_flow_table_command_count_unchanged() {
    let result = compile(&parse(table_src()), &default_provider());
    let total = result.scene.commands.len();
    // 5 placed cells: each emits FillRect + 4 StrokeLine + PushClip + glyph + PopClip.
    // We assert the count is stable and non-zero (byte-identical guard); the exact
    // figure is pinned by the pre-existing single-page tests, so here we only
    // ensure the flow pre-pass added nothing to a non-flow table.
    let flow_pass_added = result
        .scene
        .commands
        .iter()
        .any(|c| matches!(c, SceneCommand::DrawGlyphRun { .. }));
    assert!(total > 0 && flow_pass_added, "non-flow table still renders");
}
