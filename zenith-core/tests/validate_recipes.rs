//! Integration tests: recipes block validation.
//!
//! Covers all four recipe-check diagnostics:
//!   - `recipe.duplicate_id`
//!   - `recipe.unknown_palette_token`
//!   - `recipe.unknown_expanded_node`
//!   - `recipe.unknown_bounds`
//!
//! Plus a clean-recipes regression guard.

mod common;

use common::*;

// ── Helper: parse a raw .zen source and run validate ─────────────────────────

fn parse_and_validate(src: &str) -> ValidationReport {
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    validate(&doc)
}

// ── Clean recipes → no recipe.* diagnostics ──────────────────────────────────

/// A document with a well-formed `recipes` block must produce no recipe.*
/// diagnostics. Palette references a real color token; expanded names a real
/// node; bounds names a real page id.
#[test]
fn valid_recipes_block_is_clean() {
    let src = r##"zenith version=1 {
  project id="proj.rc" name="RC"
  tokens format="zenith-token-v1" {
    token id="color.brand" type="color" value="#001f3f"
  }
  styles {
  }
  recipes {
    recipe id="recipe.aurora" kind="aurora" bounds="page.main" {
      palette token="color.brand"
      expanded node="blob.a"
    }
  }
  document id="doc.rc" title="RC" {
    page id="page.main" w=(px)1920 h=(px)1080 {
      rect id="blob.a" x=(px)0 y=(px)0 w=(px)100 h=(px)100
    }
  }
}
"##;
    let report = parse_and_validate(src);
    let recipe_codes: Vec<&str> = report
        .diagnostics
        .iter()
        .filter(|d| d.code.starts_with("recipe."))
        .map(|d| d.code.as_str())
        .collect();
    assert!(
        recipe_codes.is_empty(),
        "clean recipes block must produce no recipe.* diagnostics; got {:?}",
        recipe_codes
    );
}

/// A recipe whose `bounds` names a real node id (not a page id) must also be
/// clean — bounds may reference any page or node id.
#[test]
fn valid_recipes_bounds_node_id_is_clean() {
    let src = r##"zenith version=1 {
  project id="proj.bn" name="BN"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  recipes {
    recipe id="recipe.r" kind="scatter" bounds="frame.container" {
    }
  }
  document id="doc.bn" title="BN" {
    page id="page.main" w=(px)1920 h=(px)1080 {
      frame id="frame.container" x=(px)0 y=(px)0 w=(px)400 h=(px)300 {
      }
    }
  }
}
"##;
    let report = parse_and_validate(src);
    let recipe_codes: Vec<&str> = report
        .diagnostics
        .iter()
        .filter(|d| d.code.starts_with("recipe."))
        .map(|d| d.code.as_str())
        .collect();
    assert!(
        recipe_codes.is_empty(),
        "bounds naming a real node id must produce no recipe.* diagnostics; got {:?}",
        recipe_codes
    );
}

// ── recipe.duplicate_id ───────────────────────────────────────────────────────

/// Two `recipe` entries with the same `id` → `recipe.duplicate_id`.
/// The first occurrence is accepted; only the second emits the diagnostic.
#[test]
fn duplicate_recipe_id_is_error() {
    let src = r##"zenith version=1 {
  project id="proj.dup" name="DUP"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  recipes {
    recipe id="recipe.a" kind="aurora" {
    }
    recipe id="recipe.a" kind="scatter" {
    }
  }
  document id="doc.dup" title="DUP" {
    page id="page.main" w=(px)1920 h=(px)1080 {
    }
  }
}
"##;
    let report = parse_and_validate(src);
    assert!(
        has_code(&report, "recipe.duplicate_id"),
        "duplicate recipe id must produce recipe.duplicate_id; got {:?}",
        codes(&report)
    );
}

// ── recipe.unknown_palette_token ──────────────────────────────────────────────

/// A palette entry referencing a token id that is not declared in the tokens
/// block → `recipe.unknown_palette_token`.
#[test]
fn palette_referencing_undeclared_token_is_error() {
    let src = r##"zenith version=1 {
  project id="proj.pt" name="PT"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  recipes {
    recipe id="recipe.a" kind="aurora" {
      palette token="color.missing"
    }
  }
  document id="doc.pt" title="PT" {
    page id="page.main" w=(px)1920 h=(px)1080 {
    }
  }
}
"##;
    let report = parse_and_validate(src);
    assert!(
        has_code(&report, "recipe.unknown_palette_token"),
        "palette referencing undeclared token must produce recipe.unknown_palette_token; got {:?}",
        codes(&report)
    );
}

/// A palette entry referencing a declared token whose type is NOT color (e.g.
/// `dimension`) → `recipe.unknown_palette_token` (wrong type).
#[test]
fn palette_referencing_non_color_token_is_error() {
    let src = r##"zenith version=1 {
  project id="proj.nc" name="NC"
  tokens format="zenith-token-v1" {
    token id="dim.spacing" type="dimension" value=(px)8
  }
  styles {
  }
  recipes {
    recipe id="recipe.a" kind="aurora" {
      palette token="dim.spacing"
    }
  }
  document id="doc.nc" title="NC" {
    page id="page.main" w=(px)1920 h=(px)1080 {
    }
  }
}
"##;
    let report = parse_and_validate(src);
    assert!(
        has_code(&report, "recipe.unknown_palette_token"),
        "palette referencing non-color token must produce recipe.unknown_palette_token; got {:?}",
        codes(&report)
    );
}

// ── recipe.unknown_expanded_node ──────────────────────────────────────────────

/// An `expanded` entry naming a node id that does not exist anywhere in the
/// document → `recipe.unknown_expanded_node`.
#[test]
fn expanded_referencing_absent_node_is_error() {
    let src = r##"zenith version=1 {
  project id="proj.en" name="EN"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  recipes {
    recipe id="recipe.a" kind="aurora" {
      expanded node="ghost.node"
    }
  }
  document id="doc.en" title="EN" {
    page id="page.main" w=(px)1920 h=(px)1080 {
      rect id="real.node" x=(px)0 y=(px)0 w=(px)100 h=(px)100
    }
  }
}
"##;
    let report = parse_and_validate(src);
    assert!(
        has_code(&report, "recipe.unknown_expanded_node"),
        "expanded referencing absent node must produce recipe.unknown_expanded_node; got {:?}",
        codes(&report)
    );
}

// ── recipe.unknown_bounds ─────────────────────────────────────────────────────

/// `recipe.bounds` naming an id that is neither a page id nor a node id →
/// `recipe.unknown_bounds`.
#[test]
fn unknown_bounds_id_is_error() {
    let src = r##"zenith version=1 {
  project id="proj.ub" name="UB"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  recipes {
    recipe id="recipe.a" kind="aurora" bounds="frame.nowhere" {
    }
  }
  document id="doc.ub" title="UB" {
    page id="page.main" w=(px)1920 h=(px)1080 {
    }
  }
}
"##;
    let report = parse_and_validate(src);
    assert!(
        has_code(&report, "recipe.unknown_bounds"),
        "bounds naming unknown id must produce recipe.unknown_bounds; got {:?}",
        codes(&report)
    );
}

/// `recipe.bounds` naming a real page id → no `recipe.unknown_bounds`.
#[test]
fn bounds_naming_real_page_is_clean() {
    let src = r##"zenith version=1 {
  project id="proj.bp" name="BP"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  recipes {
    recipe id="recipe.a" kind="aurora" bounds="page.main" {
    }
  }
  document id="doc.bp" title="BP" {
    page id="page.main" w=(px)1920 h=(px)1080 {
    }
  }
}
"##;
    let report = parse_and_validate(src);
    assert!(
        !has_code(&report, "recipe.unknown_bounds"),
        "bounds naming a real page id must not produce recipe.unknown_bounds; got {:?}",
        codes(&report)
    );
}
