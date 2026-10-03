mod common;
use common::*;
use zenith_core::default_provider;
use zenith_scene::compile;

/// 2×3 grid, 6 children, gap=20, pad=0. Children tile row-major into a fixed
/// `cols × rows` grid; horizontal and vertical gutters equal `gap`.
#[test]
fn grid_two_by_three_positions_children_with_gutters() {
    // frame w=320 h=300, cols=2, rows=3, gap=20, pad=0.
    //   col_w = (320 - (2-1)*20) / 2 = 150
    //   row_h = (300 - (3-1)*20) / 3 = 260/3
    let src = r##"zenith version=1 {
  project id="proj.grid1" name="Grid1"
  tokens format="zenith-token-v1" {
token id="color.k" type="color" value="#000000"
token id="space.gap" type="dimension" value=(px)20
  }
  styles {
style id="style.grid" {
  gap (token)"space.gap"
}
  }
  document id="doc.grid1" title="Grid1" {
page id="page.grid1" w=(px)400 h=(px)400 {
  frame id="frame.grid" x=(px)0 y=(px)0 w=(px)320 h=(px)300 layout="grid" columns=2 rows=3 style="style.grid" {
    rect id="r0" fill=(token)"color.k"
    rect id="r1" fill=(token)"color.k"
    rect id="r2" fill=(token)"color.k"
    rect id="r3" fill=(token)"color.k"
    rect id="r4" fill=(token)"color.k"
    rect id="r5" fill=(token)"color.k"
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    assert!(
        result.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );

    let rects = fill_rects(&result);
    assert_eq!(
        rects.len(),
        6,
        "expected six child FillRects; got {rects:?}"
    );

    let gap = 20.0;
    let col_w = (320.0 - gap) / 2.0; // 150
    let row_h = (300.0 - 2.0 * gap) / 3.0; // 260/3

    // Expected origins, row-major.
    for (i, (x, y, w, h)) in rects.iter().enumerate() {
        let col = (i % 2) as f64;
        let row = (i / 2) as f64;
        let exp_x = col * (col_w + gap);
        let exp_y = row * (row_h + gap);
        assert!(
            (*x - exp_x).abs() < 1e-9,
            "cell {i}: x expected {exp_x}; got {x}"
        );
        assert!(
            (*y - exp_y).abs() < 1e-9,
            "cell {i}: y expected {exp_y}; got {y}"
        );
        assert!(
            (*w - col_w).abs() < 1e-9,
            "cell {i}: w expected {col_w}; got {w}"
        );
        assert!(
            (*h - row_h).abs() < 1e-9,
            "cell {i}: h expected {row_h}; got {h}"
        );
    }

    // Horizontal gutter between col0 and col1 equals gap.
    let (x0, _, w0, _) = rects[0];
    let (x1, _, _, _) = rects[1];
    assert!(
        (x1 - (x0 + w0) - gap).abs() < 1e-9,
        "horizontal gutter must equal gap ({gap})"
    );
    // Vertical gutter between row0 and row1 equals gap.
    let (_, y0, _, h0) = rects[0];
    let (_, y2, _, _) = rects[2];
    assert!(
        (y2 - (y0 + h0) - gap).abs() < 1e-9,
        "vertical gutter must equal gap ({gap})"
    );
}

/// `layout="grid"` with no `columns` → single column stack; the scene defaults
/// to 1 column and validation emits a `grid.missing_columns` advisory.
#[test]
fn grid_default_columns_is_one() {
    let src = r##"zenith version=1 {
  project id="proj.grid2" name="Grid2"
  tokens format="zenith-token-v1" {
token id="color.k" type="color" value="#000000"
  }
  styles {}
  document id="doc.grid2" title="Grid2" {
page id="page.grid2" w=(px)400 h=(px)400 {
  frame id="frame.grid" x=(px)0 y=(px)0 w=(px)300 h=(px)300 layout="grid" {
    rect id="r0" fill=(token)"color.k"
    rect id="r1" fill=(token)"color.k"
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    let rects = fill_rects(&result);
    assert_eq!(
        rects.len(),
        2,
        "expected two child FillRects; got {rects:?}"
    );
    // Single column: both children share x and span the full content width.
    let (x0, _, w0, _) = rects[0];
    let (x1, _, w1, _) = rects[1];
    assert_eq!(x0, 0.0, "single-column child0 x must be content_left (0)");
    assert_eq!(x1, 0.0, "single-column child1 x must be content_left (0)");
    assert_eq!(w0, 300.0, "single column width must be full content width");
    assert_eq!(w1, 300.0, "single column width must be full content width");
    // Stacked vertically (row1 below row0).
    let (_, y0, _, _) = rects[0];
    let (_, y1, _, _) = rects[1];
    assert!(y1 > y0, "child1 must sit below child0 in a single column");

    // The validator emits a grid.missing_columns advisory.
    let report = zenith_core::validate(&doc);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "grid.missing_columns"),
        "expected grid.missing_columns advisory; codes: {:?}",
        report
            .diagnostics
            .iter()
            .map(|d| &d.code)
            .collect::<Vec<_>>()
    );
}

/// `rows` omitted → derived as `ceil(n / cols)`; the last row is positioned
/// correctly (3 children, 2 cols → 2 rows; child index 2 starts row 1).
#[test]
fn grid_derived_rows_from_child_count() {
    let src = r##"zenith version=1 {
  project id="proj.grid3" name="Grid3"
  tokens format="zenith-token-v1" {
token id="color.k" type="color" value="#000000"
  }
  styles {}
  document id="doc.grid3" title="Grid3" {
page id="page.grid3" w=(px)400 h=(px)400 {
  frame id="frame.grid" x=(px)0 y=(px)0 w=(px)300 h=(px)300 layout="grid" columns=2 {
    rect id="r0" fill=(token)"color.k"
    rect id="r1" fill=(token)"color.k"
    rect id="r2" fill=(token)"color.k"
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    assert!(
        result.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );

    let rects = fill_rects(&result);
    assert_eq!(
        rects.len(),
        3,
        "expected three child FillRects; got {rects:?}"
    );

    // n=3, cols=2 → effective_rows = ceil(3/2) = 2. gap=0, pad=0.
    //   col_w = 300/2 = 150, row_h = 300/2 = 150.
    let (x0, y0, w0, h0) = rects[0];
    let (x1, y1, _, _) = rects[1];
    let (x2, y2, _, _) = rects[2];
    assert_eq!((x0, y0, w0, h0), (0.0, 0.0, 150.0, 150.0));
    // r1 is col1 of row0.
    assert_eq!((x1, y1), (150.0, 0.0));
    // r2 wraps to the last row (row1, col0).
    assert_eq!((x2, y2), (0.0, 150.0));
}

/// A non-grid frame (absolute and column) emits the identical command stream
/// regardless of the grid fields existing on the AST — default-off identity.
#[test]
fn non_grid_frame_byte_identical() {
    // Absolute frame: child keeps its own coords.
    let abs_src = r##"zenith version=1 {
  project id="proj.grid4" name="Grid4"
  tokens format="zenith-token-v1" {
token id="color.k" type="color" value="#000000"
  }
  styles {}
  document id="doc.grid4" title="Grid4" {
page id="page.grid4" w=(px)200 h=(px)200 {
  frame id="frame.abs" x=(px)20 y=(px)30 w=(px)160 h=(px)160 {
    rect id="rect.a" x=(px)50 y=(px)60 w=(px)40 h=(px)30 fill=(token)"color.k"
  }
}
  }
}
"##;
    let abs = compile(&parse(abs_src), &default_provider());
    // The child kept its own absolute coords (no grid injection).
    assert_eq!(fill_rects(&abs), vec![(50.0, 60.0, 40.0, 30.0)]);

    // Column frame: still stacks vertically, unaffected by grid code.
    let col_src = r##"zenith version=1 {
  project id="proj.grid5" name="Grid5"
  tokens format="zenith-token-v1" {
token id="color.k" type="color" value="#000000"
  }
  styles {}
  document id="doc.grid5" title="Grid5" {
page id="page.grid5" w=(px)200 h=(px)200 {
  frame id="frame.col" x=(px)0 y=(px)0 w=(px)160 h=(px)160 layout="column" clip=#true {
    rect id="rect.a" h=(px)30 fill=(token)"color.k"
    rect id="rect.b" h=(px)30 fill=(token)"color.k"
  }
}
  }
}
"##;
    let col = compile(&parse(col_src), &default_provider());
    let rects = fill_rects(&col);
    assert_eq!(rects.len(), 2);
    // Stacked (column), NOT side-by-side (grid would tile horizontally).
    assert_eq!(rects[0].0, rects[1].0, "column children share x (stacked)");
    assert!(rects[1].1 > rects[0].1, "column child2 below child1");
}
