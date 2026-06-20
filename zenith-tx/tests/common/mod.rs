//! Shared helpers and test-document constants used across the integration-test
//! binaries in `zenith-tx/tests/`. Each file under `tests/` is compiled as a
//! separate binary by Cargo; a helper that is present in this module but unused
//! by a particular binary would otherwise trigger spurious `dead_code` /
//! `unused_imports` lints. The crate-level `#![allow(...)]` below is the
//! canonical suppression for this well-known Cargo integration-test pattern —
//! it is NOT hiding real defects.
#![allow(dead_code, unused_imports)]

use zenith_core::{Document, KdlAdapter, KdlSource};

// ── Shared helper functions ───────────────────────────────────────────────────

/// Parse a KDL source string into a [`Document`], panicking on failure.
pub fn parse(src: &str) -> Document {
    KdlAdapter
        .parse(src.as_bytes())
        .expect("test doc must parse")
}

/// Return the page ids from `source`, in document order, by scanning for
/// `page id="…"` occurrences.
pub fn page_id_order(source: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let mut rest = source;
    while let Some(idx) = rest.find("page id=\"") {
        let after = &rest[idx + "page id=\"".len()..];
        if let Some(end) = after.find('"') {
            ids.push(after[..end].to_owned());
            rest = &after[end..];
        } else {
            break;
        }
    }
    ids
}

/// Parse a node's attribute value from `source` by locating the node line via
/// `id="<node_id>"` and then reading `<attr>=(px)<value>` on that line.
/// Intentionally naive — sufficient for the deterministic test documents used
/// throughout this suite.
pub fn extract_px_attr(source: &str, node_id: &str, attr: &str) -> Option<f64> {
    source
        .lines()
        .find(|line| line.contains(&format!("id=\"{node_id}\"")))
        .and_then(|line| {
            let needle = format!("{attr}=(px)");
            let start = line.find(&needle)? + needle.len();
            let rest = &line[start..];
            let end = rest
                .find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')
                .unwrap_or(rest.len());
            rest[..end].parse::<f64>().ok()
        })
}

// ── Shared test-document constants ───────────────────────────────────────────

/// Minimal valid document with a `text` node (align `start`) and a `rect`.
pub const TEXT_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      text id="label" x=(px)10 y=(px)10 w=(px)200 h=(px)40 align="start" {
        span "Hello"
      }
    }
  }
}"##;

pub const TWO_RECT_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="a" x=(px)0 y=(px)0 w=(px)100 h=(px)100
      rect id="b" x=(px)0 y=(px)0 w=(px)100 h=(px)100
    }
  }
}"##;

pub const MIXED_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="box1" x=(px)0 y=(px)0 w=(px)100 h=(px)100
      text id="lbl" x=(px)10 y=(px)10 w=(px)200 h=(px)40 {
        span "Hi"
      }
    }
  }
}"##;

pub const ELLIPSE_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      ellipse id="dot" x=(px)0 y=(px)0 w=(px)100 h=(px)100
      text id="lbl" x=(px)10 y=(px)10 w=(px)200 h=(px)40 {
        span "Hi"
      }
    }
  }
}"##;

pub const IMAGE_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  assets {
    asset id="asset.pic" kind="image" src="pic.png"
  }
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      image id="pic" asset="asset.pic" x=(px)0 y=(px)0 w=(px)100 h=(px)100
      text id="lbl" x=(px)10 y=(px)10 w=(px)200 h=(px)40 {
        span "Hi"
      }
    }
  }
}"##;

pub const GROUP_TEXT_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Nest"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      group id="grp1" {
        text id="nested.label" x=(px)10 y=(px)10 w=(px)200 h=(px)40 align="start" {
          span "Hello"
        }
      }
    }
  }
}"##;

pub const GROUP_TWO_RECT_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      group id="grp1" {
        rect id="a" x=(px)0 y=(px)0 w=(px)100 h=(px)100
        rect id="b" x=(px)0 y=(px)0 w=(px)100 h=(px)100
      }
    }
  }
}"##;

/// Rect with fill token A; token B also declared so post-validate passes.
pub const FILL_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.a" type="color" value="#ff0000"
    token id="color.b" type="color" value="#0000ff"
  }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="r1" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"color.a"
    }
  }
}"##;

/// Line node (no fill field).
pub const LINE_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.a" type="color" value="#ff0000"
  }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      line id="ln1" x1=(px)0 y1=(px)0 x2=(px)100 y2=(px)100 stroke=(token)"color.a"
    }
  }
}"##;

/// Page with one code node (fill via a declared token).
pub const CODE_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.a" type="color" value="#ff0000"
    token id="color.b" type="color" value="#0000ff"
  }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      code id="snip" x=(px)0 y=(px)0 w=(px)200 h=(px)100 fill=(token)"color.a" {
        content "fn main() {}"
      }
    }
  }
}"##;

/// Rect inside a group.
pub const NESTED_RECT_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      group id="grp1" {
        rect id="inner" x=(px)0 y=(px)0 w=(px)50 h=(px)50
      }
    }
  }
}"##;

/// Rect, line, polygon carrying valid color + dimension tokens.
pub const STROKE_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.rule" type="color" value="#334155"
    token id="size.stroke" type="dimension" value=(px)2
  }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="r1" x=(px)0 y=(px)0 w=(px)100 h=(px)100 stroke=(token)"color.rule" stroke-width=(token)"size.stroke"
      line id="ln1" x1=(px)0 y1=(px)0 x2=(px)100 y2=(px)100 stroke=(token)"color.rule"
      ellipse id="dot" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"color.rule"
      text id="lbl" x=(px)10 y=(px)10 w=(px)200 h=(px)40 {
        span "Hi"
      }
      polygon id="poly1" stroke=(token)"color.rule" stroke-width=(token)"size.stroke" {
        point x=(px)10 y=(px)10
        point x=(px)90 y=(px)10
        point x=(px)50 y=(px)90
      }
    }
  }
}"##;

/// Rect at origin, 100×100. No tokens needed for geometry ops.
pub const RECT_GEOM_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="rect" x=(px)0 y=(px)0 w=(px)100 h=(px)100
    }
  }
}"##;

/// Polygon with exactly 3 points and a fill token (to keep post-validate happy).
pub const POLY_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.fill" type="color" value="#ff0000"
  }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      polygon id="poly" fill=(token)"color.fill" {
        point x=(px)0 y=(px)0
        point x=(px)100 y=(px)0
        point x=(px)50 y=(px)80
      }
    }
  }
}"##;

/// Three rects a (index 0, bottom), b (index 1), c (index 2, top).
pub const THREE_RECT_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="a" x=(px)0 y=(px)0 w=(px)100 h=(px)100
      rect id="b" x=(px)10 y=(px)0 w=(px)100 h=(px)100
      rect id="c" x=(px)20 y=(px)0 w=(px)100 h=(px)100
    }
  }
}"##;

/// Group containing two rects: x (bottom) then y (top).
pub const GROUP_TWO_RECT_BACKWARD_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      group id="grp1" {
        rect id="x" x=(px)0 y=(px)0 w=(px)100 h=(px)100
        rect id="y" x=(px)10 y=(px)0 w=(px)100 h=(px)100
      }
    }
  }
}"##;

/// Page with one rect; an accent color token declared so added rects that
/// reference it pass post-validation.
pub const ADD_BASE_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.accent" type="color" value="#3b82f6"
  }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)320 h=(px)200 {
      rect id="base" x=(px)0 y=(px)0 w=(px)100 h=(px)100
    }
  }
}"##;

/// Page with a group that contains two rects.
pub const ADD_GROUP_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)320 h=(px)200 {
      group id="grp1" {
        rect id="g.a" x=(px)0 y=(px)0 w=(px)50 h=(px)50
        rect id="g.b" x=(px)0 y=(px)0 w=(px)50 h=(px)50
      }
    }
  }
}"##;

/// Document with a single rect and a fill token (needed for post-validate).
pub const DUP_RECT_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.a" type="color" value="#ff0000"
  }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="orig" x=(px)10 y=(px)20 w=(px)80 h=(px)60 fill=(token)"color.a"
    }
  }
}"##;

/// Document with a group containing a rect (for container-rejection test).
pub const DUP_GROUP_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      group id="grp" {
        rect id="inner" x=(px)0 y=(px)0 w=(px)50 h=(px)50
      }
    }
  }
}"##;

/// Single page with two leaf nodes; used for duplicate_page tests.
pub const DUP_PAGE_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.a" type="color" value="#ff0000"
  }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="r1" x=(px)10 y=(px)20 w=(px)80 h=(px)60 fill=(token)"color.a"
      rect id="r2" x=(px)10 y=(px)20 w=(px)80 h=(px)60 fill=(token)"color.a"
    }
  }
}"##;

/// A two-page document used to exercise the page-structure ops.
pub const TWO_PAGE_STRUCT_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#ffffff"
  }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="r1" x=(px)0 y=(px)0 w=(px)100 h=(px)100
    }
    page id="pg2" w=(px)400 h=(px)300 {
      rect id="r2" x=(px)0 y=(px)0 w=(px)100 h=(px)100
    }
  }
}"##;

/// Two sibling rects on a page; used for group/reparent tests.
pub const TWO_SIBLING_RECTS: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="r1" x=(px)0 y=(px)0 w=(px)100 h=(px)100
      rect id="r2" x=(px)0 y=(px)0 w=(px)100 h=(px)100
    }
  }
}"##;

/// A page with a group that already exists (for ungroup / reparent tests).
pub const PAGE_WITH_GROUP: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      group id="grp1" {
        rect id="r1" x=(px)0 y=(px)0 w=(px)100 h=(px)100
        rect id="r2" x=(px)0 y=(px)0 w=(px)100 h=(px)100
      }
      rect id="r3" x=(px)0 y=(px)0 w=(px)50 h=(px)50
    }
  }
}"##;

/// A page with a group that has a non-zero x/y offset (advisory test).
pub const PAGE_WITH_OFFSET_GROUP: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      group id="grp1" x=(px)50 y=(px)20 {
        rect id="r1" x=(px)0 y=(px)0 w=(px)100 h=(px)100
      }
    }
  }
}"##;

/// A page with a group nested inside another group (cycle check + reparent).
pub const NESTED_GROUPS: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      group id="outer" {
        group id="inner" {
          rect id="r1" x=(px)0 y=(px)0 w=(px)50 h=(px)50
        }
      }
    }
  }
}"##;

/// Three sibling rects at different x positions (10, 50, 90) on a 400×300
/// page; all have the same width (80px).
pub const THREE_RECTS_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="r1" x=(px)10 y=(px)20 w=(px)80 h=(px)50
      rect id="r2" x=(px)50 y=(px)60 w=(px)80 h=(px)50
      rect id="r3" x=(px)90 y=(px)100 w=(px)80 h=(px)50
    }
  }
}"##;

/// Doc with two rects and one group; the group has no resolvable bbox.
pub const RECTS_AND_GROUP_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="r1" x=(px)20 y=(px)0 w=(px)60 h=(px)40
      rect id="r2" x=(px)80 y=(px)0 w=(px)60 h=(px)40
      group id="grp1" { }
    }
  }
}"##;

/// Three sibling text/code/rect nodes for overflow + dimension-anchor tests.
pub const TEXT_CODE_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      text id="body" x=(px)10 y=(px)10 w=(px)200 h=(px)40 align="start" {
        span "Hello"
      }
      code id="snip" x=(px)10 y=(px)60 w=(px)200 h=(px)100 {
        content "fn main() {}"
      }
    }
  }
}"##;

/// Document with a `styles` block containing a named style with an existing
/// property, plus a `fontFamily` token and a dimension token so post-validation
/// passes when either is referenced from styles.
pub const STYLE_PROP_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="font.body" type="fontFamily" value="Inter"
    token id="size.md" type="dimension" value=(px)16
    token id="color.accent" type="color" value="#3b82f6"
  }
  styles {
    style id="s.heading" {
      font-size (token)"size.md"
    }
  }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="r1" x=(px)0 y=(px)0 w=(px)100 h=(px)100
    }
  }
}"##;

/// Three rects unevenly placed on the x axis: positions 0, 30, 100, widths 20.
/// Span = (100+20) - 0 = 120. Σsizes = 60. gap = (120-60)/2 = 30.
/// Distributed leading edges: 0, 0+20+30=50, 50+20+30=100.
pub const DISTRIBUTE_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="p1" x=(px)0 y=(px)0 w=(px)20 h=(px)20
      rect id="p2" x=(px)30 y=(px)0 w=(px)20 h=(px)20
      rect id="p3" x=(px)100 y=(px)0 w=(px)20 h=(px)20
    }
  }
}"##;
