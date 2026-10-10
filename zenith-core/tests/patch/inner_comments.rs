//! Comments written inside one entry (after the key, after `=`, inside or
//! after the type) stay in place when the patch rewrites that entry.

use super::{parse, patch_exact, px, rect_mut};
use zenith_core::PropertyValue;

fn src(rect: &str) -> String {
    format!(
        r##"zenith version=1 {{
  tokens format="zenith-token-v1" {{
    token id="s" type="dimension" value=(px)5
  }}
  styles {{}}
  document id="d" {{
    page id="p" w=(px)400 h=(px)300 {{
      {rect}
    }}
  }}
}}
"##
    )
}

#[test]
fn a_comment_after_the_equals_sign_survives_a_new_value() {
    let text = src(r#"rect id="r" x= /* unit */ (px)10 y=(px)0 w=(px)10 h=(px)10"#);
    let before = parse(&text);
    let mut after = before.clone();
    rect_mut(&mut after, "r").x = Some(px(15.0));
    let out = patch_exact(&text, &before, &after);
    assert!(
        out.contains(r#"rect id="r" x= /* unit */ (px)15 y=(px)0"#),
        "{out}"
    );
    let mut token = before.clone();
    rect_mut(&mut token, "r").x = Some(PropertyValue::TokenRef("s".to_owned()));
    let out = patch_exact(&text, &before, &token);
    assert!(out.contains(r#"x= /* unit */ (token)"s" y="#), "{out}");
}

#[test]
fn comments_inside_and_after_the_type_survive() {
    let text = src(r#"rect id="r" x=(/* a */px /* b */) /* c */ 10 y=(px)0 w=(px)10 h=(px)10"#);
    let before = parse(&text);
    let mut after = before.clone();
    rect_mut(&mut after, "r").x = Some(px(-2.5));
    let out = patch_exact(&text, &before, &after);
    assert!(
        out.contains(r#"x=(/* a */px /* b */) /* c */ -2.5 y="#),
        "{out}"
    );
}

#[test]
fn type_comments_move_before_a_value_with_no_type() {
    let text = src(r#"rect id="r" x=(px)0 y=(px)0 w=(px)10 h=(px)10 opacity= /* o */ 0.5"#);
    let before = parse(&text);
    let mut after = before.clone();
    rect_mut(&mut after, "r").opacity = Some(0.25);
    let out = patch_exact(&text, &before, &after);
    assert!(out.contains("opacity= /* o */ 0.25"), "{out}");
}

#[test]
fn an_entry_without_inner_comments_takes_the_canonical_text() {
    let text = src(r#"rect id="r" /* lead */ x=(px)1 y=(px)0 w=(px)10 h=(px)10"#);
    let before = parse(&text);
    let mut after = before.clone();
    rect_mut(&mut after, "r").x = Some(px(2.0));
    let out = patch_exact(&text, &before, &after);
    assert!(
        out.contains(r#"rect id="r" /* lead */ x=(px)2 y="#),
        "{out}"
    );
}
