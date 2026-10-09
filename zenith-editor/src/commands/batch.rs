//! `commands.batch {steps}`: several commands in one call.

use serde::Deserialize;
use serde_json::{Value, json};

use super::common::params;
use crate::ctx::Ctx;
use crate::error::EditorError;
use crate::execute::{Request, run_request};

/// The most steps one batch runs.
pub(crate) const MAX_STEPS: usize = 16;

/// The commands that produce a PNG. A batch runs at most one of them.
const IMAGE_COMMANDS: [&str; 2] = ["doc.render", "gesture.preview"];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BatchParams {
    steps: Vec<Request>,
}

/// Run `steps` in order, each on the session the step before left.
///
/// A step runs exactly as the same request sent alone: the same checks
/// (version, enabled, params), the same reply, and on an error the session
/// stays as the step found it. The steps share one parse per text, so a
/// keystroke frame (`buffer.set`, `doc.render`, `doc.outline`, `doc.tokens`,
/// `select.at_offset`) parses the new text once.
///
/// A failed step that carries `version` stops the batch: the later steps
/// describe a text that did not land, so they reply `editor.skipped`.
/// Other failed steps do not stop it.
///
/// At most one step is `doc.render` or `gesture.preview`; its PNG is the
/// batch image ([`Outcome::image`]) and `image_step` is its index.
/// A step cannot be `commands.batch`.
///
/// [`Outcome::image`]: crate::Outcome::image
pub(crate) fn run(ctx: &mut Ctx<'_, '_>, raw: Value) -> Result<Value, EditorError> {
    let p: BatchParams = params(ctx, raw)?;
    check_steps(&p.steps)?;
    let mut replies = Vec::with_capacity(p.steps.len());
    let mut image_step = None;
    let mut stopped: Option<usize> = None;
    for (index, step) in p.steps.iter().enumerate() {
        if let Some(at) = stopped {
            replies.push(skipped(step, at));
            continue;
        }
        let mut sub = Ctx::new(ctx.env, &step.command, ctx.session.clone());
        sub.parsed = std::mem::take(&mut ctx.parsed);
        let result = run_request(&mut sub, step);
        ctx.parsed = std::mem::take(&mut sub.parsed);
        ctx.work += sub.work;
        let reply = match result {
            Ok(value) => {
                ctx.session = sub.session;
                if sub.image.is_some() {
                    ctx.image = sub.image;
                    image_step = Some(index);
                }
                json!({ "command": step.command, "ok": true, "result": value, "work": sub.work })
            }
            Err(error) => {
                if step.version.is_some() {
                    stopped = Some(index);
                }
                json!({ "command": step.command, "ok": false, "error": error, "work": sub.work })
            }
        };
        replies.push(reply);
    }
    let mut out = json!({ "steps": replies });
    if let (Some(index), Value::Object(map)) = (image_step, &mut out) {
        map.insert("image_step".to_owned(), json!(index));
    }
    Ok(out)
}

/// Reject a batch that nests, is too long, or has two image steps.
fn check_steps(steps: &[Request]) -> Result<(), EditorError> {
    let invalid = |detail: String| EditorError::invalid_params("commands.batch", detail);
    if steps.len() > MAX_STEPS {
        return Err(invalid(format!(
            "{} steps, but a batch runs at most {MAX_STEPS}; split it",
            steps.len()
        )));
    }
    if steps.iter().any(|s| s.command == "commands.batch") {
        return Err(invalid(
            "a step cannot be commands.batch; list its steps instead".to_owned(),
        ));
    }
    let images = steps
        .iter()
        .filter(|s| IMAGE_COMMANDS.contains(&s.command.as_str()))
        .count();
    if images > 1 {
        return Err(invalid(format!(
            "{images} steps are doc.render or gesture.preview, but a batch returns one image; \
             send one of them"
        )));
    }
    Ok(())
}

/// The reply of a step after the batch stopped at step `at`.
fn skipped(step: &Request, at: usize) -> Value {
    let error = EditorError::new(
        "editor.skipped",
        format!(
            "'{}' did not run: step {at} changes the text and failed; fix that step and \
             resend the batch",
            step.command
        ),
    );
    json!({ "command": step.command, "ok": false, "error": error })
}
