//! End-to-end integration tests for `zenith merge`.
//!
//! Calls [`zenith_cli::commands::merge::run`] directly with inline source
//! strings and a [`tempfile::TempDir`] as the output directory, following the
//! pattern established by `tx_pipeline.rs`.

use zenith_cli::commands::merge::run as merge_run;

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

    assert_eq!(report.written.len(), 2, "two rows must produce two PNGs");
    assert!(
        report.failed.is_empty(),
        "no rows should fail; got: {:?}",
        report.failed
    );

    // Default names must be row-0001.png and row-0002.png.
    assert_eq!(report.written[0], "row-0001.png");
    assert_eq!(report.written[1], "row-0002.png");

    // Both files must start with the PNG magic bytes.
    for name in &report.written {
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

    assert_eq!(report.written.len(), 2);
    assert!(
        report.failed.is_empty(),
        "no rows should fail; got: {:?}",
        report.failed
    );

    // Names come from the `name` column, sanitized, with .png extension.
    assert_eq!(report.written[0], "Alice.png");
    assert_eq!(report.written[1], "Bob.png");

    for name in &report.written {
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
        report.written.contains(&"row-0001.png".to_owned()),
        "short value must succeed; written: {:?}",
        report.written
    );

    // Row 1 (long value) must appear in failed with no PNG on disk.
    let row1_failed = report.failed.iter().any(|f| f.row == 1);
    assert!(
        row1_failed,
        "long-value row must be in failed; failed: {:?}",
        report
            .failed
            .iter()
            .map(|f| (f.row, &f.reason))
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
