//! [`execute`]: run one [`Request`] against a [`Session`].

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::ctx::{Ctx, RenderedImage, Work};
use crate::env::Env;
use crate::error::EditorError;
use crate::registry::find;
use crate::session::Session;

/// One command: its id, its params, and the session version the sender
/// last saw.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    /// The command id, for example `gesture.commit`.
    pub command: String,
    /// The command params. `null` or absent reads as `{}`.
    #[serde(default)]
    pub params: Value,
    /// The [`Session::version`] the sender's buffer is at. Required by
    /// every command that changes the text (see
    /// [`CommandSpec::needs_version`](crate::CommandSpec::needs_version));
    /// checked whenever present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u64>,
}

impl Request {
    /// A request for `command` with `params` and no version.
    #[must_use]
    pub fn new(command: &str, params: Value) -> Self {
        Self {
            command: command.to_owned(),
            params,
            version: None,
        }
    }

    /// This request at session version `version`.
    #[must_use]
    pub fn at(mut self, version: u64) -> Self {
        self.version = Some(version);
        self
    }
}

/// What one request produced.
#[derive(Debug, Clone)]
pub struct Outcome {
    /// The next session. On an error it equals the session sent in.
    pub session: Session,
    /// The command result, or why it did not run.
    pub result: Result<Value, EditorError>,
    /// The page raster, for `doc.render` and `gesture.preview`.
    pub image: Option<RenderedImage>,
    /// The engine work the command did.
    pub work: Work,
}

/// Run `request` against `session` in `env`.
///
/// Stateless: everything the engine knows comes in through `session` and
/// `env`, and leaves through the [`Outcome`]. Never panics; every failure
/// is an [`EditorError`] with the session unchanged.
///
/// Order of checks: the command exists, the version matches (and is
/// present when the command needs it), the command is enabled for the
/// session, the params decode, then the command runs.
#[must_use]
pub fn execute(env: &Env<'_>, session: Session, request: &Request) -> Outcome {
    let original = session.clone();
    let mut ctx = Ctx::new(env, &request.command, session);
    match run_request(&mut ctx, request) {
        Ok(value) => Outcome {
            session: ctx.session,
            result: Ok(value),
            image: ctx.image,
            work: ctx.work,
        },
        Err(error) => Outcome {
            session: original,
            result: Err(error),
            image: None,
            work: ctx.work,
        },
    }
}

/// Check and run `request` on `ctx` (see [`execute`] for the order of
/// checks). `commands.batch` runs each step through here.
pub(crate) fn run_request(ctx: &mut Ctx<'_, '_>, request: &Request) -> Result<Value, EditorError> {
    let spec = find(&request.command).ok_or_else(|| {
        EditorError::new(
            "editor.unknown_command",
            format!(
                "unknown command '{}'; commands.list names every command",
                request.command
            ),
        )
    })?;
    match request.version {
        Some(v) if v != ctx.session.version => {
            return Err(EditorError::new(
                "editor.stale_version",
                format!(
                    "'{}' was made at version {v}, but the session is at version {}; apply \
                     the latest delta (or reload the text) and resend",
                    request.command, ctx.session.version
                ),
            ));
        }
        None if spec.needs_version() => {
            return Err(EditorError::new(
                "editor.missing_version",
                format!(
                    "'{}' changes the text, so it needs the session version the sender saw; \
                     add \"version\": {}",
                    request.command, ctx.session.version
                ),
            ));
        }
        Some(_) | None => {}
    }
    if let Some(disabled) = spec.disabled(&ctx.session) {
        return Err(EditorError::new(
            disabled.code,
            format!("'{}' is disabled: {}", request.command, disabled.reason),
        ));
    }
    let params = match &request.params {
        Value::Null => Value::Object(Map::new()),
        other => other.clone(),
    };
    spec.run(ctx, params)
}
