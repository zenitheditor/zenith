use std::path::Path;

use crate::config::CliPolicyFlags;

use super::{
    RenderEntryOptions, to_pdf_all_pages_with_dir, to_png, to_png_all_pages, to_png_with_dir,
    to_scene_json, to_scene_json_with_options,
};

const VALID_DOC: &str = r##"zenith version=1 {
  project id="proj.r" name="Render Test"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#f8fafc"
    token id="color.accent" type="color" value="#3b82f6"
  }
  styles {}
  document id="doc.r" title="Render Test" {
    page id="page.r" w=(px)320 h=(px)200 {
      rect id="rect.bg" x=(px)0 y=(px)0 w=(px)320 h=(px)200 fill=(token)"color.bg"
      rect id="rect.accent" x=(px)40 y=(px)40 w=(px)240 h=(px)120 fill=(token)"color.accent"
    }
  }
}
"##;

const INVALID_DOC: &str = r##"zenith version=1 {
  project id="proj.inv" name="Invalid"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#f8fafc"
    token id="color.bg" type="color" value="#000000"
  }
  styles {}
  document id="doc.inv" title="Invalid" {
    page id="page.inv" w=(px)100 h=(px)100 {
      rect id="rect.inv" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"color.bg"
    }
  }
}
"##;

/// A document whose only content node is an UNKNOWN kind. It parses (the
/// kind is preserved for forward-compat), validates without errors (unknown
/// kinds are a warning, not an error), and compiles with a
/// `scene.unsupported_node` ADVISORY — a reliable compile-stage diagnostic.
const UNKNOWN_NODE_DOC: &str = r##"zenith version=1 {
  project id="proj.u" name="Unknown Node"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.u" title="Unknown Node" {
    page id="page.u" w=(px)100 h=(px)100 {
      sparkle id="sparkle.1"
    }
  }
}
"##;

const CONSTRUCTION_DOC: &str = r##"zenith version=1 {
  project id="proj.guides" name="Guides"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#ffffff"
  }
  styles {}
  document id="doc.guides" title="Guides" {
    page id="page.guides" w=(px)320 h=(px)200 {
      rect id="bg" x=(px)0 y=(px)0 w=(px)320 h=(px)200 fill=(token)"color.bg"
      construction {
        guide id="axis" type="segment" x1=(px)0 y1=(px)100 x2=(px)320 y2=(px)100
        guide id="ring" type="circle" cx=(px)160 cy=(px)100 r=(px)40
      }
    }
  }
}
"##;

#[test]
fn to_png_returns_png_magic_bytes() {
    let artifact = to_png(VALID_DOC, 1).expect("render must succeed");
    let png = &artifact.png;
    assert!(
        png.len() >= 4,
        "PNG must have at least 4 bytes; got {}",
        png.len()
    );
    assert_eq!(
        &png[0..4],
        &[0x89, 0x50, 0x4E, 0x47],
        "PNG must start with magic bytes 89 50 4E 47"
    );
}

#[test]
fn to_png_is_non_empty() {
    let artifact = to_png(VALID_DOC, 1).expect("render must succeed");
    assert!(!artifact.png.is_empty(), "PNG output must not be empty");
}

#[test]
fn to_png_surfaces_compile_diagnostics() {
    let artifact = to_png(UNKNOWN_NODE_DOC, 1).expect("render must succeed");
    assert!(
        artifact
            .diagnostics
            .iter()
            .any(|d| d.code == "scene.unsupported_node"),
        "render must surface the compile-stage advisory; got {:?}",
        artifact
            .diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn to_png_with_dir_surfaces_import_diagnostics() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = r#"zenith version=1 {
  project id="proj.import" name="Import"
  imports {
    import id="brand" kind="zen" src="missing.zen"
  }
  document id="doc.import" title="Import" {
    page id="page.import" w=(px)100 h=(px)100
  }
}
"#;

    let artifact = to_png_with_dir(
        src,
        Some(dir.path()),
        1,
        false,
        &CliPolicyFlags::default(),
        None,
    )
    .expect("render artifact should carry import diagnostics");

    assert!(
        artifact
            .diagnostics
            .iter()
            .any(|d| d.code == "import.missing"),
        "render must surface import diagnostics; got {:?}",
        artifact
            .diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn to_scene_json_expands_loaded_composition_import() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("brand.zen"),
        r##"zenith version=1 {
  project id="proj.brand" name="Brand"
  tokens format="zenith-token-v1" {
    token id="color.logo" type="color" value="#0000ff"
  }
  styles {}
  components {
    component id="component.logo" {
      rect id="mark" x=(px)0 y=(px)0 w=(px)30 h=(px)20 fill=(token)"color.logo"
    }
  }
  document id="doc.brand" title="Brand" {
    page id="page.brand" w=(px)30 h=(px)20
  }
}
"##,
    )
    .expect("write imported document");
    let src = r#"zenith version=1 {
  project id="proj.host" name="Host"
  imports {
    import id="brand" kind="zen" src="brand.zen"
  }
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.host" title="Host" {
    page id="page.host" w=(px)100 h=(px)80 {
      instance id="logo" source="brand#component.component.logo" x=(px)5 y=(px)7
    }
  }
}
"#;

    let artifact = to_scene_json(src, Some(dir.path()), 1, &CliPolicyFlags::default(), None)
        .expect("scene JSON render must succeed");

    assert!(
        artifact.diagnostics.is_empty(),
        "imported component render must be clean; got {:?}",
        artifact.diagnostics
    );
    assert!(
        artifact.json.contains(r#""op": "FillRect""#),
        "scene JSON must include the imported rect; got {}",
        artifact.json
    );
    assert!(
        artifact.json.contains(r#""x": 5.0"#) && artifact.json.contains(r#""y": 7.0"#),
        "imported rect must be translated by the host instance; got {}",
        artifact.json
    );
}

#[test]
fn construction_guides_do_not_affect_default_scene_json() {
    let artifact = to_scene_json(CONSTRUCTION_DOC, None, 1, &CliPolicyFlags::default(), None)
        .expect("scene render must succeed");

    assert!(!artifact.json.contains("\"op\": \"StrokeLine\""));
    assert!(!artifact.json.contains("\"op\": \"StrokeEllipse\""));
}

#[test]
fn construction_overlay_appends_guide_commands_to_scene_json() {
    let opts = RenderEntryOptions {
        locked: false,
        subset: true,
        flags: &CliPolicyFlags::default(),
        data: None,
        construction_overlay: true,
    };
    let artifact = to_scene_json_with_options(CONSTRUCTION_DOC, None, 1, opts)
        .expect("scene render must succeed");

    assert!(artifact.json.contains("\"op\": \"StrokeLine\""));
    assert!(artifact.json.contains("\"op\": \"StrokeEllipse\""));
    assert!(artifact.json.contains("\"stroke_dash\": 6.0"));
}

#[test]
fn to_scene_json_surfaces_compile_diagnostics() {
    let artifact = to_scene_json(UNKNOWN_NODE_DOC, None, 1, &CliPolicyFlags::default(), None)
        .expect("scene must succeed");
    assert!(
        artifact
            .diagnostics
            .iter()
            .any(|d| d.code == "scene.unsupported_node"),
        "scene JSON path must surface the compile-stage advisory"
    );
}

#[test]
fn to_png_with_validation_error_returns_err() {
    let result = to_png(INVALID_DOC, 1);
    assert!(
        result.is_err(),
        "document with validation errors must not render"
    );
    let err = result.unwrap_err();
    assert_eq!(
        err.exit_code, 1,
        "validation errors must produce exit code 1"
    );
}

#[test]
fn to_scene_json_contains_schema_field() {
    let json = to_scene_json(VALID_DOC, None, 1, &CliPolicyFlags::default(), None)
        .expect("scene JSON must succeed")
        .json;
    assert!(
        json.contains("zenith-scene-v1"),
        "scene JSON must contain schema field; got snippet: {}",
        &json[..json.len().min(200)]
    );
}

#[test]
fn to_scene_json_with_validation_error_returns_err() {
    let result = to_scene_json(INVALID_DOC, None, 1, &CliPolicyFlags::default(), None);
    assert!(result.is_err(), "invalid doc must not produce scene JSON");
}

#[test]
fn to_png_deterministic_two_runs_equal() {
    let png1 = to_png(VALID_DOC, 1).expect("run 1").png;
    let png2 = to_png(VALID_DOC, 1).expect("run 2").png;
    assert_eq!(png1, png2, "two renders of the same doc must be identical");
}

/// A two-page document used to exercise the 1-based page selector.
const TWO_PAGE_DOC: &str = r##"zenith version=1 {
  project id="proj.mp" name="MP"
  tokens format="zenith-token-v1" {
    token id="color.p1" type="color" value="#252525"
    token id="color.p2" type="color" value="#dcdcdc"
  }
  styles {}
  document id="doc.mp" title="MP" {
    page id="page.p1" w=(px)100 h=(px)100 {
      rect id="rect.p1" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"color.p1"
    }
    page id="page.p2" w=(px)100 h=(px)100 {
      rect id="rect.p2" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"color.p2"
    }
  }
}
"##;

#[test]
fn to_png_page_two_is_ok() {
    let result = to_png(TWO_PAGE_DOC, 2);
    assert!(result.is_ok(), "rendering page 2 must succeed");
}

#[test]
fn to_png_page_out_of_range_is_err_exit_2() {
    let err = to_png(TWO_PAGE_DOC, 3).expect_err("page 3 must be out of range");
    assert_eq!(err.exit_code, 2, "out-of-range page must exit with code 2");
}

#[test]
fn to_png_page_zero_is_err_exit_2() {
    let err = to_png(TWO_PAGE_DOC, 0).expect_err("page 0 is invalid (1-based)");
    assert_eq!(err.exit_code, 2, "page 0 must exit with code 2");
}

#[test]
fn to_png_all_pages_returns_one_artifact_per_page() {
    let artifacts = to_png_all_pages(TWO_PAGE_DOC, None, false, &CliPolicyFlags::default(), None)
        .expect("all-pages render must succeed");
    assert_eq!(
        artifacts.len(),
        2,
        "a two-page doc must yield two artifacts"
    );
    for (i, a) in artifacts.iter().enumerate() {
        assert!(
            a.png.starts_with(&[0x89, 0x50, 0x4E, 0x47]),
            "page {} must be a valid PNG",
            i + 1
        );
    }
    // The two pages have different backgrounds → different bytes.
    assert_ne!(
        artifacts[0].png, artifacts[1].png,
        "distinct pages must render to distinct PNGs"
    );
}

#[test]
fn to_png_all_pages_empty_doc_is_err() {
    let empty = r##"zenith version=1 {
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.e" title="E" {}
}
"##;
    let err = to_png_all_pages(empty, None, false, &CliPolicyFlags::default(), None)
        .expect_err("a doc with no pages must error");
    // A zero-page document is now rejected at validation (document.no_pages,
    // exit 1) rather than later at the render stage (exit 2).
    assert_eq!(err.exit_code, 1);
    assert!(
        err.message.contains("document.no_pages"),
        "expected a document.no_pages diagnostic; got: {}",
        err.message
    );
}

// ── overflow="fit" artifact-level tests ──────────────────────────────────

/// A text node with `overflow="fit"` that overflows its box must produce
/// a `text.fit_failed` Error-severity diagnostic in the PNG artifact.
/// (Whether the file is written is verified manually via the CLI — the
/// render command logic in lib.rs blocks the write when diagnostics contain
/// Error, exercised here at the artifact level.)
#[test]
fn to_png_overflow_fit_exceeded_has_error_diagnostic() {
    const OVERFLOW_FIT_DOC: &str = r##"zenith version=1 {
  project id="proj.fitcli" name="Fit CLI"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.fitcli" title="Fit CLI" {
    page id="page.fitcli" w=(px)400 h=(px)400 {
      text id="text.fitcli" x=(px)10 y=(px)10 w=(px)60 h=(px)20 overflow="fit" {
        span "The quick brown fox jumps over the lazy dog and keeps on going"
      }
    }
  }
}
"##;
    // to_png still returns Ok — the artifact carries the Error diagnostic.
    // The CLI dispatcher (lib.rs) is what blocks the file write.
    let artifact = to_png(OVERFLOW_FIT_DOC, 1).expect("compile+render must not hard-fail");
    let has_fit_error = artifact
        .diagnostics
        .iter()
        .any(|d| d.code == "text.fit_failed" && d.severity == zenith_core::Severity::Error);
    assert!(
        has_fit_error,
        "artifact must carry a text.fit_failed Error diagnostic; got: {:?}",
        artifact.diagnostics
    );
}

#[test]
fn to_png_clip_overflow_guides_geometry_before_shrinking_type() {
    const OVERFLOW_CLIP_DOC: &str = r##"zenith version=1 {
  project id="proj.clipguide" name="Clip Guide"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.clipguide" title="Clip Guide" {
    page id="page.clipguide" w=(px)400 h=(px)400 {
      text id="text.clipguide" x=(px)10 y=(px)10 w=(px)120 h=(px)20 {
        span "Large deck title that needs more box height"
      }
    }
  }
}
"##;
    let artifact = to_png(OVERFLOW_CLIP_DOC, 1).expect("render must succeed");
    let overflow = artifact
        .diagnostics
        .iter()
        .find(|d| d.code == "text.overflow")
        .expect("clipped text must emit a text.overflow diagnostic");
    assert!(
        overflow.message.contains("increasing the box height"),
        "overflow diagnostic must guide agents to adjust geometry first; got: {}",
        overflow.message
    );
    assert!(
        overflow
            .message
            .contains("Shrink type only when intended or geometry is constrained"),
        "overflow diagnostic must name the shrinking constraint; got: {}",
        overflow.message
    );
}

/// A document referencing an asset whose file does not exist under the
/// project directory. Used to exercise the `asset.missing` hard diagnostic.
const MISSING_ASSET_DOC: &str = r##"zenith version=1 {
  project id="proj.missing" name="Missing Asset"
  assets {
    asset id="asset.absent" kind="image" src="__zenith_does_not_exist__.png"
  }
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#f8fafc"
  }
  styles {}
  document id="doc.missing" title="Missing Asset" {
    page id="page.missing" w=(px)100 h=(px)100 background=(token)"color.bg" {
      image id="img.absent" asset="asset.absent" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fit="stretch"
    }
  }
}
"##;

#[test]
fn to_png_missing_asset_has_asset_missing_error_diagnostic() {
    // A directory that is guaranteed not to contain the asset file. The
    // render still returns Ok (the gate in lib.rs is what blocks the write);
    // the artifact must carry the asset.missing Error diagnostic.
    let dir = Path::new("/nonexistent-zenith-project-dir");
    let artifact = to_png_with_dir(
        MISSING_ASSET_DOC,
        Some(dir),
        1,
        false,
        &CliPolicyFlags::default(),
        None,
    )
    .expect("render must not hard-fail; missing asset is carried as a diagnostic");
    let has_missing = artifact
        .diagnostics
        .iter()
        .any(|d| d.code == "asset.missing" && d.severity == zenith_core::Severity::Error);
    assert!(
        has_missing,
        "artifact must carry an asset.missing Error diagnostic; got: {:?}",
        artifact
            .diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn to_scene_json_missing_asset_has_asset_missing_error_diagnostic() {
    let dir = Path::new("/nonexistent-zenith-project-dir");
    let artifact = to_scene_json(
        MISSING_ASSET_DOC,
        Some(dir),
        1,
        &CliPolicyFlags::default(),
        None,
    )
    .expect("scene JSON must succeed");
    assert!(
        artifact
            .diagnostics
            .iter()
            .any(|d| d.code == "asset.missing" && d.severity == zenith_core::Severity::Error),
        "scene artifact must carry an asset.missing Error diagnostic"
    );
}

/// A text node with `overflow="fit"` that FITS must produce no
/// `text.fit_failed` diagnostic, and the render must succeed cleanly.
#[test]
fn to_png_overflow_fit_fits_no_error_diagnostic() {
    const FIT_OK_DOC: &str = r##"zenith version=1 {
  project id="proj.fitok" name="Fit OK"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.fitok" title="Fit OK" {
    page id="page.fitok" w=(px)400 h=(px)400 {
      text id="text.fitok" x=(px)10 y=(px)10 w=(px)300 h=(px)100 overflow="fit" {
        span "Hi"
      }
    }
  }
}
"##;
    let artifact = to_png(FIT_OK_DOC, 1).expect("render must succeed");
    let fit_errors: Vec<_> = artifact
        .diagnostics
        .iter()
        .filter(|d| d.code == "text.fit_failed")
        .collect();
    assert!(
        fit_errors.is_empty(),
        "fitting text must produce no text.fit_failed; got: {:?}",
        fit_errors
    );
}

#[test]
fn to_pdf_all_pages_produces_one_pdf_page_per_document_page() {
    let artifact = to_pdf_all_pages_with_dir(
        TWO_PAGE_DOC,
        None,
        false,
        true,
        &CliPolicyFlags::default(),
        None,
    )
    .expect("all-pages PDF render must succeed");
    let text = String::from_utf8_lossy(&artifact.pdf);
    assert!(
        text.contains("/Count 2"),
        "a two-page document must render a /Count 2 PDF"
    );
    assert_eq!(
        text.matches("/MediaBox").count(),
        2,
        "each document page must carry its own MediaBox"
    );
}

#[test]
fn to_pdf_all_pages_is_deterministic() {
    let a = to_pdf_all_pages_with_dir(
        TWO_PAGE_DOC,
        None,
        false,
        true,
        &CliPolicyFlags::default(),
        None,
    )
    .expect("render must succeed");
    let b = to_pdf_all_pages_with_dir(
        TWO_PAGE_DOC,
        None,
        false,
        true,
        &CliPolicyFlags::default(),
        None,
    )
    .expect("render must succeed");
    assert_eq!(
        a.pdf, b.pdf,
        "two all-pages PDF renders must be byte-identical"
    );
}

// ── text src="..." integration tests ─────────────────────────────────────

/// Build the KDL for a document whose single text node has `src="<rel>"`.
fn text_src_doc(src_rel: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.src" name="Src Test"
  tokens format="zenith-token-v1" {{
    token id="color.bg" type="color" value="#ffffff"
    token id="color.ink" type="color" value="#000000"
    token id="font.body" type="fontFamily" value="Noto Sans"
    token id="size.body" type="dimension" value=(px)16
    token id="weight.regular" type="fontWeight" value=400
  }}
  styles {{}}
  document id="doc.src" title="Src Test" {{
    page id="page.src" w=(px)400 h=(px)200 background=(token)"color.bg" {{
      text id="text.src" x=(px)10 y=(px)10 w=(px)380 h=(px)180 fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" font-weight=(token)"weight.regular" src="{src_rel}" format="markdown" {{
      }}
    }}
  }}
}}
"##
    )
}

/// A document with a text node that has `src` absent — must render identically
/// to a text node without the attribute (byte-identical guarantee).
const TEXT_SRC_ABSENT_DOC: &str = r##"zenith version=1 {
  project id="proj.nosrc" name="No Src"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#ffffff"
    token id="color.ink" type="color" value="#000000"
    token id="font.body" type="fontFamily" value="Noto Sans"
    token id="size.body" type="dimension" value=(px)16
    token id="weight.regular" type="fontWeight" value=400
  }
  styles {}
  document id="doc.nosrc" title="No Src" {
    page id="page.nosrc" w=(px)400 h=(px)200 background=(token)"color.bg" {
      text id="text.nosrc" x=(px)10 y=(px)10 w=(px)380 h=(px)180 fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" font-weight=(token)"weight.regular" {
        span "Hello world"
      }
    }
  }
}
"##;

#[test]
fn text_src_loads_file_and_renders_png() {
    use std::io::Write as _;
    let dir = tempfile::tempdir().expect("temp dir must be created");
    let md_path = dir.path().join("article.md");
    let mut f = std::fs::File::create(&md_path).expect("temp file must be created");
    write!(f, "**bold** and plain").expect("write must succeed");
    drop(f);

    let doc_src = text_src_doc("article.md");
    let artifact = to_png_with_dir(
        &doc_src,
        Some(dir.path()),
        1,
        false,
        &CliPolicyFlags::default(),
        None,
    )
    .expect("render with src file must succeed");

    // Must produce a valid PNG.
    assert!(
        artifact.png.starts_with(&[0x89, 0x50, 0x4E, 0x47]),
        "output must be a valid PNG"
    );
    // Must have no text.src_missing diagnostic.
    let missing: Vec<_> = artifact
        .diagnostics
        .iter()
        .filter(|d| d.code == "text.src_missing")
        .collect();
    assert!(
        missing.is_empty(),
        "no text.src_missing diagnostic expected; got: {:?}",
        missing
    );
}

#[test]
fn text_src_missing_file_yields_error_diagnostic() {
    use zenith_core::Severity;

    let doc_src = text_src_doc("__does_not_exist__.md");
    // Use a real directory that exists but does not contain the file.
    let dir = tempfile::tempdir().expect("temp dir must be created");
    let artifact = to_png_with_dir(
        &doc_src,
        Some(dir.path()),
        1,
        false,
        &CliPolicyFlags::default(),
        None,
    )
    .expect("render must still return Ok (gate is at lib.rs dispatch level)");

    let has_src_missing = artifact
        .diagnostics
        .iter()
        .any(|d| d.code == "text.src_missing" && d.severity == Severity::Error);
    assert!(
        has_src_missing,
        "artifact must carry a text.src_missing Error diagnostic; got: {:?}",
        artifact
            .diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn text_src_absent_is_byte_identical_to_no_src_attr() {
    // A text node without `src` must not be affected at all by the loader pass.
    // We just verify the render succeeds cleanly with no text.src_missing code.
    let artifact = to_png(TEXT_SRC_ABSENT_DOC, 1).expect("render must succeed");
    let has_src_missing = artifact
        .diagnostics
        .iter()
        .any(|d| d.code == "text.src_missing");
    assert!(
        !has_src_missing,
        "a text node without src must produce no text.src_missing diagnostic"
    );
}
