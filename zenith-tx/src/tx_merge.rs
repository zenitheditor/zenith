//! Deterministic conflict detection and merging for concurrent scoped
//! transactions (`merge_transactions`).
//!
//! Enables multiple humans or AI agents to propose independent [`Transaction`]s
//! against the same base [`Document`] without a CRDT server:
//! - Disjoint edits (different nodes, tokens, or pages) merge cleanly into one
//!   validated, deterministic [`TxResult`].
//! - Overlapping edits (same node id, same token/style/master id, or a
//!   document-wide op such as `find_replace_text` / `reorder_pages`) are
//!   surfaced as explicit [`TxConflict`] records with zero silent overwrites.

use std::collections::BTreeSet;

use zenith_core::Document;

use crate::engine::run_transaction;
use crate::op::{Op, Permissions, Transaction};
use crate::result::{TxError, TxResult, TxStatus};

/// Sentinel scope marking an op that touches document-wide order or text.
const DOC_SCOPE: &str = "__document__";

/// One labeled transaction proposed against a shared base document.
#[derive(Debug, Clone, Copy)]
pub struct ScopedTx<'a> {
    /// Human- or agent-readable label (e.g. `"agent-copy"`, `"agent-layout"`).
    pub label: &'a str,
    /// Transaction proposed against the shared base document.
    pub tx: &'a Transaction,
}

/// Pairwise conflict between two concurrent transactions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TxConflict {
    /// Label of the earlier transaction in input order.
    pub left: String,
    /// Label of the later transaction in input order.
    pub right: String,
    /// Sorted list of conflicting node, token, style, page, or document scopes.
    pub scopes: Vec<String>,
}

/// Outcome of [`merge_transactions`].
#[derive(Debug)]
pub enum TxMergeOutcome {
    /// All transactions were pairwise disjoint and merged cleanly.
    Merged(TxResult),
    /// Two or more transactions touched overlapping scopes; nothing was merged.
    Conflicts(Vec<TxConflict>),
    /// One transaction failed validation on its own against `base`.
    IndividualRejected {
        /// Label of the rejected transaction.
        label: String,
        /// Rejected transaction result with diagnostics.
        result: TxResult,
    },
    /// Individual transactions were disjoint, but the combined document failed
    /// post-merge validation (e.g. duplicate created ids).
    PostMergeRejected(TxResult),
}

/// Deterministically detect conflicts among concurrent `inputs` proposed
/// against `base`, and merge them into a single validated transaction when
/// pairwise disjoint.
///
/// # Errors
///
/// Returns [`TxError`] if the underlying transaction engine fails to serialize
/// or execute.
pub fn merge_transactions(
    base: &Document,
    inputs: &[ScopedTx<'_>],
) -> Result<TxMergeOutcome, TxError> {
    let mut touch_sets: Vec<(&str, BTreeSet<String>)> = Vec::with_capacity(inputs.len());

    for input in inputs {
        let res = run_transaction(base, input.tx)?;
        if matches!(res.status, TxStatus::Rejected) {
            return Ok(TxMergeOutcome::IndividualRejected {
                label: input.label.to_owned(),
                result: res,
            });
        }
        let scopes = collect_scopes(input.tx, &res);
        touch_sets.push((input.label, scopes));
    }

    let mut conflicts = Vec::new();
    for (i, (left_label, left_scopes)) in touch_sets.iter().enumerate() {
        for (right_label, right_scopes) in touch_sets.iter().skip(i + 1) {
            let overlap = scope_overlap(left_scopes, right_scopes);
            if !overlap.is_empty() {
                conflicts.push(TxConflict {
                    left: (*left_label).to_owned(),
                    right: (*right_label).to_owned(),
                    scopes: overlap,
                });
            }
        }
    }
    if !conflicts.is_empty() {
        return Ok(TxMergeOutcome::Conflicts(conflicts));
    }

    let mut combined_ops = Vec::new();
    let mut combined_permissions = Permissions::default();
    for input in inputs {
        combined_ops.extend(input.tx.ops.iter().cloned());
        combined_permissions.allow_locked |= input.tx.permissions.allow_locked;
        combined_permissions.allow_raw_visual_literals |=
            input.tx.permissions.allow_raw_visual_literals;
    }
    let combined = Transaction {
        permissions: combined_permissions,
        ops: combined_ops,
    };
    let merged = run_transaction(base, &combined)?;
    match merged.status {
        TxStatus::Accepted | TxStatus::AcceptedWithWarnings => Ok(TxMergeOutcome::Merged(merged)),
        TxStatus::Rejected => Ok(TxMergeOutcome::PostMergeRejected(merged)),
    }
}

fn scope_overlap(left: &BTreeSet<String>, right: &BTreeSet<String>) -> Vec<String> {
    if left.contains(DOC_SCOPE) || right.contains(DOC_SCOPE) {
        if left.is_empty() || right.is_empty() {
            return Vec::new();
        }
        return vec![DOC_SCOPE.to_owned()];
    }
    left.intersection(right).cloned().collect()
}

fn collect_scopes(tx: &Transaction, res: &TxResult) -> BTreeSet<String> {
    let mut scopes = BTreeSet::new();
    for id in &res.affected_node_ids {
        scopes.insert(id.clone());
    }
    for op in &tx.ops {
        for id in op.edited_node_ids() {
            scopes.insert(id.to_owned());
        }
        record_op_scope(op, &mut scopes);
    }
    scopes
}

fn record_op_scope(op: &Op, scopes: &mut BTreeSet<String>) {
    match op {
        Op::FindReplaceText { .. }
        | Op::ReorderPages { .. }
        | Op::MovePage { .. }
        | Op::SwapPages { .. } => {
            scopes.insert(DOC_SCOPE.to_owned());
        }
        Op::CreateToken { id, .. } | Op::UpdateTokenValue { id, .. } => {
            scopes.insert(format!("token:{id}"));
        }
        Op::CreateStyle { id, .. } | Op::DeleteStyle { id, .. } => {
            scopes.insert(format!("style:{id}"));
        }
        Op::SetStyleProperty { style_id, .. } => {
            scopes.insert(format!("style:{style_id}"));
        }
        Op::CreateMaster { id, .. } | Op::DeleteMaster { id, .. } => {
            scopes.insert(format!("master:{id}"));
        }
        Op::AddAsset { id, .. } => {
            scopes.insert(format!("asset:{id}"));
        }
        Op::SetAsset { node_id, .. } => {
            scopes.insert(node_id.clone());
        }
        Op::CreateRecipe { id, .. } | Op::UpdateRecipe { id, .. } | Op::DeleteRecipe { id, .. } => {
            scopes.insert(format!("recipe:{id}"));
        }
        Op::SetDefault { kind, .. } | Op::RemoveDefault { kind, .. } => {
            scopes.insert(format!("default:{kind}"));
        }
        Op::AddPage { id, .. } => {
            scopes.insert(format!("page:{id}"));
        }
        Op::DeletePage { page, .. }
        | Op::SetPageSize { page, .. }
        | Op::SetPageMaster { page, .. } => {
            scopes.insert(format!("page:{page}"));
        }
        Op::DuplicatePage { new_id, .. } => {
            scopes.insert(format!("page:{new_id}"));
        }
        // Node-level ops are already covered by `res.affected_node_ids` + `op.edited_node_ids()`.
        Op::SetFill { .. }
        | Op::SetFillRule { .. }
        | Op::SetStroke { .. }
        | Op::SetStrokeWidth { .. }
        | Op::SetNodeToken { .. }
        | Op::SetSpanText { .. }
        | Op::SetVisible { .. }
        | Op::SetLocked { .. }
        | Op::SetGeometry { .. }
        | Op::NudgeGeometry { .. }
        | Op::SetAnchor(_)
        | Op::NudgeAnchorGap { .. }
        | Op::DetachAnchor { .. }
        | Op::NudgeLinePoints { .. }
        | Op::SetTextAlign { .. }
        | Op::SetPoints { .. }
        | Op::SetPathAnchors { .. }
        | Op::SetPathAnchorKind { .. }
        | Op::RemovePathAnchor { .. }
        | Op::MovePathAnchor { .. }
        | Op::MovePathHandle { .. }
        | Op::InsertPathAnchor { .. }
        | Op::InsertPathAnchorAtPoint { .. }
        | Op::SimplifyPathAnchors { .. }
        | Op::TransformPathAnchors { .. }
        | Op::SnapPathAnchors { .. }
        | Op::MakePathSymmetric { .. }
        | Op::PathBoolean { .. }
        | Op::AddNode { .. }
        | Op::AddPath { .. }
        | Op::RemoveNode { .. }
        | Op::SetOpacity { .. }
        | Op::ReplaceText { .. }
        | Op::DuplicateNode { .. }
        | Op::AlignNodes { .. }
        | Op::DistributeNodes { .. }
        | Op::AlignToEdge { .. }
        | Op::MoveForward { .. }
        | Op::MoveBackward { .. }
        | Op::MoveToFront { .. }
        | Op::MoveToBack { .. }
        | Op::Group { .. }
        | Op::Ungroup { .. }
        | Op::Reparent { .. }
        | Op::SetTextOverflow { .. }
        | Op::SetTextDirection { .. }
        | Op::SetOverride { .. }
        | Op::SetLayout(_)
        | Op::DetachPattern { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::{KdlAdapter, KdlSource};

    const BASE_DOC: &str = r##"zenith version=1 {
  project id="proj.merge" name="Merge"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#ffffff"
    token id="color.ink" type="color" value="#111827"
    token id="color.accent" type="color" value="#2563eb"
    token id="font.body" type="fontFamily" value="Noto Sans"
    token id="size.body" type="dimension" value=(px)16
  }
  styles {}
  document id="doc.merge" title="Merge" {
    page id="page.1" w=(px)400 h=(px)300 background=(token)"color.bg" {
      rect id="card" x=(px)20 y=(px)20 w=(px)120 h=(px)80 fill=(token)"color.ink"
      text id="title" x=(px)20 y=(px)120 w=(px)300 h=(px)40 fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" { span "Old Title" }
    }
  }
}
"##;

    #[test]
    fn disjoint_agent_transactions_merge_cleanly_and_deterministically() {
        let base = KdlAdapter.parse(BASE_DOC.as_bytes()).expect("parse base");
        let tx_a = Transaction::from_json(
            r#"{"ops":[{"op":"set_fill","node":"card","fill":"color.accent"}]}"#,
        )
        .expect("tx a");
        let tx_b = Transaction::from_json(
            r#"{"ops":[{"op":"set_span_text","node":"title","span":0,"text":"New Title"}]}"#,
        )
        .expect("tx b");

        let outcome = merge_transactions(
            &base,
            &[
                ScopedTx {
                    label: "agent-color",
                    tx: &tx_a,
                },
                ScopedTx {
                    label: "agent-copy",
                    tx: &tx_b,
                },
            ],
        )
        .expect("merge");

        let TxMergeOutcome::Merged(result) = outcome else {
            panic!("expected Merged, got {outcome:?}");
        };
        assert_eq!(result.status, TxStatus::Accepted);
        assert!(result.source_after.contains("color.accent"));
        assert!(result.source_after.contains("New Title"));
    }

    #[test]
    fn overlapping_node_edits_report_conflict_without_silent_overwrite() {
        let base = KdlAdapter.parse(BASE_DOC.as_bytes()).expect("parse base");
        let tx_a = Transaction::from_json(
            r#"{"ops":[{"op":"set_span_text","node":"title","span":0,"text":"Title A"}]}"#,
        )
        .expect("tx a");
        let tx_b = Transaction::from_json(
            r#"{"ops":[{"op":"set_span_text","node":"title","span":0,"text":"Title B"}]}"#,
        )
        .expect("tx b");

        let outcome = merge_transactions(
            &base,
            &[
                ScopedTx {
                    label: "agent-a",
                    tx: &tx_a,
                },
                ScopedTx {
                    label: "agent-b",
                    tx: &tx_b,
                },
            ],
        )
        .expect("merge");

        let TxMergeOutcome::Conflicts(conflicts) = outcome else {
            panic!("expected Conflicts, got {outcome:?}");
        };
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].left, "agent-a");
        assert_eq!(conflicts[0].right, "agent-b");
        assert_eq!(conflicts[0].scopes, vec!["title"]);
    }

    #[test]
    fn document_wide_find_replace_conflicts_with_concurrent_edit() {
        let base = KdlAdapter.parse(BASE_DOC.as_bytes()).expect("parse base");
        let tx_a = Transaction::from_json(
            r#"{"ops":[{"op":"find_replace_text","find":"Old","replace":"Fresh"}]}"#,
        )
        .expect("tx a");
        let tx_b = Transaction::from_json(
            r#"{"ops":[{"op":"set_fill","node":"card","fill":"color.accent"}]}"#,
        )
        .expect("tx b");

        let outcome = merge_transactions(
            &base,
            &[
                ScopedTx {
                    label: "agent-global",
                    tx: &tx_a,
                },
                ScopedTx {
                    label: "agent-card",
                    tx: &tx_b,
                },
            ],
        )
        .expect("merge");

        let TxMergeOutcome::Conflicts(conflicts) = outcome else {
            panic!("expected Conflicts, got {outcome:?}");
        };
        assert_eq!(conflicts[0].scopes, vec![DOC_SCOPE]);
    }
}
