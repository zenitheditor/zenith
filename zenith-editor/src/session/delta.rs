//! [`TextDelta`]: one contiguous text replacement, the unit of history and
//! of every change the engine hands the page.

use serde::{Deserialize, Serialize};

use crate::error::EditorError;

/// Replace bytes `start..end` of a text with `insert`.
///
/// Offsets are byte offsets on UTF-8 character boundaries. A delta made by
/// [`TextDelta::between`] is minimal: the common prefix and suffix of the
/// two texts are left out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextDelta {
    /// First replaced byte.
    pub start: usize,
    /// One past the last replaced byte.
    pub end: usize,
    /// The replacement text.
    pub insert: String,
}

impl TextDelta {
    /// The minimal delta that turns `old` into `new`, or `None` when they
    /// are equal.
    #[must_use]
    pub fn between(old: &str, new: &str) -> Option<TextDelta> {
        if old == new {
            return None;
        }
        let (a, b) = (old.as_bytes(), new.as_bytes());
        let mut prefix = a.iter().zip(b).take_while(|(x, y)| x == y).count();
        while !old.is_char_boundary(prefix) || !new.is_char_boundary(prefix) {
            prefix -= 1;
        }
        let max_suffix = a.len().min(b.len()) - prefix;
        let mut suffix = a
            .iter()
            .rev()
            .zip(b.iter().rev())
            .take(max_suffix)
            .take_while(|(x, y)| x == y)
            .count();
        while !old.is_char_boundary(a.len() - suffix) || !new.is_char_boundary(b.len() - suffix) {
            suffix -= 1;
        }
        Some(TextDelta {
            start: prefix,
            end: a.len() - suffix,
            insert: new
                .get(prefix..b.len() - suffix)
                .unwrap_or_default()
                .to_owned(),
        })
    }

    /// `text` with this delta applied.
    ///
    /// # Errors
    ///
    /// `editor.history_mismatch` when the range is outside `text` or not on
    /// character boundaries: the delta was made for another text.
    pub fn apply(&self, text: &str) -> Result<String, EditorError> {
        let (Some(head), Some(tail)) = (text.get(..self.start), text.get(self.end..)) else {
            return Err(self.mismatch(text));
        };
        if self.start > self.end {
            return Err(self.mismatch(text));
        }
        let mut out = String::with_capacity(head.len() + self.insert.len() + tail.len());
        out.push_str(head);
        out.push_str(&self.insert);
        out.push_str(tail);
        Ok(out)
    }

    /// The delta that undoes this one, given `before`, the text it applies
    /// to.
    ///
    /// # Errors
    ///
    /// `editor.history_mismatch` when the range is not in `before`.
    pub fn inverse(&self, before: &str) -> Result<TextDelta, EditorError> {
        let removed = before
            .get(self.start..self.end)
            .ok_or_else(|| self.mismatch(before))?;
        Ok(TextDelta {
            start: self.start,
            end: self.start + self.insert.len(),
            insert: removed.to_owned(),
        })
    }

    /// The bytes this delta stores: the length of `insert`.
    #[must_use]
    pub fn stored_bytes(&self) -> usize {
        self.insert.len()
    }

    fn mismatch(&self, text: &str) -> EditorError {
        EditorError::new(
            "editor.history_mismatch",
            format!(
                "delta {}..{} does not fit a text of {} bytes; the history was made for \
                 another text. Send doc.open to start a fresh history",
                self.start,
                self.end,
                text.len()
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(old: &str, new: &str) {
        let d = TextDelta::between(old, new).expect("differs");
        assert_eq!(d.apply(old).expect("apply"), new);
        let inv = d.inverse(old).expect("inverse");
        assert_eq!(inv.apply(new).expect("undo"), old);
    }

    #[test]
    fn minimal_delta_keeps_prefix_and_suffix() {
        let d = TextDelta::between("hello world", "hello brave world").expect("differs");
        assert_eq!(
            d,
            TextDelta {
                start: 6,
                end: 6,
                insert: "brave ".to_owned()
            }
        );
        round_trip("hello world", "hello brave world");
        round_trip("aaa", "aa");
        round_trip("", "x");
        round_trip("x", "");
        assert!(TextDelta::between("same", "same").is_none());
    }

    #[test]
    fn multibyte_boundaries_hold() {
        round_trip("é", "è");
        round_trip("a\u{1F600}b", "a\u{1F601}b");
        round_trip("ü1", "ü2");
    }

    #[test]
    fn bad_range_is_a_mismatch() {
        let d = TextDelta {
            start: 2,
            end: 9,
            insert: String::new(),
        };
        assert_eq!(
            d.apply("abc").expect_err("range").code,
            "editor.history_mismatch"
        );
        let mid = TextDelta {
            start: 1,
            end: 1,
            insert: String::new(),
        };
        assert!(mid.apply("é").is_err());
    }
}
