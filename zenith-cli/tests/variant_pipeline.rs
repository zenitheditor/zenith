//! End-to-end integration tests for `zenith variant`.
//!
//! Calls [`zenith_cli::commands::variant::run_variant`] directly with inline
//! source strings and a [`tempfile::TempDir`] as the output directory,
//! following the pattern established by `merge_pipeline.rs`.
//!
//! Note: `serde_json` and `sha2` are direct dependencies of zenith-cli but are
//! not listed as dev-dependencies for the integration test crate, so they are
//! not in scope here.  We verify manifest reproducibility by field-equality of
//! two independently-built manifests rather than re-serialising to JSON bytes.

use zenith_cli::commands::variant::{build_manifest, run_variant, to_json_output};

// ── Fixtures ──────────────────────────────────────────────────────────────────

/// A document with two variants:
/// - `var.large` → 1920×1080, overrides `text.label` text.
/// - `var.small` → 320×180, hides `rect.bg`.
const DOC_TWO_VARIANTS: &str = r##"zenith version=1 {
  project id="proj.v" name="Variant Test"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#ffffff"
    token id="color.ink" type="color" value="#111111"
  }
  styles {}
  document id="doc.v" title="Variant Test" {
    page id="page.a" w=(px)800 h=(px)600 {
      rect id="rect.bg" x=(px)0 y=(px)0 w=(px)800 h=(px)600 fill=(token)"color.bg"
      text id="text.label" x=(px)10 y=(px)10 w=(px)780 h=(px)80 fill=(token)"color.ink" {
        span "original text"
      }
    }
  }
  variants {
    variant id="var.large" source="page.a" w=(px)1920 h=(px)1080 {
      override node="text.label" text="large variant"
    }
    variant id="var.small" source="page.a" w=(px)320 h=(px)180 {
      override node="rect.bg" visible=#false
    }
  }
}
"##;

/// A document whose single variant overrides a node that does NOT exist — used
/// to assert that the failed variant is recorded without aborting the sibling.
const DOC_MISSING_NODE_VARIANT: &str = r##"zenith version=1 {
  project id="proj.mv" name="Missing Node Test"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#ffffff"
  }
  styles {}
  document id="doc.mv" title="Missing Node Test" {
    page id="page.m" w=(px)400 h=(px)300 {
      rect id="rect.only" x=(px)0 y=(px)0 w=(px)400 h=(px)300 fill=(token)"color.bg"
    }
  }
  variants {
    variant id="var.bad" source="page.m" w=(px)800 h=(px)600 {
      override node="node.does.not.exist" visible=#false
    }
    variant id="var.good" source="page.m" w=(px)200 h=(px)150 {
    }
  }
}
"##;

/// A document with no variants block — exercises the empty-expansion path.
const DOC_NO_VARIANTS: &str = r##"zenith version=1 {
  project id="proj.nv" name="No Variants"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#ffffff"
  }
  styles {}
  document id="doc.nv" title="No Variants" {
    page id="page.nv" w=(px)400 h=(px)300 {
      rect id="rect.bg" x=(px)0 y=(px)0 w=(px)400 h=(px)300 fill=(token)"color.bg"
    }
  }
}
"##;

// ── (a) Two variants → two .zen + two .png written ───────────────────────────

/// Running `run_variant` on a doc with two variants writes both a `.zen` and
/// a `.png` for each generated variant, and the output PNGs are valid PNG bytes.
#[test]
fn two_variants_write_zen_and_png_files() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let report =
        run_variant(DOC_TWO_VARIANTS, None, tmp.path(), "doc").expect("run_variant must succeed");

    assert_eq!(
        report.generated(),
        2,
        "both variants must generate; report: {:?}",
        report.variants
    );
    assert!(
        report.failed().is_empty(),
        "no variants should fail; failed: {:?}",
        report.failed()
    );

    // Results are in ascending id order: var.large < var.small.
    assert_eq!(report.variants[0].id, "var.large");
    assert_eq!(report.variants[1].id, "var.small");

    // Both PNG + .zen files must exist and PNG must start with magic bytes.
    for record in &report.variants {
        let outputs = record
            .outputs
            .as_ref()
            .expect("generated variant must have outputs");

        let zen_path = tmp.path().join(&outputs.zen);
        assert!(
            zen_path.exists(),
            "{} must exist on disk",
            zen_path.display()
        );

        let png_path = tmp.path().join(&outputs.png);
        let png_bytes = std::fs::read(&png_path)
            .unwrap_or_else(|e| panic!("could not read {}: {}", png_path.display(), e));
        assert!(
            png_bytes.len() >= 4 && &png_bytes[0..4] == b"\x89PNG",
            "{} must be a valid PNG; got {} bytes",
            outputs.png,
            png_bytes.len()
        );
    }
}

// ── (b) Output file naming convention ────────────────────────────────────────

/// The output files are named `<stem>-<variant-id>.zen` and
/// `<stem>-<variant-id>.png`.
#[test]
fn output_files_follow_stem_id_naming_convention() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let report = run_variant(DOC_TWO_VARIANTS, None, tmp.path(), "myfile")
        .expect("run_variant must succeed");

    let large = report
        .variants
        .iter()
        .find(|r| r.id == "var.large")
        .expect("var.large");
    let outputs = large.outputs.as_ref().expect("must have outputs");
    assert_eq!(outputs.zen, "myfile-var.large.zen");
    assert_eq!(outputs.png, "myfile-var.large.png");

    let small = report
        .variants
        .iter()
        .find(|r| r.id == "var.small")
        .expect("var.small");
    let outputs = small.outputs.as_ref().expect("must have outputs");
    assert_eq!(outputs.zen, "myfile-var.small.zen");
    assert_eq!(outputs.png, "myfile-var.small.png");
}

// ── (c) Generated .zen parses and has correct target dimensions ───────────────

/// The materialized `.zen` file for `var.large` must parse successfully and
/// its source page must carry the variant's target dimensions (1920×1080).
#[test]
fn generated_zen_parses_and_has_target_dimensions() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let report =
        run_variant(DOC_TWO_VARIANTS, None, tmp.path(), "doc").expect("run_variant must succeed");

    let large = report
        .variants
        .iter()
        .find(|r| r.id == "var.large")
        .expect("var.large");
    let outputs = large.outputs.as_ref().expect("must have outputs");

    let zen_src =
        std::fs::read_to_string(tmp.path().join(&outputs.zen)).expect("must read generated .zen");

    use zenith_core::{KdlAdapter, KdlSource};
    let doc = KdlAdapter
        .parse(zen_src.as_bytes())
        .expect("generated .zen must parse");

    let page = doc
        .body
        .pages
        .iter()
        .find(|p| p.id == "page.a")
        .expect("page.a must be present in generated .zen");

    assert_eq!(page.width.value, 1920.0, "page width must be 1920");
    assert_eq!(page.height.value, 1080.0, "page height must be 1080");
}

// ── (d) Failed variant is reported, sibling still generated ──────────────────

/// A variant that overrides a missing node fails; the sibling variant still
/// generates successfully.
#[test]
fn failed_variant_reported_sibling_still_generated() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let report = run_variant(DOC_MISSING_NODE_VARIANT, None, tmp.path(), "doc")
        .expect("run_variant must not return Err");

    assert_eq!(report.variants.len(), 2);

    // Results are in ascending id order: var.bad < var.good.
    let bad = report
        .variants
        .iter()
        .find(|r| r.id == "var.bad")
        .expect("var.bad");
    let good = report
        .variants
        .iter()
        .find(|r| r.id == "var.good")
        .expect("var.good");

    assert!(
        bad.failure.is_some(),
        "var.bad must fail; got: {:?}",
        bad.failure
    );
    assert!(
        good.failure.is_none(),
        "var.good must succeed; got: {:?}",
        good.failure
    );

    // The failure reason must mention the missing node id.
    let reason = bad.failure.as_deref().unwrap_or("");
    assert!(
        reason.contains("node.does.not.exist"),
        "failure reason must mention the missing node; got: {reason}"
    );

    // var.bad must not have written any files.
    assert!(
        bad.outputs.is_none(),
        "failed variant must not have output paths"
    );

    // var.good must have valid PNG.
    let good_outputs = good.outputs.as_ref().expect("var.good must have outputs");
    let png_bytes =
        std::fs::read(tmp.path().join(&good_outputs.png)).expect("var.good PNG must exist");
    assert!(
        png_bytes.len() >= 4 && &png_bytes[0..4] == b"\x89PNG",
        "var.good PNG must be valid"
    );
}

// ── (e) --json envelope has schema zenith-variant-v1 with correct counts ──────

/// `to_json_output` produces an envelope with schema `zenith-variant-v1`,
/// correct total_variants, generated, and failed counts, and per-variant
/// status/output fields.
#[test]
fn json_envelope_schema_and_counts() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let report =
        run_variant(DOC_TWO_VARIANTS, None, tmp.path(), "doc").expect("run_variant must succeed");

    let output = to_json_output(&report);

    assert_eq!(output.schema, "zenith-variant-v1");
    assert_eq!(output.total_variants, 2);
    assert_eq!(output.generated, 2);
    assert_eq!(output.failed, 0);
    assert_eq!(output.variants.len(), 2);

    // All variants must have status "ok".
    for v in &output.variants {
        assert_eq!(v.status, "ok", "variant {} must have status ok", v.id);
        assert!(v.outputs_zen.is_some(), "ok variant must have outputs_zen");
        assert!(v.outputs_png.is_some(), "ok variant must have outputs_png");
        assert!(
            v.diagnostics.is_empty(),
            "ok variant must have no diagnostics"
        );
    }
}

/// Mixed run: one failed variant must appear in the JSON envelope with
/// status "failed" and a diagnostic entry.
#[test]
fn json_envelope_mixed_failed_variant() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let report = run_variant(DOC_MISSING_NODE_VARIANT, None, tmp.path(), "doc")
        .expect("run_variant must not return Err");

    let output = to_json_output(&report);

    assert_eq!(output.schema, "zenith-variant-v1");
    assert_eq!(output.total_variants, 2);
    assert_eq!(output.generated, 1);
    assert_eq!(output.failed, 1);

    let bad = output
        .variants
        .iter()
        .find(|v| v.id == "var.bad")
        .expect("var.bad");
    assert_eq!(bad.status, "failed");
    assert_eq!(bad.diagnostics.len(), 1);
    assert_eq!(bad.diagnostics[0].severity, "error");
    assert_eq!(bad.diagnostics[0].code, "variant.failed");
    assert!(bad.outputs_zen.is_none());
    assert!(bad.outputs_png.is_none());

    let good = output
        .variants
        .iter()
        .find(|v| v.id == "var.good")
        .expect("var.good");
    assert_eq!(good.status, "ok");
}

// ── (f) --manifest: byte-identical across two runs (determinism) ──────────────

/// `build_manifest` must produce identical field values across two independent
/// calls with the same inputs (the S-variant-10 determinism requirement).
/// Field-level equality is used here because `serde_json` is not available as
/// a direct dev-dependency; the serialisation roundtrip is tested structurally.
#[test]
fn manifest_is_deterministic_across_two_runs() {
    let tmp1 = tempfile::TempDir::new().expect("tempdir 1");
    let tmp2 = tempfile::TempDir::new().expect("tempdir 2");

    let report1 = run_variant(DOC_TWO_VARIANTS, None, tmp1.path(), "doc")
        .expect("run_variant run 1 must succeed");
    let report2 = run_variant(DOC_TWO_VARIANTS, None, tmp2.path(), "doc")
        .expect("run_variant run 2 must succeed");

    let m1 = build_manifest(DOC_TWO_VARIANTS, &report1);
    let m2 = build_manifest(DOC_TWO_VARIANTS, &report2);

    assert_eq!(
        m1.schema, m2.schema,
        "schema must be identical across two runs"
    );
    assert_eq!(
        m1.generator, m2.generator,
        "generator must be identical across two runs"
    );
    assert_eq!(
        m1.source_sha256, m2.source_sha256,
        "source_sha256 must be identical across two runs with the same input"
    );
    assert_eq!(
        m1.targets.len(),
        m2.targets.len(),
        "target count must be identical"
    );
    for (t1, t2) in m1.targets.iter().zip(m2.targets.iter()) {
        assert_eq!(t1.id, t2.id, "target id must match");
        assert_eq!(t1.source, t2.source, "target source must match");
        assert_eq!(
            t1.outputs_zen, t2.outputs_zen,
            "target outputs_zen must match"
        );
        assert_eq!(
            t1.outputs_png, t2.outputs_png,
            "target outputs_png must match"
        );
    }
}

// ── (g) --manifest structural correctness ────────────────────────────────────

/// The manifest has schema `zenith-variant-manifest-v1`, a 64-char lowercase
/// hex `source_sha256`, and includes only successfully-generated variants.
#[test]
fn manifest_structural_correctness() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let report =
        run_variant(DOC_TWO_VARIANTS, None, tmp.path(), "doc").expect("run_variant must succeed");

    let m = build_manifest(DOC_TWO_VARIANTS, &report);

    assert_eq!(m.schema, "zenith-variant-manifest-v1");
    assert_eq!(
        m.source_sha256.len(),
        64,
        "source_sha256 must be 64-char hex"
    );
    assert!(
        m.source_sha256
            .chars()
            .all(|c| matches!(c, '0'..='9' | 'a'..='f')),
        "source_sha256 must be lowercase hex; got: {}",
        m.source_sha256
    );

    // Both variants must appear in targets.
    assert_eq!(
        m.targets.len(),
        2,
        "both generated variants must be in targets"
    );

    // Targets are in variant-id ascending order (var.large < var.small).
    assert_eq!(m.targets[0].id, "var.large");
    assert_eq!(m.targets[0].source, "page.a");
    assert_eq!(m.targets[0].outputs_zen, "doc-var.large.zen");
    assert_eq!(m.targets[0].outputs_png, "doc-var.large.png");

    assert_eq!(m.targets[1].id, "var.small");
}

/// Failed variants must NOT appear in the manifest targets.
#[test]
fn manifest_excludes_failed_variants() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let report = run_variant(DOC_MISSING_NODE_VARIANT, None, tmp.path(), "doc")
        .expect("run_variant must not error");

    assert_eq!(report.failed().len(), 1, "sanity: one variant must fail");

    let m = build_manifest(DOC_MISSING_NODE_VARIANT, &report);
    assert_eq!(
        m.targets.len(),
        1,
        "only the successful variant must appear in the manifest"
    );
    assert_eq!(m.targets[0].id, "var.good");
}

/// Different source documents must produce different source_sha256 values.
#[test]
fn manifest_different_input_different_sha256() {
    let tmp1 = tempfile::TempDir::new().expect("tempdir 1");
    let tmp2 = tempfile::TempDir::new().expect("tempdir 2");

    let r1 = run_variant(DOC_TWO_VARIANTS, None, tmp1.path(), "doc").expect("run 1");
    let r2 = run_variant(DOC_MISSING_NODE_VARIANT, None, tmp2.path(), "doc").expect("run 2");

    let m1 = build_manifest(DOC_TWO_VARIANTS, &r1);
    let m2 = build_manifest(DOC_MISSING_NODE_VARIANT, &r2);

    assert_ne!(
        m1.source_sha256, m2.source_sha256,
        "different source documents must produce different source_sha256"
    );
}

// ── (h) No variants block → empty report, no files written ───────────────────

/// A document with no `variants` block produces an empty report and no output
/// files.  `run_variant` must not return `Err` in this case.
#[test]
fn no_variants_block_returns_empty_report() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let report = run_variant(DOC_NO_VARIANTS, None, tmp.path(), "doc")
        .expect("run_variant must not error for no-variants doc");

    assert_eq!(report.generated(), 0);
    assert_eq!(report.failed().len(), 0);
    assert_eq!(report.variants.len(), 0);

    // No files must have been written.
    let entries: Vec<_> = std::fs::read_dir(tmp.path()).expect("read_dir").collect();
    assert!(
        entries.is_empty(),
        "no files must be written for a doc with no variants"
    );
}

#[test]
fn explicit_png_matches_default_bytes_and_manifest() {
    use zenith_cli::commands::render::BatchFormat;
    let first = tempfile::TempDir::new().unwrap();
    let second = tempfile::TempDir::new().unwrap();
    let default = run_variant(DOC_TWO_VARIANTS, None, first.path(), "doc").unwrap();
    let explicit = zenith_cli::commands::variant::run_variant_with_format(
        DOC_TWO_VARIANTS,
        None,
        second.path(),
        "doc",
        BatchFormat::Png,
    )
    .unwrap();
    assert_eq!(
        serde_json::to_vec(&build_manifest(DOC_TWO_VARIANTS, &default)).unwrap(),
        serde_json::to_vec(&build_manifest(DOC_TWO_VARIANTS, &explicit)).unwrap()
    );
    for record in &default.variants {
        let outputs = record.outputs.as_ref().unwrap();
        assert_eq!(
            std::fs::read(first.path().join(&outputs.png)).unwrap(),
            std::fs::read(second.path().join(&outputs.png)).unwrap()
        );
        assert!(outputs.svg.is_none());
    }
}

#[test]
fn svg_variants_keep_companions_and_omit_png_fields() {
    use zenith_cli::commands::render::BatchFormat;
    let first = tempfile::TempDir::new().unwrap();
    let second = tempfile::TempDir::new().unwrap();
    let generate = |dir| {
        zenith_cli::commands::variant::run_variant_with_format(
            DOC_TWO_VARIANTS,
            None,
            dir,
            "doc",
            BatchFormat::Svg,
        )
        .unwrap()
    };
    let report = generate(first.path());
    let again = generate(second.path());
    assert_eq!(report.generated(), 2);
    for record in &report.variants {
        let outputs = record.outputs.as_ref().unwrap();
        assert!(outputs.png.is_empty());
        let name = outputs.svg.as_ref().unwrap();
        assert_eq!(name, &format!("doc-{}.svg", record.id));
        let bytes = std::fs::read(first.path().join(name)).unwrap();
        assert_eq!(bytes, std::fs::read(second.path().join(name)).unwrap());
        assert!(String::from_utf8(bytes).unwrap().contains("<svg"));
        assert!(first.path().join(&outputs.zen).is_file());
    }
    let json = serde_json::to_value(to_json_output(&report)).unwrap();
    assert!(json["variants"][0].get("outputs_png").is_none());
    assert!(
        json["variants"][0]["outputs_svg"]
            .as_str()
            .unwrap()
            .ends_with(".svg")
    );
    let manifest = serde_json::to_value(build_manifest(DOC_TWO_VARIANTS, &report)).unwrap();
    assert!(manifest["targets"][0].get("outputs_png").is_none());
    assert!(
        manifest["targets"][0]["outputs_svg"]
            .as_str()
            .unwrap()
            .ends_with(".svg")
    );
    assert_eq!(
        manifest,
        serde_json::to_value(build_manifest(DOC_TWO_VARIANTS, &again)).unwrap()
    );
}

#[test]
fn svg_compile_advisory_keeps_node_subject_and_machine_fix() {
    use zenith_cli::commands::render::BatchFormat;
    let out = tempfile::TempDir::new().unwrap();
    let report = zenith_cli::commands::variant::run_variant_with_format(
        DOC_TWO_VARIANTS,
        None,
        out.path(),
        "doc",
        BatchFormat::Svg,
    )
    .unwrap();
    let record = report
        .variants
        .iter()
        .find(|r| r.id == "var.large")
        .unwrap();
    assert!(record.failure.is_none());
    let original = record
        .diagnostics
        .iter()
        .find(|d| d.code == "text.edge_crowding")
        .unwrap();
    assert_eq!(original.subject_id.as_deref(), Some("text.label"));
    let original_json = serde_json::to_value(zenith_cli::json_types::DiagnosticJson::located(
        original, "",
    ))
    .unwrap();
    assert!(original_json.get("fix").is_some());
    let json = serde_json::to_value(to_json_output(&report)).unwrap();
    let variant = json["variants"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["id"] == "var.large")
        .unwrap();
    let diagnostic = variant["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == "text.edge_crowding")
        .unwrap();
    assert_eq!(diagnostic["subject_id"], "text.label");
    assert_eq!(diagnostic["fix"], original_json["fix"]);
}
