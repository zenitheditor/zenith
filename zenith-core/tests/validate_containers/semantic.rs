use super::*;

// ── Group: semantic scalar validation ────────────────────────────────

/// `intensity=1.5` (above 1.0) must produce a `group.invalid_intensity` warning.
#[test]
fn group_intensity_out_of_range_warns() {
    let src = r##"zenith version=1 {
  project id="proj.gi" name="GI"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.gi" title="GI" {
    page id="page.gi" w=(px)800 h=(px)600 {
      group id="grp.gi" intensity=1.5 {
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);
    assert!(
        has_code(&report, "group.invalid_intensity"),
        "intensity=1.5 must fire group.invalid_intensity; codes: {:?}",
        codes(&report)
    );
    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "group.invalid_intensity")
        .expect("diagnostic must exist");
    assert_eq!(diag.severity, Severity::Warning);
    assert!(!report.has_errors());
}

/// `intensity=0.5` (in range) must not produce any `group.invalid_intensity` warning.
#[test]
fn group_intensity_in_range_no_warning() {
    let src = r##"zenith version=1 {
  project id="proj.giv" name="GIV"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.giv" title="GIV" {
    page id="page.giv" w=(px)800 h=(px)600 {
      group id="grp.giv" intensity=0.5 {
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);
    assert!(
        !has_code(&report, "group.invalid_intensity"),
        "intensity=0.5 must not fire group.invalid_intensity; codes: {:?}",
        codes(&report)
    );
}

#[test]
fn group_live_symmetry_valid_no_warning() {
    let src = r##"zenith version=1 {
  project id="proj.gsv" name="GSV"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.gsv" title="GSV" {
    page id="page.gsv" w=(px)800 h=(px)600 {
      group id="grp.gsv" symmetry-count=4 symmetry-cx=(px)400 symmetry-cy=(px)300 symmetry-start-angle=(deg)0 {
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);
    assert!(
        !has_code(&report, "group.invalid_symmetry"),
        "valid live symmetry must not warn; codes: {:?}",
        codes(&report)
    );
}

#[test]
fn group_live_symmetry_invalid_count_errors() {
    let src = r##"zenith version=1 {
  project id="proj.gsi" name="GSI"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.gsi" title="GSI" {
    page id="page.gsi" w=(px)800 h=(px)600 {
      group id="grp.gsi" symmetry-count=73 symmetry-cx=(px)400 symmetry-cy=(px)300 {
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);
    assert!(
        has_code(&report, "group.invalid_symmetry"),
        "symmetry-count=73 must be flagged; codes: {:?}",
        codes(&report)
    );
    assert!(report.has_errors());
}

#[test]
fn group_live_symmetry_missing_center_errors() {
    let src = r##"zenith version=1 {
  project id="proj.gsm" name="GSM"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.gsm" title="GSM" {
    page id="page.gsm" w=(px)800 h=(px)600 {
      group id="grp.gsm" symmetry-count=3 symmetry-cx=(px)400 {
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);
    assert!(
        has_code(&report, "group.invalid_symmetry"),
        "missing symmetry-cy must be flagged; codes: {:?}",
        codes(&report)
    );
    assert!(report.has_errors());
}

/// `semantic-role` with any string value must not produce any `group.invalid_*`
/// diagnostic — the field is open-ended.
#[test]
fn group_semantic_role_open_ended_no_warning() {
    let src = r##"zenith version=1 {
  project id="proj.gsr" name="GSR"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.gsr" title="GSR" {
    page id="page.gsr" w=(px)800 h=(px)600 {
      group id="grp.gsr" semantic-role="anything.here" {
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);
    let group_invalid_codes: Vec<&str> = report
        .diagnostics
        .iter()
        .filter(|d| d.code.starts_with("group.invalid_"))
        .map(|d| d.code.as_str())
        .collect();
    assert!(
        group_invalid_codes.is_empty(),
        "semantic-role open-ended value must produce no group.invalid_* diagnostic; got: {:?}",
        group_invalid_codes
    );
}
