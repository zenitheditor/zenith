//! Block-level markdown parser → [`MdBlock`] sequence.
//!
//! Parses a multi-line string into a flat list of block-level elements.
//! For blocks that carry inline text (headings, paragraphs, blockquotes,
//! list items) the text is forwarded to [`super::inline::parse_inline_markdown`]
//! to produce the final [`crate::ast::node::TextSpan`] spans.
//!
//! # Supported block syntax
//!
//! | Syntax                               | Produces                           |
//! |--------------------------------------|------------------------------------|
//! | `# …` … `###### …`                  | `Heading { level: 1..=6, … }`      |
//! | Two or more consecutive non-blank    | `Paragraph { … }`                  |
//! | `> …` (consecutive)                  | `Blockquote { … }`                 |
//! | `- `/ `* `/ `+ ` prefix             | `ListItem { kind: Unordered, … }`  |
//! | `1. ` / `2. ` … prefix              | `ListItem { kind: Ordered, … }`    |
//! | ` ``` ` … ` ``` ` (fenced)          | `CodeBlock { lang, content }`      |
//! | `---` / `***` / `___` / `- - -` …   | `HorizontalRule`                   |
//!
//! # Non-goals (V1)
//!
//! Setext headings, nested blockquote trees, true list-tree nesting, GFM
//! tables, reference links, HTML blocks, indented (4-space) code blocks, and
//! task checkboxes are not recognised; they degrade to `Paragraph` or the
//! nearest applicable block kind.

use crate::ast::node::TextSpan;

use super::inline::parse_inline_markdown;

/// The kind of a markdown list item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListKind {
    Unordered,
    Ordered,
}

/// A parsed block-level markdown element.
#[derive(Debug, Clone, PartialEq)]
pub enum MdBlock {
    /// ATX heading (`#` … `######`). `level` is 1–6.
    Heading { level: u8, spans: Vec<TextSpan> },
    /// One or more consecutive non-blank lines (not matching any other rule).
    Paragraph { spans: Vec<TextSpan> },
    /// A `>` blockquote (consecutive lines merged with a space).
    Blockquote { spans: Vec<TextSpan> },
    /// A single list item (flat; `depth` encodes indentation level).
    ListItem {
        kind: ListKind,
        depth: u32,
        ordinal: Option<u32>,
        spans: Vec<TextSpan>,
    },
    /// A fenced code block (``` … ```). `content` is RAW — no inline parsing.
    CodeBlock {
        lang: Option<String>,
        content: String,
    },
    /// A horizontal rule (`---`, `***`, `___`, `- - -`, …).
    HorizontalRule,
}

// ---------------------------------------------------------------------------
// Internal open-block state machine
// ---------------------------------------------------------------------------

/// The block currently being accumulated.
#[derive(Debug)]
enum Open {
    None,
    Paragraph(Vec<String>),
    Blockquote(Vec<String>),
    Code {
        lang: Option<String>,
        lines: Vec<String>,
    },
}

/// Parse a markdown document into a flat list of [`MdBlock`]s.
///
/// Infallible: no panics, no errors; malformed markdown degrades gracefully.
/// Fully deterministic: same input → same output.
pub fn parse_block_markdown(input: &str) -> Vec<MdBlock> {
    let mut out: Vec<MdBlock> = Vec::new();
    let mut open: Open = Open::None;

    for raw_line in input.split('\n') {
        // Strip trailing CR (handle \r\n line endings).
        let line = raw_line.trim_end_matches('\r');

        // ── Inside a fenced code block ──────────────────────────────────────
        if let Open::Code { lang, lines } = &mut open {
            // A line that trims to ``` closes the fence.
            if line.trim() == "```" {
                let content = lines.join("\n");
                let lang_out = lang.take();
                out.push(MdBlock::CodeBlock {
                    lang: lang_out,
                    content,
                });
                open = Open::None;
            } else {
                lines.push(line.to_owned());
            }
            continue;
        }

        // ── Blank line: flush whatever is open ──────────────────────────────
        if line.trim().is_empty() {
            flush(&mut open, &mut out);
            continue;
        }

        // ── Fenced code opener: trim_start begins with ``` ──────────────────
        let line_trimmed_start = line.trim_start();
        if line_trimmed_start.starts_with("```") {
            flush(&mut open, &mut out);
            let after_backticks = line_trimmed_start.get(3..).unwrap_or("").trim();
            let lang = if after_backticks.is_empty() {
                None
            } else {
                Some(after_backticks.to_owned())
            };
            open = Open::Code {
                lang,
                lines: Vec::new(),
            };
            continue;
        }

        // ── Horizontal rule ─────────────────────────────────────────────────
        if is_horizontal_rule(line) {
            flush(&mut open, &mut out);
            out.push(MdBlock::HorizontalRule);
            continue;
        }

        // ── ATX heading ─────────────────────────────────────────────────────
        if let Some((level, text)) = parse_atx_heading(line) {
            flush(&mut open, &mut out);
            let spans = parse_inline_markdown(text);
            out.push(MdBlock::Heading { level, spans });
            continue;
        }

        // ── Blockquote ──────────────────────────────────────────────────────
        if let Some(inner) = strip_blockquote_prefix(line) {
            match &mut open {
                Open::Blockquote(lines) => {
                    lines.push(inner.to_owned());
                }
                Open::None | Open::Paragraph(_) | Open::Code { .. } => {
                    flush(&mut open, &mut out);
                    open = Open::Blockquote(vec![inner.to_owned()]);
                }
            }
            continue;
        }

        // If we were accumulating a blockquote and this line is NOT a `>`
        // line, flush the blockquote first (it's terminated).
        if matches!(&open, Open::Blockquote(_)) {
            flush(&mut open, &mut out);
            // Fall through: the current line starts a new block below.
        }

        // ── List item ───────────────────────────────────────────────────────
        if let Some(item) = parse_list_item(line) {
            flush(&mut open, &mut out);
            let spans = parse_inline_markdown(item.text);
            out.push(MdBlock::ListItem {
                kind: item.kind,
                depth: item.depth,
                ordinal: item.ordinal,
                spans,
            });
            continue;
        }

        // ── Paragraph (default) ─────────────────────────────────────────────
        match &mut open {
            Open::Paragraph(lines) => {
                lines.push(line.to_owned());
            }
            Open::None | Open::Blockquote(_) | Open::Code { .. } => {
                open = Open::Paragraph(vec![line.to_owned()]);
            }
        }
    }

    // Flush any open block at EOF.
    flush(&mut open, &mut out);
    out
}

// ---------------------------------------------------------------------------
// Flush helpers
// ---------------------------------------------------------------------------

/// Flush the currently open block into `out`, resetting `open` to `None`.
fn flush(open: &mut Open, out: &mut Vec<MdBlock>) {
    let done = std::mem::replace(open, Open::None);
    match done {
        Open::None => {}
        Open::Paragraph(lines) => {
            if lines.is_empty() {
                return;
            }
            let text = lines.join(" ");
            let spans = parse_inline_markdown(&text);
            out.push(MdBlock::Paragraph { spans });
        }
        Open::Blockquote(lines) => {
            if lines.is_empty() {
                return;
            }
            let text = lines.join(" ");
            let spans = parse_inline_markdown(&text);
            out.push(MdBlock::Blockquote { spans });
        }
        Open::Code { lang, lines } => {
            // Unclosed fence at EOF: flush with content so far.
            let content = lines.join("\n");
            out.push(MdBlock::CodeBlock { lang, content });
        }
    }
}

// ---------------------------------------------------------------------------
// Line classifiers
// ---------------------------------------------------------------------------

/// Returns `true` if `line` (not yet trimmed) is a thematic break.
///
/// Rule: trimmed line consists solely of 3+ identical chars from `{-, *, _}`
/// with optional spaces between them (e.g. `---`, `* * *`, `___`).
fn is_horizontal_rule(line: &str) -> bool {
    let trimmed = line.trim();
    // Must start with one of the three break characters.
    let ch = match trimmed.chars().next() {
        Some(c) if matches!(c, '-' | '*' | '_') => c,
        _ => return false,
    };
    let mut count = 0u32;
    for c in trimmed.chars() {
        if c == ch {
            count += 1;
        } else if c == ' ' {
            // Spaces between chars are allowed.
        } else {
            // Any other character means this is not a HR.
            return false;
        }
    }
    count >= 3
}

/// Parse an ATX heading. Returns `(level, inner_text)` if the line matches,
/// where `inner_text` has the leading `#` run and trailing `#` run stripped.
///
/// A valid ATX heading: trim_start matches 1–6 `#` chars then either a space
/// or end-of-line. 7+ `#` or `#` with no following space are not headings.
fn parse_atx_heading(line: &str) -> Option<(u8, &str)> {
    let s = line.trim_start();
    // Count leading `#` chars.
    let hash_count = s.bytes().take_while(|&b| b == b'#').count();
    if hash_count == 0 || hash_count > 6 {
        return None;
    }
    let rest = s.get(hash_count..)?;
    // Must be followed by a space or be EOL.
    let inner = if rest.is_empty() {
        ""
    } else if rest.starts_with(' ') {
        rest.get(1..).unwrap_or("")
    } else {
        // `#` not followed by space → not a heading.
        return None;
    };
    // Strip optional trailing `#` run (e.g. `## heading ##`).
    let inner = inner.trim_end();
    let stripped = inner.trim_end_matches('#').trim_end();
    // If stripping removed something, use the stripped version; otherwise keep
    // the original (so `## ##` → `##` → `""` correctly).
    let text = if stripped.len() < inner.len() {
        stripped
    } else {
        inner
    };
    Some((hash_count as u8, text))
}

/// Strip the `> ` or `>` prefix from a blockquote line. Returns the inner
/// text slice (with the prefix removed), or `None` if the line is not a `>`
/// line.
fn strip_blockquote_prefix(line: &str) -> Option<&str> {
    let s = line.trim_start();
    if !s.starts_with('>') {
        return None;
    }
    let after = s.get(1..).unwrap_or("");
    // Strip the optional single space after `>`.
    Some(if after.starts_with(' ') {
        after.get(1..).unwrap_or("")
    } else {
        after
    })
}

/// Data extracted from a parsed list-item line.
struct ListItemData<'a> {
    kind: ListKind,
    depth: u32,
    ordinal: Option<u32>,
    text: &'a str,
}

/// Try to parse `line` as a list item. The leading-space count (before the
/// marker) determines `depth` (spaces / 2, clamped to `u32`).
///
/// Unordered markers: `-`, `*`, `+` followed by a space.
/// Ordered markers: one or more ASCII digits followed by `.` then a space.
fn parse_list_item(line: &str) -> Option<ListItemData<'_>> {
    // Count leading spaces to determine depth.
    let leading_spaces = line.count_ascii_lead_spaces();
    let depth = (leading_spaces / 2) as u32;

    let s = line.trim_start();

    // Unordered: `-`/`*`/`+` + space.
    if let Some(first) = s.chars().next()
        && matches!(first, '-' | '*' | '+')
    {
        let rest = s.get(1..).unwrap_or("");
        if rest.starts_with(' ') {
            let text = rest.get(1..).unwrap_or("").trim_end();
            return Some(ListItemData {
                kind: ListKind::Unordered,
                depth,
                ordinal: None,
                text,
            });
        }
    }

    // Ordered: digits + `.` + space.
    let digit_end = s.bytes().take_while(|b| b.is_ascii_digit()).count();
    if digit_end > 0 {
        let after_digits = s.get(digit_end..)?;
        if after_digits.starts_with(". ") {
            let ordinal_str = s.get(..digit_end)?;
            let ordinal: u32 = ordinal_str.parse().ok()?;
            let text = after_digits.get(2..).unwrap_or("").trim_end();
            return Some(ListItemData {
                kind: ListKind::Ordered,
                depth,
                ordinal: Some(ordinal),
                text,
            });
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Small utility trait to count leading spaces without allocation
// ---------------------------------------------------------------------------

trait CountAsciiLeadSpaces {
    fn count_ascii_lead_spaces(&self) -> usize;
}

impl CountAsciiLeadSpaces for str {
    fn count_ascii_lead_spaces(&self) -> usize {
        self.bytes().take_while(|&b| b == b' ').count()
    }
}
