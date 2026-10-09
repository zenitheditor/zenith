//! Entry points: [`patch_source`] and [`try_patch_source`].

use kdl::KdlDocument;

use super::diff::{Differ, Index, Layout, Texts};
use super::error::{PatchError, PatchErrorCode};
use super::text::{apply_edits, indent_unit, line_ending};
use crate::ast::{Document, strip_spans};
use crate::format::format_document;
use crate::parse::{KdlAdapter, KdlSource};

/// The text to write after an edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Patched {
    /// The new source text.
    pub text: String,
    /// `true` when `text` is the canonical form of the after document, so
    /// the source's comments and layout are gone.
    pub reformatted: bool,
}

/// Rewrite `text` so it parses to `after`, keeping comments and layout.
///
/// `text` must be the source that parsed to `before`. The patcher diffs the
/// canonical forms of `before` and `after` and edits only the changed
/// entries and child lines of `text`. Added, removed, reordered, and
/// reparented nodes patch in place too: a moved node carries its own source
/// text and attached comments. The fallback lives here, so every
/// caller gets the same rule: when [`try_patch_source`] errors, the result is
/// the canonical text of `after` with `reformatted: true`.
///
/// # Errors
///
/// Returns `patch.format_failed` only when `after` has no canonical text.
pub fn patch_source(
    text: &str,
    before: &Document,
    after: &Document,
) -> Result<Patched, PatchError> {
    let canon_after = canonical(after)?;
    match patch_with(text, before, after, &canon_after) {
        Ok(text) => Ok(Patched {
            text,
            reformatted: false,
        }),
        Err(_) => Ok(Patched {
            text: canon_after,
            reformatted: true,
        }),
    }
}

/// [`patch_source`] without the fallback: the patched text, or why the
/// patcher cannot produce it.
///
/// The returned text always parses to `after` (spans aside), or to the
/// parse of `after`'s canonical text. This re-parse check runs on every
/// call.
///
/// # Errors
///
/// - `patch.invalid_source`: `text` is not valid KDL.
/// - `patch.format_failed`: a document has no canonical text.
/// - `patch.unaligned_source`: a changed node or property has no single
///   counterpart in `text`.
/// - `patch.unsupported_layout`: the layout at an edit point has no exact
///   in-place edit: a new or removed child inside an inline `{ … }` block,
///   a node that shares its line with another node, a new top-level node,
///   or a comment-only child block that closes on a text line.
/// - `patch.reparse_failed` / `patch.verify_mismatch`: the patched text does
///   not parse to `after`.
pub fn try_patch_source(
    text: &str,
    before: &Document,
    after: &Document,
) -> Result<String, PatchError> {
    let canon_after = canonical(after)?;
    patch_with(text, before, after, &canon_after)
}

fn patch_with(
    text: &str,
    before: &Document,
    after: &Document,
    canon_after: &str,
) -> Result<String, PatchError> {
    let canon_before = canonical(before)?;
    let patched = if text == canon_before {
        // A canonical source stays canonical.
        canon_after.to_owned()
    } else {
        diff_and_apply(text, &canon_before, canon_after)?
    };
    verify(&patched, after, canon_after)?;
    Ok(patched)
}

fn diff_and_apply(text: &str, canon_before: &str, canon_after: &str) -> Result<String, PatchError> {
    let src_tree = parse_kdl(text, PatchErrorCode::InvalidSource)?;
    let before_tree = parse_kdl(canon_before, PatchErrorCode::FormatFailed)?;
    let after_tree = parse_kdl(canon_after, PatchErrorCode::FormatFailed)?;
    let texts = Texts {
        src: text,
        before: canon_before,
        after: canon_after,
    };
    let layout = Layout {
        eol: line_ending(text),
        unit: indent_unit(text),
    };
    let index = Index::new(src_tree.nodes(), before_tree.nodes(), after_tree.nodes());
    let mut differ = Differ::new(texts, layout, &index);
    differ.diff_root(src_tree.nodes(), before_tree.nodes(), after_tree.nodes())?;
    apply_edits(text, differ.edits)
}

/// The patched text must parse to `after`, or to what `after`'s canonical
/// text parses to (the bytes a reformatting write would produce).
fn verify(patched: &str, after: &Document, canon_after: &str) -> Result<(), PatchError> {
    let got = KdlAdapter
        .parse(patched.as_bytes())
        .map(strip_spans)
        .map_err(|e| PatchError::new(PatchErrorCode::ReparseFailed, e.message))?;
    if got == strip_spans(after.clone()) {
        return Ok(());
    }
    let canon = KdlAdapter
        .parse(canon_after.as_bytes())
        .map(strip_spans)
        .map_err(|e| PatchError::new(PatchErrorCode::FormatFailed, e.message))?;
    if got == canon {
        return Ok(());
    }
    Err(PatchError::new(
        PatchErrorCode::VerifyMismatch,
        "the patched text does not parse to the edited document",
    ))
}

fn canonical(doc: &Document) -> Result<String, PatchError> {
    let bytes = format_document(doc)
        .map_err(|e| PatchError::new(PatchErrorCode::FormatFailed, e.to_string()))?;
    String::from_utf8(bytes)
        .map_err(|e| PatchError::new(PatchErrorCode::FormatFailed, e.to_string()))
}

fn parse_kdl(text: &str, code: PatchErrorCode) -> Result<KdlDocument, PatchError> {
    text.parse::<KdlDocument>()
        .map_err(|e| PatchError::new(code, e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = r##"zenith version=1 {
  // brand colours
  tokens format="zenith-token-v1" {
    token id="c" type="color" value="#112233"
  }
  styles {}
  document id="d" {
    page id="p" w=(px)100 h=(px)100 {
      rect id="r" x=(px)0 y=(px)0 w=(px)10 h=(px)10 // the box
    }
  }
}
"##;

    fn parse(src: &str) -> Document {
        KdlAdapter.parse(src.as_bytes()).expect("parse")
    }

    #[test]
    fn verify_rejects_text_for_another_document() {
        let before = parse(SRC);
        let mut after = before.clone();
        after.doc_id = Some("01ABC".to_owned());
        let canon_after = canonical(&after).expect("format");
        let err = verify(SRC, &after, &canon_after).expect_err("mismatch");
        assert_eq!(err.code, PatchErrorCode::VerifyMismatch);
    }

    #[test]
    fn verify_rejects_unparsable_text() {
        let doc = parse(SRC);
        let canon = canonical(&doc).expect("format");
        let err = verify("zenith {", &doc, &canon).expect_err("bad");
        assert_eq!(err.code, PatchErrorCode::ReparseFailed);
    }

    #[test]
    fn canonical_source_takes_canonical_after() {
        let before = parse(SRC);
        let canon_before = canonical(&before).expect("format");
        let mut after = before.clone();
        after.doc_id = Some("01ABC".to_owned());
        let canon_after = canonical(&after).expect("format");
        let out = patch_with(&canon_before, &before, &after, &canon_after).expect("patch");
        assert_eq!(out, canon_after);
    }

    #[test]
    fn invalid_source_is_reported() {
        let doc = parse(SRC);
        let err = try_patch_source("zenith {", &doc, &doc).expect_err("invalid");
        assert_eq!(err.code, PatchErrorCode::InvalidSource);
    }
}
