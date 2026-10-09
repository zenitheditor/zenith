//! The bytes an applied transaction writes: the user's source, patched.

use zenith_core::{Document, Patched, patch_source};
use zenith_tx::{TxResult, TxStatus};

/// The source text an `--apply` writes for `result`.
///
/// `doc_src` is the text that parsed to `before`. An accepted edit patches
/// `doc_src` in place, so comments and layout survive, structural edits
/// included. The patcher falls back to canonical text (`reformatted: true`)
/// only for layouts it cannot edit exactly. A rejected or no-op transaction
/// keeps `doc_src` unchanged.
pub fn written_source(doc_src: &str, before: &Document, result: &TxResult) -> Patched {
    let unchanged = Patched {
        text: doc_src.to_owned(),
        reformatted: false,
    };
    match result.status {
        TxStatus::Rejected => return unchanged,
        TxStatus::Accepted | TxStatus::AcceptedWithWarnings => {}
    }
    if result.source_before == result.source_after {
        return unchanged;
    }
    // `patch_source` errors only when `document_after` has no canonical text.
    // `source_after` is that text from the engine, so it is the fallback.
    patch_source(doc_src, before, &result.document_after).unwrap_or_else(|_| Patched {
        text: result.source_after.clone(),
        reformatted: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::{KdlAdapter, KdlSource};
    use zenith_tx::{Transaction, run_transaction};

    const DOC: &str = r##"zenith version=1 {
  tokens format="zenith-token-v1" {
    token id="c" type="color" value="#112233"
  }
  styles {}
  document id="d" {
    // The only page.
    page id="p" w=(px)200 h=(px)100 {
      rect id="r" x=(px)0 y=(px)0 w=(px)10 h=(px)10 // box
    }
  }
}
"##;

    fn run(tx: &str) -> (Document, TxResult) {
        let doc = KdlAdapter.parse(DOC.as_bytes()).expect("parse");
        let tx = Transaction::from_json(tx).expect("tx");
        let result = run_transaction(&doc, &tx).expect("run");
        (doc, result)
    }

    #[test]
    fn accepted_edit_keeps_comments() {
        let (doc, result) = run(r#"{"ops":[{"op":"set_opacity","node":"r","opacity":0.5}]}"#);
        let out = written_source(DOC, &doc, &result);
        assert!(!out.reformatted);
        assert!(out.text.contains("// The only page."), "{}", out.text);
        assert!(out.text.contains("opacity=0.5"), "{}", out.text);
        assert!(out.text.contains("// box"), "{}", out.text);
    }

    #[test]
    fn rejected_edit_keeps_the_source() {
        let (doc, result) = run(r#"{"ops":[{"op":"set_opacity","node":"nope","opacity":0.5}]}"#);
        assert_eq!(result.status, TxStatus::Rejected);
        let out = written_source(DOC, &doc, &result);
        assert_eq!(out.text, DOC);
        assert!(!out.reformatted);
    }

    #[test]
    fn structural_edit_keeps_comments() {
        let (doc, result) = run(r#"{"ops":[{"op":"remove_node","node":"r"}]}"#);
        let out = written_source(DOC, &doc, &result);
        assert!(!out.reformatted);
        assert_eq!(
            out.text,
            DOC.replace(
                " {\n      rect id=\"r\" x=(px)0 y=(px)0 w=(px)10 h=(px)10 // box\n    }",
                " {}"
            )
        );
    }

    #[test]
    fn inline_block_insert_reformats() {
        let src = DOC.replace(
            "      rect id=\"r\" x=(px)0 y=(px)0 w=(px)10 h=(px)10 // box\n",
            "      text id=\"t\" x=(px)0 y=(px)0 w=(px)10 h=(px)10 { span \"a\" }\n",
        );
        let doc = KdlAdapter.parse(src.as_bytes()).expect("parse");
        let tx = Transaction::from_json(
            r#"{"ops":[{"op":"replace_text","node":"t","spans":[{"text":"a"},{"text":"b"}]}]}"#,
        )
        .expect("tx");
        let result = run_transaction(&doc, &tx).expect("run");
        let out = written_source(&src, &doc, &result);
        assert!(out.reformatted);
        assert_eq!(out.text, result.source_after);
    }
}
