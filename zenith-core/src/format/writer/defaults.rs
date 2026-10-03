//! `defaults { … }` block writing (document and page scope).
//!
//! Accepted rows are written in kind order (the [`DefaultsBlock::entries`]
//! map order), then rejected rows in source order, so a re-parse keeps the
//! same accepted row for every kind. An empty block is not written.

use crate::ast::{DefaultsBlock, DefaultsEntry};

use super::{fmt_unknown_property, indent};

/// Emit the `defaults { … }` block at `depth`, or nothing when it is empty.
pub(in crate::format::writer) fn write_defaults_block(
    block: &DefaultsBlock,
    out: &mut String,
    depth: usize,
) {
    if block.is_empty() {
        return;
    }
    indent(out, depth);
    out.push_str("defaults {\n");
    for (kind, entry) in &block.entries {
        write_row(kind.name(), entry, out, depth + 1);
    }
    for rejected in &block.rejected {
        write_row(&rejected.name, &rejected.entry, out, depth + 1);
    }
    indent(out, depth);
    out.push_str("}\n");
}

/// Emit one `<name> style="…" [text-style="…"]` row plus unknown attributes.
fn write_row(name: &str, entry: &DefaultsEntry, out: &mut String, depth: usize) {
    indent(out, depth);
    out.push_str(name);
    out.push_str(" style=\"");
    out.push_str(&entry.style);
    out.push('"');
    if let Some(ts) = &entry.text_style {
        out.push_str(" text-style=\"");
        out.push_str(ts);
        out.push('"');
    }
    for (key, prop) in &entry.unknown_props {
        out.push(' ');
        out.push_str(key);
        out.push('=');
        out.push_str(&fmt_unknown_property(prop));
    }
    out.push('\n');
}
