//! Pure logic for `zenith tx` and `zenith outline-text`.
//!
//! The entry points operate on in-memory source text. The caller does all
//! filesystem I/O and decides whether to persist `source_after` (the
//! `--apply` flag lives in the dispatcher, not here).

use std::path::Path;

use zenith_core::fix::unified_diff;
use zenith_core::{Document, KdlAdapter, KdlSource, Severity};
use zenith_scene::collect_text_outline_paths;
use zenith_tx::{
    Op, TextOutlineRequest, Transaction, TxResult, TxStatus, apply_text_outline_paths,
    check_text_outline_source, reject_text_outline, run_transaction,
};

use super::boxes::{BoxSides, box_deltas, page_box_warnings, page_boxes};
use super::render::{TxView, render_human, render_json};
use super::tree::Tree;

// ── Error type ────────────────────────────────────────────────────────────────

/// An error that prevents a [`TxOutcome`] from being produced.
///
/// Returned for doc-parse failures or transaction-JSON-parse failures.
/// A *rejected* transaction still produces a `TxOutcome` (not a `TxCmdErr`).
#[derive(Debug)]
pub struct TxCmdErr {
    /// Human-readable message.
    pub message: String,
    /// Recommended exit code (always 2 for parse errors).
    pub exit_code: u8,
}

// ── Outcome type ──────────────────────────────────────────────────────────────

/// The computed outcome of a successful transaction run (even a rejected one).
#[derive(Debug)]
pub struct TxOutcome {
    /// The structured result from the engine.
    pub result: TxResult,
    /// Human-readable summary string (ready to print).
    pub human: String,
    /// JSON summary string (ready to print).
    pub json_str: String,
    /// Status-derived exit code: 0 for Accepted/AcceptedWithWarnings, 1 for Rejected.
    pub exit_code: u8,
}

/// What the compiled review of a tx run needs from the caller.
#[derive(Debug, Clone, Copy)]
pub struct TxCtx<'a> {
    /// The document's directory: project fonts, text sources, imports, and
    /// image assets, as on render.
    pub project_dir: Option<&'a Path>,
    /// The file name in the diff headers.
    pub label: &'a str,
    /// Print the source diff and the box delta. A dry-run sets it always;
    /// `--apply` sets it only with `--diff`.
    pub show_diff: bool,
}

// ── Public entry points ───────────────────────────────────────────────────────

/// Parse the document source and transaction JSON, run the transaction engine,
/// and return a [`TxOutcome`]. AST only: no compile, no diff, no box delta.
///
/// Returns `Err(TxCmdErr { exit_code: 2 })` if either the document or the
/// transaction JSON fails to parse.  A *rejected* transaction is **not** an
/// error at this level — it returns `Ok(TxOutcome { exit_code: 1 })`.
///
/// This function never touches the filesystem.
pub fn run(doc_src: &str, tx_json: &str) -> Result<TxOutcome, TxCmdErr> {
    run_inner(doc_src, tx_json, None)
}

/// [`run`], then compile the document before and after the transaction.
///
/// Adds `tx.page_box_changed` warnings for reparent / group / ungroup
/// subjects whose page box changed, and, when `ctx.show_diff` is set, the
/// source diff and the box delta. The compile runs only when the result is
/// not Rejected and the source changed. It reads project files under
/// `ctx.project_dir`.
pub fn run_with(doc_src: &str, tx_json: &str, ctx: &TxCtx<'_>) -> Result<TxOutcome, TxCmdErr> {
    run_inner(doc_src, tx_json, Some(ctx))
}

fn run_inner(doc_src: &str, tx_json: &str, ctx: Option<&TxCtx<'_>>) -> Result<TxOutcome, TxCmdErr> {
    let doc = parse_doc(doc_src)?;

    let tx = Transaction::from_json(tx_json).map_err(|e| TxCmdErr {
        message: format!("error[tx.parse]: {}", e.message),
        exit_code: 2,
    })?;

    let result = run_transaction(&doc, &tx).map_err(engine_err)?;
    Ok(finish(&doc, &tx.ops, result, ctx))
}

/// Parse the document source, build the render-path font provider, materialize
/// a text/code node into outlines, and return a standard tx outcome.
pub fn run_outline_text(
    doc_src: &str,
    ctx: &TxCtx<'_>,
    node: &str,
    id_prefix: &str,
    locked: bool,
) -> Result<TxOutcome, TxCmdErr> {
    let doc = parse_doc(doc_src)?;

    let fonts =
        super::super::render::build_font_provider(&doc, ctx.project_dir, locked).map_err(|e| {
            TxCmdErr {
                message: e.message,
                exit_code: e.exit_code,
            }
        })?;

    // Validate source before multi-page compile (parity with pre-split short-circuit).
    let result = match check_text_outline_source(&doc, node) {
        Err(diags) => reject_text_outline(&doc, diags),
        Ok(()) => {
            let (paths, outline_diags) = collect_text_outline_paths(&doc, &fonts, node, id_prefix);
            apply_text_outline_paths(
                &doc,
                &TextOutlineRequest {
                    node: node.to_owned(),
                },
                paths,
                outline_diags,
            )
        }
    }
    .map_err(engine_err)?;

    Ok(finish(&doc, &[], result, Some(ctx)))
}

// ── Shared tail ───────────────────────────────────────────────────────────────

fn parse_doc(doc_src: &str) -> Result<Document, TxCmdErr> {
    KdlAdapter.parse(doc_src.as_bytes()).map_err(|e| TxCmdErr {
        message: crate::report::parse_error_line(doc_src, &e),
        exit_code: 2,
    })
}

fn engine_err(e: zenith_tx::TxError) -> TxCmdErr {
    TxCmdErr {
        message: format!("error[tx.engine]: {}", e.message),
        exit_code: 2,
    }
}

/// Add the compiled review when `ctx` is set, recompute the status, and
/// render both outputs.
fn finish(doc: &Document, ops: &[Op], mut result: TxResult, ctx: Option<&TxCtx<'_>>) -> TxOutcome {
    let mut view = TxView::default();
    let changed = result.source_before != result.source_after;
    if let Some(ctx) = ctx
        && result.status != TxStatus::Rejected
        && changed
    {
        if ctx.show_diff {
            view.source_diff = Some(unified_diff(
                ctx.label,
                &result.source_before,
                &result.source_after,
            ));
        }
        // `source_after` is canonical output, so it parses. A parse error
        // leaves the box delta out.
        if let Ok(after) = KdlAdapter.parse(result.source_after.as_bytes()) {
            let before_boxes = page_boxes(doc, ctx.project_dir);
            let after_boxes = page_boxes(&after, ctx.project_dir);
            let sides = BoxSides {
                before: &before_boxes,
                after: &after_boxes,
            };
            let after_tree = Tree::of(&after);
            page_box_warnings(ops, doc, &after_tree, &sides, &mut result.diagnostics);
            if ctx.show_diff {
                view.boxes = Some(box_deltas(&before_boxes, &after_boxes, &after_tree));
            }
        }
        if result.status == TxStatus::Accepted
            && result
                .diagnostics
                .iter()
                .any(|d| d.severity == Severity::Warning)
        {
            result.status = TxStatus::AcceptedWithWarnings;
        }
    }

    let exit_code = status_exit_code(&result.status);
    let human = render_human(&result, &view);
    let json_str = render_json(&result, view);
    TxOutcome {
        result,
        human,
        json_str,
        exit_code,
    }
}

// ── Exit-code helper ──────────────────────────────────────────────────────────

/// Map a `TxStatus` to an exit code.
///
/// `Accepted` and `AcceptedWithWarnings` → 0.  `Rejected` → 1.
pub fn status_exit_code(status: &TxStatus) -> u8 {
    match status {
        TxStatus::Accepted | TxStatus::AcceptedWithWarnings => 0,
        TxStatus::Rejected => 1,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal document with a text node and a rect node.
    const SMALL_DOC: &str = r##"zenith version=1 {
  project id="proj.tx" name="Tx Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc.tx" title="Tx" {
    page id="pg.tx" w=(px)400 h=(px)300 {
      rect id="box.tx" x=(px)0 y=(px)0 w=(px)400 h=(px)300
      text id="lbl.tx" x=(px)10 y=(px)10 w=(px)200 h=(px)40 {
        span "hello"
      }
    }
  }
}"##;

    const CTX: TxCtx<'static> = TxCtx {
        project_dir: None,
        label: "doc.zen",
        show_diff: true,
    };

    // ── 1. Valid set_text_align → Accepted, changed, exit 0 ──────────────────

    #[test]
    fn valid_set_text_align_accepted() {
        let tx_json = r#"{"ops":[{"op":"set_text_align","node":"lbl.tx","align":"center"}]}"#;
        let outcome = run(SMALL_DOC, tx_json).expect("should not be a parse error");

        assert_eq!(outcome.exit_code, 0, "Accepted must yield exit code 0");
        assert_eq!(outcome.result.status, TxStatus::Accepted);

        let changed = outcome.result.source_before != outcome.result.source_after;
        assert!(changed, "source must differ after set_text_align");

        assert!(
            outcome
                .result
                .affected_node_ids
                .contains(&"lbl.tx".to_owned()),
            "affected_node_ids must contain lbl.tx"
        );

        assert!(
            outcome.result.source_after.contains("center"),
            "source_after must contain align=\"center\""
        );
    }

    // ── 2. Unknown node → Rejected, unchanged, exit 1 ────────────────────────

    #[test]
    fn unknown_node_rejected_exit_1() {
        let tx_json = r#"{"ops":[{"op":"set_text_align","node":"no.such.node","align":"center"}]}"#;
        let outcome = run(SMALL_DOC, tx_json).expect("should not be a parse error");

        assert_eq!(outcome.exit_code, 1, "Rejected must yield exit code 1");
        assert_eq!(outcome.result.status, TxStatus::Rejected);

        let changed = outcome.result.source_before != outcome.result.source_after;
        assert!(!changed, "source must not change on rejection");

        assert!(
            outcome.result.affected_node_ids.is_empty(),
            "no nodes should be affected on rejection"
        );
    }

    // ── 3. Malformed tx JSON → Err(exit_code 2) ───────────────────────────────

    #[test]
    fn malformed_tx_json_returns_err_exit_2() {
        let tx_json = r#"{"ops": [THIS IS NOT JSON]}"#;
        let err = run(SMALL_DOC, tx_json).expect_err("malformed JSON must be Err");
        assert_eq!(err.exit_code, 2, "parse error must yield exit code 2");
        assert!(!err.message.is_empty(), "error message must not be empty");
    }

    // ── 4. Malformed doc → Err(exit_code 2) ──────────────────────────────────

    #[test]
    fn malformed_doc_returns_err_exit_2() {
        let tx_json = r#"{"ops":[{"op":"set_text_align","node":"x","align":"center"}]}"#;
        let err = run("not kdl at all {{{", tx_json).expect_err("malformed doc must be Err");
        assert_eq!(err.exit_code, 2, "doc parse error must yield exit code 2");
    }

    // ── 5. JSON output contains schema ───────────────────────────────────────

    #[test]
    fn json_output_contains_schema() {
        let tx_json = r#"{"ops":[{"op":"set_text_align","node":"lbl.tx","align":"center"}]}"#;
        let outcome = run(SMALL_DOC, tx_json).expect("should succeed");
        assert!(
            outcome.json_str.contains("zenith-tx-v1"),
            "JSON output must contain schema field; got: {}",
            outcome.json_str
        );
    }

    // ── 6. Human output contains status line ─────────────────────────────────

    #[test]
    fn human_output_contains_status() {
        let tx_json = r#"{"ops":[{"op":"set_text_align","node":"lbl.tx","align":"center"}]}"#;
        let outcome = run(SMALL_DOC, tx_json).expect("should succeed");
        assert!(
            outcome.human.contains("status:"),
            "human output must contain 'status:'; got: {}",
            outcome.human
        );
    }

    // ── 7. AST-only run has no diff; run_with adds it ────────────────────────

    #[test]
    fn run_has_no_review_and_run_with_has_one() {
        let tx_json = r#"{"ops":[{"op":"set_text_align","node":"lbl.tx","align":"center"}]}"#;
        let plain = run(SMALL_DOC, tx_json).expect("should succeed");
        assert!(
            !plain.json_str.contains("source_diff"),
            "{}",
            plain.json_str
        );
        assert!(!plain.json_str.contains("\"boxes\""), "{}", plain.json_str);

        let with = run_with(SMALL_DOC, tx_json, &CTX).expect("should succeed");
        assert!(with.human.contains("--- a/doc.zen"), "{}", with.human);
        assert!(
            with.json_str.contains("\"source_diff\""),
            "{}",
            with.json_str
        );
        assert!(with.json_str.contains("\"boxes\""), "{}", with.json_str);
    }

    #[test]
    fn rejected_run_with_has_no_review() {
        let tx_json = r#"{"ops":[{"op":"set_text_align","node":"nope","align":"center"}]}"#;
        let out = run_with(SMALL_DOC, tx_json, &CTX).expect("should succeed");
        assert_eq!(out.exit_code, 1);
        assert!(!out.json_str.contains("source_diff"), "{}", out.json_str);
    }

    #[test]
    fn outline_text_outputs_standard_tx_summary() {
        let src = r##"zenith version=1 {
  project id="proj.tx" name="Tx Test"
  tokens format="zenith-token-v1" {
    token id="color.ink" type="color" value="#112233"
    token id="size.text" type="dimension" value=(px)32
  }
  styles { }
  document id="doc.tx" title="Tx" {
    page id="pg.tx" w=(px)400 h=(px)300 {
      text id="lbl.tx" x=(px)10 y=(px)40 w=(px)200 h=(px)60 fill=(token)"color.ink" font-size=(token)"size.text" {
        span "Hi"
      }
    }
  }
}"##;
        let outcome = run_outline_text(src, &CTX, "lbl.tx", "lbl.outline", false)
            .expect("outline text should run");

        assert_eq!(outcome.result.status, TxStatus::Accepted);
        assert_eq!(outcome.exit_code, 0);
        assert!(outcome.human.contains("affected: lbl.outline-0"));
        assert!(
            outcome
                .result
                .source_after
                .contains("path id=\"lbl.outline-0\"")
        );
        assert!(
            outcome.human.contains("added lbl.outline-0"),
            "{}",
            outcome.human
        );
    }
}
