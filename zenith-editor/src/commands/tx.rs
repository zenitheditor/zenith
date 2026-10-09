//! `tx.apply {ops, permissions?, label?, select?}`: run a transaction, as
//! an agent or a panel does.

use serde::Deserialize;
use serde_json::Value;
use zenith_tx::{Op, Permissions};

use super::common::params;
use crate::ctx::Ctx;
use crate::edit::ops::{OpsEdit, apply_ops};
use crate::error::EditorError;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TxParams {
    ops: Vec<Op>,
    #[serde(default)]
    permissions: Permissions,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    select: Option<Vec<String>>,
}

/// Run `ops` (the `zenith tx` op set) on the current text and patch the
/// result in. A rejection lists the transaction's diagnostics and offers
/// `unlock` for a locked node. `label` names the history entry (default
/// `tx.apply`); `select` sets the selection after the edit.
pub(crate) fn run(ctx: &mut Ctx<'_, '_>, raw: Value) -> Result<Value, EditorError> {
    let p: TxParams = params(ctx, raw.clone())?;
    let doc = ctx.editable()?;
    apply_ops(
        ctx,
        &doc,
        OpsEdit {
            label: p.label,
            ops: p.ops,
            permissions: p.permissions,
            selection: p.select,
            notes: Vec::new(),
            gesture: false,
            raw,
        },
    )
}
