//! Integration tests for `table` header-row styling.
//!
//! Covers: `header-fill` applied to the header row's cell backgrounds, a header
//! cell's own `fill` overriding `header-fill`, `header-style` recoloring header
//! text children (and not body children), the inert no-header-rows path, and the
//! header-style measurement bug regression (bold header widens an AUTO column).

mod common;

use common::{Paint, SceneCommand, compile, default_provider, parse};

// ── Header-row styling tests ──────────────────────────────────────────────────

/// A 2-row table with `header-rows=1` and a distinct `header-fill` token.
/// The first row's cell background must use the header-fill color; the second
/// row's cell must use the table body `fill` color.
#[test]
fn header_fill_applied_to_first_row_cells() {
    let src = r##"zenith version=1 {
  project id="proj.hf" name="HF"
  tokens format="zenith-token-v1" {
    token id="color.header" type="color" value="#aabbcc"
    token id="color.body"   type="color" value="#112233"
  }
  styles {}
  document id="doc.hf" title="HF" {
    page id="page.hf" w=(px)400 h=(px)300 {
      table id="t.hf" x=(px)0 y=(px)0 w=(px)200 h=(px)200 fill=(token)"color.body" header-rows=1 header-fill=(token)"color.header" cell-padding=(px)0 gap=(px)0 {
        column width=(px)100
        column width=(px)100
        row {
          cell { text id="h1" x=(px)0 y=(px)0 { span "H1" } }
          cell { text id="h2" x=(px)0 y=(px)0 { span "H2" } }
        }
        row {
          cell { text id="b1" x=(px)0 y=(px)0 { span "B1" } }
          cell { text id="b2" x=(px)0 y=(px)0 { span "B2" } }
        }
      }
    }
  }
}
"##;
    let result = compile(&parse(src), &default_provider());

    // Row-major FillRects: [0],[1] = header row, [2],[3] = body row.
    let fill_colors: Vec<(u8, u8, u8)> = result
        .scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::FillRect {
                paint: Paint::Solid { color },
                ..
            } => Some((color.r, color.g, color.b)),
            _ => None,
        })
        .collect();

    assert_eq!(
        fill_colors.len(),
        4,
        "expected 4 cell fills; got {fill_colors:?}"
    );
    // Header-fill: #aabbcc = r=0xaa=170, g=0xbb=187, b=0xcc=204.
    assert_eq!(
        fill_colors[0],
        (0xaa, 0xbb, 0xcc),
        "header cell 0 must use header-fill; got {fill_colors:?}"
    );
    assert_eq!(
        fill_colors[1],
        (0xaa, 0xbb, 0xcc),
        "header cell 1 must use header-fill; got {fill_colors:?}"
    );
    // Body fill: #112233 = r=0x11=17, g=0x22=34, b=0x33=51.
    assert_eq!(
        fill_colors[2],
        (0x11, 0x22, 0x33),
        "body cell 0 must use table fill; got {fill_colors:?}"
    );
    assert_eq!(
        fill_colors[3],
        (0x11, 0x22, 0x33),
        "body cell 1 must use table fill; got {fill_colors:?}"
    );
}

/// A header cell with its OWN `fill` must keep that fill, overriding
/// the table's `header-fill`. (cell.fill precedence is highest.)
#[test]
fn header_cell_own_fill_overrides_header_fill() {
    let src = r##"zenith version=1 {
  project id="proj.hco" name="HCO"
  tokens format="zenith-token-v1" {
    token id="color.header" type="color" value="#aabbcc"
    token id="color.cell"   type="color" value="#ff0000"
    token id="color.body"   type="color" value="#112233"
  }
  styles {}
  document id="doc.hco" title="HCO" {
    page id="page.hco" w=(px)400 h=(px)300 {
      table id="t.hco" x=(px)0 y=(px)0 w=(px)200 h=(px)200 fill=(token)"color.body" header-rows=1 header-fill=(token)"color.header" cell-padding=(px)0 gap=(px)0 {
        column width=(px)200
        row {
          cell fill=(token)"color.cell" { text id="hc" x=(px)0 y=(px)0 { span "Header" } }
        }
        row {
          cell { text id="bc" x=(px)0 y=(px)0 { span "Body" } }
        }
      }
    }
  }
}
"##;
    let result = compile(&parse(src), &default_provider());

    let fill_colors: Vec<(u8, u8, u8)> = result
        .scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::FillRect {
                paint: Paint::Solid { color },
                ..
            } => Some((color.r, color.g, color.b)),
            _ => None,
        })
        .collect();

    assert_eq!(
        fill_colors.len(),
        2,
        "expected 2 cell fills; got {fill_colors:?}"
    );
    // Header cell has its OWN fill=#ff0000; cell.fill wins over header-fill.
    assert_eq!(
        fill_colors[0],
        (0xff, 0x00, 0x00),
        "header cell with own fill must use cell.fill; got {fill_colors:?}"
    );
    // Body cell falls back to table fill=#112233.
    assert_eq!(
        fill_colors[1],
        (0x11, 0x22, 0x33),
        "body cell must use table fill; got {fill_colors:?}"
    );
}

/// `header-rows=1` + `header-style` with a distinct fill: a header text child
/// with no own `style` picks up the header style's fill color (distinct glyph
/// color), while a body-row text child does not.
///
/// The text node fill cascade is: node.fill → style.fill → default(black).
/// When header-style is injected (the text node has no `style` attr), the
/// style-resolved fill takes effect. The text nodes here have NO own fill so
/// the style fill is the only source of color; body text also has no own fill
/// and no injected style, so it falls through to default black.
#[test]
fn header_style_applied_to_unstyled_text_children() {
    // style.header declares fill=#ff8800 (orange).
    // Row 0 (header): text has no style attr → header-style injected → orange.
    // Row 1 (body): text has no style attr, no injection → default black.
    let src = r##"zenith version=1 {
  project id="proj.hs" name="HS"
  tokens format="zenith-token-v1" {
    token id="color.orange" type="color" value="#ff8800"
    token id="color.bg"     type="color" value="#ffffff"
  }
  styles {
    style id="style.header" {
      fill (token)"color.orange"
    }
  }
  document id="doc.hs" title="HS" {
    page id="page.hs" w=(px)400 h=(px)300 {
      table id="t.hs" x=(px)0 y=(px)0 w=(px)200 h=(px)200 fill=(token)"color.bg" header-rows=1 header-style="style.header" cell-padding=(px)0 gap=(px)0 {
        column width=(px)200
        row {
          cell { text id="ht" x=(px)0 y=(px)0 { span "Header" } }
        }
        row {
          cell { text id="bt" x=(px)0 y=(px)0 { span "Body" } }
        }
      }
    }
  }
}
"##;
    let result = compile(&parse(src), &default_provider());

    // Collect DrawGlyphRun colors in emission order: [0]=header run, [1]=body run.
    let run_colors: Vec<(u8, u8, u8)> = result
        .scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::DrawGlyphRun { color, .. } => Some((color.r, color.g, color.b)),
            _ => None,
        })
        .collect();

    assert_eq!(
        run_colors.len(),
        2,
        "expected 2 glyph runs; got {run_colors:?}"
    );
    // Header run: style.header fill=#ff8800 (r=255, g=136, b=0).
    assert_eq!(
        run_colors[0],
        (0xff, 0x88, 0x00),
        "header text must use header-style fill; got {run_colors:?}"
    );
    // Body run: no header-style applied, no node fill → default black (0,0,0).
    assert_eq!(
        run_colors[1],
        (0x00, 0x00, 0x00),
        "body text must NOT use header-style; got {run_colors:?}"
    );
}

/// Regression: a table WITHOUT `header-rows` emits fills byte-identical to the
/// same table. Specifically the body fill color appears for all cells, and adding
/// `header-fill` without `header-rows` changes nothing (header_rows=0 → no cell
/// is a header, the header path is inert).
#[test]
fn no_header_rows_emits_table_fill_for_all_cells() {
    let src_no_header = r##"zenith version=1 {
  project id="proj.nh" name="NH"
  tokens format="zenith-token-v1" {
    token id="color.body" type="color" value="#334455"
  }
  styles {}
  document id="doc.nh" title="NH" {
    page id="page.nh" w=(px)400 h=(px)300 {
      table id="t.nh" x=(px)0 y=(px)0 w=(px)200 h=(px)200 fill=(token)"color.body" cell-padding=(px)0 gap=(px)0 {
        column width=(px)200
        row {
          cell { text id="r0" x=(px)0 y=(px)0 { span "R0" } }
        }
        row {
          cell { text id="r1" x=(px)0 y=(px)0 { span "R1" } }
        }
      }
    }
  }
}
"##;
    // Variant with header-fill declared but header-rows absent (so header_rows=0).
    let src_with_header_fill = src_no_header.replace(
        "fill=(token)\"color.body\" cell-padding",
        "fill=(token)\"color.body\" header-fill=(token)\"color.body\" cell-padding",
    );

    let result_a = compile(&parse(src_no_header), &default_provider());
    let result_b = compile(&parse(&src_with_header_fill), &default_provider());

    let colors_a: Vec<(u8, u8, u8)> = result_a
        .scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::FillRect {
                paint: Paint::Solid { color },
                ..
            } => Some((color.r, color.g, color.b)),
            _ => None,
        })
        .collect();
    let colors_b: Vec<(u8, u8, u8)> = result_b
        .scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::FillRect {
                paint: Paint::Solid { color },
                ..
            } => Some((color.r, color.g, color.b)),
            _ => None,
        })
        .collect();

    assert_eq!(
        colors_a.len(),
        2,
        "expected 2 fills without header-rows; got {colors_a:?}"
    );
    // All cells must carry the table body fill (#334455).
    assert!(
        colors_a.iter().all(|&c| c == (0x33, 0x44, 0x55)),
        "all cells must use table fill; got {colors_a:?}"
    );
    // Variant with header-fill but header-rows=0: must be identical.
    assert_eq!(
        colors_a, colors_b,
        "header-fill with no header-rows must not change fill output"
    );
}

/// Regression test for the header-style measurement bug: an AUTO column whose
/// WIDEST cell is a header row with `header-style` (e.g. bold) must be sized
/// to the BOLD-measured width, not the non-bold width. We prove this by
/// building two documents that differ only in whether `style.bold` applies
/// `font-weight=700` or `font-weight=400` to the header text. The header cell
/// holds the widest text in the column ("Supercalifragilistic") while the body
/// cell holds a shorter word. With the fix the bold-header column is strictly
/// wider than the normal-weight-header column.
#[test]
fn header_style_bold_widens_auto_column() {
    // Template: two-row AUTO-column table. header-rows=1, header-style="style.bold".
    // The header cell has the longest text; body cell has a short word.
    // style.bold sets font-weight to either 700 (bold) or 400 (normal) depending
    // on the token value — only this differs between the two compiled documents.
    fn make_src(weight: u32) -> String {
        format!(
            r##"zenith version=1 {{
  project id="proj.bh" name="BH"
  tokens format="zenith-token-v1" {{
    token id="weight.val" type="fontWeight" value={weight}
    token id="color.ink"  type="color" value="#000000"
  }}
  styles {{
    style id="style.bold" {{
      font-weight (token)"weight.val"
    }}
  }}
  document id="doc.bh" title="BH" {{
    page id="page.bh" w=(px)800 h=(px)400 {{
      table id="t.bh" x=(px)0 y=(px)0 w=(px)800 h=(px)300 fill=(token)"color.ink" header-rows=1 header-style="style.bold" cell-padding=(px)0 gap=(px)0 {{
        column
        row {{
          cell {{ text id="hdr" x=(px)0 y=(px)0 {{ span "Supercalifragilistic" }} }}
        }}
        row {{
          cell {{ text id="bod" x=(px)0 y=(px)0 {{ span "Hi" }} }}
        }}
      }}
    }}
  }}
}}
"##
        )
    }

    let result_bold = compile(&parse(&make_src(700)), &default_provider());
    let result_norm = compile(&parse(&make_src(400)), &default_provider());

    // The first body-row cell's FillRect width is the auto column width.
    // Emission order: [0]=header cell fill, [1]=body cell fill.
    let col_w = |result: &zenith_scene::CompileResult| -> f64 {
        result
            .scene
            .commands
            .iter()
            .filter_map(|c| match c {
                SceneCommand::FillRect { w, .. } => Some(*w),
                _ => None,
            })
            .nth(1) // body-row (index 1) — its width is the resolved column width
            .expect("body cell FillRect must exist")
    };

    let bold_col_w = col_w(&result_bold);
    let norm_col_w = col_w(&result_norm);

    assert!(
        bold_col_w > norm_col_w,
        "bold header-style must widen the AUTO column vs normal weight: \
         bold={bold_col_w} normal={norm_col_w}"
    );
}
