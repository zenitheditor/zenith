//! `commands.list`: the registry, with each command's enabled state for the
//! current session.

use serde_json::{Value, json};

use crate::ctx::Ctx;
use crate::error::EditorError;
use crate::registry::commands;

/// Every command: id, label, params and result docs, whether it changes
/// the text, whether it needs the version, and whether it is enabled now
/// (with the disabled code and reason when not).
pub(crate) fn run(ctx: &mut Ctx<'_, '_>, _raw: Value) -> Result<Value, EditorError> {
    let list: Vec<Value> = commands()
        .iter()
        .map(|spec| {
            let disabled = spec.disabled(&ctx.session);
            json!({
                "id": spec.id(),
                "label": spec.label(),
                "params": spec.params(),
                "result": spec.result(),
                "mutates": spec.mutates(),
                "needs_version": spec.needs_version(),
                "enabled": disabled.is_none(),
                "disabled": disabled,
            })
        })
        .collect();
    Ok(json!({ "commands": list }))
}
