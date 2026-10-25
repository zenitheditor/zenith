//! [`Owners`]: which compiled node drew each scene command.
//!
//! A node's commands start at its [`CompiledBox::command_index`]. The ledger
//! lists nodes in paint order, a parent before its children, so a node's
//! subtree is the run of entries after it that it contains. Its commands end
//! where the first later node outside that subtree starts. The owner of a
//! command is the innermost node whose range holds it.

use std::collections::BTreeMap;

use zenith_core::Span;

use super::super::super::boxes::CompiledBox;
use super::super::ledger::{Entry, PageLedger};
use super::run::PrintInput;
use crate::ir::SceneCommand;

/// The command range `[start, end)` of one ledger entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Range {
    start: usize,
    end: usize,
    entry: usize,
}

/// The command range of every drawn ledger entry, in ledger order.
#[derive(Debug, Default)]
pub(super) struct Owners {
    ranges: Vec<Range>,
}

impl Owners {
    /// The ranges of `ledger`'s entries over a stream of `len` commands.
    /// Hidden entries and entries with no box draw nothing and own nothing.
    pub(super) fn build(
        ledger: &PageLedger,
        boxes: &BTreeMap<String, CompiledBox>,
        len: usize,
    ) -> Self {
        let start_of = |i: usize| {
            ledger
                .entry(i)
                .and_then(|e| boxes.get(&e.id))
                .filter(|b| !b.hidden)
                .map(|b| b.command_index)
        };
        let count = ledger.entries.len();
        let mut ranges = Vec::new();
        for i in 0..count {
            let Some(start) = start_of(i) else {
                continue;
            };
            let end = (i + 1..count)
                .filter(|&j| !ledger.is_ancestor(i, j))
                .find_map(start_of)
                .unwrap_or(len)
                .max(start);
            ranges.push(Range {
                start,
                end,
                entry: i,
            });
        }
        Self { ranges }
    }

    /// The ledger index of the innermost node that drew command `index`.
    /// `None` for a command no node drew, such as the page background.
    pub(super) fn owner(&self, index: usize) -> Option<usize> {
        self.ranges
            .iter()
            .rev()
            .find(|r| r.start <= index && index < r.end)
            .map(|r| r.entry)
    }
}

/// The node a command belongs to, for the message.
pub(super) struct Subject<'a> {
    pub(super) id: &'a str,
    /// `text 'cap'`, or `page 'p1'` for a command no node drew.
    pub(super) name: String,
    pub(super) span: Option<Span>,
    /// Ancestor scale of the node's box: the square root of the area scale.
    pub(super) scale: f64,
}

/// The node that drew command `index`: a glyph run's own source node when
/// the ledger holds it, else the command's owner, else the page.
pub(super) fn subject<'a>(
    input: &'a PrintInput<'_, '_>,
    owners: &Owners,
    index: usize,
    command: &SceneCommand,
) -> Subject<'a> {
    let by_source = if let SceneCommand::DrawGlyphRun {
        source_node_id: Some(id),
        ..
    } = command
    {
        input.ledger.entries.iter().find(|e| &e.id == id)
    } else {
        None
    };
    let entry = by_source.or_else(|| owners.owner(index).and_then(|i| input.ledger.entry(i)));
    match entry {
        Some(e) => entry_subject(input, e),
        None => Subject {
            id: &input.env.page.id,
            name: format!("page '{}'", input.env.page.id),
            span: input.env.page.source_span,
            scale: 1.0,
        },
    }
}

fn entry_subject<'a>(input: &'a PrintInput<'_, '_>, e: &'a Entry) -> Subject<'a> {
    let scale = input.boxes.get(&e.id).map_or(1.0, |b| {
        let w = b.world;
        (w.a * w.d - w.b * w.c).abs().sqrt()
    });
    Subject {
        id: &e.id,
        name: format!("{} '{}'", e.kind, e.id),
        span: e.span,
        scale,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owners(ranges: &[(usize, usize, usize)]) -> Owners {
        Owners {
            ranges: ranges
                .iter()
                .map(|&(start, end, entry)| Range { start, end, entry })
                .collect(),
        }
    }

    #[test]
    fn innermost_range_owns_the_command() {
        // Parent 0 draws [1, 9), child 1 draws [2, 4), child 2 draws [4, 9).
        let o = owners(&[(1, 9, 0), (2, 4, 1), (4, 9, 2)]);
        assert_eq!(o.owner(0), None, "before any node: the page");
        assert_eq!(o.owner(1), Some(0), "the parent's own fill");
        assert_eq!(o.owner(3), Some(1));
        assert_eq!(o.owner(5), Some(2));
        assert_eq!(o.owner(9), None, "past every range");
    }

    #[test]
    fn empty_range_owns_nothing() {
        let o = owners(&[(1, 5, 0), (3, 3, 1)]);
        assert_eq!(o.owner(3), Some(0));
    }
}
