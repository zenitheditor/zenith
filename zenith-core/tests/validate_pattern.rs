//! Validation tests for the `pattern` node.
//!
//! Covers id-uniqueness, motif template isolation, complete token-ref
//! collection on all visual props (including border/stroke-outer/blur), and
//! every pattern-specific semantic diagnostic.

mod common;

use common::*;

/// A pattern whose id duplicates another node's id fires `id.duplicate` — this
/// proves the pattern's own id participates in id-uniqueness.
#[test]
fn pattern_duplicate_id_fires_id_duplicate() {
    let src = r##"zenith version=1 {
  project id="proj.dup" name="Dup"
  tokens format="zenith-token-v1" {
    token id="color.dot" type="color" value="#334155"
  }
  styles {
  }
  document id="doc.dup" title="Dup" {
    page id="page.dup" w=(px)800 h=(px)600 {
      rect id="node.dup" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill=(token)"color.dot"
      pattern id="node.dup" kind="grid" x=(px)0 y=(px)0 w=(px)800 h=(px)600 fill=(token)"color.dot" {
        ellipse id="e.dot" w=(px)8 h=(px)8 fill=(token)"color.dot"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);
    assert!(
        has_code(&report, "id.duplicate"),
        "pattern id duplicate must fire id.duplicate; got: {:?}",
        codes(&report)
    );
}

/// A pattern node alone validates without crashing AND its motif's id is NOT
/// collected: a motif whose id collides with a real sibling node must NOT
/// produce `id.duplicate` (the motif is a template, invisible to id-collection).
#[test]
fn pattern_motif_id_not_collected() {
    let src = r##"zenith version=1 {
  project id="proj.motif" name="Motif"
  tokens format="zenith-token-v1" {
    token id="color.dot" type="color" value="#334155"
  }
  styles {
  }
  document id="doc.motif" title="Motif" {
    page id="page.motif" w=(px)800 h=(px)600 {
      rect id="shared.id" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill=(token)"color.dot"
      pattern id="p.dots" kind="grid" x=(px)0 y=(px)0 w=(px)800 h=(px)600 spacing=(px)20 fill=(token)"color.dot" {
        ellipse id="shared.id" w=(px)8 h=(px)8 fill=(token)"color.dot"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);
    // The motif shares an id with a real node, but the motif is a TEMPLATE and is
    // never id-collected — so NO id.duplicate is produced.
    assert!(
        !has_code(&report, "id.duplicate"),
        "motif id must NOT be collected (no id.duplicate); got: {:?}",
        codes(&report)
    );
    assert!(
        !report.has_errors(),
        "a clean pattern + motif must not error; got: {:?}",
        codes(&report)
    );
}

/// A valid grid pattern with all required fields and an in-range jitter
/// produces no pattern-specific diagnostics.
#[test]
fn pattern_valid_grid_no_diagnostics() {
    let src = r##"zenith version=1 {
  project id="proj.vg" name="ValidGrid"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#ffffff"
  }
  styles {
  }
  document id="doc.vg" title="ValidGrid" {
    page id="page.vg" w=(px)800 h=(px)600 {
      pattern id="p.grid" kind="grid" x=(px)0 y=(px)0 w=(px)800 h=(px)600 spacing=(px)24 jitter=0.2 fill=(token)"color.bg" {
        ellipse id="e.motif" w=(px)8 h=(px)8
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);
    let pattern_diags: Vec<&str> = codes(&report)
        .into_iter()
        .filter(|c| c.starts_with("pattern."))
        .collect();
    assert!(
        pattern_diags.is_empty(),
        "valid grid pattern must produce no pattern.* diagnostics; got: {:?}",
        pattern_diags
    );
}

/// A valid scatter pattern with count > 0 produces no pattern-specific
/// diagnostics.
#[test]
fn pattern_valid_scatter_no_diagnostics() {
    let src = r##"zenith version=1 {
  project id="proj.vs" name="ValidScatter"
  tokens format="zenith-token-v1" {
    token id="color.dot" type="color" value="#334155"
  }
  styles {
  }
  document id="doc.vs" title="ValidScatter" {
    page id="page.vs" w=(px)800 h=(px)600 {
      pattern id="p.scatter" kind="scatter" x=(px)0 y=(px)0 w=(px)800 h=(px)600 count=50 fill=(token)"color.dot" {
        ellipse id="e.motif" w=(px)6 h=(px)6
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);
    let pattern_diags: Vec<&str> = codes(&report)
        .into_iter()
        .filter(|c| c.starts_with("pattern."))
        .collect();
    assert!(
        pattern_diags.is_empty(),
        "valid scatter pattern must produce no pattern.* diagnostics; got: {:?}",
        pattern_diags
    );
}

/// An unrecognized kind string fires `pattern.unknown_kind` and does NOT also
/// fire kind-specific requirement errors (e.g. grid_missing_spacing).
#[test]
fn pattern_unknown_kind_fires_diagnostic() {
    let src = r##"zenith version=1 {
  project id="proj.uk" name="UnknownKind"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.uk" title="UnknownKind" {
    page id="page.uk" w=(px)800 h=(px)600 {
      pattern id="p.bad" kind="hexagonal" x=(px)0 y=(px)0 w=(px)800 h=(px)600 {
        ellipse id="e.m" w=(px)8 h=(px)8
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);
    assert!(
        has_code(&report, "pattern.unknown_kind"),
        "unknown kind must fire pattern.unknown_kind; got: {:?}",
        codes(&report)
    );
    assert!(
        !has_code(&report, "pattern.grid_missing_spacing"),
        "unknown kind must NOT fire grid_missing_spacing; got: {:?}",
        codes(&report)
    );
    assert!(
        !has_code(&report, "pattern.scatter_missing_count"),
        "unknown kind must NOT fire scatter_missing_count; got: {:?}",
        codes(&report)
    );
}

/// A grid pattern without a spacing value fires `pattern.grid_missing_spacing`.
#[test]
fn pattern_grid_missing_spacing() {
    let src = r##"zenith version=1 {
  project id="proj.gms" name="GridNoSpacing"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.gms" title="GridNoSpacing" {
    page id="page.gms" w=(px)800 h=(px)600 {
      pattern id="p.grid" kind="grid" x=(px)0 y=(px)0 w=(px)800 h=(px)600 {
        ellipse id="e.m" w=(px)8 h=(px)8
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);
    assert!(
        has_code(&report, "pattern.grid_missing_spacing"),
        "grid without spacing must fire pattern.grid_missing_spacing; got: {:?}",
        codes(&report)
    );
}

/// A scatter pattern without a count value fires `pattern.scatter_missing_count`.
#[test]
fn pattern_scatter_missing_count() {
    let src = r##"zenith version=1 {
  project id="proj.smc" name="ScatterNoCount"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.smc" title="ScatterNoCount" {
    page id="page.smc" w=(px)800 h=(px)600 {
      pattern id="p.scatter" kind="scatter" x=(px)0 y=(px)0 w=(px)800 h=(px)600 {
        ellipse id="e.m" w=(px)8 h=(px)8
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);
    assert!(
        has_code(&report, "pattern.scatter_missing_count"),
        "scatter without count must fire pattern.scatter_missing_count; got: {:?}",
        codes(&report)
    );
}

/// A pattern with count=0 fires `pattern.invalid_count`.
#[test]
fn pattern_invalid_count_zero() {
    let src = r##"zenith version=1 {
  project id="proj.ic" name="InvalidCount"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.ic" title="InvalidCount" {
    page id="page.ic" w=(px)800 h=(px)600 {
      pattern id="p.scatter" kind="scatter" x=(px)0 y=(px)0 w=(px)800 h=(px)600 count=0 {
        ellipse id="e.m" w=(px)8 h=(px)8
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);
    assert!(
        has_code(&report, "pattern.invalid_count"),
        "count=0 must fire pattern.invalid_count; got: {:?}",
        codes(&report)
    );
}

/// A pattern with count=-5 fires `pattern.invalid_count`.
#[test]
fn pattern_invalid_count_negative() {
    let src = r##"zenith version=1 {
  project id="proj.icn" name="InvalidCountNeg"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.icn" title="InvalidCountNeg" {
    page id="page.icn" w=(px)800 h=(px)600 {
      pattern id="p.scatter" kind="scatter" x=(px)0 y=(px)0 w=(px)800 h=(px)600 count=-5 {
        ellipse id="e.m" w=(px)8 h=(px)8
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);
    assert!(
        has_code(&report, "pattern.invalid_count"),
        "count=-5 must fire pattern.invalid_count; got: {:?}",
        codes(&report)
    );
}

/// A pattern with spacing=0 fires `pattern.invalid_spacing`.
#[test]
fn pattern_invalid_spacing_zero() {
    let src = r##"zenith version=1 {
  project id="proj.is" name="InvalidSpacing"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.is" title="InvalidSpacing" {
    page id="page.is" w=(px)800 h=(px)600 {
      pattern id="p.grid" kind="grid" x=(px)0 y=(px)0 w=(px)800 h=(px)600 spacing=(px)0 {
        ellipse id="e.m" w=(px)8 h=(px)8
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);
    assert!(
        has_code(&report, "pattern.invalid_spacing"),
        "spacing=0 must fire pattern.invalid_spacing; got: {:?}",
        codes(&report)
    );
}

/// A pattern with jitter=1.5 fires `pattern.jitter_out_of_range` as a warning
/// (not an error).
#[test]
fn pattern_jitter_out_of_range_warning() {
    let src = r##"zenith version=1 {
  project id="proj.jor" name="JitterOOR"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.jor" title="JitterOOR" {
    page id="page.jor" w=(px)800 h=(px)600 {
      pattern id="p.grid" kind="grid" x=(px)0 y=(px)0 w=(px)800 h=(px)600 spacing=(px)20 jitter=1.5 {
        ellipse id="e.m" w=(px)8 h=(px)8
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);
    assert!(
        has_code(&report, "pattern.jitter_out_of_range"),
        "jitter=1.5 must fire pattern.jitter_out_of_range; got: {:?}",
        codes(&report)
    );
    // Must be a warning, not an error.
    let is_warning = report
        .diagnostics
        .iter()
        .any(|d| d.code == "pattern.jitter_out_of_range" && d.severity == Severity::Warning);
    assert!(
        is_warning,
        "pattern.jitter_out_of_range must be a Warning, not an Error; got: {:?}",
        codes(&report)
    );
    assert!(
        !report.has_errors(),
        "jitter out of range must not produce any errors; got: {:?}",
        codes(&report)
    );
}

/// A token used ONLY on a pattern's `border_top` must NOT be flagged as
/// `token.unused`. This proves that the complete visual-prop token-ref
/// collection covers the border/stroke-outer family.
#[test]
fn pattern_border_token_not_flagged_unused() {
    let src = r##"zenith version=1 {
  project id="proj.bt" name="BorderToken"
  tokens format="zenith-token-v1" {
    token id="color.border" type="color" value="#0000ff"
  }
  styles {
  }
  document id="doc.bt" title="BorderToken" {
    page id="page.bt" w=(px)800 h=(px)600 {
      pattern id="p.grid" kind="grid" x=(px)0 y=(px)0 w=(px)800 h=(px)600 spacing=(px)20 border-top=(token)"color.border" {
        ellipse id="e.m" w=(px)8 h=(px)8
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);
    assert!(
        !has_code(&report, "token.unused"),
        "token used only on pattern border-top must NOT be flagged token.unused; got: {:?}",
        codes(&report)
    );
}
