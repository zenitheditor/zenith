//! Integration tests for CLI-layer diagnostic-policy and brand-contract
//! resolution.
//!
//! These exercise the public `zenith_cli::config` loaders/merge plus
//! `zenith_core::validate_with_policy`, and the `validate::run` exit-code path
//! for a malformed local config. All filesystem state is rooted in tempdirs and
//! the injectable loaders are used with explicit paths — no `$HOME`/cwd mutation,
//! so the tests are parallel-safe.

use std::fs;

use tempfile::TempDir;
use zenith_cli::config::{
    CliPolicyFlags, find_local_brand, find_local_policy, load_brand_file, load_global_brand_in,
    load_global_policy_in, load_policy_file, merge_policy,
};
use zenith_core::{
    BrandContract, KdlAdapter, KdlSource as _, Severity, merge_brand_contract, validate_with_policy,
};

// ── Fixture ─────────────────────────────────────────────────────────────────

/// A document with one unused token → `token.unused` (Advisory by default).
/// It is non-Error, so policy can move it around.
const DOC: &str = r##"zenith version=1 {
  project id="proj.p" name="Policy"
  tokens format="zenith-token-v1" {
    token id="color.unused" type="color" value="#abcdef"
  }
  styles {}
  document id="doc.p" title="Policy" {
    page id="page.p" w=(px)100 h=(px)100 {
      rect id="r.one" x=(px)0 y=(px)0 w=(px)10 h=(px)10
    }
  }
}
"##;

/// A document whose in-file policy `allow`s `token.unused`, suppressing it.
const DOC_IN_FILE_ALLOW: &str = r##"zenith version=1 {
  project id="proj.p" name="Policy"
  diagnostics {
    allow "token.unused"
  }
  tokens format="zenith-token-v1" {
    token id="color.unused" type="color" value="#abcdef"
  }
  styles {}
  document id="doc.p" title="Policy" {
    page id="page.p" w=(px)100 h=(px)100 {
      rect id="r.one" x=(px)0 y=(px)0 w=(px)10 h=(px)10
    }
  }
}
"##;

fn parse(src: &str) -> zenith_core::Document {
    KdlAdapter.parse(src.as_bytes()).expect("doc must parse")
}

fn has_error(diags: &[zenith_core::Diagnostic], code: &str) -> bool {
    diags
        .iter()
        .any(|d| d.code == code && d.severity == Severity::Error)
}

fn present(diags: &[zenith_core::Diagnostic], code: &str) -> bool {
    diags.iter().any(|d| d.code == code)
}

// ── Local config walk-up ──────────────────────────────────────────────────────

#[test]
fn local_config_discovered_by_walk_up() {
    let tmp = TempDir::new().expect("tempdir");
    // Config at the root of the temp tree.
    fs::write(
        tmp.path().join(".zenith.kdl"),
        b"diagnostics {\n  deny \"token.unused\"\n}\n",
    )
    .expect("write config");
    // The document lives in a nested subdirectory.
    let nested = tmp.path().join("a").join("b");
    fs::create_dir_all(&nested).expect("mkdir");

    let local = find_local_policy(&nested).expect("walk-up must succeed");
    let doc = parse(DOC);
    let merged = merge_policy(
        &zenith_core::DiagnosticPolicy::default(),
        &local,
        &doc.diagnostic_policy,
        &CliPolicyFlags::default(),
    );
    let report = validate_with_policy(&doc, &merged, &doc.brand_contract);
    assert!(
        has_error(&report.diagnostics, "token.unused"),
        "local deny must elevate token.unused to Error"
    );
}

// ── Precedence: CLI > in-file > local > global ─────────────────────────────────

#[test]
fn cli_deny_overrides_global_allow() {
    let tmp = TempDir::new().expect("tempdir");
    // Global config allows token.unused (would suppress it).
    let zenith_dir = tmp.path().join("zenith");
    fs::create_dir_all(&zenith_dir).expect("mkdir");
    fs::write(
        zenith_dir.join("config.kdl"),
        b"diagnostics {\n  allow \"token.unused\"\n}\n",
    )
    .expect("write global");

    let global = load_global_policy_in(tmp.path()).expect("global load");
    let doc = parse(DOC);
    let flags = CliPolicyFlags {
        deny: vec!["token.unused".to_owned()],
        ..Default::default()
    };
    let merged = merge_policy(
        &global,
        &zenith_core::DiagnosticPolicy::default(),
        &doc.diagnostic_policy,
        &flags,
    );
    let report = validate_with_policy(&doc, &merged, &doc.brand_contract);
    assert!(
        has_error(&report.diagnostics, "token.unused"),
        "CLI deny must win over global allow"
    );
}

#[test]
fn cli_deny_overrides_in_file_allow() {
    // The document's own policy allows token.unused; --deny must override it,
    // making the suppressed-in-file code reappear as an Error.
    let doc = parse(DOC_IN_FILE_ALLOW);
    // Sanity: in-file allow alone suppresses it.
    let baseline = validate_with_policy(&doc, &doc.diagnostic_policy, &doc.brand_contract);
    assert!(
        !present(&baseline.diagnostics, "token.unused"),
        "in-file allow should suppress token.unused"
    );

    let flags = CliPolicyFlags {
        deny: vec!["token.unused".to_owned()],
        ..Default::default()
    };
    let merged = merge_policy(
        &zenith_core::DiagnosticPolicy::default(),
        &zenith_core::DiagnosticPolicy::default(),
        &doc.diagnostic_policy,
        &flags,
    );
    let report = validate_with_policy(&doc, &merged, &doc.brand_contract);
    assert!(
        has_error(&report.diagnostics, "token.unused"),
        "CLI deny must override in-file allow and surface token.unused as Error"
    );
}

#[test]
fn in_file_beats_local_and_global() {
    // global=deny, local=deny, in-file=allow → suppressed (in-file wins).
    let global = load_policy_inline("diagnostics {\n  deny \"token.unused\"\n}");
    let local = load_policy_inline("diagnostics {\n  deny \"token.unused\"\n}");
    let doc = parse(DOC_IN_FILE_ALLOW);
    let merged = merge_policy(
        &global,
        &local,
        &doc.diagnostic_policy,
        &CliPolicyFlags::default(),
    );
    let report = validate_with_policy(&doc, &merged, &doc.brand_contract);
    assert!(
        !present(&report.diagnostics, "token.unused"),
        "in-file allow must override local+global deny"
    );
}

// ── Malformed config → exit 2 ──────────────────────────────────────────────────

#[test]
fn malformed_local_config_exits_two() {
    let tmp = TempDir::new().expect("tempdir");
    fs::write(tmp.path().join(".zenith.kdl"), b"diagnostics {{{ not kdl")
        .expect("write bad config");

    let out = zenith_cli::commands::validate::run(
        DOC,
        Some(tmp.path()),
        false,
        &CliPolicyFlags::default(),
    );
    assert_eq!(
        out.exit_code, 2,
        "malformed local config must exit 2; stdout: {}",
        out.stdout
    );
    assert!(
        out.stdout.contains("config.error"),
        "error output must name config.error; got: {}",
        out.stdout
    );
}

// ── Brand-contract config integration ────────────────────────────────────────

/// A document with a color token that is OFF the brand palette when the
/// contract restricts colors to `#ffffff` only.
const DOC_WITH_COLOR_TOKEN: &str = r##"zenith version=1 {
  project id="proj.b" name="Brand"
  tokens format="zenith-token-v1" {
    token id="color.primary" type="color" value="#ff0000"
  }
  styles {}
  document id="doc.b" title="Brand" {
    page id="page.b" w=(px)100 h=(px)100 {
      rect id="r.b" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"color.primary"
    }
  }
}
"##;

/// Same document but with an in-file `brand` block restricting colors to
/// `#ff0000` (matches the token → no brand warning).
const DOC_WITH_IN_FILE_BRAND_ALLOW: &str = r##"zenith version=1 {
  project id="proj.b2" name="Brand2"
  brand {
    colors "#ff0000"
  }
  tokens format="zenith-token-v1" {
    token id="color.primary" type="color" value="#ff0000"
  }
  styles {}
  document id="doc.b2" title="Brand2" {
    page id="page.b2" w=(px)100 h=(px)100 {
      rect id="r.b2" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"color.primary"
    }
  }
}
"##;

#[test]
fn local_config_brand_picked_up_for_doc_with_no_in_file_brand() {
    let tmp = TempDir::new().expect("tempdir");
    // Local config has brand restricting colors to #ffffff only.
    fs::write(
        tmp.path().join(".zenith.kdl"),
        b"brand {\n  colors \"#ffffff\"\n}\n",
    )
    .expect("write config");
    let nested = tmp.path().join("sub");
    fs::create_dir_all(&nested).expect("mkdir");

    let local_brand = find_local_brand(&nested).expect("walk-up must succeed");
    let doc = KdlAdapter
        .parse(DOC_WITH_COLOR_TOKEN.as_bytes())
        .expect("parse");
    // Doc has no in-file brand, so effective = merge(default, local) = local.
    let effective_brand = merge_brand_contract(
        &merge_brand_contract(&BrandContract::default(), &local_brand),
        &doc.brand_contract,
    );
    let report = validate_with_policy(&doc, &doc.diagnostic_policy, &effective_brand);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "brand.color_off_palette"),
        "local brand restricting #ffffff should fire brand.color_off_palette for #ff0000; \
         got: {:?}",
        report
            .diagnostics
            .iter()
            .map(|d| &d.code)
            .collect::<Vec<_>>()
    );
}

#[test]
fn in_file_brand_overrides_local_config_per_category() {
    let tmp = TempDir::new().expect("tempdir");
    // Local config restricts colors to #ffffff only (would fire for #ff0000).
    fs::write(
        tmp.path().join(".zenith.kdl"),
        b"brand {\n  colors \"#ffffff\"\n}\n",
    )
    .expect("write config");

    let local_brand = find_local_brand(tmp.path()).expect("local brand load");
    let doc = KdlAdapter
        .parse(DOC_WITH_IN_FILE_BRAND_ALLOW.as_bytes())
        .expect("parse");
    // In-file brand allows #ff0000 for colors → overrides local's #ffffff.
    let effective_brand = merge_brand_contract(
        &merge_brand_contract(&BrandContract::default(), &local_brand),
        &doc.brand_contract,
    );
    let report = validate_with_policy(&doc, &doc.diagnostic_policy, &effective_brand);
    assert!(
        !report
            .diagnostics
            .iter()
            .any(|d| d.code == "brand.color_off_palette"),
        "in-file brand allowing #ff0000 must override local config's #ffffff restriction; \
         got: {:?}",
        report
            .diagnostics
            .iter()
            .map(|d| &d.code)
            .collect::<Vec<_>>()
    );
}

#[test]
fn global_config_brand_picked_up() {
    let tmp = TempDir::new().expect("tempdir");
    // Global config dir: tmp/zenith/config.kdl
    let zenith_dir = tmp.path().join("zenith");
    fs::create_dir_all(&zenith_dir).expect("mkdir");
    fs::write(
        zenith_dir.join("config.kdl"),
        b"brand {\n  colors \"#ffffff\"\n}\n",
    )
    .expect("write global");

    let global_brand = load_global_brand_in(tmp.path()).expect("global brand load");
    let doc = KdlAdapter
        .parse(DOC_WITH_COLOR_TOKEN.as_bytes())
        .expect("parse");
    let effective_brand = merge_brand_contract(
        &merge_brand_contract(&global_brand, &BrandContract::default()),
        &doc.brand_contract,
    );
    let report = validate_with_policy(&doc, &doc.diagnostic_policy, &effective_brand);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "brand.color_off_palette"),
        "global brand restricting #ffffff should fire brand.color_off_palette; \
         got: {:?}",
        report
            .diagnostics
            .iter()
            .map(|d| &d.code)
            .collect::<Vec<_>>()
    );
}

#[test]
fn brand_precedence_in_file_over_local_over_global() {
    // global=restrict to #ffffff, local=restrict to #000000, in-file=allow #ff0000
    // → no brand.color_off_palette because in-file wins for the colors category.
    let tmp_global = TempDir::new().expect("tempdir");
    let zenith_dir = tmp_global.path().join("zenith");
    fs::create_dir_all(&zenith_dir).expect("mkdir");
    fs::write(
        zenith_dir.join("config.kdl"),
        b"brand {\n  colors \"#ffffff\"\n}\n",
    )
    .expect("write global");

    let tmp_local = TempDir::new().expect("tempdir");
    fs::write(
        tmp_local.path().join(".zenith.kdl"),
        b"brand {\n  colors \"#000000\"\n}\n",
    )
    .expect("write local");

    let global_brand = load_global_brand_in(tmp_global.path()).expect("global");
    let local_brand = find_local_brand(tmp_local.path()).expect("local");
    let doc = KdlAdapter
        .parse(DOC_WITH_IN_FILE_BRAND_ALLOW.as_bytes())
        .expect("parse");
    let effective_brand = merge_brand_contract(
        &merge_brand_contract(&global_brand, &local_brand),
        &doc.brand_contract,
    );
    let report = validate_with_policy(&doc, &doc.diagnostic_policy, &effective_brand);
    assert!(
        !report
            .diagnostics
            .iter()
            .any(|d| d.code == "brand.color_off_palette"),
        "in-file brand must win: #ff0000 is on the in-file palette; \
         got: {:?}",
        report
            .diagnostics
            .iter()
            .map(|d| &d.code)
            .collect::<Vec<_>>()
    );
}

#[test]
fn load_brand_file_missing_is_default() {
    let brand = load_brand_file(std::path::Path::new("/no/such/brand.kdl"))
        .expect("missing file must be ok");
    assert!(
        brand.is_empty(),
        "missing file must yield default brand contract"
    );
}

#[test]
fn find_local_brand_root_walk_terminates() {
    let brand = find_local_brand(std::path::Path::new("/")).expect("root walk must be ok");
    assert!(
        brand.is_empty(),
        "root walk with no config must yield default"
    );
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Write `body` to a temp `.zenith.kdl` and load it (exercises `load_policy_file`).
fn load_policy_inline(body: &str) -> zenith_core::DiagnosticPolicy {
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("inline.kdl");
    fs::write(&path, body.as_bytes()).expect("write");
    load_policy_file(&path).expect("policy must load")
}
