//! Integration tests for the `variants` block: parse, serialize, and round-trip.
//!
//! Mirrors the provenance round-trip tests in `format_document.rs`. Exercises:
//! - Full parse → field access → format → re-parse → AST equality (spans stripped).
//! - Absent `variants` block → empty vec, no output, byte-identical to before.
//! - Unknown-prop capture (annotated) on both `variant` and `override` nodes.

use zenith_core::format::format_document;
use zenith_core::{KdlAdapter, KdlSource, strip_spans};

// ── variants: parse, serialize, and round-trip ────────────────────────

/// **Round-trip**: parse a doc with a `variants` block (two variants — one
/// with several overrides incl `visible`/`text`/`fill` and an annotated
/// unknown prop, one with no overrides) → format → re-parse → variants
/// identical (spans stripped). Also asserts canonical position (after
/// `provenance`, before `actions`/`document`) and that all fields emit.
#[test]
fn test_variants_round_trip() {
    let src = r##"zenith version=1 {
  project id="proj.var" name="VAR"
  tokens format="zenith-token-v1" {
    token id="color.brand" type="color" value="#ff0000"
  }
  styles {
  }
  provenance {
    origin id="prov.x" node="headline" library="@acme/brand"
  }
  libraries {
    library id="@acme/brand"
  }
  variants {
    variant id="square" source="page.main" w=(px)1080 h=(px)1080 {
      override node="qr" visible=#false
      override node="legal" text="© 2026 Acme"
      override node="headline" fill=(token)"color.brand"
    }
    variant id="story" source="page.main" w=(px)1080 h=(px)1920 {
    }
  }
  document id="doc.var" title="VAR" {
    page id="page.main" w=(px)1920 h=(px)1080 {
      rect id="qr" x=(px)0 y=(px)0 w=(px)100 h=(px)100
      rect id="legal" x=(px)0 y=(px)100 w=(px)200 h=(px)40
      rect id="headline" x=(px)0 y=(px)200 w=(px)400 h=(px)80
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");

    assert_eq!(doc.variants.len(), 2, "expected 2 variants");

    let square = &doc.variants[0];
    assert_eq!(square.id, "square");
    assert_eq!(square.source, "page.main");
    assert_eq!(square.w.value, 1080.0);
    assert_eq!(square.h.value, 1080.0);
    assert_eq!(square.overrides.len(), 3, "square must have 3 overrides");

    let ov_qr = &square.overrides[0];
    assert_eq!(ov_qr.node, "qr");
    assert_eq!(ov_qr.visible, Some(false));
    assert_eq!(ov_qr.text, None);
    assert_eq!(ov_qr.fill, None);

    let ov_legal = &square.overrides[1];
    assert_eq!(ov_legal.node, "legal");
    assert_eq!(ov_legal.visible, None);
    assert_eq!(ov_legal.text.as_deref(), Some("© 2026 Acme"));
    assert_eq!(ov_legal.fill, None);

    let ov_headline = &square.overrides[2];
    assert_eq!(ov_headline.node, "headline");
    assert_eq!(ov_headline.visible, None);
    assert_eq!(ov_headline.text, None);
    assert_eq!(
        ov_headline.fill,
        Some(zenith_core::PropertyValue::TokenRef(
            "color.brand".to_owned()
        ))
    );

    let story = &doc.variants[1];
    assert_eq!(story.id, "story");
    assert_eq!(story.source, "page.main");
    assert_eq!(story.w.value, 1080.0);
    assert_eq!(story.h.value, 1920.0);
    assert!(story.overrides.is_empty(), "story has no overrides");

    let formatted = format_document(&doc).expect("format");
    let formatted_str = String::from_utf8(formatted.clone()).expect("utf8");

    // All key fields must be present.
    assert!(
        formatted_str.contains(r#"variant id="square" source="page.main" w=(px)1080 h=(px)1080"#),
        "square variant line must be present; got:\n{formatted_str}"
    );
    assert!(
        formatted_str.contains("override node=\"qr\" visible=#false"),
        "qr override must emit visible=#false; got:\n{formatted_str}"
    );
    assert!(
        formatted_str.contains(r#"override node="legal" text="© 2026 Acme""#),
        "legal override must emit text; got:\n{formatted_str}"
    );
    assert!(
        formatted_str.contains(r#"override node="headline" fill=(token)"color.brand""#),
        "headline override must emit fill token ref; got:\n{formatted_str}"
    );
    assert!(
        formatted_str.contains(r#"variant id="story" source="page.main" w=(px)1080 h=(px)1920"#),
        "story variant line must be present; got:\n{formatted_str}"
    );

    // Canonical order: provenance, then variants, then document.
    let prov_at = formatted_str
        .find("provenance {")
        .expect("provenance block");
    let variants_at = formatted_str.find("variants {").expect("variants block");
    let doc_at = formatted_str.find("document ").expect("document block");
    assert!(
        prov_at < variants_at && variants_at < doc_at,
        "variants must be emitted after provenance and before document; got:\n{formatted_str}"
    );

    let reparsed = adapter.parse(&formatted).expect("re-parse");
    assert_eq!(
        strip_spans(doc).variants,
        strip_spans(reparsed).variants,
        "variants must survive a parse → format → parse round-trip (idempotent)"
    );
}

/// **Absent `variants` block is an empty vec**: a document with no `variants`
/// block must parse with `doc.variants` empty and format identically (no
/// `variants { … }` emitted in the output).
#[test]
fn test_absent_variants_is_empty_and_byte_identical() {
    let src = r##"zenith version=1 {
  project id="proj.nov" name="NoVariants"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.nov" title="NoVariants" {
    page id="p" w=(px)640 h=(px)360 {
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");

    assert!(
        doc.variants.is_empty(),
        "absent variants block must yield an empty vec"
    );

    let formatted = format_document(&doc).expect("format");
    let formatted_str = String::from_utf8(formatted.clone()).expect("utf8");

    assert!(
        !formatted_str.contains("variants"),
        "no variants block must be emitted for an empty variants vec; got:\n{formatted_str}"
    );

    // Idempotency: output matches byte-for-byte on a second pass.
    let reparsed = adapter.parse(&formatted).expect("re-parse");
    let formatted2 = format_document(&reparsed).expect("format 2");
    assert_eq!(
        formatted, formatted2,
        "absent variants must be byte-identical across two format passes"
    );
}

/// **Override `text` with KDL-special characters must be escaped on emit**: a
/// `text` override is free-form node content and can contain `"`, `\`, and
/// newlines. The writer must escape them so the output re-parses to the exact
/// same string (regression guard — a bare push_str would corrupt the document).
#[test]
fn test_variant_override_text_escaping_round_trip() {
    let tricky = "Say \"hi\"\tand\\or\nbye";
    let src = format!(
        r##"zenith version=1 {{
  project id="proj.esc" name="ESC"
  tokens format="zenith-token-v1" {{
  }}
  styles {{
  }}
  variants {{
    variant id="v" source="page.main" w=(px)800 h=(px)600 {{
      override node="legal" text={text:?}
    }}
  }}
  document id="doc.esc" title="ESC" {{
    page id="page.main" w=(px)800 h=(px)600 {{
      rect id="legal" x=(px)0 y=(px)0 w=(px)10 h=(px)10
    }}
  }}
}}
"##,
        text = tricky
    );
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");
    assert_eq!(
        doc.variants[0].overrides[0].text.as_deref(),
        Some(tricky),
        "the tricky text must parse back exactly"
    );

    // Format then re-parse: the escaped emit must round-trip to the same string.
    let formatted = format_document(&doc).expect("format");
    let reparsed = adapter.parse(&formatted).expect("re-parse escaped output");
    assert_eq!(
        reparsed.variants[0].overrides[0].text.as_deref(),
        Some(tricky),
        "text with quotes/backslash/newline must survive parse → format → parse"
    );
    assert_eq!(
        strip_spans(doc).variants,
        strip_spans(reparsed).variants,
        "escaped-text variants must be round-trip identical"
    );
}

/// **Geometry override round-trip**: `x`, `y`, `w`, `h` on an override node
/// must parse into the new AST fields, emit in canonical order (after
/// `visible`, before `fill`/`text`), and survive a full parse → format →
/// parse round-trip with byte-identical output.
#[test]
fn test_variant_override_geometry_round_trip() {
    let src = r##"zenith version=1 {
  project id="proj.geo" name="GEO"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  variants {
    variant id="v.geo" source="page.main" w=(px)1920 h=(px)1080 {
      override node="hero.title" x=(px)100 y=(px)266 w=(px)880 h=(px)340
    }
  }
  document id="doc.geo" title="GEO" {
    page id="page.main" w=(px)1920 h=(px)1080 {
      rect id="hero.title" x=(px)0 y=(px)0 w=(px)400 h=(px)200
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");

    assert_eq!(doc.variants.len(), 1);
    let v = &doc.variants[0];
    assert_eq!(v.overrides.len(), 1);
    let ov = &v.overrides[0];
    assert_eq!(ov.node, "hero.title");

    // Geometry fields must be parsed into the new AST fields.
    let x = ov.x.as_ref().expect("x must be parsed");
    let y = ov.y.as_ref().expect("y must be parsed");
    let w = ov.w.as_ref().expect("w must be parsed");
    let h = ov.h.as_ref().expect("h must be parsed");
    assert_eq!(x.value, 100.0);
    assert_eq!(y.value, 266.0);
    assert_eq!(w.value, 880.0);
    assert_eq!(h.value, 340.0);

    // Geometry must NOT spill into unknown_props.
    assert!(
        ov.unknown_props.is_empty(),
        "geometry keys must not appear in unknown_props; got: {:?}",
        ov.unknown_props.keys().collect::<Vec<_>>()
    );

    let formatted = format_document(&doc).expect("format");
    let formatted_str = String::from_utf8(formatted.clone()).expect("utf8");

    // Canonical emit order: node, then x y w h.
    assert!(
        formatted_str
            .contains(r#"override node="hero.title" x=(px)100 y=(px)266 w=(px)880 h=(px)340"#),
        "geometry override must emit in canonical order; got:\n{formatted_str}"
    );

    let reparsed = adapter.parse(&formatted).expect("re-parse");

    // Second format pass must produce identical bytes (idempotency).
    let formatted2 = format_document(&reparsed).expect("format 2");
    assert_eq!(
        formatted, formatted2,
        "geometry override format must be idempotent"
    );

    assert_eq!(
        strip_spans(doc).variants,
        strip_spans(reparsed).variants,
        "geometry override must survive parse → format → parse round-trip"
    );
}

/// **Partial geometry override**: only some of x/y/w/h present — the absent
/// ones must be `None` in the AST and omitted from the formatted output.
#[test]
fn test_variant_override_partial_geometry_round_trip() {
    let src = r##"zenith version=1 {
  project id="proj.pgeo" name="PGEO"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  variants {
    variant id="v.pgeo" source="page.main" w=(px)800 h=(px)600 {
      override node="box" y=(px)50
    }
  }
  document id="doc.pgeo" title="PGEO" {
    page id="page.main" w=(px)800 h=(px)600 {
      rect id="box" x=(px)0 y=(px)0 w=(px)100 h=(px)100
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");

    let ov = &doc.variants[0].overrides[0];
    assert!(ov.x.is_none(), "x must be absent");
    assert_eq!(ov.y.as_ref().expect("y must be parsed").value, 50.0);
    assert!(ov.w.is_none(), "w must be absent");
    assert!(ov.h.is_none(), "h must be absent");
    assert!(ov.unknown_props.is_empty(), "no spill into unknown_props");

    let formatted = format_document(&doc).expect("format");
    let formatted_str = String::from_utf8(formatted.clone()).expect("utf8");

    // Only y is present on the override line; x/w/h must be omitted there.
    let override_line = formatted_str
        .lines()
        .find(|l| l.trim_start().starts_with("override "))
        .expect("override line present");
    assert_eq!(
        override_line.trim(),
        r#"override node="box" y=(px)50"#,
        "partial geometry override must emit only y; got:\n{formatted_str}"
    );

    let reparsed = adapter.parse(&formatted).expect("re-parse");
    assert_eq!(
        strip_spans(doc).variants,
        strip_spans(reparsed).variants,
        "partial geometry override must survive full round-trip"
    );
}

/// **Byte-identical-when-absent**: an override WITHOUT geometry keys must
/// parse and format exactly as before — no new fields, no new output.
#[test]
fn test_variant_override_without_geometry_is_unchanged() {
    let src = r##"zenith version=1 {
  project id="proj.nog" name="NOG"
  tokens format="zenith-token-v1" {
    token id="color.brand" type="color" value="#ff0000"
  }
  styles {
  }
  variants {
    variant id="v.nog" source="page.main" w=(px)800 h=(px)600 {
      override node="qr" visible=#false
    }
  }
  document id="doc.nog" title="NOG" {
    page id="page.main" w=(px)800 h=(px)600 {
      rect id="qr" x=(px)0 y=(px)0 w=(px)100 h=(px)100
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");

    let ov = &doc.variants[0].overrides[0];
    assert!(ov.x.is_none());
    assert!(ov.y.is_none());
    assert!(ov.w.is_none());
    assert!(ov.h.is_none());

    let formatted = format_document(&doc).expect("format");
    let formatted_str = String::from_utf8(formatted.clone()).expect("utf8");

    // The override line must be exactly as in the original — no geometry noise.
    assert!(
        formatted_str.contains("override node=\"qr\" visible=#false"),
        "override without geometry must be unchanged; got:\n{formatted_str}"
    );

    let reparsed = adapter.parse(&formatted).expect("re-parse");
    assert_eq!(
        strip_spans(doc).variants,
        strip_spans(reparsed).variants,
        "no-geometry override must survive round-trip unchanged"
    );
}

/// **Unknown-prop capture on variant and override**: annotated unknown props
/// must survive parse → format → parse byte-identically on both the `variant`
/// node and its `override` children.
#[test]
fn test_variant_unknown_props_round_trip() {
    let src = r##"zenith version=1 {
  project id="proj.ukn" name="UKN"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  variants {
    variant id="v1" source="page.main" w=(px)800 h=(px)600 format=(token)"fmt.token" {
      override node="box" priority=(px)2
    }
  }
  document id="doc.ukn" title="UKN" {
    page id="page.main" w=(px)800 h=(px)600 {
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");

    assert_eq!(doc.variants.len(), 1);
    let v = &doc.variants[0];

    let format_prop = v
        .unknown_props
        .get("format")
        .expect("annotated unknown prop `format` must be captured on variant");
    assert_eq!(
        format_prop.ty.as_deref(),
        Some("token"),
        "annotation on variant unknown prop must survive"
    );

    assert_eq!(v.overrides.len(), 1);
    let ov = &v.overrides[0];
    let priority_prop = ov
        .unknown_props
        .get("priority")
        .expect("annotated unknown prop `priority` must be captured on override");
    assert_eq!(
        priority_prop.ty.as_deref(),
        Some("px"),
        "annotation on override unknown prop must survive"
    );

    let formatted = format_document(&doc).expect("format");
    let formatted_str = String::from_utf8(formatted.clone()).expect("utf8");

    assert!(
        formatted_str.contains(r#"format=(token)"fmt.token""#),
        "annotated unknown prop on variant must round-trip; got:\n{formatted_str}"
    );
    assert!(
        formatted_str.contains("priority=(px)2"),
        "annotated unknown prop on override must round-trip; got:\n{formatted_str}"
    );

    let reparsed = adapter.parse(&formatted).expect("re-parse");
    assert_eq!(
        strip_spans(doc).variants,
        strip_spans(reparsed).variants,
        "variants with unknown props must survive full round-trip"
    );
}
