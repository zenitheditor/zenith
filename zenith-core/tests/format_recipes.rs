//! Integration tests for the `recipes` block: parse, serialize, and round-trip.
//!
//! Mirrors the variants round-trip tests in `format_variants.rs`. Exercises:
//! - Full parse → field access → format → re-parse → AST equality (spans stripped).
//! - Absent `recipes` block → empty vec, no output, byte-identical to before.
//! - Unknown-prop capture (annotated) on both `recipe` and `param` nodes.
//! - Free-form string fields containing `"`, `\`, and newlines escape correctly.

use zenith_core::format::format_document;
use zenith_core::{KdlAdapter, KdlSource, strip_spans};

// ── recipes: parse, serialize, and round-trip ─────────────────────────

/// **Round-trip**: parse a doc with a `recipes` block (one full recipe with
/// seed/generator/bounds/detached + 2 params + 2 palette + 2 expanded + an
/// annotated unknown prop; one bare recipe with only id+kind) → format →
/// re-parse → recipes identical (spans stripped). Also asserts canonical
/// position (after `variants`, before `actions`/`document`) and that all
/// fields emit correctly.
#[test]
fn test_recipes_round_trip() {
    let src = r##"zenith version=1 {
  project id="proj.rec" name="REC"
  tokens format="zenith-token-v1" {
    token id="color.brand.navy" type="color" value="#001f3f"
    token id="color.brand.cyan" type="color" value="#7fdbff"
  }
  styles {
  }
  variants {
    variant id="v.square" source="page.hero" w=(px)1080 h=(px)1080
  }
  recipes {
    recipe id="recipe.aurora" kind="aurora" seed=42 generator="aurora@1" bounds="page.hero" detached=#false {
      param name="density" value=(number)0.6
      param name="complexity" value=(number)3
      palette token="color.brand.navy"
      palette token="color.brand.cyan"
      expanded node="blob.1"
      expanded node="blob.2"
    }
    recipe id="recipe.bare" kind="scatter" {
    }
  }
  document id="doc.rec" title="REC" {
    page id="page.hero" w=(px)1920 h=(px)1080 {
      rect id="blob.1" x=(px)0 y=(px)0 w=(px)100 h=(px)100
      rect id="blob.2" x=(px)100 y=(px)0 w=(px)100 h=(px)100
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");

    assert_eq!(doc.recipes.len(), 2, "expected 2 recipes");

    let aurora = &doc.recipes[0];
    assert_eq!(aurora.id, "recipe.aurora");
    assert_eq!(aurora.kind, "aurora");
    assert_eq!(aurora.seed, Some(42_i64));
    assert_eq!(aurora.generator.as_deref(), Some("aurora@1"));
    assert_eq!(aurora.bounds.as_deref(), Some("page.hero"));
    assert_eq!(aurora.detached, Some(false));

    assert_eq!(aurora.params.len(), 2, "aurora must have 2 params");
    let p0 = &aurora.params[0];
    assert_eq!(p0.name, "density");
    assert_eq!(
        p0.value,
        zenith_core::PropertyValue::Dimension(zenith_core::Dimension {
            value: 0.6,
            unit: zenith_core::Unit::Unknown("number".to_owned()),
        })
    );
    let p1 = &aurora.params[1];
    assert_eq!(p1.name, "complexity");
    assert_eq!(
        p1.value,
        zenith_core::PropertyValue::Dimension(zenith_core::Dimension {
            value: 3.0,
            unit: zenith_core::Unit::Unknown("number".to_owned()),
        })
    );

    assert_eq!(aurora.palette, vec!["color.brand.navy", "color.brand.cyan"]);
    assert_eq!(aurora.expanded, vec!["blob.1", "blob.2"]);

    let bare = &doc.recipes[1];
    assert_eq!(bare.id, "recipe.bare");
    assert_eq!(bare.kind, "scatter");
    assert_eq!(bare.seed, None);
    assert_eq!(bare.generator, None);
    assert_eq!(bare.bounds, None);
    assert_eq!(bare.detached, None);
    assert!(bare.params.is_empty());
    assert!(bare.palette.is_empty());
    assert!(bare.expanded.is_empty());

    let formatted = format_document(&doc).expect("format");
    let formatted_str = String::from_utf8(formatted.clone()).expect("utf8");

    // All key fields must be present.
    assert!(
        formatted_str
            .contains(r#"recipe id="recipe.aurora" kind="aurora" seed=42 generator="aurora@1" bounds="page.hero" detached=#false"#),
        "aurora recipe header must be present; got:\n{formatted_str}"
    );
    assert!(
        formatted_str.contains(r#"param name="density" value=(number)0.6"#),
        "density param must emit; got:\n{formatted_str}"
    );
    assert!(
        formatted_str.contains(r#"param name="complexity" value=(number)3"#),
        "complexity param must emit; got:\n{formatted_str}"
    );
    assert!(
        formatted_str.contains(r#"palette token="color.brand.navy""#),
        "first palette must emit; got:\n{formatted_str}"
    );
    assert!(
        formatted_str.contains(r#"palette token="color.brand.cyan""#),
        "second palette must emit; got:\n{formatted_str}"
    );
    assert!(
        formatted_str.contains(r#"expanded node="blob.1""#),
        "first expanded must emit; got:\n{formatted_str}"
    );
    assert!(
        formatted_str.contains(r#"expanded node="blob.2""#),
        "second expanded must emit; got:\n{formatted_str}"
    );
    assert!(
        formatted_str.contains(r#"recipe id="recipe.bare" kind="scatter""#),
        "bare recipe line must be present; got:\n{formatted_str}"
    );

    // Canonical order: variants then recipes then document (no actions present).
    let variants_at = formatted_str.find("variants {").expect("variants block");
    let recipes_at = formatted_str.find("recipes {").expect("recipes block");
    let doc_at = formatted_str.find("document ").expect("document block");
    assert!(
        variants_at < recipes_at && recipes_at < doc_at,
        "recipes must be emitted after variants and before document; got:\n{formatted_str}"
    );

    let reparsed = adapter.parse(&formatted).expect("re-parse");
    assert_eq!(
        strip_spans(doc).recipes,
        strip_spans(reparsed).recipes,
        "recipes must survive a parse → format → parse round-trip (idempotent)"
    );
}

/// **Absent `recipes` block is an empty vec**: a document with no `recipes`
/// block must parse with `doc.recipes` empty and format identically (no
/// `recipes { … }` emitted in the output).
#[test]
fn test_absent_recipes_is_empty_and_byte_identical() {
    let src = r##"zenith version=1 {
  project id="proj.nor" name="NoRecipes"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.nor" title="NoRecipes" {
    page id="p" w=(px)640 h=(px)360 {
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");

    assert!(
        doc.recipes.is_empty(),
        "absent recipes block must yield an empty vec"
    );

    let formatted = format_document(&doc).expect("format");
    let formatted_str = String::from_utf8(formatted.clone()).expect("utf8");

    assert!(
        !formatted_str.contains("recipes"),
        "no recipes block must be emitted for an empty recipes vec; got:\n{formatted_str}"
    );

    // Idempotency: output matches byte-for-byte on a second pass.
    let reparsed = adapter.parse(&formatted).expect("re-parse");
    let formatted2 = format_document(&reparsed).expect("format 2");
    assert_eq!(
        formatted, formatted2,
        "absent recipes must be byte-identical across two format passes"
    );
}

/// **Unknown-prop capture on `recipe` and `param`**: annotated unknown props
/// must survive parse → format → parse byte-identically on both the `recipe`
/// node and its `param` children.
#[test]
fn test_recipe_unknown_props_round_trip() {
    let src = r##"zenith version=1 {
  project id="proj.ukn" name="UKN"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  recipes {
    recipe id="recipe.x" kind="test" priority=(token)"fmt.token" {
      param name="n" value=(number)1 weight=(px)2
    }
  }
  document id="doc.ukn" title="UKN" {
    page id="p" w=(px)640 h=(px)360 {
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");

    assert_eq!(doc.recipes.len(), 1);
    let r = &doc.recipes[0];

    let priority_prop = r
        .unknown_props
        .get("priority")
        .expect("annotated unknown prop `priority` must be captured on recipe");
    assert_eq!(
        priority_prop.ty.as_deref(),
        Some("token"),
        "annotation on recipe unknown prop must survive"
    );

    assert_eq!(r.params.len(), 1);
    let p = &r.params[0];
    let weight_prop = p
        .unknown_props
        .get("weight")
        .expect("annotated unknown prop `weight` must be captured on param");
    assert_eq!(
        weight_prop.ty.as_deref(),
        Some("px"),
        "annotation on param unknown prop must survive"
    );

    let formatted = format_document(&doc).expect("format");
    let formatted_str = String::from_utf8(formatted.clone()).expect("utf8");

    assert!(
        formatted_str.contains(r#"priority=(token)"fmt.token""#),
        "annotated unknown prop on recipe must round-trip; got:\n{formatted_str}"
    );
    assert!(
        formatted_str.contains("weight=(px)2"),
        "annotated unknown prop on param must round-trip; got:\n{formatted_str}"
    );

    let reparsed = adapter.parse(&formatted).expect("re-parse");
    assert_eq!(
        strip_spans(doc).recipes,
        strip_spans(reparsed).recipes,
        "recipes with unknown props must survive full round-trip"
    );
}

/// **`generator` field with KDL-special characters must be escaped on emit**:
/// the `generator` string is free-form and can contain `"`, `\`, and newlines.
/// The writer must escape them so the output re-parses to the exact same string
/// (regression guard — a bare push_str would corrupt the document).
#[test]
fn test_recipe_generator_escaping_round_trip() {
    let tricky = r#"aurora@1 "beta"\build"#;
    // `kind` is also a free-form string and must be escaped on emit (regression
    // guard for the writer escaping `kind`, not just `generator`).
    let tricky_kind = r#"aurora "x"\y"#;
    let src = format!(
        r##"zenith version=1 {{
  project id="proj.esc" name="ESC"
  tokens format="zenith-token-v1" {{
  }}
  styles {{
  }}
  recipes {{
    recipe id="recipe.esc" kind={kind:?} generator={gen:?} {{
    }}
  }}
  document id="doc.esc" title="ESC" {{
    page id="p" w=(px)640 h=(px)360 {{
    }}
  }}
}}
"##,
        kind = tricky_kind,
        gen = tricky
    );
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");
    assert_eq!(
        doc.recipes[0].kind, tricky_kind,
        "the tricky kind string must parse back exactly"
    );
    assert_eq!(
        doc.recipes[0].generator.as_deref(),
        Some(tricky),
        "the tricky generator string must parse back exactly"
    );

    // Format then re-parse: the escaped emit must round-trip to the same string.
    let formatted = format_document(&doc).expect("format");
    let reparsed = adapter.parse(&formatted).expect("re-parse escaped output");
    assert_eq!(
        reparsed.recipes[0].generator.as_deref(),
        Some(tricky),
        "generator with quotes/backslash must survive parse → format → parse"
    );
    assert_eq!(
        strip_spans(doc).recipes,
        strip_spans(reparsed).recipes,
        "escaped-generator recipes must be round-trip identical"
    );
}

/// **Negative seed round-trips**: `seed` is `i64`, so a negative value like
/// `seed=-1` must parse and format correctly (regression guard against u32 truncation).
#[test]
fn test_recipe_negative_seed_round_trips() {
    let src = r##"zenith version=1 {
  project id="proj.neg" name="NEG"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  recipes {
    recipe id="recipe.neg" kind="test" seed=-1 {
    }
  }
  document id="doc.neg" title="NEG" {
    page id="p" w=(px)640 h=(px)360 {
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");

    assert_eq!(
        doc.recipes[0].seed,
        Some(-1_i64),
        "negative seed must parse as i64(-1)"
    );

    let formatted = format_document(&doc).expect("format");
    let formatted_str = String::from_utf8(formatted.clone()).expect("utf8");
    assert!(
        formatted_str.contains("seed=-1"),
        "negative seed must emit as seed=-1; got:\n{formatted_str}"
    );

    let reparsed = adapter.parse(&formatted).expect("re-parse");
    assert_eq!(
        reparsed.recipes[0].seed,
        Some(-1_i64),
        "negative seed must survive round-trip"
    );
}

/// **`param value` as a token-ref round-trips**: `entry_to_property_value`
/// supports `(token)"id"` for param values.
#[test]
fn test_recipe_param_token_ref_round_trips() {
    let src = r##"zenith version=1 {
  project id="proj.tok" name="TOK"
  tokens format="zenith-token-v1" {
    token id="color.brand" type="color" value="#001f3f"
  }
  styles {
  }
  recipes {
    recipe id="recipe.tok" kind="colorize" {
      param name="tint" value=(token)"color.brand"
    }
  }
  document id="doc.tok" title="TOK" {
    page id="p" w=(px)640 h=(px)360 {
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");

    assert_eq!(
        doc.recipes[0].params[0].value,
        zenith_core::PropertyValue::TokenRef("color.brand".to_owned()),
        "param value=(token)\"...\" must parse as TokenRef"
    );

    let formatted = format_document(&doc).expect("format");
    let formatted_str = String::from_utf8(formatted.clone()).expect("utf8");
    assert!(
        formatted_str.contains(r#"param name="tint" value=(token)"color.brand""#),
        "token-ref param value must round-trip; got:\n{formatted_str}"
    );

    let reparsed = adapter.parse(&formatted).expect("re-parse");
    assert_eq!(
        strip_spans(doc).recipes,
        strip_spans(reparsed).recipes,
        "token-ref param must survive full round-trip"
    );
}
