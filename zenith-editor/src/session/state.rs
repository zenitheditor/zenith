//! [`Session`]: everything the engine knows between two requests.

use serde::{Deserialize, Serialize};

use super::history::History;

/// The editor state a request carries in and a response carries out.
///
/// The engine keeps no state between calls: the page (or the native
/// server) holds the session and sends it back with the next request.
/// Fields are plain data, so the JSON is stable and the same bytes on every
/// target.
///
/// Size: `text`, plus `last_valid_text` only while the text has errors,
/// plus the history deltas (bounded by [`History::limit`]). No render
/// output is kept: a PNG would grow every request by its size, and the
/// render is a pure function of the text, the project files, and the
/// fonts. So the session keeps the last text that validated, and render
/// commands re-render from it while the current text has errors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    /// Bumped by every change to `text`. A request that carries an older
    /// `version` is rejected with `editor.stale_version`.
    pub version: u64,
    /// The document text, as the page's code editor holds it.
    pub text: String,
    /// `true` when `text` parsed and its last validation had no Error
    /// diagnostic.
    pub valid: bool,
    /// The last text that was valid, kept only while `valid` is `false`.
    /// Render, hit, and handle commands use it and report `stale: true`.
    /// `None` while `valid`, or when no text was valid yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_valid_text: Option<String>,
    /// The current 1-based page.
    pub page: usize,
    /// Selected node ids (authored ids, never compiled ids), in selection
    /// order.
    pub selection: Vec<String>,
    /// The page's view of the canvas.
    pub viewport: Viewport,
    /// Undo and redo.
    pub history: History,
}

impl Session {
    /// A session over `text` at version 1, page 1, with nothing selected
    /// and no history. `valid` is `false` until a command validates it.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            version: 1,
            text: text.into(),
            valid: false,
            last_valid_text: None,
            page: 1,
            selection: Vec::new(),
            viewport: Viewport::default(),
            history: History::default(),
        }
    }

    /// `true` when render commands use `last_valid_text` instead of `text`.
    #[must_use]
    pub fn stale(&self) -> bool {
        !self.valid
    }

    /// The text render commands use: `text` when valid, else the last valid
    /// text, else `None`.
    #[must_use]
    pub fn display_text(&self) -> Option<&str> {
        if self.valid {
            Some(&self.text)
        } else {
            self.last_valid_text.as_deref()
        }
    }

    /// Record the validation result of the current `text`: keep the
    /// previous valid text while the new text has errors.
    pub fn set_valid(&mut self, valid: bool) {
        if valid {
            self.last_valid_text = None;
        } else if self.valid {
            self.last_valid_text = Some(self.text.clone());
        }
        self.valid = valid;
    }

    /// Replace the text, keeping the last valid text when the old text was
    /// valid. Bumps `version`. The caller validates the new text next.
    pub fn replace_text(&mut self, text: String) {
        if self.valid {
            self.last_valid_text = Some(std::mem::replace(&mut self.text, text));
        } else {
            self.text = text;
        }
        self.valid = false;
        self.version += 1;
    }
}

/// The page's view of the canvas. The engine stores it and reads `zoom` as
/// the default render scale; pan is the page's own concern.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Viewport {
    /// Screen pixels per page pixel. Default `1.0`.
    pub zoom: f64,
    /// Horizontal pan, in page px.
    pub pan_x: f64,
    /// Vertical pan, in page px.
    pub pan_y: f64,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_valid_text_follows_validity() {
        let mut s = Session::new("a");
        s.set_valid(true);
        assert_eq!(s.display_text(), Some("a"));
        s.replace_text("b".to_owned());
        assert_eq!(s.version, 2);
        s.set_valid(false);
        assert_eq!(s.display_text(), Some("a"));
        assert!(s.stale());
        s.replace_text("c".to_owned());
        s.set_valid(false);
        assert_eq!(s.display_text(), Some("a"), "the last valid text stays");
        s.replace_text("d".to_owned());
        s.set_valid(true);
        assert_eq!(s.last_valid_text, None);
        assert_eq!(s.display_text(), Some("d"));
    }
}
