//! A KDL parse error names the line that caused it, not only the block it
//! surfaced at.

use zenith_core::{KdlAdapter, KdlSource as _};

const HEAD: &str = "zenith version=1 {\n  project id=\"p\" name=\"P\"\n  tokens format=\"zenith-token-v1\" {\n  }\n  styles {\n  }\n  document id=\"d\" title=\"T\" {\n    page id=\"pg\" w=(px)100 h=(px)100 {\n";

fn message(body: &str) -> String {
    let src = format!("{HEAD}{body}    }}\n  }}\n}}\n");
    KdlAdapter
        .parse(src.as_bytes())
        .expect_err("source must not parse")
        .message
}

#[test]
fn a_stray_paren_names_its_line() {
    let msg = message(
        "      rect id=\"a\" w=(px)10 h=(px)10\n      text id=\"b\" h=(px)80) { span \"hi\" }\n",
    );
    assert!(
        msg.contains("hint: line 10 does not parse on its own"),
        "{msg}"
    );
}

#[test]
fn a_forgotten_close_names_the_line_it_belongs_before() {
    let src = format!(
        "{HEAD}      frame id=\"f\" w=(px)10 h=(px)10 {{\n        rect id=\"a\" w=(px)5 h=(px)5\n      rect id=\"b\" w=(px)5 h=(px)5\n    }}\n  }}\n}}\n"
    );
    let msg = KdlAdapter
        .parse(src.as_bytes())
        .expect_err("source must not parse")
        .message;
    assert!(msg.contains("opened on line 9"), "{msg}");
    assert!(msg.contains("before line 11"), "{msg}");
}

#[test]
fn sibling_nodes_after_a_block_name_the_line_and_the_semicolon() {
    let msg = message(
        "      text id=\"a\" w=(px)10 h=(px)10 { span \"A\" } text id=\"b\" w=(px)10 h=(px)10 { span \"B\" }\n",
    );
    assert!(
        msg.contains("hint: line 9 has sibling nodes on one line"),
        "{msg}"
    );
    assert!(msg.contains("`;`"), "{msg}");
    assert!(!msg.contains("ONE line"), "{msg}");
}

#[test]
fn merged_sibling_rects_are_an_error_and_a_semicolon_fixes_them() {
    let msg = message("      rect id=\"a\" w=(px)10 h=(px)10 rect id=\"b\" w=(px)10 h=(px)10\n");
    assert!(
        msg.contains("sets `id` again after the word `rect`"),
        "{msg}"
    );

    let fixed = format!(
        "{HEAD}      rect id=\"a\" w=(px)10 h=(px)10; rect id=\"b\" w=(px)10 h=(px)10\n      text id=\"c\" w=(px)10 h=(px)10 {{ span \"C\" }}; text id=\"d\" w=(px)10 h=(px)10 {{ span \"D\" }}\n    }}\n  }}\n}}\n"
    );
    let doc = KdlAdapter
        .parse(fixed.as_bytes())
        .expect("`;` separates siblings");
    assert_eq!(doc.body.pages[0].children.len(), 4);
}
