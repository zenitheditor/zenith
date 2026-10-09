//! [`CommandSpec`]: one entry of the command registry.

use serde::Serialize;
use serde_json::Value;

use crate::ctx::Ctx;
use crate::error::EditorError;
use crate::session::Session;

/// The function that runs a command.
pub(crate) type RunFn = fn(&mut Ctx<'_, '_>, Value) -> Result<Value, EditorError>;

/// The function that says whether a command can run on a session.
pub(crate) type EnabledFn = fn(&Session) -> Option<Disabled>;

/// Why a command cannot run on a session now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Disabled {
    /// The error code the command returns: for example
    /// `editor.buffer_invalid` or `editor.nothing_to_undo`.
    pub code: &'static str,
    /// What to do so it can run.
    pub reason: &'static str,
}

/// One registry entry: the command's id, label, params and result docs,
/// whether it changes the text, when it is enabled, and how it runs.
///
/// Every surface (the page, the native server, MCP, tests) reaches the
/// engine through these entries, so the UI holds no logic.
#[derive(Clone, Copy)]
pub struct CommandSpec {
    pub(crate) id: &'static str,
    pub(crate) label: &'static str,
    pub(crate) params: &'static str,
    pub(crate) result: &'static str,
    pub(crate) mutates: bool,
    pub(crate) needs_version: bool,
    pub(crate) enabled: EnabledFn,
    pub(crate) run: RunFn,
}

impl std::fmt::Debug for CommandSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandSpec")
            .field("id", &self.id)
            .field("mutates", &self.mutates)
            .finish()
    }
}

impl CommandSpec {
    /// The command id, for example `gesture.commit`.
    #[must_use]
    pub fn id(&self) -> &'static str {
        self.id
    }

    /// A short human label for menus.
    #[must_use]
    pub fn label(&self) -> &'static str {
        self.label
    }

    /// The params, in a compact JSON-like notation: `?` marks an optional
    /// field, `=` its default.
    #[must_use]
    pub fn params(&self) -> &'static str {
        self.params
    }

    /// The result fields, in the same notation.
    #[must_use]
    pub fn result(&self) -> &'static str {
        self.result
    }

    /// `true` when the command can change the text (and so returns a
    /// delta and records history).
    #[must_use]
    pub fn mutates(&self) -> bool {
        self.mutates
    }

    /// `true` when a request must carry the session `version`.
    #[must_use]
    pub fn needs_version(&self) -> bool {
        self.needs_version
    }

    /// Why the command cannot run on `session`, or `None` when it can.
    #[must_use]
    pub fn disabled(&self, session: &Session) -> Option<Disabled> {
        (self.enabled)(session)
    }

    /// Run the command.
    pub(crate) fn run(&self, ctx: &mut Ctx<'_, '_>, params: Value) -> Result<Value, EditorError> {
        (self.run)(ctx, params)
    }
}

/// Always enabled.
pub(crate) fn always(_: &Session) -> Option<Disabled> {
    None
}

/// Enabled while the text is valid: edits need an error-free document.
pub(crate) fn when_valid(session: &Session) -> Option<Disabled> {
    (!session.valid).then_some(Disabled {
        code: "editor.buffer_invalid",
        reason: "the text has error diagnostics; fix them first",
    })
}

/// Enabled while some text (current or last valid) renders.
pub(crate) fn when_displayable(session: &Session) -> Option<Disabled> {
    session.display_text().is_none().then_some(Disabled {
        code: "editor.no_valid_render",
        reason: "no text of this session has been valid yet; fix the errors first",
    })
}

/// Enabled while there is something to undo.
pub(crate) fn when_undo(session: &Session) -> Option<Disabled> {
    session.history.undo.is_empty().then_some(Disabled {
        code: "editor.nothing_to_undo",
        reason: "the undo history is empty",
    })
}

/// Enabled while there is something to redo.
pub(crate) fn when_redo(session: &Session) -> Option<Disabled> {
    session.history.redo.is_empty().then_some(Disabled {
        code: "editor.nothing_to_redo",
        reason: "the redo history is empty",
    })
}
