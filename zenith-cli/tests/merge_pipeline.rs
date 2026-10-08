//! End-to-end integration tests for `zenith merge`.
//!
//! Calls [`zenith_cli::commands::merge::run`] directly with inline source
//! strings and a [`tempfile::TempDir`] as the output directory, following the
//! pattern established by `tx_pipeline.rs`.

use tempfile::TempDir;
use zenith_cli::commands::merge::{build_manifest, run as merge_run, to_json_output};

// ── Shared fixture helper ─────────────────────────────────────────────────────

/// Write a minimal valid PNG of `w × h` pixels into `dir/<name>.png` and
/// return the relative filename (e.g. `"a.png"`). The pixmap is filled with an
/// OPAQUE color derived from `name`, so two differently-named fixtures produce
/// visibly different renders — this is what lets the image-swap tests prove a
/// per-row asset actually reached the raster (a transparent fixture would
/// render identically regardless of the swap).
fn write_test_png(dir: &TempDir, name: &str, w: u32, h: u32) -> String {
    let filename = format!("{name}.png");
    let path = dir.path().join(&filename);
    let mut pixmap =
        tiny_skia::Pixmap::new(w, h).expect("Pixmap::new must succeed for positive dimensions");
    // Distinct opaque color per fixture name (simple deterministic hash).
    let seed = name.bytes().fold(0u32, |acc, b| {
        acc.wrapping_mul(31).wrapping_add(u32::from(b))
    });
    let r = (seed & 0xff) as u8;
    let g = ((seed >> 8) & 0xff) as u8;
    let b = ((seed >> 16) & 0xff) as u8;
    pixmap.fill(tiny_skia::Color::from_rgba8(r, g, b, 255));
    let png_bytes = pixmap.encode_png().expect("encode_png must succeed");
    std::fs::write(&path, &png_bytes).expect("could not write PNG fixture");
    filename
}

// ── Minimal template document ──────────────────────────────────────────────────

/// Template doc with two `data.*`-role text nodes (`name` and `title`),
/// with boxes large enough to fit any reasonable CSV value without overflowing.
const TEMPLATE_DOC: &str = r##"zenith version=1 {
  project id="proj.merge" name="Merge Test"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#ffffff"
    token id="color.ink" type="color" value="#111111"
  }
  styles {}
  document id="doc.merge" title="Merge Test" {
    page id="page.merge" w=(px)400 h=(px)200 {
      rect id="rect.bg" x=(px)0 y=(px)0 w=(px)400 h=(px)200 fill=(token)"color.bg"
      text id="text.name" x=(px)10 y=(px)10 w=(px)380 h=(px)80 fill=(token)"color.ink" role="data.name" {
        span "PLACEHOLDER_NAME"
      }
      text id="text.title" x=(px)10 y=(px)100 w=(px)380 h=(px)80 fill=(token)"color.ink" role="data.title" {
        span "PLACEHOLDER_TITLE"
      }
    }
  }
}
"##;

/// Template doc with a single `data.name` text node and `overflow="fit"` on a
/// very small box, so a long value triggers `text.fit_failed`.
const OVERFLOW_FIT_TEMPLATE: &str = r##"zenith version=1 {
  project id="proj.fit" name="Fit Test"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#ffffff"
    token id="color.ink" type="color" value="#111111"
  }
  styles {}
  document id="doc.fit" title="Fit Test" {
    page id="page.fit" w=(px)400 h=(px)200 {
      rect id="rect.bg" x=(px)0 y=(px)0 w=(px)400 h=(px)200 fill=(token)"color.bg"
      text id="text.label" x=(px)10 y=(px)10 w=(px)60 h=(px)40 overflow="fit" fill=(token)"color.ink" role="data.name" {
        span "X"
      }
    }
  }
}
"##;

/// Template doc where the `data.*` role is placed on a rect (non-text) node.
const ROLE_ON_RECT_DOC: &str = r##"zenith version=1 {
  project id="proj.badrole" name="Bad Role"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#ffffff"
  }
  styles {}
  document id="doc.badrole" title="Bad Role" {
    page id="page.badrole" w=(px)200 h=(px)100 {
      rect id="rect.data" x=(px)0 y=(px)0 w=(px)200 h=(px)100 fill=(token)"color.bg" role="data.name"
    }
  }
}
"##;

// ── (a) Two-row CSV → two PNGs with default names ─────────────────────────────

#[test]
fn two_row_csv_writes_two_pngs_with_default_names() {
    let csv = "name,title\nAlice,Engineer\nBob,Designer\n";
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let report = merge_run(TEMPLATE_DOC, csv, None, tmp.path(), None).expect("merge must succeed");

    let written = report.written();
    assert_eq!(written.len(), 2, "two rows must produce two PNGs");
    assert!(
        report.failed().is_empty(),
        "no rows should fail; got: {:?}",
        report.rows
    );

    // Default names must be row-0001.png and row-0002.png.
    assert_eq!(written[0], "row-0001.png");
    assert_eq!(written[1], "row-0002.png");

    // Both files must start with the PNG magic bytes.
    for name in &written {
        let path = tmp.path().join(name);
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|e| panic!("could not read {}: {}", path.display(), e));
        assert!(
            bytes.len() >= 4 && &bytes[0..4] == b"\x89PNG",
            "{} must be a valid PNG; got {} bytes",
            name,
            bytes.len()
        );
    }
}

// ── (b) --name-by → files named by sanitized cell value ──────────────────────

#[test]
fn name_by_column_produces_named_files() {
    let csv = "name,title\nAlice,Engineer\nBob,Designer\n";
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let report =
        merge_run(TEMPLATE_DOC, csv, None, tmp.path(), Some("name")).expect("merge must succeed");

    let written = report.written();
    assert_eq!(written.len(), 2);
    assert!(
        report.failed().is_empty(),
        "no rows should fail; got: {:?}",
        report.rows
    );

    // Names come from the `name` column, sanitized, with .png extension.
    assert_eq!(written[0], "Alice.png");
    assert_eq!(written[1], "Bob.png");

    for name in &written {
        let path = tmp.path().join(name);
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|e| panic!("could not read {}: {}", path.display(), e));
        assert!(
            bytes.len() >= 4 && &bytes[0..4] == b"\x89PNG",
            "{} must be a valid PNG",
            name
        );
    }
}

// ── (c) text.fit_failed → that row in report.failed, no PNG written ───────────

#[test]
fn overflow_fit_failure_goes_to_failed_not_written() {
    // Row 1 has a short value that fits; row 2 has a very long value that
    // cannot be made to fit the 60×20 box even at minimum font size.
    let csv = "name\nHi\nThe quick brown fox jumps over the lazy dog and keeps on going\n";
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let report = merge_run(OVERFLOW_FIT_TEMPLATE, csv, None, tmp.path(), None)
        .expect("merge run itself must not error");

    // Row 0 ("Hi") should succeed.
    assert!(
        report.written().contains(&"row-0001.png".to_owned()),
        "short value must succeed; written: {:?}",
        report.written()
    );

    // Row 1 (long value) must appear in failed with no PNG on disk.
    let row1_failed = report.failed().iter().any(|r| r.row == 1);
    assert!(
        row1_failed,
        "long-value row must be in failed; failed: {:?}",
        report
            .failed()
            .iter()
            .map(|r| (r.row, r.failure.as_deref().unwrap_or("")))
            .collect::<Vec<_>>()
    );

    let row1_png = tmp.path().join("row-0002.png");
    assert!(
        !row1_png.exists(),
        "row-0002.png must NOT have been written"
    );
}

// ── (d) Unknown column in binding → Err(MergeError) ─────────────────────────

#[test]
fn unknown_column_in_csv_returns_merge_error() {
    // CSV has `name` and `title` columns, so binding to a non-existent column
    // requires a template that references a column the CSV doesn't have.
    // We use TEMPLATE_DOC (binds `name` and `title`) but give a CSV with only
    // `name` — `title` is missing.
    let csv = "name\nAlice\n";
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let result = merge_run(TEMPLATE_DOC, csv, None, tmp.path(), None);

    assert!(
        result.is_err(),
        "missing CSV column must produce MergeError; got Ok"
    );
    let err = result.unwrap_err();
    assert_eq!(err.exit_code, 2, "setup error must have exit_code 2");
    assert!(
        err.message.contains("title"),
        "error message must mention the missing column; got: {}",
        err.message
    );
}

// ── (e) data.* role on a non-text node → Err(MergeError) ────────────────────

#[test]
fn data_role_on_non_text_node_returns_merge_error() {
    let csv = "name\nAlice\n";
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let result = merge_run(ROLE_ON_RECT_DOC, csv, None, tmp.path(), None);

    assert!(
        result.is_err(),
        "data.* role on a rect must produce MergeError; got Ok"
    );
    let err = result.unwrap_err();
    assert_eq!(err.exit_code, 2);
    assert!(
        err.message.contains("non-text"),
        "error message must mention non-text; got: {}",
        err.message
    );
}

// ── (f) Image column binding — two rows → two distinct PNGs ──────────────────

/// A template with a `role="data.photo"` image node receives two rows with two
/// distinct PNG files; both rows render successfully, and their output PNGs are
/// different (because they embed different images).
#[test]
fn image_column_two_rows_produce_distinct_pngs() {
    let project_dir = TempDir::new().expect("tempdir for project");
    let out_dir = TempDir::new().expect("tempdir for output");

    // Write two distinct PNG fixtures.  Different dimensions guarantee
    // different pixel data even though the renderer renders them into the same
    // box (the raster frame will differ).
    let img_a = write_test_png(&project_dir, "photo_a", 32, 32);
    let img_b = write_test_png(&project_dir, "photo_b", 64, 64);

    // Template: one image node with role="data.photo"; a template asset
    // ("asset.placeholder") is declared so the document parses cleanly.
    // The merge engine will AddAsset a per-row asset and SetAsset the image
    // node to point at it.
    let placeholder = write_test_png(&project_dir, "placeholder", 8, 8);
    let template = format!(
        r##"zenith version=1 {{
  project id="proj.imgmerge" name="Image Merge"
  assets {{
    asset id="asset.placeholder" kind="image" src="{placeholder}"
  }}
  tokens format="zenith-token-v1" {{
    token id="color.bg" type="color" value="#ffffff"
  }}
  styles {{}}
  document id="doc.imgmerge" title="Image Merge" {{
    page id="page.imgmerge" w=(px)200 h=(px)200 {{
      rect id="rect.bg" x=(px)0 y=(px)0 w=(px)200 h=(px)200 fill=(token)"color.bg"
      image id="img.photo" asset="asset.placeholder" x=(px)10 y=(px)10 w=(px)180 h=(px)180 fit="stretch" role="data.photo"
    }}
  }}
}}"##
    );

    let csv = format!("photo\n{img_a}\n{img_b}\n");
    let report = merge_run(
        &template,
        &csv,
        Some(project_dir.path()),
        out_dir.path(),
        None,
    )
    .expect("merge must succeed");

    let written = report.written();
    assert_eq!(
        written.len(),
        2,
        "two rows must produce two PNGs; failed: {:?}",
        report.rows
    );
    assert!(
        report.failed().is_empty(),
        "no rows should fail; got: {:?}",
        report.rows
    );

    // Both files must be valid PNGs.
    for name in &written {
        let path = out_dir.path().join(name);
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|e| panic!("could not read {}: {}", path.display(), e));
        assert!(
            bytes.len() >= 4 && &bytes[0..4] == b"\x89PNG",
            "{} must be a valid PNG",
            name
        );
    }

    // The two output PNGs must differ (different source images).
    let bytes_a = std::fs::read(out_dir.path().join(&written[0])).unwrap();
    let bytes_b = std::fs::read(out_dir.path().join(&written[1])).unwrap();
    assert_ne!(
        bytes_a, bytes_b,
        "rows with different images must produce different output PNGs"
    );
}

// ── (g) Missing image file → that row fails, others succeed ──────────────────

/// Row 0 has a valid image; row 1 references a path that does not exist on
/// disk.  Row 1 must appear in `failed` and its PNG must NOT be written; row 0
/// must still succeed.
#[test]
fn missing_image_file_fails_only_that_row() {
    let project_dir = TempDir::new().expect("tempdir for project");
    let out_dir = TempDir::new().expect("tempdir for output");

    let good_img = write_test_png(&project_dir, "good", 16, 16);
    let placeholder = write_test_png(&project_dir, "placeholder", 8, 8);

    let template = format!(
        r##"zenith version=1 {{
  project id="proj.missingimg" name="Missing Img"
  assets {{
    asset id="asset.placeholder" kind="image" src="{placeholder}"
  }}
  tokens format="zenith-token-v1" {{
    token id="color.bg" type="color" value="#ffffff"
  }}
  styles {{}}
  document id="doc.missingimg" title="Missing Img" {{
    page id="page.missingimg" w=(px)200 h=(px)200 {{
      rect id="rect.bg" x=(px)0 y=(px)0 w=(px)200 h=(px)200 fill=(token)"color.bg"
      image id="img.photo" asset="asset.placeholder" x=(px)10 y=(px)10 w=(px)180 h=(px)180 fit="stretch" role="data.photo"
    }}
  }}
}}"##
    );

    // Row 0: valid file.  Row 1: file does not exist on disk.
    let csv = format!("photo\n{good_img}\n__does_not_exist__.png\n");
    let report = merge_run(
        &template,
        &csv,
        Some(project_dir.path()),
        out_dir.path(),
        None,
    )
    .expect("merge run must not return Err (per-row failure is in report.failed)");

    // Row 0 must succeed.
    assert!(
        report.written().contains(&"row-0001.png".to_owned()),
        "row 0 (valid image) must succeed; written: {:?}",
        report.written()
    );

    // Row 1 must be in failed.
    let row1_failed = report.failed().iter().any(|r| r.row == 1);
    assert!(
        row1_failed,
        "row 1 (missing image) must be in failed; failed: {:?}",
        report
            .failed()
            .iter()
            .map(|r| (r.row, r.failure.as_deref().unwrap_or("")))
            .collect::<Vec<_>>()
    );

    // The failed row's reason must mention the missing asset.
    let row1_reason = report
        .failed()
        .into_iter()
        .find(|r| r.row == 1)
        .and_then(|r| r.failure.as_deref())
        .unwrap_or("");
    assert!(
        row1_reason.contains("asset.missing") || row1_reason.contains("not found"),
        "failure reason must mention asset.missing or not found; got: {}",
        row1_reason
    );

    // Row 1's PNG must NOT have been written.
    let row1_png = out_dir.path().join("row-0002.png");
    assert!(
        !row1_png.exists(),
        "row-0002.png must NOT have been written for the missing-image row"
    );
}

// ── (i) Two-page template × two CSV rows → four PNGs ─────────────────────────

/// Template with two pages, each carrying a `role="data.name"` text node in a
/// generously-sized box so any reasonable value fits without overflow.
const TWO_PAGE_TEMPLATE: &str = r##"zenith version=1 {
  project id="proj.twopage" name="Two Page"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#ffffff"
    token id="color.ink" type="color" value="#111111"
  }
  styles {}
  document id="doc.twopage" title="Two Page" {
    page id="page.one" w=(px)400 h=(px)200 {
      rect id="rect.bg1" x=(px)0 y=(px)0 w=(px)400 h=(px)200 fill=(token)"color.bg"
      text id="text.name1" x=(px)10 y=(px)10 w=(px)380 h=(px)180 fill=(token)"color.ink" role="data.name" {
        span "PLACEHOLDER"
      }
    }
    page id="page.two" w=(px)400 h=(px)200 {
      rect id="rect.bg2" x=(px)0 y=(px)0 w=(px)400 h=(px)200 fill=(token)"color.bg"
      text id="text.name2" x=(px)10 y=(px)10 w=(px)380 h=(px)180 fill=(token)"color.ink" role="data.name" {
        span "PLACEHOLDER"
      }
    }
  }
}
"##;

/// Template with two pages; page 2 has a `role="data.name"` text node with
/// `overflow="fit"` on a tiny box, so a long value on that page triggers
/// `text.fit_failed`.
const TWO_PAGE_OVERFLOW_FIT_TEMPLATE: &str = r##"zenith version=1 {
  project id="proj.twopagefit" name="Two Page Fit"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#ffffff"
    token id="color.ink" type="color" value="#111111"
  }
  styles {}
  document id="doc.twopagefit" title="Two Page Fit" {
    page id="page.one" w=(px)400 h=(px)200 {
      rect id="rect.bg1" x=(px)0 y=(px)0 w=(px)400 h=(px)200 fill=(token)"color.bg"
      text id="text.name1" x=(px)10 y=(px)10 w=(px)380 h=(px)180 fill=(token)"color.ink" role="data.name" {
        span "PLACEHOLDER"
      }
    }
    page id="page.two" w=(px)400 h=(px)200 {
      rect id="rect.bg2" x=(px)0 y=(px)0 w=(px)400 h=(px)200 fill=(token)"color.bg"
      text id="text.name2" x=(px)10 y=(px)10 w=(px)60 h=(px)40 overflow="fit" fill=(token)"color.ink" role="data.name" {
        span "X"
      }
    }
  }
}
"##;

#[test]
fn two_page_template_two_rows_produces_four_pngs() {
    let csv = "name\nAlice\nBob\n";
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let report =
        merge_run(TWO_PAGE_TEMPLATE, csv, None, tmp.path(), None).expect("merge must succeed");

    let written = report.written();
    assert_eq!(
        written.len(),
        4,
        "2 rows × 2 pages must produce 4 PNGs; failed: {:?}",
        report.rows
    );
    assert!(
        report.failed().is_empty(),
        "no rows should fail; got: {:?}",
        report.rows
    );

    // Names must follow the -page-N suffix convention in row-ascending order.
    assert_eq!(
        written,
        vec![
            "row-0001-page-1.png",
            "row-0001-page-2.png",
            "row-0002-page-1.png",
            "row-0002-page-2.png",
        ]
    );

    // Every file must start with PNG magic bytes.
    for name in &written {
        let path = tmp.path().join(name);
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|e| panic!("could not read {}: {}", path.display(), e));
        assert!(
            bytes.len() >= 4 && &bytes[0..4] == b"\x89PNG",
            "{} must be a valid PNG; got {} bytes",
            name,
            bytes.len()
        );
    }
}

// ── (j) Multi-page compile failure → whole row fails atomically ───────────────

#[test]
fn multipage_compile_failure_fails_whole_row() {
    // Row 0 has a short name that fits on both pages.
    // Row 1 has a very long name; page 2 has overflow="fit" on a 60×40 box,
    // so it cannot fit — triggering text.fit_failed.
    let csv = "name\nHi\nThe quick brown fox jumps over the lazy dog and keeps on going forever\n";
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let report = merge_run(TWO_PAGE_OVERFLOW_FIT_TEMPLATE, csv, None, tmp.path(), None)
        .expect("merge run itself must not error");

    // Row 0 (short name) must succeed with two pages.
    assert!(
        report.written().contains(&"row-0001-page-1.png".to_owned()),
        "row 0 page 1 must succeed; written: {:?}",
        report.written()
    );
    assert!(
        report.written().contains(&"row-0001-page-2.png".to_owned()),
        "row 0 page 2 must succeed; written: {:?}",
        report.written()
    );

    // Row 1 must appear in failed with the page number in the reason.
    let row1_failure = report.failed().into_iter().find(|r| r.row == 1);
    assert!(
        row1_failure.is_some(),
        "row 1 (long name) must be in failed; failed: {:?}",
        report
            .failed()
            .iter()
            .map(|r| (r.row, r.failure.as_deref().unwrap_or("")))
            .collect::<Vec<_>>()
    );
    let row1_reason = row1_failure.unwrap().failure.as_deref().unwrap_or("");
    assert!(
        row1_reason.contains("page 2"),
        "failure reason must mention 'page 2'; got: {}",
        row1_reason
    );

    // Row 1's pages must NOT have been written (atomic failure).
    let row1_p1 = tmp.path().join("row-0002-page-1.png");
    let row1_p2 = tmp.path().join("row-0002-page-2.png");
    assert!(
        !row1_p1.exists(),
        "row-0002-page-1.png must NOT have been written"
    );
    assert!(
        !row1_p2.exists(),
        "row-0002-page-2.png must NOT have been written"
    );
}

// ── (h) Empty image cell → template image used, row renders ──────────────────

/// When the image column cell is empty for a row, the template image is left
/// in place (no op is emitted for that node); the row must render successfully.
#[test]
fn empty_image_cell_uses_template_image_and_renders() {
    let project_dir = TempDir::new().expect("tempdir for project");
    let out_dir = TempDir::new().expect("tempdir for output");

    let placeholder = write_test_png(&project_dir, "placeholder", 8, 8);

    let template = format!(
        r##"zenith version=1 {{
  project id="proj.emptyimg" name="Empty Img"
  assets {{
    asset id="asset.placeholder" kind="image" src="{placeholder}"
  }}
  tokens format="zenith-token-v1" {{
    token id="color.bg" type="color" value="#ffffff"
  }}
  styles {{}}
  document id="doc.emptyimg" title="Empty Img" {{
    page id="page.emptyimg" w=(px)200 h=(px)200 {{
      rect id="rect.bg" x=(px)0 y=(px)0 w=(px)200 h=(px)200 fill=(token)"color.bg"
      image id="img.photo" asset="asset.placeholder" x=(px)10 y=(px)10 w=(px)180 h=(px)180 fit="stretch" role="data.photo"
    }}
  }}
}}"##
    );

    // Single row with an empty photo cell. A second (unbound) column keeps the
    // row from being read as a blank line (which the CSV reader would drop).
    let csv = "photo,note\n,placeholder-row\n";
    let report = merge_run(
        &template,
        csv,
        Some(project_dir.path()),
        out_dir.path(),
        None,
    )
    .expect("merge must succeed");

    let written = report.written();
    assert_eq!(
        written.len(),
        1,
        "one row with empty image cell must still produce one PNG; failed: {:?}",
        report.rows
    );
    assert!(
        report.failed().is_empty(),
        "empty image cell must not fail the row; got: {:?}",
        report.rows
    );

    // Output must be a valid PNG.
    let path = out_dir.path().join(&written[0]);
    let bytes =
        std::fs::read(&path).unwrap_or_else(|e| panic!("could not read {}: {}", path.display(), e));
    assert!(
        bytes.len() >= 4 && &bytes[0..4] == b"\x89PNG",
        "output must be a valid PNG"
    );
}

// ── (k) JSON batch report with mixed ok/failed rows ───────────────────────────

/// Two-row run using TWO_PAGE_OVERFLOW_FIT_TEMPLATE with --name-by name,
/// asserting the MergeReport structure and the written()/failed() accessors:
/// - row 0 ("alice", short name) succeeds and produces 2 pages.
/// - row 1 ("bob_long_name_overflow", very long) fails on page 2 of the fit box.
#[test]
fn json_batch_report_mixed_run() {
    // Row 0: short value fits on both pages.
    // Row 1: long value cannot fit the 60×40 overflow=fit box on page 2.
    let csv =
        "name\nalice\nThe quick brown fox jumps over the lazy dog and keeps on going forever\n";
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let report = merge_run(
        TWO_PAGE_OVERFLOW_FIT_TEMPLATE,
        csv,
        None,
        tmp.path(),
        Some("name"),
    )
    .expect("merge run itself must not error");

    // Two rows total.
    assert_eq!(report.rows.len(), 2, "must have exactly 2 row results");

    // Row 0: success, key == "alice", two output pages.
    let r0 = &report.rows[0];
    assert_eq!(r0.row, 0);
    assert_eq!(r0.key.as_deref(), Some("alice"));
    assert!(
        r0.failure.is_none(),
        "row 0 must succeed; failure: {:?}",
        r0.failure
    );
    assert_eq!(
        r0.outputs,
        vec!["alice-page-1.png", "alice-page-2.png"],
        "row 0 must produce two named pages"
    );

    // Row 1: failure, outputs empty.
    let r1 = &report.rows[1];
    assert_eq!(r1.row, 1);
    assert!(r1.failure.is_some(), "row 1 must fail");
    assert!(r1.outputs.is_empty(), "row 1 must have no outputs");

    // Accessors agree.
    assert_eq!(
        report.written().len(),
        2,
        "written() must return row-0's two pages"
    );
    assert_eq!(
        report.failed().len(),
        1,
        "failed() must return exactly row 1"
    );

    // Verify the two page files exist on disk.
    for fname in &r0.outputs {
        let path = tmp.path().join(fname);
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|e| panic!("could not read {}: {}", path.display(), e));
        assert!(
            bytes.len() >= 4 && &bytes[0..4] == b"\x89PNG",
            "{} must be a valid PNG",
            fname
        );
    }

    // Row 1's pages must NOT exist on disk (atomic failure).
    assert!(
        !tmp.path()
            .join(
                "The quick brown fox jumps over the lazy dog and keeps on going forever-page-1.png"
            )
            .exists(),
        "row 1 page 1 must not have been written"
    );
}

// ── (l) Generation manifest — deterministic and structurally correct ──────────

/// Build the manifest twice from the same inputs and assert field-by-field
/// equality (reproducibility), correct sha256 format, correct structural
/// fields, and that only successful rows appear.
///
/// Note: `serde_json` and `sha2` are direct dependencies of zenith-cli but are
/// not listed as dev-dependencies for the integration test crate, so they are
/// not in scope here.  We verify reproducibility by field-equality of two
/// independently-built manifests and hash format (64 lowercase hex chars)
/// rather than by re-computing the hash independently.
#[test]
fn manifest_is_deterministic_and_correct() {
    let csv = "name,title\nAlice,Engineer\nBob,Designer\n";
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let report =
        merge_run(TEMPLATE_DOC, csv, None, tmp.path(), Some("name")).expect("merge must succeed");

    // Build twice — all fields must match (reproducibility).
    let m1 = zenith_cli::commands::merge::build_manifest(TEMPLATE_DOC, csv, Some("name"), &report);
    let m2 = zenith_cli::commands::merge::build_manifest(TEMPLATE_DOC, csv, Some("name"), &report);

    assert_eq!(
        m1.source_sha256, m2.source_sha256,
        "source_sha256 must be identical across two calls with the same input"
    );
    assert_eq!(
        m1.data_sha256, m2.data_sha256,
        "data_sha256 must be identical across two calls with the same input"
    );
    assert_eq!(m1.schema, m2.schema);
    assert_eq!(m1.generator, m2.generator);
    assert_eq!(m1.name_by, m2.name_by);
    assert_eq!(m1.rows.len(), m2.rows.len());

    // Hashes must be 64-char lowercase hex (SHA-256 output).
    assert_eq!(
        m1.source_sha256.len(),
        64,
        "source_sha256 must be a 64-char hex string"
    );
    assert!(
        m1.source_sha256
            .chars()
            .all(|c| matches!(c, '0'..='9' | 'a'..='f')),
        "source_sha256 must be lowercase hex; got: {}",
        m1.source_sha256
    );
    assert_eq!(
        m1.data_sha256.len(),
        64,
        "data_sha256 must be a 64-char hex string"
    );
    assert!(
        m1.data_sha256
            .chars()
            .all(|c| matches!(c, '0'..='9' | 'a'..='f')),
        "data_sha256 must be lowercase hex; got: {}",
        m1.data_sha256
    );

    // Different inputs must produce different hashes.
    let m_other = zenith_cli::commands::merge::build_manifest(
        TEMPLATE_DOC,
        "name,title\nCarol,PM\n",
        Some("name"),
        &report,
    );
    assert_ne!(
        m1.data_sha256, m_other.data_sha256,
        "different CSV inputs must produce different data_sha256"
    );

    // Schema and name_by.
    assert_eq!(m1.schema, "zenith-merge-manifest-v1");
    assert_eq!(m1.name_by, Some("name".to_owned()));

    // Only successful rows are included (both rows succeed here).
    assert_eq!(m1.rows.len(), 2, "both rows must appear in the manifest");

    // Row 0: key == "Alice", outputs == ["Alice.png"].
    assert_eq!(m1.rows[0].row, 0);
    assert_eq!(m1.rows[0].key.as_deref(), Some("Alice"));
    assert_eq!(m1.rows[0].outputs, vec!["Alice.png"]);

    // Row 1: key == "Bob", outputs == ["Bob.png"].
    assert_eq!(m1.rows[1].row, 1);
    assert_eq!(m1.rows[1].key.as_deref(), Some("Bob"));
    assert_eq!(m1.rows[1].outputs, vec!["Bob.png"]);

    // A failed row must NOT appear in the manifest.
    // Use the mixed-result template to produce one failure.
    let csv_mixed =
        "name\nalice\nThe quick brown fox jumps over the lazy dog and keeps on going forever\n";
    let tmp2 = tempfile::TempDir::new().expect("tempdir2");
    let report_mixed = merge_run(
        TWO_PAGE_OVERFLOW_FIT_TEMPLATE,
        csv_mixed,
        None,
        tmp2.path(),
        None,
    )
    .expect("merge run must not error");
    assert_eq!(
        report_mixed.failed().len(),
        1,
        "sanity: one row must fail in the mixed run"
    );
    let m_mixed = zenith_cli::commands::merge::build_manifest(
        TWO_PAGE_OVERFLOW_FIT_TEMPLATE,
        csv_mixed,
        None,
        &report_mixed,
    );
    assert_eq!(
        m_mixed.rows.len(),
        1,
        "only the successful row must appear in the manifest"
    );
    assert_eq!(m_mixed.rows[0].row, 0, "the successful row must be row 0");
    assert_eq!(
        m_mixed.name_by, None,
        "name_by must be None when not supplied"
    );
}

#[test]
fn explicit_png_matches_default_bytes_and_manifest() {
    use zenith_cli::commands::render::BatchFormat;
    let first = TempDir::new().unwrap();
    let second = TempDir::new().unwrap();
    let csv = "name,title\nAlice,Engineer\nBob,Designer\n";
    let default = merge_run(TEMPLATE_DOC, csv, None, first.path(), None).unwrap();
    let explicit = zenith_cli::commands::merge::run_with_format(
        TEMPLATE_DOC,
        csv,
        None,
        second.path(),
        None,
        BatchFormat::Png,
    )
    .unwrap();
    assert_eq!(
        serde_json::to_vec(&build_manifest(TEMPLATE_DOC, csv, None, &default)).unwrap(),
        serde_json::to_vec(&build_manifest(TEMPLATE_DOC, csv, None, &explicit)).unwrap()
    );
    for name in default.written() {
        assert_eq!(
            std::fs::read(first.path().join(&name)).unwrap(),
            std::fs::read(second.path().join(&name)).unwrap()
        );
    }
}

#[test]
fn svg_merge_keeps_multipage_names_determinism_and_collisions() {
    use zenith_cli::commands::render::BatchFormat;
    let first = TempDir::new().unwrap();
    let second = TempDir::new().unwrap();
    let csv = "name\nAlice\nAlice\nBob\n";
    let generate = |dir| {
        zenith_cli::commands::merge::run_with_format(
            TWO_PAGE_TEMPLATE,
            csv,
            None,
            dir,
            Some("name"),
            BatchFormat::Svg,
        )
        .unwrap()
    };
    let report = generate(first.path());
    let again = generate(second.path());
    assert_eq!(
        report.written(),
        [
            "Alice-page-1.svg",
            "Alice-page-2.svg",
            "Bob-page-1.svg",
            "Bob-page-2.svg"
        ]
    );
    assert_eq!(report.failed().len(), 1);
    assert!(report.rows[1].outputs.is_empty());
    let manifest = serde_json::to_vec(&build_manifest(
        TWO_PAGE_TEMPLATE,
        csv,
        Some("name"),
        &report,
    ))
    .unwrap();
    assert_eq!(
        manifest,
        serde_json::to_vec(&build_manifest(
            TWO_PAGE_TEMPLATE,
            csv,
            Some("name"),
            &again
        ))
        .unwrap()
    );
    for name in report.written() {
        let bytes = std::fs::read(first.path().join(&name)).unwrap();
        assert_eq!(bytes, std::fs::read(second.path().join(&name)).unwrap());
        assert!(String::from_utf8(bytes).unwrap().contains("<svg"));
    }
    let json = serde_json::to_value(to_json_output(&report)).unwrap();
    assert_eq!(json["rows"][0]["outputs"][0], "Alice-page-1.svg");
}

#[test]
fn svg_merge_embeds_row_scoped_image_assets() {
    use zenith_cli::commands::render::BatchFormat;
    let project = TempDir::new().unwrap();
    let out = TempDir::new().unwrap();
    let first = write_test_png(&project, "first", 8, 8);
    let second = write_test_png(&project, "second", 16, 16);
    let doc = format!(
        r##"zenith version=1 {{
      project id="project" name="Images"
      assets {{ asset id="photo" kind="image" src="{first}"; }}
      tokens format="zenith-token-v1" {{}}
      styles {{}}
      document id="document" title="Images" {{
        page id="page" w=(px)40 h=(px)40 {{
          image id="image" asset="photo" x=(px)0 y=(px)0 w=(px)40 h=(px)40 fit="stretch" role="data.photo"
        }}
      }}
    }}"##
    );
    let report = zenith_cli::commands::merge::run_with_format(
        &doc,
        &format!("photo\n{first}\n{second}\n"),
        Some(project.path()),
        out.path(),
        None,
        BatchFormat::Svg,
    )
    .unwrap();
    assert_eq!(report.written(), ["row-0001.svg", "row-0002.svg"]);
    let first = std::fs::read_to_string(out.path().join("row-0001.svg")).unwrap();
    let second = std::fs::read_to_string(out.path().join("row-0002.svg")).unwrap();
    assert!(first.contains("data:image/png;base64,"));
    assert!(second.contains("data:image/png;base64,"));
    assert_ne!(first, second);
}
