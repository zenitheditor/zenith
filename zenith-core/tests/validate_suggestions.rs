//! Integration tests: unknown-property "did you mean?" suggestion diagnostics.
//!
//! Each node kind (rect, ellipse, text, group, connector, polygon, pattern) is
//! covered by a triplet of tests: a near-miss typo that produces a suggestion,
//! a far-miss typo that produces no suggestion, and a correctly-spelled document
//! that produces no unknown-property diagnostic at all.

mod common;

use common::*;

// ── rect: "did you mean?" triplet ────────────────────────────────────

/// A rect with a near-miss typo `fil` (one edit from `fill`) must produce a
/// `node.unknown_property` warning whose message contains "did you mean 'fill'?".
#[test]
fn rect_near_miss_unknown_prop_suggests_did_you_mean() {
    // Parse via KDL so the unknown-prop is actually stored in `unknown_props`.
    let src = r##"zenith version=1 {
  project id="proj.nm" name="NearMiss"
  tokens format="zenith-token-v1" {
    token id="color.x" type="color" value="#ffffff"
  }
  styles {
  }
  document id="doc.nm" title="NearMiss" {
    page id="page.nm" w=(px)800 h=(px)600 {
      rect id="rect.nm" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fil=(token)"color.x"
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);

    // Must have the node.unknown_property warning.
    assert!(
        has_code(&report, "node.unknown_property"),
        "near-miss typo must fire node.unknown_property; got: {:?}",
        codes(&report)
    );

    // The warning message must contain the suggestion.
    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "node.unknown_property")
        .expect("diagnostic must exist");
    assert!(
        diag.message.contains("did you mean 'fill'?"),
        "message must contain \"did you mean 'fill'?\"; got: {:?}",
        diag.message
    );
    assert_eq!(diag.severity, Severity::Warning);
}

/// A rect with a completely unrelated unknown property (`quantum_flux`) must
/// produce a `node.unknown_property` warning using the version-relative message,
/// NOT a "did you mean?" suggestion.
#[test]
fn rect_far_miss_unknown_prop_no_suggestion() {
    let src = r##"zenith version=1 {
  project id="proj.fm" name="FarMiss"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.fm" title="FarMiss" {
    page id="page.fm" w=(px)800 h=(px)600 {
      rect id="rect.fm" x=(px)0 y=(px)0 w=(px)100 h=(px)100 quantum_flux=1
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);

    // Must have the node.unknown_property warning.
    assert!(
        has_code(&report, "node.unknown_property"),
        "far-miss unknown prop must fire node.unknown_property; got: {:?}",
        codes(&report)
    );

    // Must NOT suggest "did you mean?" because the edit distance exceeds 2.
    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "node.unknown_property")
        .expect("diagnostic must exist");
    assert!(
        !diag.message.contains("did you mean"),
        "far-miss message must NOT contain \"did you mean\"; got: {:?}",
        diag.message
    );
    // Must still carry the version-relative note.
    assert!(
        diag.message.contains("version-relative"),
        "far-miss message must mention version-relative; got: {:?}",
        diag.message
    );
    assert_eq!(diag.severity, Severity::Warning);
}

/// A correctly-spelled document (all props are recognised) must produce no
/// `node.unknown_property` diagnostic at all.
#[test]
fn rect_correctly_spelled_props_no_unknown_property() {
    let src = r##"zenith version=1 {
  project id="proj.ok" name="AllKnown"
  tokens format="zenith-token-v1" {
    token id="color.ok" type="color" value="#000000"
  }
  styles {
  }
  document id="doc.ok" title="AllKnown" {
    page id="page.ok" w=(px)800 h=(px)600 {
      rect id="rect.ok" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"color.ok"
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);

    assert!(
        !has_code(&report, "node.unknown_property"),
        "correctly-spelled rect must produce no node.unknown_property; got: {:?}",
        codes(&report)
    );
}

// ── ellipse: "did you mean?" triplet ─────────────────────────────────

/// An ellipse with a near-miss typo `fil` (one edit from `fill`) must produce
/// a `node.unknown_property` warning whose message contains "did you mean 'fill'?".
#[test]
fn ellipse_near_miss_unknown_prop_suggests_did_you_mean() {
    let src = r##"zenith version=1 {
  project id="proj.enm" name="EllipseNearMiss"
  tokens format="zenith-token-v1" {
    token id="color.e" type="color" value="#aabbcc"
  }
  styles {
  }
  document id="doc.enm" title="EllipseNearMiss" {
    page id="page.enm" w=(px)800 h=(px)600 {
      ellipse id="ell.nm" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fil=(token)"color.e"
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);

    assert!(
        has_code(&report, "node.unknown_property"),
        "near-miss typo must fire node.unknown_property; got: {:?}",
        codes(&report)
    );

    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "node.unknown_property")
        .expect("diagnostic must exist");
    assert!(
        diag.message.contains("did you mean 'fill'?"),
        "message must contain \"did you mean 'fill'?\"; got: {:?}",
        diag.message
    );
    assert_eq!(diag.severity, Severity::Warning);
}

/// An ellipse with a completely unrelated unknown property (`quantum_flux`) must
/// produce a `node.unknown_property` warning with NO "did you mean?" suggestion.
#[test]
fn ellipse_far_miss_unknown_prop_no_suggestion() {
    let src = r##"zenith version=1 {
  project id="proj.efm" name="EllipseFarMiss"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.efm" title="EllipseFarMiss" {
    page id="page.efm" w=(px)800 h=(px)600 {
      ellipse id="ell.fm" x=(px)0 y=(px)0 w=(px)100 h=(px)100 quantum_flux=1
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);

    assert!(
        has_code(&report, "node.unknown_property"),
        "far-miss unknown prop must fire node.unknown_property; got: {:?}",
        codes(&report)
    );

    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "node.unknown_property")
        .expect("diagnostic must exist");
    assert!(
        !diag.message.contains("did you mean"),
        "far-miss message must NOT contain \"did you mean\"; got: {:?}",
        diag.message
    );
    assert!(
        diag.message.contains("version-relative"),
        "far-miss message must mention version-relative; got: {:?}",
        diag.message
    );
    assert_eq!(diag.severity, Severity::Warning);
}

/// A correctly-spelled ellipse must produce no `node.unknown_property` diagnostic.
#[test]
fn ellipse_correctly_spelled_props_no_unknown_property() {
    let src = r##"zenith version=1 {
  project id="proj.eok" name="EllipseAllKnown"
  tokens format="zenith-token-v1" {
    token id="color.eok" type="color" value="#000000"
  }
  styles {
  }
  document id="doc.eok" title="EllipseAllKnown" {
    page id="page.eok" w=(px)800 h=(px)600 {
      ellipse id="ell.ok" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"color.eok"
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);

    assert!(
        !has_code(&report, "node.unknown_property"),
        "correctly-spelled ellipse must produce no node.unknown_property; got: {:?}",
        codes(&report)
    );
}

// ── text: "did you mean?" triplet ────────────────────────────────────

/// A text node with the near-miss typo `alin` (one edit from `align`) must
/// produce a `node.unknown_property` warning whose message contains "did you mean 'align'?".
#[test]
fn text_near_miss_unknown_prop_suggests_did_you_mean() {
    let src = r##"zenith version=1 {
  project id="proj.tnm" name="TextNearMiss"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.tnm" title="TextNearMiss" {
    page id="page.tnm" w=(px)800 h=(px)600 {
      text id="txt.nm" x=(px)0 y=(px)0 w=(px)200 h=(px)40 alin="left" {
        span "hello"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);

    assert!(
        has_code(&report, "node.unknown_property"),
        "near-miss typo must fire node.unknown_property; got: {:?}",
        codes(&report)
    );

    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "node.unknown_property")
        .expect("diagnostic must exist");
    assert!(
        diag.message.contains("did you mean 'align'?"),
        "message must contain \"did you mean 'align'?\"; got: {:?}",
        diag.message
    );
    assert_eq!(diag.severity, Severity::Warning);
}

/// A text node with a completely unrelated unknown property (`quantum_flux`) must
/// produce a `node.unknown_property` warning with NO "did you mean?" suggestion.
#[test]
fn text_far_miss_unknown_prop_no_suggestion() {
    let src = r##"zenith version=1 {
  project id="proj.tfm" name="TextFarMiss"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.tfm" title="TextFarMiss" {
    page id="page.tfm" w=(px)800 h=(px)600 {
      text id="txt.fm" x=(px)0 y=(px)0 w=(px)200 h=(px)40 quantum_flux=1 {
        span "hello"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);

    assert!(
        has_code(&report, "node.unknown_property"),
        "far-miss unknown prop must fire node.unknown_property; got: {:?}",
        codes(&report)
    );

    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "node.unknown_property")
        .expect("diagnostic must exist");
    assert!(
        !diag.message.contains("did you mean"),
        "far-miss message must NOT contain \"did you mean\"; got: {:?}",
        diag.message
    );
    assert!(
        diag.message.contains("version-relative"),
        "far-miss message must mention version-relative; got: {:?}",
        diag.message
    );
    assert_eq!(diag.severity, Severity::Warning);
}

/// A correctly-spelled text node must produce no `node.unknown_property` diagnostic.
#[test]
fn text_correctly_spelled_props_no_unknown_property() {
    let src = r##"zenith version=1 {
  project id="proj.tok" name="TextAllKnown"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.tok" title="TextAllKnown" {
    page id="page.tok" w=(px)800 h=(px)600 {
      text id="txt.ok" x=(px)0 y=(px)0 w=(px)200 h=(px)40 align="left" {
        span "hello"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);

    assert!(
        !has_code(&report, "node.unknown_property"),
        "correctly-spelled text must produce no node.unknown_property; got: {:?}",
        codes(&report)
    );
}

// ── group: "did you mean?" triplet ───────────────────────────────────

/// A group with the near-miss typo `opacty` (one edit from `opacity`) must
/// produce a `node.unknown_property` warning whose message contains "did you mean 'opacity'?".
#[test]
fn group_near_miss_unknown_prop_suggests_did_you_mean() {
    let src = r##"zenith version=1 {
  project id="proj.gnm" name="GroupNearMiss"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.gnm" title="GroupNearMiss" {
    page id="page.gnm" w=(px)800 h=(px)600 {
      group id="grp.nm" opacty=0.5 {
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);

    assert!(
        has_code(&report, "node.unknown_property"),
        "near-miss typo must fire node.unknown_property; got: {:?}",
        codes(&report)
    );

    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "node.unknown_property")
        .expect("diagnostic must exist");
    assert!(
        diag.message.contains("did you mean 'opacity'?"),
        "message must contain \"did you mean 'opacity'?\"; got: {:?}",
        diag.message
    );
    assert_eq!(diag.severity, Severity::Warning);
}

/// A group with a completely unrelated unknown property (`quantum_flux`) must
/// produce a `node.unknown_property` warning with NO "did you mean?" suggestion.
#[test]
fn group_far_miss_unknown_prop_no_suggestion() {
    let src = r##"zenith version=1 {
  project id="proj.gfm" name="GroupFarMiss"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.gfm" title="GroupFarMiss" {
    page id="page.gfm" w=(px)800 h=(px)600 {
      group id="grp.fm" quantum_flux=1 {
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);

    assert!(
        has_code(&report, "node.unknown_property"),
        "far-miss unknown prop must fire node.unknown_property; got: {:?}",
        codes(&report)
    );

    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "node.unknown_property")
        .expect("diagnostic must exist");
    assert!(
        !diag.message.contains("did you mean"),
        "far-miss message must NOT contain \"did you mean\"; got: {:?}",
        diag.message
    );
    assert!(
        diag.message.contains("version-relative"),
        "far-miss message must mention version-relative; got: {:?}",
        diag.message
    );
    assert_eq!(diag.severity, Severity::Warning);
}

/// A correctly-spelled group must produce no `node.unknown_property` diagnostic.
#[test]
fn group_correctly_spelled_props_no_unknown_property() {
    let src = r##"zenith version=1 {
  project id="proj.gok" name="GroupAllKnown"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.gok" title="GroupAllKnown" {
    page id="page.gok" w=(px)800 h=(px)600 {
      group id="grp.ok" visible=#true {
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);

    assert!(
        !has_code(&report, "node.unknown_property"),
        "correctly-spelled group must produce no node.unknown_property; got: {:?}",
        codes(&report)
    );
}

// ── connector: "did you mean?" triplet ───────────────────────────────

/// A connector with the near-miss typo `frm` (one edit from `from`) must
/// produce a `node.unknown_property` warning whose message contains "did you mean 'from'?".
#[test]
fn connector_near_miss_unknown_prop_suggests_did_you_mean() {
    let src = r##"zenith version=1 {
  project id="proj.cnm" name="ConnectorNearMiss"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.cnm" title="ConnectorNearMiss" {
    page id="page.cnm" w=(px)800 h=(px)600 {
      rect id="n.a" x=(px)0 y=(px)0 w=(px)80 h=(px)40
      rect id="n.b" x=(px)200 y=(px)0 w=(px)80 h=(px)40
      connector id="conn.nm" frm="n.a" to="n.b"
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);

    assert!(
        has_code(&report, "node.unknown_property"),
        "near-miss typo must fire node.unknown_property; got: {:?}",
        codes(&report)
    );

    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "node.unknown_property")
        .expect("diagnostic must exist");
    assert!(
        diag.message.contains("did you mean 'from'?"),
        "message must contain \"did you mean 'from'?\"; got: {:?}",
        diag.message
    );
    assert_eq!(diag.severity, Severity::Warning);
}

/// A connector with a completely unrelated unknown property (`quantum_flux`) must
/// produce a `node.unknown_property` warning with NO "did you mean?" suggestion.
#[test]
fn connector_far_miss_unknown_prop_no_suggestion() {
    let src = r##"zenith version=1 {
  project id="proj.cfm" name="ConnectorFarMiss"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.cfm" title="ConnectorFarMiss" {
    page id="page.cfm" w=(px)800 h=(px)600 {
      rect id="n.a" x=(px)0 y=(px)0 w=(px)80 h=(px)40
      rect id="n.b" x=(px)200 y=(px)0 w=(px)80 h=(px)40
      connector id="conn.fm" from="n.a" to="n.b" quantum_flux=1
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);

    assert!(
        has_code(&report, "node.unknown_property"),
        "far-miss unknown prop must fire node.unknown_property; got: {:?}",
        codes(&report)
    );

    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "node.unknown_property")
        .expect("diagnostic must exist");
    assert!(
        !diag.message.contains("did you mean"),
        "far-miss message must NOT contain \"did you mean\"; got: {:?}",
        diag.message
    );
    assert!(
        diag.message.contains("version-relative"),
        "far-miss message must mention version-relative; got: {:?}",
        diag.message
    );
    assert_eq!(diag.severity, Severity::Warning);
}

/// A correctly-spelled connector must produce no `node.unknown_property` diagnostic.
#[test]
fn connector_correctly_spelled_props_no_unknown_property() {
    let src = r##"zenith version=1 {
  project id="proj.cok" name="ConnectorAllKnown"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.cok" title="ConnectorAllKnown" {
    page id="page.cok" w=(px)800 h=(px)600 {
      rect id="n.a" x=(px)0 y=(px)0 w=(px)80 h=(px)40
      rect id="n.b" x=(px)200 y=(px)0 w=(px)80 h=(px)40
      connector id="conn.ok" from="n.a" to="n.b"
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);

    assert!(
        !has_code(&report, "node.unknown_property"),
        "correctly-spelled connector must produce no node.unknown_property; got: {:?}",
        codes(&report)
    );
}

// ── polygon: "did you mean?" triplet ─────────────────────────────────

/// A polygon with the near-miss typo `fil` (one edit from `fill`) must
/// produce a `node.unknown_property` warning whose message contains "did you mean 'fill'?".
#[test]
fn polygon_near_miss_unknown_prop_suggests_did_you_mean() {
    let src = r##"zenith version=1 {
  project id="proj.pnm" name="PolygonNearMiss"
  tokens format="zenith-token-v1" {
    token id="color.p" type="color" value="#334155"
  }
  styles {
  }
  document id="doc.pnm" title="PolygonNearMiss" {
    page id="page.pnm" w=(px)800 h=(px)600 {
      polygon id="poly.nm" fil=(token)"color.p" {
        point x=(px)100 y=(px)0
        point x=(px)200 y=(px)200
        point x=(px)0 y=(px)200
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);

    assert!(
        has_code(&report, "node.unknown_property"),
        "near-miss typo must fire node.unknown_property; got: {:?}",
        codes(&report)
    );

    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "node.unknown_property")
        .expect("diagnostic must exist");
    assert!(
        diag.message.contains("did you mean 'fill'?"),
        "message must contain \"did you mean 'fill'?\"; got: {:?}",
        diag.message
    );
    assert_eq!(diag.severity, Severity::Warning);
}

/// A polygon with a completely unrelated unknown property (`quantum_flux`) must
/// produce a `node.unknown_property` warning with NO "did you mean?" suggestion.
#[test]
fn polygon_far_miss_unknown_prop_no_suggestion() {
    let src = r##"zenith version=1 {
  project id="proj.pfm" name="PolygonFarMiss"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.pfm" title="PolygonFarMiss" {
    page id="page.pfm" w=(px)800 h=(px)600 {
      polygon id="poly.fm" quantum_flux=1 {
        point x=(px)100 y=(px)0
        point x=(px)200 y=(px)200
        point x=(px)0 y=(px)200
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);

    assert!(
        has_code(&report, "node.unknown_property"),
        "far-miss unknown prop must fire node.unknown_property; got: {:?}",
        codes(&report)
    );

    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "node.unknown_property")
        .expect("diagnostic must exist");
    assert!(
        !diag.message.contains("did you mean"),
        "far-miss message must NOT contain \"did you mean\"; got: {:?}",
        diag.message
    );
    assert!(
        diag.message.contains("version-relative"),
        "far-miss message must mention version-relative; got: {:?}",
        diag.message
    );
    assert_eq!(diag.severity, Severity::Warning);
}

/// A correctly-spelled polygon must produce no `node.unknown_property` diagnostic.
#[test]
fn polygon_correctly_spelled_props_no_unknown_property() {
    let src = r##"zenith version=1 {
  project id="proj.pok" name="PolygonAllKnown"
  tokens format="zenith-token-v1" {
    token id="color.pok" type="color" value="#334155"
  }
  styles {
  }
  document id="doc.pok" title="PolygonAllKnown" {
    page id="page.pok" w=(px)800 h=(px)600 {
      polygon id="poly.ok" fill=(token)"color.pok" {
        point x=(px)100 y=(px)0
        point x=(px)200 y=(px)200
        point x=(px)0 y=(px)200
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);

    assert!(
        !has_code(&report, "node.unknown_property"),
        "correctly-spelled polygon must produce no node.unknown_property; got: {:?}",
        codes(&report)
    );
}

// ── pattern: "did you mean?" triplet ─────────────────────────────────

/// A pattern with the near-miss typo `spacng` (one edit from `spacing`) must
/// produce a `node.unknown_property` warning whose message contains "did you mean 'spacing'?".
#[test]
fn pattern_near_miss_unknown_prop_suggests_did_you_mean() {
    let src = r##"zenith version=1 {
  project id="proj.ptnm" name="PatternNearMiss"
  tokens format="zenith-token-v1" {
    token id="color.dot" type="color" value="#334155"
  }
  styles {
  }
  document id="doc.ptnm" title="PatternNearMiss" {
    page id="page.ptnm" w=(px)800 h=(px)600 {
      pattern id="pat.nm" kind="grid" x=(px)0 y=(px)0 w=(px)800 h=(px)600 spacng=(px)20 {
        ellipse id="e.nm" w=(px)8 h=(px)8 fill=(token)"color.dot"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);

    assert!(
        has_code(&report, "node.unknown_property"),
        "near-miss typo must fire node.unknown_property; got: {:?}",
        codes(&report)
    );

    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "node.unknown_property")
        .expect("diagnostic must exist");
    assert!(
        diag.message.contains("did you mean 'spacing'?"),
        "message must contain \"did you mean 'spacing'?\"; got: {:?}",
        diag.message
    );
    assert_eq!(diag.severity, Severity::Warning);
}

/// A pattern with a completely unrelated unknown property (`quantum_flux`) must
/// produce a `node.unknown_property` warning with NO "did you mean?" suggestion.
#[test]
fn pattern_far_miss_unknown_prop_no_suggestion() {
    let src = r##"zenith version=1 {
  project id="proj.ptfm" name="PatternFarMiss"
  tokens format="zenith-token-v1" {
    token id="color.dot" type="color" value="#334155"
  }
  styles {
  }
  document id="doc.ptfm" title="PatternFarMiss" {
    page id="page.ptfm" w=(px)800 h=(px)600 {
      pattern id="pat.fm" kind="grid" x=(px)0 y=(px)0 w=(px)800 h=(px)600 quantum_flux=1 {
        ellipse id="e.fm" w=(px)8 h=(px)8 fill=(token)"color.dot"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);

    assert!(
        has_code(&report, "node.unknown_property"),
        "far-miss unknown prop must fire node.unknown_property; got: {:?}",
        codes(&report)
    );

    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "node.unknown_property")
        .expect("diagnostic must exist");
    assert!(
        !diag.message.contains("did you mean"),
        "far-miss message must NOT contain \"did you mean\"; got: {:?}",
        diag.message
    );
    assert!(
        diag.message.contains("version-relative"),
        "far-miss message must mention version-relative; got: {:?}",
        diag.message
    );
    assert_eq!(diag.severity, Severity::Warning);
}

/// A correctly-spelled pattern must produce no `node.unknown_property` diagnostic.
#[test]
fn pattern_correctly_spelled_props_no_unknown_property() {
    let src = r##"zenith version=1 {
  project id="proj.ptok" name="PatternAllKnown"
  tokens format="zenith-token-v1" {
    token id="color.dot" type="color" value="#334155"
  }
  styles {
  }
  document id="doc.ptok" title="PatternAllKnown" {
    page id="page.ptok" w=(px)800 h=(px)600 {
      pattern id="pat.ok" kind="grid" x=(px)0 y=(px)0 w=(px)800 h=(px)600 spacing=(px)20 {
        ellipse id="e.ok" w=(px)8 h=(px)8 fill=(token)"color.dot"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let report = validate(&doc);

    assert!(
        !has_code(&report, "node.unknown_property"),
        "correctly-spelled pattern must produce no node.unknown_property; got: {:?}",
        codes(&report)
    );
}
