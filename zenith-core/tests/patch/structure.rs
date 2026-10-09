//! Structural edits patch in place.
//!
//! Each case gives a source and the exact text the patch must write. The
//! after document is the parse of that text, so the case also checks that
//! the patch parses to the edited document.
//!
//! Attachment rule under test: `//` lines and whole-line `/* … */` blocks
//! directly above a node belong to it. A blank line detaches them.

use super::{parse, patch_exact};
use zenith_core::{PatchErrorCode, patch_source, strip_spans, try_patch_source};

const S: &str = r##"zenith version=1 {
  tokens format="zenith-token-v1" {
    token id="c.a" type="color" value="#112233" // first colour
    token id="c.b" type="color" value="#445566"
  }
  styles {}
  document id="d" {
    page id="p" w=(px)400 h=(px)300 {
      // Header block.
      // Two lines.
      rect id="a" x=(px)0 y=(px)0 w=(px)10 h=(px)10 // trailing a

      /* Block comment
         for b. */
      rect id="b" x=(px)0 y=(px)20 w=(px)10 h=(px)10
      rect id="c" x=(px)0 y=(px)20 w=(px)10 h=(px)10

      // Detached note.

      frame id="f" x=(px)0 y=(px)100 w=(px)200 h=(px)100 {
          // Hand-indented children.
          rect id="f1" x=(px)0 y=(px)0 w=(px)10 h=(px)10 // inner
      }
      frame id="empty" x=(px)0 y=(px)200 w=(px)10 h=(px)10 {}
      frame id="notes" x=(px)0 y=(px)220 w=(px)10 h=(px)10 {
        // only a comment
      }
    }
  }
}
"##;

const A: &str = "      // Header block.\n      // Two lines.\n      rect id=\"a\" x=(px)0 y=(px)0 w=(px)10 h=(px)10 // trailing a\n";
const B: &str = "      /* Block comment\n         for b. */\n      rect id=\"b\" x=(px)0 y=(px)20 w=(px)10 h=(px)10\n";
const C: &str = "      rect id=\"c\" x=(px)0 y=(px)20 w=(px)10 h=(px)10\n";
const F1: &str = "          // Hand-indented children.\n          rect id=\"f1\" x=(px)0 y=(px)0 w=(px)10 h=(px)10 // inner\n";
const NOTES_END: &str = "        // only a comment\n      }\n";
const NEW: &str = "rect id=\"n\" x=(px)1 y=(px)1 w=(px)2 h=(px)2";

/// `S` with `from` replaced once by `to`. Panics when `from` is absent.
fn s_with(from: &str, to: &str) -> String {
    replace_once(S, from, to)
}

fn replace_once(src: &str, from: &str, to: &str) -> String {
    assert!(src.contains(from), "missing {from:?}");
    src.replacen(from, to, 1)
}

/// `lines` moved from six-space to `indent` indentation.
fn indented(lines: &str, indent: &str) -> String {
    lines
        .lines()
        .map(|l| format!("{indent}{}\n", &l[6..]))
        .collect()
}

/// Patch `src` to the parse of `expected` and require exactly `expected`.
fn edit(src: &str, expected: &str) {
    let before = parse(src);
    let after = parse(expected);
    let out = patch_exact(src, &before, &after);
    assert_eq!(out, expected, "patched text differs");
}

/// Patch `src` to the parse of `target` and require the fallback `code`.
fn falls_back(src: &str, target: &str, code: PatchErrorCode) {
    let before = parse(src);
    let after = parse(target);
    let err = try_patch_source(src, &before, &after).expect_err("must fall back");
    assert_eq!(err.code, code, "{err}");
    let out = patch_source(src, &before, &after).expect("fallback");
    assert!(out.reformatted);
    assert_eq!(strip_spans(parse(&out.text)), strip_spans(after));
}

// ── Remove ────────────────────────────────────────────────────────────────────

#[test]
fn remove_takes_attached_line_comments_and_the_blank_after_the_opener() {
    edit(S, &s_with(&format!("{A}\n"), ""));
}

#[test]
fn remove_takes_an_attached_block_comment() {
    edit(S, &s_with(B, ""));
}

#[test]
fn remove_picks_the_right_identical_looking_neighbour() {
    edit(S, &s_with(C, ""));
}

#[test]
fn remove_leaves_a_detached_comment() {
    let f =
        format!("      frame id=\"f\" x=(px)0 y=(px)100 w=(px)200 h=(px)100 {{\n{F1}      }}\n");
    edit(S, &s_with(&f, ""));
}

#[test]
fn remove_of_the_only_child_collapses_the_block() {
    // The canonical frame keeps an empty `{}` block.
    let f = format!("w=(px)200 h=(px)100 {{\n{F1}      }}\n");
    edit(S, &s_with(&f, "w=(px)200 h=(px)100 {}\n"));
}

#[test]
fn remove_of_neighbours_drops_one_doubled_blank_line() {
    edit(S, &s_with(&format!("{B}{C}\n"), ""));
}

#[test]
fn remove_before_the_closer_drops_the_blank_line_above() {
    let src = s_with(
        NOTES_END,
        "        // only a comment\n\n        rect id=\"x\" x=(px)0 y=(px)0 w=(px)1 h=(px)1\n      }\n",
    );
    edit(&src, S);
}

#[test]
fn remove_of_a_token_takes_its_trailing_comment() {
    edit(
        S,
        &s_with(
            "    token id=\"c.a\" type=\"color\" value=\"#112233\" // first colour\n",
            "",
        ),
    );
}

#[test]
fn remove_of_a_page_takes_its_comment() {
    let page2 = "    // Second page.\n    page id=\"p2\" w=(px)100 h=(px)100 {\n      rect id=\"q\" x=(px)0 y=(px)0 w=(px)1 h=(px)1 // q\n    }\n";
    let src = s_with("    }\n  }\n}\n", &format!("    }}\n{page2}  }}\n}}\n"));
    edit(&src, S);
}

// ── Add ───────────────────────────────────────────────────────────────────────

#[test]
fn add_first_child_goes_above_the_first_childs_comments() {
    edit(S, &s_with(A, &format!("      {NEW}\n{A}")));
}

#[test]
fn add_after_a_sibling_goes_past_its_trailing_comment() {
    edit(S, &s_with(A, &format!("{A}      {NEW}\n")));
}

#[test]
fn add_last_child_goes_after_the_last_block() {
    edit(S, &s_with(NOTES_END, &format!("{NOTES_END}      {NEW}\n")));
}

#[test]
fn add_into_an_empty_brace_block() {
    edit(
        S,
        &s_with(
            "h=(px)10 {}\n",
            &format!("h=(px)10 {{\n        {NEW}\n      }}\n"),
        ),
    );
}

#[test]
fn add_into_a_comment_only_block() {
    edit(
        S,
        &s_with(
            NOTES_END,
            &format!("        // only a comment\n        {NEW}\n      }}\n"),
        ),
    );
}

#[test]
fn add_a_token_and_a_page() {
    let token = "    token id=\"c.n\" type=\"color\" value=\"#000000\"\n";
    // The canonical page writes its empty block on two lines.
    let page = "    page id=\"p2\" w=(px)100 h=(px)100 {\n    }\n";
    let expected = replace_once(
        &s_with(
            "    token id=\"c.b\" type=\"color\" value=\"#445566\"\n",
            &format!("    token id=\"c.b\" type=\"color\" value=\"#445566\"\n{token}"),
        ),
        "    }\n  }\n}\n",
        &format!("    }}\n{page}  }}\n}}\n"),
    );
    edit(S, &expected);
}

// ── Move ──────────────────────────────────────────────────────────────────────

#[test]
fn reorder_carries_the_nodes_comments() {
    let removed = s_with(&format!("{A}\n"), "");
    edit(
        S,
        &replace_once(&removed, NOTES_END, &format!("{NOTES_END}{A}")),
    );
}

#[test]
fn reparent_out_of_a_block_reindents_and_collapses_it() {
    let f = format!("w=(px)200 h=(px)100 {{\n{F1}      }}\n");
    let out = s_with(&f, "w=(px)200 h=(px)100 {}\n");
    let moved: String = F1.lines().map(|l| format!("{}\n", &l[4..])).collect();
    edit(
        S,
        &replace_once(&out, NOTES_END, &format!("{NOTES_END}{moved}")),
    );
}

#[test]
fn reparent_into_a_block_with_other_indentation() {
    let removed = s_with(&format!("{A}\n"), "");
    edit(
        S,
        &replace_once(&removed, F1, &format!("{F1}{}", indented(A, "          "))),
    );
}

#[test]
fn reparent_into_an_empty_brace_block() {
    let removed = s_with(&format!("{A}\n"), "");
    edit(
        S,
        &replace_once(
            &removed,
            "h=(px)10 {}\n",
            &format!("h=(px)10 {{\n{}      }}\n", indented(A, "        ")),
        ),
    );
}

const GROUPED: &str = "      group id=\"g\" {\n";

fn grouped() -> String {
    let body = format!("{}{}", indented(B, "        "), indented(C, "        "));
    let with_group = s_with(A, &format!("{A}{GROUPED}{body}      }}\n"));
    replace_once(&with_group, &format!("\n{B}{C}\n"), "\n")
}

#[test]
fn group_wraps_moved_nodes_with_their_comments() {
    edit(S, &grouped());
}

#[test]
fn ungroup_moves_children_out_with_their_comments() {
    edit(
        &grouped(),
        &s_with(&format!("{A}\n{B}"), &format!("{A}{B}")),
    );
}

const NEST: &str = r##"zenith version=1 {
  tokens format="zenith-token-v1" {}
  styles {}
  document id="d" {
    page id="p" w=(px)400 h=(px)300 {
      frame id="f1" x=(px)0 y=(px)0 w=(px)400 h=(px)300 {
        frame id="f2" x=(px)0 y=(px)0 w=(px)300 h=(px)200 {
          frame id="f3" x=(px)0 y=(px)0 w=(px)200 h=(px)100 {
            // deep note
            rect id="deep" x=(px)1 y=(px)2 w=(px)3 h=(px)4
            rect id="keep" x=(px)1 y=(px)2 w=(px)3 h=(px)4
          }
        }
      }
      rect id="top" x=(px)0 y=(px)0 w=(px)1 h=(px)1 // top
    }
  }
}
"##;

#[test]
fn deep_node_moves_to_the_page_and_back() {
    let deep =
        "            // deep note\n            rect id=\"deep\" x=(px)1 y=(px)2 w=(px)3 h=(px)4\n";
    let top = "      rect id=\"top\" x=(px)0 y=(px)0 w=(px)1 h=(px)1 // top\n";
    let up = replace_once(
        &replace_once(NEST, deep, ""),
        top,
        &format!("{top}{}", indented_from(deep, 12, "      ")),
    );
    edit(NEST, &up);
    edit(&up, NEST);
    // Swap a deep node with a top one in one edit.
    let top_in = indented_from(top, 6, "            ");
    let swapped = replace_once(
        &replace_once(NEST, top, &indented_from(deep, 12, "      ")),
        deep,
        &top_in,
    );
    edit(NEST, &swapped);
}

fn indented_from(lines: &str, depth: usize, indent: &str) -> String {
    lines
        .lines()
        .map(|l| format!("{indent}{}\n", &l[depth..]))
        .collect()
}

// ── Layout variants ───────────────────────────────────────────────────────────

#[test]
fn crlf_structural_edits_keep_crlf() {
    let src = S.replace('\n', "\r\n");
    let expected = replace_once(
        &s_with(&format!("{A}\n"), ""),
        NOTES_END,
        &format!("{NOTES_END}      {NEW}\n"),
    )
    .replace('\n', "\r\n");
    edit(&src, &expected);
}

fn tabbed(src: &str) -> String {
    src.lines()
        .map(|l| {
            let n = l.len() - l.trim_start_matches(' ').len();
            format!("{}{}{}\n", "\t".repeat(n / 2), " ".repeat(n % 2), &l[n..])
        })
        .collect()
}

#[test]
fn tab_indented_move_and_add_use_tabs() {
    let removed = s_with(&format!("{A}\n"), "");
    let moved = replace_once(&removed, F1, &format!("{F1}{}", indented(A, "          ")));
    edit(&tabbed(S), &tabbed(&moved));
    let added = s_with(
        "h=(px)10 {}\n",
        &format!("h=(px)10 {{\n        {NEW}\n      }}\n"),
    );
    edit(&tabbed(S), &tabbed(&added));
}

// ── Cases that still fall back ────────────────────────────────────────────────

#[test]
fn inline_block_child_removal_falls_back() {
    let src = s_with(
        "h=(px)10 {}\n",
        "h=(px)10 { rect id=\"x\" x=(px)0 y=(px)0 w=(px)1 h=(px)1 }\n",
    );
    falls_back(&src, S, PatchErrorCode::UnsupportedLayout);
}

#[test]
fn node_sharing_a_line_falls_back() {
    let src = s_with(
        C,
        "      rect id=\"c\" x=(px)0 y=(px)20 w=(px)10 h=(px)10; rect id=\"y\" x=(px)0 y=(px)0 w=(px)1 h=(px)1\n",
    );
    falls_back(&src, S, PatchErrorCode::UnsupportedLayout);
}

#[test]
fn comment_only_block_closing_on_a_text_line_falls_back() {
    let src = s_with(NOTES_END, "        /* only a comment */ }\n");
    let target = s_with(
        NOTES_END,
        &format!("        // only a comment\n        {NEW}\n      }}\n"),
    );
    falls_back(&src, &target, PatchErrorCode::UnsupportedLayout);
}
