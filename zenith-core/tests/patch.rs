//! Format-preserving source patcher: table tests over the public API.
//!
//! Each case parses a source, edits the AST, patches the source, and checks
//! the result: it parses to the edited document, keeps every comment, and
//! leaves untouched lines byte-identical. Cases the patcher cannot do exactly
//! must fall back to canonical text with `reformatted: true`.
//!
//! Structural edits (add, remove, move, group, reparent) live in
//! `patch/structure.rs`.

#[path = "patch/structure.rs"]
mod structure;

use zenith_core::{
    Dimension, Document, KdlAdapter, KdlSource, Node, PatchErrorCode, PropertyValue, Unit,
    patch_source, strip_spans, try_patch_source,
};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parse(src: &str) -> Document {
    KdlAdapter
        .parse(src.as_bytes())
        .unwrap_or_else(|e| panic!("parse failed: {}\n{src}", e.message))
}

fn find_in<'a>(nodes: &'a mut [Node], id: &str) -> Option<&'a mut Node> {
    for node in nodes {
        if node.id() == Some(id) {
            return Some(node);
        }
        if let Some(children) = node.children_mut()
            && let Some(found) = find_in(children, id)
        {
            return Some(found);
        }
    }
    None
}

fn node_mut<'a>(doc: &'a mut Document, id: &str) -> &'a mut Node {
    for page in &mut doc.body.pages {
        if let Some(n) = find_in(&mut page.children, id) {
            return n;
        }
    }
    panic!("node {id} not found");
}

fn rect_mut<'a>(doc: &'a mut Document, id: &str) -> &'a mut zenith_core::RectNode {
    match node_mut(doc, id) {
        Node::Rect(r) => r,
        other => panic!("{id} is not a rect: {other:?}"),
    }
}

fn text_mut<'a>(doc: &'a mut Document, id: &str) -> &'a mut zenith_core::TextNode {
    match node_mut(doc, id) {
        Node::Text(t) => t,
        other => panic!("{id} is not a text: {other:?}"),
    }
}

fn px(v: f64) -> PropertyValue {
    PropertyValue::Dimension(Dimension {
        value: v,
        unit: Unit::Px,
    })
}

/// Patch `src` from `before` to `after` and require an exact in-place patch.
fn patch_exact(src: &str, before: &Document, after: &Document) -> String {
    let text =
        try_patch_source(src, before, after).unwrap_or_else(|e| panic!("patch failed: {e}\n{src}"));
    assert_eq!(
        strip_spans(parse(&text)),
        strip_spans(after.clone()),
        "patched text must parse to the edited document:\n{text}"
    );
    let full = patch_source(src, before, after).expect("patch_source");
    assert!(!full.reformatted);
    assert_eq!(full.text, text);
    text
}

/// Every `//` comment in `src` survives in `out`.
fn assert_comments_kept(src: &str, out: &str) {
    for line in src.lines() {
        if let Some(i) = line.find("//") {
            let comment = &line[i..];
            assert!(out.contains(comment), "lost comment {comment:?}:\n{out}");
        }
    }
}

/// Lines of `src` and `out` are equal except the lines that contain `marker`.
fn assert_only_marked_lines_change(src: &str, out: &str, marker: &str) {
    let a: Vec<&str> = src.lines().filter(|l| !l.contains(marker)).collect();
    let b: Vec<&str> = out.lines().filter(|l| !l.contains(marker)).collect();
    assert_eq!(a, b, "lines without {marker:?} changed:\n{out}");
}

const BASE: &str = r##"zenith version=1 {
  // Palette.
  tokens format="zenith-token-v1" {
    token id="c.a" type="color" value="#112233" // first colour
    token id="c.b" type="color" value="#445566"
  }
  styles {}
  /* Body follows. */
  document id="d" {
    page id="p" w=(px)400 h=(px)300 {
      // The card.
      rect id="card" x=(px)0 y=(px)0 w=(px)100 h=(px)50 fill=(token)"c.a"

      rect id="card.a" x=(pt)10 y=(px)60 w=(px)100 h=(px)50 fill=(token)"c.a" // second card
      rect id="card.a.b" x=(px)0 y=(px)120 w=(px)100 h=(px)50 fill=(token)"c.a"
      text id="label" x=(px)0 y=(px)200 w=(px)200 h=(px)40 fill=(token)"c.b" {
        span "Hello" // greeting
      }
      text id="empty" x=(px)0 y=(px)250 w=(px)200 h=(px)40
    }
  }
}
"##;

// ── Property edits ────────────────────────────────────────────────────────────

#[test]
fn property_value_changed() {
    let before = parse(BASE);
    let mut after = before.clone();
    rect_mut(&mut after, "card").x = Some(px(24.0));
    let out = patch_exact(BASE, &before, &after);
    assert!(out.contains(r#"rect id="card" x=(px)24 y=(px)0"#), "{out}");
    assert_comments_kept(BASE, &out);
    assert_only_marked_lines_change(BASE, &out, r#"id="card" "#);
}

#[test]
fn property_added() {
    let before = parse(BASE);
    let mut after = before.clone();
    rect_mut(&mut after, "card").opacity = Some(0.5);
    let out = patch_exact(BASE, &before, &after);
    assert!(out.contains("opacity=0.5"), "{out}");
    assert_only_marked_lines_change(BASE, &out, r#"id="card" "#);
}

#[test]
fn property_removed() {
    let before = parse(BASE);
    let mut after = before.clone();
    rect_mut(&mut after, "card").fill = None;
    let out = patch_exact(BASE, &before, &after);
    assert!(
        out.contains("      rect id=\"card\" x=(px)0 y=(px)0 w=(px)100 h=(px)50\n"),
        "{out}"
    );
    assert_only_marked_lines_change(BASE, &out, r#"id="card" "#);
}

#[test]
fn unit_of_untouched_property_is_preserved() {
    let before = parse(BASE);
    let mut after = before.clone();
    rect_mut(&mut after, "card.a").y = Some(px(64.0));
    let out = patch_exact(BASE, &before, &after);
    assert!(
        out.contains(r#"rect id="card.a" x=(pt)10 y=(px)64"#),
        "{out}"
    );
}

#[test]
fn changed_value_keeps_its_unit() {
    let before = parse(BASE);
    let mut after = before.clone();
    rect_mut(&mut after, "card.a").x = Some(PropertyValue::Dimension(Dimension {
        value: 12.0,
        unit: Unit::Pt,
    }));
    let out = patch_exact(BASE, &before, &after);
    assert!(
        out.contains(r#"rect id="card.a" x=(pt)12 y=(px)60"#),
        "{out}"
    );
}

#[test]
fn token_reference_changed() {
    let before = parse(BASE);
    let mut after = before.clone();
    rect_mut(&mut after, "card").fill = Some(PropertyValue::TokenRef("c.b".to_owned()));
    let out = patch_exact(BASE, &before, &after);
    assert!(out.contains(r#"h=(px)50 fill=(token)"c.b""#), "{out}");
    assert_only_marked_lines_change(BASE, &out, r#"id="card" "#);
}

#[test]
fn similar_ids_edit_only_the_named_node() {
    let before = parse(BASE);
    let mut after = before.clone();
    rect_mut(&mut after, "card.a").fill = Some(PropertyValue::TokenRef("c.b".to_owned()));
    let out = patch_exact(BASE, &before, &after);
    assert_only_marked_lines_change(BASE, &out, r#"id="card.a" "#);
    assert!(out.contains(r#"fill=(token)"c.b" // second card"#), "{out}");
}

#[test]
fn trailing_comment_on_edited_line_survives() {
    let before = parse(BASE);
    let mut after = before.clone();
    rect_mut(&mut after, "card.a").h = Some(px(55.0));
    let out = patch_exact(BASE, &before, &after);
    assert!(
        out.contains(r#"h=(px)55 fill=(token)"c.a" // second card"#),
        "{out}"
    );
    assert_comments_kept(BASE, &out);
}

#[test]
fn token_value_changed() {
    let before = parse(BASE);
    let mut after = before.clone();
    after.tokens.tokens[0].value =
        zenith_core::TokenValue::Literal(zenith_core::TokenLiteral::String("#abcdef".to_owned()));
    let out = patch_exact(BASE, &before, &after);
    assert!(
        out.contains(r##"token id="c.a" type="color" value="#abcdef" // first colour"##),
        "{out}"
    );
    assert_only_marked_lines_change(BASE, &out, r#"id="c.a""#);
}

#[test]
fn doc_id_is_inserted_on_the_root_line() {
    let before = parse(BASE);
    let mut after = before.clone();
    after.doc_id = Some("01HZX".to_owned());
    let out = patch_exact(BASE, &before, &after);
    assert!(
        out.starts_with("zenith version=1 doc-id=\"01HZX\" {\n"),
        "{out}"
    );
    assert_only_marked_lines_change(BASE, &out, "zenith version=1");
}

#[test]
fn unchanged_document_returns_source_bytes() {
    let before = parse(BASE);
    let out = patch_exact(BASE, &before, &before);
    assert_eq!(out, BASE);
}

// ── Child edits ───────────────────────────────────────────────────────────────

#[test]
fn span_text_changed() {
    let before = parse(BASE);
    let mut after = before.clone();
    text_mut(&mut after, "label").spans[0].text = "Bye".to_owned();
    let out = patch_exact(BASE, &before, &after);
    assert!(out.contains("        span \"Bye\" // greeting\n"), "{out}");
    assert_only_marked_lines_change(BASE, &out, "span ");
}

#[test]
fn child_added_after_existing_children() {
    let before = parse(BASE);
    let mut after = before.clone();
    let label = text_mut(&mut after, "label");
    let mut extra = label.spans[0].clone();
    extra.text = " world".to_owned();
    label.spans.push(extra);
    let out = patch_exact(BASE, &before, &after);
    assert!(
        out.contains("        span \"Hello\" // greeting\n        span \" world\"\n      }"),
        "{out}"
    );
    assert_comments_kept(BASE, &out);
    assert_only_marked_lines_change(BASE, &out, "span \" world\"");
}

#[test]
fn child_removed() {
    let src = BASE.replace(
        "        span \"Hello\" // greeting\n",
        "        span \"Hello\" // greeting\n        span \"!\"\n",
    );
    let before = parse(&src);
    let mut after = before.clone();
    text_mut(&mut after, "label").spans.pop();
    let out = patch_exact(&src, &before, &after);
    assert_eq!(out, BASE);
}

#[test]
fn node_without_children_gets_its_first_child() {
    let before = parse(BASE);
    let mut after = before.clone();
    let span = text_mut(&mut after, "label").spans[0].clone();
    text_mut(&mut after, "empty").spans.push(span);
    let out = patch_exact(BASE, &before, &after);
    assert!(
        out.contains(
            "      text id=\"empty\" x=(px)0 y=(px)250 w=(px)200 h=(px)40 {\n        span \"Hello\"\n      }\n"
        ),
        "{out}"
    );
    assert_comments_kept(BASE, &out);
}

#[test]
fn duplicate_looking_lines_edit_the_right_one() {
    let src = r##"zenith version=1 {
  tokens format="zenith-token-v1" {}
  styles {}
  document id="d" {
    page id="p" w=(px)400 h=(px)300 {
      text id="t1" x=(px)0 y=(px)0 w=(px)100 h=(px)20 {
        span "Same"
        span "Same"
      }
      text id="t2" x=(px)0 y=(px)0 w=(px)100 h=(px)20 {
        span "Same"
        span "Same"
      }
    }
  }
}
"##;
    let before = parse(src);
    let mut after = before.clone();
    text_mut(&mut after, "t2").spans[1].text = "Other".to_owned();
    let out = patch_exact(src, &before, &after);
    let expected = src.replacen(
        "        span \"Same\"\n      }\n    }",
        "        span \"Other\"\n      }\n    }",
        1,
    );
    assert_eq!(out, expected);
}

// ── Layout variants ───────────────────────────────────────────────────────────

#[test]
fn crlf_line_endings_are_kept() {
    let src = BASE.replace('\n', "\r\n");
    let before = parse(&src);
    let mut after = before.clone();
    rect_mut(&mut after, "card").x = Some(px(7.0));
    let span = text_mut(&mut after, "label").spans[0].clone();
    text_mut(&mut after, "empty").spans.push(span);
    let out = patch_exact(&src, &before, &after);
    assert_eq!(
        out.matches('\n').count(),
        out.matches("\r\n").count(),
        "{out:?}"
    );
    assert!(out.contains("x=(px)7"));
    assert!(
        out.contains("h=(px)40 {\r\n        span \"Hello\"\r\n      }\r\n"),
        "{out:?}"
    );
}

#[test]
fn tab_indentation_is_kept() {
    let src = BASE
        .lines()
        .map(|l| {
            let n = l.len() - l.trim_start_matches(' ').len();
            format!("{}{}\n", "\t".repeat(n / 2), &l[n..])
        })
        .collect::<String>();
    let before = parse(&src);
    let mut after = before.clone();
    let span = text_mut(&mut after, "label").spans[0].clone();
    text_mut(&mut after, "empty").spans.push(span);
    let out = patch_exact(&src, &before, &after);
    assert!(
        out.contains("\t\t\ttext id=\"empty\" x=(px)0 y=(px)250 w=(px)200 h=(px)40 {\n\t\t\t\tspan \"Hello\"\n\t\t\t}\n"),
        "{out}"
    );
}

#[test]
fn deeply_nested_node_is_patched() {
    let src = r##"zenith version=1 {
  tokens format="zenith-token-v1" {}
  styles {}
  document id="d" {
    page id="p" w=(px)400 h=(px)300 {
      frame id="f1" x=(px)0 y=(px)0 w=(px)400 h=(px)300 {
        frame id="f2" x=(px)0 y=(px)0 w=(px)300 h=(px)200 {
          frame id="f3" x=(px)0 y=(px)0 w=(px)200 h=(px)100 {
                // odd indentation on purpose
                rect id="deep" x=(px)1 y=(px)2 w=(px)3 h=(px)4
          }
        }
      }
    }
  }
}
"##;
    let before = parse(src);
    let mut after = before.clone();
    rect_mut(&mut after, "deep").w = Some(px(30.0));
    let out = patch_exact(src, &before, &after);
    assert_eq!(out, src.replace("w=(px)3 h=(px)4", "w=(px)30 h=(px)4"));
}

#[test]
fn inline_brace_block_edits_in_place() {
    let src = r##"zenith version=1 {
  tokens format="zenith-token-v1" {}
  styles {}
  document id="d" {
    page id="p" w=(px)400 h=(px)300 {
      text id="t" x=(px)0 y=(px)0 w=(px)100 h=(px)20 { span "Hi" } // inline
    }
  }
}
"##;
    let before = parse(src);
    let mut after = before.clone();
    text_mut(&mut after, "t").spans[0].text = "Hey".to_owned();
    text_mut(&mut after, "t").x = Some(px(5.0));
    let out = patch_exact(src, &before, &after);
    assert_eq!(
        out,
        src.replace(
            r#"x=(px)0 y=(px)0 w=(px)100 h=(px)20 { span "Hi" }"#,
            r#"x=(px)5 y=(px)0 w=(px)100 h=(px)20 { span "Hey" }"#
        )
    );
}

#[test]
fn inline_brace_block_new_child_falls_back() {
    let src = r##"zenith version=1 {
  tokens format="zenith-token-v1" {}
  styles {}
  document id="d" {
    page id="p" w=(px)400 h=(px)300 {
      text id="t" x=(px)0 y=(px)0 w=(px)100 h=(px)20 { span "Hi" }
    }
  }
}
"##;
    let before = parse(src);
    let mut after = before.clone();
    let t = text_mut(&mut after, "t");
    let extra = t.spans[0].clone();
    t.spans.push(extra);
    let err = try_patch_source(src, &before, &after).expect_err("inline block");
    assert_eq!(err.code, PatchErrorCode::UnsupportedLayout);
    let out = patch_source(src, &before, &after).expect("fallback");
    assert!(out.reformatted);
    assert_eq!(strip_spans(parse(&out.text)), strip_spans(after));
}

#[test]
fn line_continuation_layout_is_patched() {
    let src = r##"zenith version=1 {
  tokens format="zenith-token-v1" {}
  styles {}
  document id="d" {
    page id="p" w=(px)400 h=(px)300 {
      rect id="r" \
        x=(px)0 y=(px)0 \
        w=(px)10 h=(px)10
    }
  }
}
"##;
    let before = parse(src);
    let mut after = before.clone();
    rect_mut(&mut after, "r").y = Some(px(9.0));
    let out = patch_exact(src, &before, &after);
    assert_eq!(out, src.replace("y=(px)0 \\", "y=(px)9 \\"));
}

// ── Canonical and wrong sources ─────────────────────────────────────────────

#[test]
fn canonical_source_patches_to_canonical_text() {
    let before = parse(BASE);
    let canon = String::from_utf8(KdlAdapter.format(&before).expect("format")).expect("utf8");
    let mut after = before.clone();
    rect_mut(&mut after, "card").opacity = Some(0.25);
    let out = patch_exact(&canon, &before, &after);
    let canon_after = String::from_utf8(KdlAdapter.format(&after).expect("format")).expect("utf8");
    assert_eq!(out, canon_after);
}

#[test]
fn wrong_source_never_yields_wrong_text() {
    // The source does not parse to `before`: the patch must still parse to
    // `after` or fall back.
    let before = parse(BASE);
    let other = BASE.replace(r#"rect id="card" x=(px)0"#, r#"rect id="card" x=(px)3"#);
    let mut after = before.clone();
    rect_mut(&mut after, "card").y = Some(px(1.0));
    let out = patch_source(&other, &before, &after).expect("patch");
    assert_eq!(strip_spans(parse(&out.text)), strip_spans(after));
}
