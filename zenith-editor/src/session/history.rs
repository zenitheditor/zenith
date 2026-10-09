//! [`History`]: bounded undo and redo stacks of text deltas.
//!
//! Each entry stores the forward delta and its inverse, never a snapshot,
//! so the session stays near the size of its text. Human typing and engine
//! edits (gestures, agent transactions) share one history.

use serde::{Deserialize, Serialize};

use super::delta::TextDelta;
use crate::error::EditorError;

/// The default maximum number of undo entries.
pub const DEFAULT_MAX_ENTRIES: usize = 200;

/// The default maximum number of bytes the undo and redo stacks store
/// together: 1 MiB of delta text.
pub const DEFAULT_MAX_BYTES: usize = 1 << 20;

/// The longest insert, in bytes, a typing burst grows to before a new
/// entry starts.
pub const MAX_BURST_BYTES: usize = 1024;

/// How large the history may grow. When an edit pushes it past either
/// bound, the oldest undo entries go first. The newest entry always stays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryLimit {
    /// Maximum undo entries. Default [`DEFAULT_MAX_ENTRIES`].
    pub entries: usize,
    /// Maximum bytes of delta text across both stacks. Default
    /// [`DEFAULT_MAX_BYTES`].
    pub bytes: usize,
}

impl Default for HistoryLimit {
    fn default() -> Self {
        Self {
            entries: DEFAULT_MAX_ENTRIES,
            bytes: DEFAULT_MAX_BYTES,
        }
    }
}

/// One undoable text change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// The command that made the change (`buffer.set`, `gesture.commit`, …).
    pub label: String,
    /// `true` for a typing burst (`buffer.set`), which a following
    /// contiguous `buffer.set` can extend.
    pub typing: bool,
    /// The change: text before → text after.
    pub forward: TextDelta,
    /// The undo: text after → text before.
    pub inverse: TextDelta,
    /// The selection before the change, restored by undo.
    pub selection_before: Vec<String>,
    /// The selection after the change, restored by redo.
    pub selection_after: Vec<String>,
}

impl HistoryEntry {
    /// The delta bytes this entry stores.
    #[must_use]
    pub fn stored_bytes(&self) -> usize {
        self.forward.stored_bytes() + self.inverse.stored_bytes()
    }

    /// `true` when `next`, a change made right after this one, continues
    /// the same typing burst: both are typing, `next` touches the range
    /// this entry inserted, adds no line break, and the burst stays under
    /// [`MAX_BURST_BYTES`].
    fn continues_with(&self, next: &HistoryEntry) -> bool {
        let inserted_end = self.forward.start + self.forward.insert.len();
        self.typing
            && next.typing
            && next.forward.start <= inserted_end
            && next.forward.end >= self.forward.start
            && !next.forward.insert.contains('\n')
            && self.forward.insert.len() + next.forward.insert.len() <= MAX_BURST_BYTES
    }
}

/// The undo and redo stacks.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct History {
    /// Undo entries, oldest first.
    pub undo: Vec<HistoryEntry>,
    /// Redo entries, the next redo last.
    pub redo: Vec<HistoryEntry>,
    /// The size bound.
    pub limit: HistoryLimit,
}

impl History {
    /// Record `entry`, the change from `before` to `after`, and clear the
    /// redo stack.
    ///
    /// With `coalesce`, a typing entry that continues the last typing entry
    /// merges into it: one undo then reverts the whole burst. The merged
    /// delta is recomputed from the texts, so it stays minimal.
    ///
    /// # Errors
    ///
    /// `editor.history_mismatch` when the last entry does not fit `before`.
    pub fn record(
        &mut self,
        entry: HistoryEntry,
        before: &str,
        after: &str,
        coalesce: bool,
    ) -> Result<(), EditorError> {
        self.redo.clear();
        let merged = match self.undo.last() {
            Some(last) if coalesce && last.continues_with(&entry) => {
                let origin = last.inverse.apply(before)?;
                let selection_before = last.selection_before.clone();
                TextDelta::between(&origin, after)
                    .map(|forward| {
                        forward.inverse(&origin).map(|inverse| HistoryEntry {
                            label: entry.label.clone(),
                            typing: true,
                            forward,
                            inverse,
                            selection_before,
                            selection_after: entry.selection_after.clone(),
                        })
                    })
                    .transpose()?
                    .map_or(Merge::Cancelled, Merge::Into)
            }
            Some(_) | None => Merge::Push,
        };
        match merged {
            Merge::Push => self.undo.push(entry),
            Merge::Into(combined) => {
                self.undo.pop();
                self.undo.push(combined);
            }
            // The burst typed and then erased the same text.
            Merge::Cancelled => {
                self.undo.pop();
            }
        }
        self.enforce_limit();
        Ok(())
    }

    /// Drop the oldest undo entries until both bounds hold, keeping the
    /// newest entry.
    pub fn enforce_limit(&mut self) {
        let max_entries = self.limit.entries.max(1);
        let mut bytes = self.stored_bytes();
        let mut drop = 0;
        for entry in &self.undo {
            let over = self.undo.len() - drop > max_entries || bytes > self.limit.bytes;
            if !over || self.undo.len() - drop <= 1 {
                break;
            }
            bytes -= entry.stored_bytes();
            drop += 1;
        }
        self.undo.drain(..drop);
    }

    /// The delta bytes both stacks store.
    #[must_use]
    pub fn stored_bytes(&self) -> usize {
        self.undo
            .iter()
            .chain(&self.redo)
            .map(HistoryEntry::stored_bytes)
            .sum()
    }
}

/// How a new entry joins the stack.
enum Merge {
    Push,
    Into(HistoryEntry),
    Cancelled,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typing(before: &str, after: &str) -> HistoryEntry {
        let forward = TextDelta::between(before, after).expect("differs");
        let inverse = forward.inverse(before).expect("inverse");
        HistoryEntry {
            label: "buffer.set".to_owned(),
            typing: true,
            forward,
            inverse,
            selection_before: Vec::new(),
            selection_after: Vec::new(),
        }
    }

    fn type_in(history: &mut History, before: &str, after: &str) {
        history
            .record(typing(before, after), before, after, true)
            .expect("record");
    }

    #[test]
    fn contiguous_typing_coalesces() {
        let mut h = History::default();
        type_in(&mut h, "ab", "abc");
        type_in(&mut h, "abc", "abcd");
        type_in(&mut h, "abcd", "abc");
        assert_eq!(h.undo.len(), 1);
        let only = h.undo.first().expect("entry");
        assert_eq!(only.inverse.apply("abc").expect("undo"), "ab");
    }

    #[test]
    fn typing_then_erasing_cancels_and_newline_splits() {
        let mut h = History::default();
        type_in(&mut h, "ab", "abc");
        type_in(&mut h, "abc", "ab");
        assert!(h.undo.is_empty());
        type_in(&mut h, "ab", "abc");
        type_in(&mut h, "abc", "abc\n");
        assert_eq!(h.undo.len(), 2);
        type_in(&mut h, "abc\n", "xabc\n");
        assert_eq!(h.undo.len(), 3, "a distant edit starts a new entry");
    }

    #[test]
    fn limits_drop_the_oldest_and_keep_the_newest() {
        let mut h = History {
            limit: HistoryLimit {
                entries: 2,
                bytes: 1 << 20,
            },
            ..History::default()
        };
        let mut text = String::new();
        for i in 0..5 {
            let next = format!("{text}{i}\n");
            h.record(typing(&text, &next), &text, &next, false)
                .expect("record");
            text = next;
        }
        assert_eq!(h.undo.len(), 2);
        let mut big = History {
            limit: HistoryLimit {
                entries: 10,
                bytes: 4,
            },
            ..History::default()
        };
        big.record(typing("", "123456"), "", "123456", false)
            .expect("record");
        assert_eq!(big.undo.len(), 1, "the newest entry stays");
        big.record(typing("123456", "1234567\n"), "123456", "1234567\n", false)
            .expect("record");
        assert_eq!(big.undo.len(), 1);
    }
}
