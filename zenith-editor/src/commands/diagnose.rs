//! `doc.diagnose`: the `zenith validate` pipeline over the current text.

use serde_json::{Value, json};

use crate::ctx::Ctx;
use crate::error::EditorError;

/// Validate the current text against the project: config policy and
/// brand, composition imports, assets, fonts, and (with no error) a
/// compile of every page. Updates `valid` and the last valid text, since
/// project files can change between requests.
///
/// `exit_code` is the `zenith validate` exit code: 0, 1, or 2.
pub(crate) fn run(ctx: &mut Ctx<'_, '_>, _raw: Value) -> Result<Value, EditorError> {
    let text = ctx.session.text.clone();
    let checked = ctx.validate(&text);
    ctx.session.set_valid(checked.valid());
    Ok(json!({
        "valid": ctx.session.valid,
        "exit_code": checked.validation.exit_code,
        "stale": ctx.session.stale(),
        "diagnostics": checked.diagnostics(&text),
    }))
}
