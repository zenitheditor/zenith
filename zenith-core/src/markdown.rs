//! Pure, deterministic inline-markdown → [`TextSpan`] parser.
//!
//! This converts an INLINE markdown string (emphasis within a single text
//! block) into a `Vec<TextSpan>`, setting the per-span marks Zenith already
//! supports. It is **inline only** — there is no block-level structure here
//! (no headings, lists, tables, or paragraph splitting). It is intended to be
//! invoked when a text node opts into `data-format="markdown"`; this module is
//! just the parser.
//!
//! # Supported syntax
//!
//! | Markdown                  | Span mark set                                   |
//! |---------------------------|-------------------------------------------------|
//! | `**bold**` / `__bold__`   | `font_weight = Literal("700")` (resolves to 700)|
//! | `*italic*` / `_italic_`   | `italic = Some(true)`                           |
//! | `~~strike~~`              | `strikethrough = Some(true)`                    |
//! | `++underline++`           | `underline = Some(true)`                        |
//! | `==highlight==`           | `highlight = Literal("#fff59d")` (marker yellow)|
//! | `` `code` ``              | `code = Some(true)` (RAW: no inner parsing)     |
//! | `[label](url)`            | span(s) with `link = Some(url)`; label parsed   |
//!
//! # Rules
//!
//! - Plain text between marks becomes plain spans (no marks set).
//! - Backslash escapes (`\*`, `\_`, `\~`, `\=`, `\+`, `` \` ``, `\[`, `\]`,
//!   `\\`) emit the literal character, not a delimiter.
//! - Marks may nest when they cleanly close in LIFO order, e.g.
//!   `**_bold italic_**` → one span with bold weight + italic.
//! - A code span (`` ` ``) is a RAW context: no other marks and no escapes are
//!   parsed inside it; its text is verbatim.
//! - In `[label](url)`, the `label` is parsed for inline marks (all carrying
//!   the link); the `url` is taken verbatim. A `[` with no matching `](...)`
//!   is literal text.
//! - Flanking rule: a delimiter run only OPENS emphasis when immediately
//!   followed by a non-whitespace char, and only CLOSES when immediately
//!   preceded by a non-whitespace char (start/end of input count as
//!   whitespace). So `a * b` and `a ** b` are literal `*` / `**`.
//! - Unmatched / dangling delimiters degrade to literal text AT THEIR ORIGINAL
//!   POSITION. The function is infallible: malformed markdown never errors and
//!   never drops or reorders input — concatenating the span texts reproduces the
//!   input minus exactly the characters consumed as MATCHED delimiters or escape
//!   backslashes.
//! - Adjacent runs with identical mark sets are coalesced into one span.
//! - Fully deterministic: same input → same `Vec<TextSpan>`.

use crate::ast::node::TextSpan;
use crate::ast::value::PropertyValue;

/// Font-weight literal for `**bold**` text. Stored as a bare numeric literal so
/// the scene `resolve_font_weight` resolver parses it directly to `700` without
/// requiring a token. (See `zenith-scene` `compile/text/shape.rs`.)
const BOLD_WEIGHT: &str = "700";

/// Default highlight color for `==highlight==` (markdown highlight carries no
/// color). A conventional marker yellow; stored as a raw sRGB hex literal that
/// the scene `resolve_property_color` resolver parses directly via `parse_color`
/// / `parse_srgb_hex`. (See `zenith-scene` `compile/paint.rs`.)
const HIGHLIGHT_DEFAULT: &str = "#fff59d";

/// The set of inline marks active at a point in the scan. Pure value type so
/// it can be cheaply cloned/compared while descending and ascending delimiters.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct MarkSet {
    bold: bool,
    italic: bool,
    underline: bool,
    strikethrough: bool,
    highlight: bool,
    code: bool,
}

impl MarkSet {
    /// Build a [`TextSpan`] carrying `text` styled by this mark set plus an
    /// optional `link`.
    fn span(&self, text: String, link: Option<String>) -> TextSpan {
        TextSpan {
            text,
            fill: None,
            font_weight: if self.bold {
                Some(PropertyValue::Literal(BOLD_WEIGHT.to_owned()))
            } else {
                None
            },
            italic: if self.italic { Some(true) } else { None },
            underline: if self.underline { Some(true) } else { None },
            strikethrough: if self.strikethrough { Some(true) } else { None },
            vertical_align: None,
            footnote_ref: None,
            data_ref: None,
            data_format: None,
            highlight: if self.highlight {
                Some(PropertyValue::Literal(HIGHLIGHT_DEFAULT.to_owned()))
            } else {
                None
            },
            code: if self.code { Some(true) } else { None },
            link,
        }
    }
}

/// Which delimiter a given marker run corresponds to. Used to track the open
/// delimiter stack so a closing run pops the matching mark (LIFO nesting).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Delim {
    Bold,          // ** or __
    Italic,        // * or _
    Strikethrough, // ~~
    Underline,     // ++
    Highlight,     // ==
}

/// A token produced by the first (lexing) pass over the input.
///
/// Delimiter markers are emitted as-is and only RESOLVED into mark
/// open/close (or demoted to literal text) in the second pass, so an unmatched
/// delimiter can be re-surfaced as literal text AT ITS ORIGINAL POSITION.
#[derive(Debug, Clone)]
enum Token {
    /// A literal text fragment (escapes already decoded to their literal char).
    Text(String),
    /// A code span's verbatim contents (no inner parsing).
    Code(String),
    /// A resolved link: the label spans, each already carrying the link url.
    Link(Vec<TextSpan>),
    /// A delimiter marker run. `literal` is the exact source glyphs (`"**"`,
    /// `"_"`, …) so it can be demoted to literal text if it never pairs. `can_open`
    /// / `can_close` are the flanking flags computed at lex time. `role` is set by
    /// [`resolve_markers`]: an unpaired marker becomes [`Token::Text`] instead.
    Marker {
        delim: Delim,
        literal: String,
        can_open: bool,
        can_close: bool,
        role: MarkerRole,
    },
}

/// The pairing decision for a [`Token::Marker`], set by [`resolve_markers`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarkerRole {
    /// Not yet resolved (lex-time default). Any marker left as `Unresolved`
    /// after [`resolve_markers`] is demoted to literal text.
    Unresolved,
    /// A matched opener: the build pass pushes `delim` onto the mark stack.
    Open,
    /// A matched closer: the build pass pops `delim` off the mark stack.
    Close,
}

/// Parse an inline-markdown string into styled [`TextSpan`]s.
///
/// Infallible: malformed markdown degrades to literal text (it never errors and
/// never drops input). See the module docs for the supported syntax and rules.
pub fn parse_inline_markdown(input: &str) -> Vec<TextSpan> {
    let chars: Vec<char> = input.chars().collect();
    let link: Option<String> = None;
    let mut out: Vec<TextSpan> = Vec::new();
    parse_run(&chars, link, &mut out);
    out
}

/// Parse a character slice as a styled run, appending coalesced spans to `out`.
/// All spans produced carry `link` (the active hyperlink, if any).
///
/// Three passes: lex into [`Token`]s, resolve which delimiter markers pair up
/// (the rest are demoted to literal text in place), then build spans by walking
/// the resolved tokens with a live mark stack.
fn parse_run(chars: &[char], link: Option<String>, out: &mut Vec<TextSpan>) {
    let mut tokens = lex(chars);
    resolve_markers(&mut tokens);
    build_spans(&tokens, link, out);
}

/// First pass: turn the character slice into a flat list of [`Token`]s. Escapes
/// are decoded here, code spans and links are fully consumed, and delimiter runs
/// become [`Token::Marker`]s carrying their flanking flags. Nothing is matched or
/// demoted yet.
fn lex(chars: &[char]) -> Vec<Token> {
    let mut tokens: Vec<Token> = Vec::new();
    let mut buf = String::new();
    let mut i: usize = 0;

    while i < chars.len() {
        let Some(&c) = chars.get(i) else { break };

        // --- Backslash escape: the next char is literal. ---
        if c == '\\' {
            match chars.get(i + 1) {
                Some(&next) if is_escapable(next) => {
                    buf.push(next);
                    i += 2;
                    continue;
                }
                _ => {
                    buf.push('\\');
                    i += 1;
                    continue;
                }
            }
        }

        // --- Code span: raw, verbatim, no inner parsing. ---
        if c == '`' {
            if let Some(end) = find_code_close(chars, i + 1) {
                flush_text(&mut buf, &mut tokens);
                let raw: String = chars.get(i + 1..end).unwrap_or(&[]).iter().collect();
                tokens.push(Token::Code(raw));
                i = end + 1;
                continue;
            }
            buf.push('`');
            i += 1;
            continue;
        }

        // --- Link: [label](url) ---
        if c == '[' {
            if let Some((label, url, next)) = try_parse_link(chars, i) {
                flush_text(&mut buf, &mut tokens);
                let label_chars: Vec<char> = label.chars().collect();
                let mut label_spans: Vec<TextSpan> = Vec::new();
                parse_run(&label_chars, Some(url), &mut label_spans);
                tokens.push(Token::Link(label_spans));
                i = next;
                continue;
            }
            buf.push('[');
            i += 1;
            continue;
        }

        // --- Two-character delimiters: ** __ ~~ ++ == ---
        if let Some((delim, lit)) = match_two_char(chars, i) {
            flush_text(&mut buf, &mut tokens);
            let (can_open, can_close) = flanking(chars, i, 2);
            tokens.push(Token::Marker {
                delim,
                literal: lit,
                can_open,
                can_close,
                role: MarkerRole::Unresolved,
            });
            i += 2;
            continue;
        }

        // --- One-character emphasis: * or _ ---
        if c == '*' || c == '_' {
            flush_text(&mut buf, &mut tokens);
            let (can_open, can_close) = flanking(chars, i, 1);
            tokens.push(Token::Marker {
                delim: Delim::Italic,
                literal: c.to_string(),
                can_open,
                can_close,
                role: MarkerRole::Unresolved,
            });
            i += 1;
            continue;
        }

        // --- Ordinary character. ---
        buf.push(c);
        i += 1;
    }
    flush_text(&mut buf, &mut tokens);
    tokens
}

/// Flush the pending literal-text buffer into a [`Token::Text`] (if non-empty).
fn flush_text(buf: &mut String, tokens: &mut Vec<Token>) {
    if !buf.is_empty() {
        tokens.push(Token::Text(std::mem::take(buf)));
    }
}

/// Compute the `(can_open, can_close)` flanking flags for a delimiter run of
/// `width` chars starting at `i`. A run can OPEN only when immediately followed
/// by a non-whitespace char, and can CLOSE only when immediately preceded by a
/// non-whitespace char. (End-of-input / start-of-input count as whitespace.)
fn flanking(chars: &[char], i: usize, width: usize) -> (bool, bool) {
    let before = if i == 0 {
        None
    } else {
        chars.get(i - 1).copied()
    };
    let after = chars.get(i + width).copied();
    let followed_by_nonspace = matches!(after, Some(ch) if !ch.is_whitespace());
    let preceded_by_nonspace = matches!(before, Some(ch) if !ch.is_whitespace());
    (followed_by_nonspace, preceded_by_nonspace)
}

/// Second pass: decide which delimiter markers pair into open/close and which are
/// demoted to literal text. A marker that never pairs is rewritten to a
/// [`Token::Text`] of its own literal glyphs, IN PLACE — so no character moves or
/// vanishes. Matched pairs are left as `Marker`s for the build pass to act on.
///
/// Matching is greedy + LIFO: scanning left to right, a marker that `can_close`
/// is matched against the nearest still-open same-delim marker that `can_open`.
fn resolve_markers(tokens: &mut [Token]) {
    // Indices of open candidate markers, as a single stack preserving source
    // order so closing honors strict LIFO nesting across all delimiter kinds.
    let mut open_stack: Vec<usize> = Vec::new();

    for idx in 0..tokens.len() {
        let (delim, can_open, can_close) = match tokens.get(idx) {
            Some(Token::Marker {
                delim,
                can_open,
                can_close,
                ..
            }) => (*delim, *can_open, *can_close),
            _ => continue,
        };

        // Try to CLOSE against the nearest matching open on the stack (LIFO).
        if can_close
            && let Some(stack_pos) = open_stack.iter().rposition(
                |&oi| matches!(tokens.get(oi), Some(Token::Marker { delim: d, .. }) if *d == delim),
            )
            && let Some(&open_idx) = open_stack.get(stack_pos)
        {
            // Strict LIFO: any opens sitting ABOVE the matched one are now
            // crossed/unreachable. Drop them from the candidate set so they
            // fall through to literal-text demotion (no character is lost).
            open_stack.truncate(stack_pos);
            set_role(tokens, open_idx, MarkerRole::Open);
            set_role(tokens, idx, MarkerRole::Close);
            continue;
        }

        // Otherwise, if it can open, push as a candidate.
        if can_open {
            open_stack.push(idx);
        }
        // A marker that can neither close (here) nor open stays `Unresolved` and
        // is demoted to literal text below.
    }

    // Demote every still-unresolved marker to literal text IN PLACE.
    for idx in 0..tokens.len() {
        if let Some(Token::Marker {
            literal,
            role: MarkerRole::Unresolved,
            ..
        }) = tokens.get(idx)
        {
            let lit = literal.clone();
            if let Some(slot) = tokens.get_mut(idx) {
                *slot = Token::Text(lit);
            }
        }
    }
}

/// Set the resolved [`MarkerRole`] of the marker token at `idx` (no-op if the
/// token at `idx` is not a marker).
fn set_role(tokens: &mut [Token], idx: usize, new_role: MarkerRole) {
    if let Some(Token::Marker { role, .. }) = tokens.get_mut(idx) {
        *role = new_role;
    }
}

/// Third pass: walk the resolved tokens with a live mark stack, emitting spans.
/// A matched `Marker` toggles its mark (push on first sight = open, pop on second
/// = close); `Text` / `Code` / `Link` tokens emit styled content.
fn build_spans(tokens: &[Token], link: Option<String>, out: &mut Vec<TextSpan>) {
    let mut sink = SpanSink::new(link);
    let mut stack: Vec<Delim> = Vec::new();

    for tok in tokens {
        match tok {
            Token::Text(t) => {
                for ch in t.chars() {
                    sink.push_char(&stack, ch);
                }
            }
            Token::Code(raw) => {
                let mut marks = sink.marks_from_stack(&stack);
                marks.code = true;
                sink.push_span(marks.span(raw.clone(), sink.link.clone()));
            }
            Token::Link(spans) => {
                for s in spans {
                    sink.push_span(s.clone());
                }
            }
            Token::Marker { delim, role, .. } => match role {
                // `resolve_markers` paired these as clean LIFO open/close, so an
                // `Open` always pushes and a `Close` always pops its partner
                // (the topmost entry, which is the matching delim).
                MarkerRole::Open => stack.push(*delim),
                MarkerRole::Close => {
                    stack.pop();
                }
                // Unresolved markers were rewritten to `Text` already; this arm
                // is unreachable in practice but kept exhaustive (no `_`).
                MarkerRole::Unresolved => {}
            },
        }
    }

    sink.finish(out);
}

/// Accumulates characters into spans, coalescing adjacent runs that share an
/// identical mark set and link.
struct SpanSink {
    link: Option<String>,
    spans: Vec<TextSpan>,
    /// The mark set the current pending buffer is being styled with.
    pending_marks: MarkSet,
    pending_text: String,
    have_pending: bool,
}

impl SpanSink {
    fn new(link: Option<String>) -> Self {
        SpanSink {
            link,
            spans: Vec::new(),
            pending_marks: MarkSet::default(),
            pending_text: String::new(),
            have_pending: false,
        }
    }

    /// Derive the active mark set from the open delimiter stack.
    fn marks_from_stack(&self, stack: &[Delim]) -> MarkSet {
        let mut m = MarkSet::default();
        for delim in stack {
            match delim {
                Delim::Bold => m.bold = true,
                Delim::Italic => m.italic = true,
                Delim::Strikethrough => m.strikethrough = true,
                Delim::Underline => m.underline = true,
                Delim::Highlight => m.highlight = true,
            }
        }
        m
    }

    /// Push one character, styled by the marks currently active on `stack`.
    fn push_char(&mut self, stack: &[Delim], c: char) {
        let marks = self.marks_from_stack(stack);
        if self.have_pending && marks == self.pending_marks {
            self.pending_text.push(c);
        } else {
            self.flush_pending();
            self.pending_marks = marks;
            self.pending_text.push(c);
            self.have_pending = true;
        }
    }

    /// Push a fully-formed span (used for code spans and link sub-spans), first
    /// flushing any pending buffered text. Empty-text spans are dropped (an
    /// empty `` `` `` code span or `[](u)` link has no glyphs to render).
    fn push_span(&mut self, span: TextSpan) {
        if span.text.is_empty() {
            return;
        }
        self.flush_pending();
        if let Some(last) = self.spans.last_mut()
            && spans_mergeable(last, &span)
        {
            last.text.push_str(&span.text);
            return;
        }
        self.spans.push(span);
    }

    /// Flush the pending buffered text into a span (coalescing if possible).
    fn flush_pending(&mut self) {
        if !self.have_pending {
            return;
        }
        let text = std::mem::take(&mut self.pending_text);
        let marks = std::mem::take(&mut self.pending_marks);
        self.have_pending = false;
        if text.is_empty() {
            return;
        }
        let span = marks.span(text, self.link.clone());
        if let Some(last) = self.spans.last_mut()
            && spans_mergeable(last, &span)
        {
            last.text.push_str(&span.text);
            return;
        }
        self.spans.push(span);
    }

    /// Finalize: flush pending text and append spans to `out`.
    fn finish(mut self, out: &mut Vec<TextSpan>) {
        self.flush_pending();
        out.append(&mut self.spans);
    }
}

/// Two spans may be merged when every styling field and the link are identical.
fn spans_mergeable(a: &TextSpan, b: &TextSpan) -> bool {
    a.fill == b.fill
        && a.font_weight == b.font_weight
        && a.italic == b.italic
        && a.underline == b.underline
        && a.strikethrough == b.strikethrough
        && a.vertical_align == b.vertical_align
        && a.footnote_ref == b.footnote_ref
        && a.data_ref == b.data_ref
        && a.data_format == b.data_format
        && a.highlight == b.highlight
        && a.code == b.code
        && a.link == b.link
}

/// Whether `c` is a character a backslash may escape into a literal.
fn is_escapable(c: char) -> bool {
    matches!(c, '*' | '_' | '~' | '=' | '+' | '`' | '[' | ']' | '\\')
}

/// Match a two-character delimiter starting at `i`. Returns the delimiter and
/// its literal text, or `None`.
fn match_two_char(chars: &[char], i: usize) -> Option<(Delim, String)> {
    let a = *chars.get(i)?;
    let b = *chars.get(i + 1)?;
    let delim = match (a, b) {
        ('*', '*') | ('_', '_') => Delim::Bold,
        ('~', '~') => Delim::Strikethrough,
        ('+', '+') => Delim::Underline,
        ('=', '=') => Delim::Highlight,
        _ => return None,
    };
    Some((delim, format!("{a}{b}")))
}

/// Find the index of the closing backtick for a code span that opened just
/// before `start`. Returns the index of the closing `` ` ``, or `None` if there
/// is no closing backtick.
fn find_code_close(chars: &[char], start: usize) -> Option<usize> {
    let mut j = start;
    while j < chars.len() {
        if chars.get(j) == Some(&'`') {
            return Some(j);
        }
        j += 1;
    }
    None
}

/// Attempt to parse a `[label](url)` link beginning at `open` (which must be the
/// `[`). Returns `(label, url, next_index)` on success, where `next_index` is
/// the index just past the closing `)`. The label scan respects backslash
/// escapes for `]` so an escaped bracket does not close the label.
fn try_parse_link(chars: &[char], open: usize) -> Option<(String, String, usize)> {
    if chars.get(open) != Some(&'[') {
        return None;
    }
    // Scan label until an unescaped ']'.
    let mut j = open + 1;
    let mut label: Vec<char> = Vec::new();
    let mut closed_label: Option<usize> = None;
    while j < chars.len() {
        match chars.get(j) {
            Some(&'\\') => {
                // Preserve the escape sequence verbatim into the label so the
                // recursive label parse re-handles it.
                if let Some(&next) = chars.get(j + 1) {
                    label.push('\\');
                    label.push(next);
                    j += 2;
                    continue;
                }
                label.push('\\');
                j += 1;
            }
            Some(&']') => {
                closed_label = Some(j);
                break;
            }
            Some(&ch) => {
                label.push(ch);
                j += 1;
            }
            None => break,
        }
    }
    let label_end = closed_label?;
    // Immediately after ']' must come '('.
    let paren_open = label_end + 1;
    if chars.get(paren_open) != Some(&'(') {
        return None;
    }
    // Scan url verbatim until the matching ')'. No nested parens handling
    // (basic markdown); the first ')' closes.
    let mut k = paren_open + 1;
    let mut url: Vec<char> = Vec::new();
    let mut closed_url: Option<usize> = None;
    while k < chars.len() {
        match chars.get(k) {
            Some(&')') => {
                closed_url = Some(k);
                break;
            }
            Some(&ch) => {
                url.push(ch);
                k += 1;
            }
            None => break,
        }
    }
    let url_end = closed_url?;
    Some((
        label.into_iter().collect(),
        url.into_iter().collect(),
        url_end + 1,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(spans: &[TextSpan]) -> String {
        spans.iter().map(|s| s.text.as_str()).collect()
    }

    fn bold() -> Option<PropertyValue> {
        Some(PropertyValue::Literal(BOLD_WEIGHT.to_owned()))
    }
    fn hl() -> Option<PropertyValue> {
        Some(PropertyValue::Literal(HIGHLIGHT_DEFAULT.to_owned()))
    }

    #[test]
    fn empty_input_yields_no_spans() {
        assert!(parse_inline_markdown("").is_empty());
    }

    #[test]
    fn plain_text_single_span() {
        let s = parse_inline_markdown("hello world");
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].text, "hello world");
        assert_eq!(s[0].font_weight, None);
        assert_eq!(s[0].italic, None);
    }

    #[test]
    fn bold_star_and_underscore() {
        for src in ["**bold**", "__bold__"] {
            let s = parse_inline_markdown(src);
            assert_eq!(s.len(), 1, "src={src}");
            assert_eq!(s[0].text, "bold");
            assert_eq!(s[0].font_weight, bold());
        }
    }

    #[test]
    fn italic_star_and_underscore() {
        for src in ["*it*", "_it_"] {
            let s = parse_inline_markdown(src);
            assert_eq!(s.len(), 1, "src={src}");
            assert_eq!(s[0].text, "it");
            assert_eq!(s[0].italic, Some(true));
            assert_eq!(s[0].font_weight, None);
        }
    }

    #[test]
    fn strikethrough() {
        let s = parse_inline_markdown("~~gone~~");
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].text, "gone");
        assert_eq!(s[0].strikethrough, Some(true));
    }

    #[test]
    fn underline() {
        let s = parse_inline_markdown("++under++");
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].text, "under");
        assert_eq!(s[0].underline, Some(true));
    }

    #[test]
    fn highlight_uses_default_color() {
        let s = parse_inline_markdown("==mark==");
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].text, "mark");
        assert_eq!(s[0].highlight, hl());
    }

    #[test]
    fn code_span_basic() {
        let s = parse_inline_markdown("`fn main()`");
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].text, "fn main()");
        assert_eq!(s[0].code, Some(true));
    }

    #[test]
    fn code_span_is_verbatim_no_inner_parsing() {
        let s = parse_inline_markdown("`**not bold** \\n _x_`");
        assert_eq!(s.len(), 1);
        // Backticks content is raw: delimiters and backslash are literal.
        assert_eq!(s[0].text, "**not bold** \\n _x_");
        assert_eq!(s[0].code, Some(true));
        assert_eq!(s[0].font_weight, None);
        assert_eq!(s[0].italic, None);
    }

    #[test]
    fn nested_bold_italic_single_span() {
        // **_bold italic_** → one span with bold + italic.
        let s = parse_inline_markdown("**_bold italic_**");
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].text, "bold italic");
        assert_eq!(s[0].font_weight, bold());
        assert_eq!(s[0].italic, Some(true));
    }

    #[test]
    fn nested_highlight_bold() {
        // ==**important**== → highlight + bold.
        let s = parse_inline_markdown("==**important**==");
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].text, "important");
        assert_eq!(s[0].highlight, hl());
        assert_eq!(s[0].font_weight, bold());
    }

    #[test]
    fn partial_nesting_splits_spans() {
        // a **b _c_ d** e
        let s = parse_inline_markdown("a **b _c_ d** e");
        assert_eq!(texts(&s), "a b c d e");
        // "a " plain, "b " bold, "c" bold+italic, " d" bold, " e" plain.
        let joined: Vec<(&str, bool, bool)> = s
            .iter()
            .map(|x| {
                (
                    x.text.as_str(),
                    x.font_weight.is_some(),
                    x.italic == Some(true),
                )
            })
            .collect();
        assert_eq!(
            joined,
            vec![
                ("a ", false, false),
                ("b ", true, false),
                ("c", true, true),
                (" d", true, false),
                (" e", false, false),
            ]
        );
    }

    #[test]
    fn escapes_emit_literals() {
        let s = parse_inline_markdown(r##"\*not italic\* \_ \~ \= \+ \` \[ \] \\"##);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].text, r##"*not italic* _ ~ = + ` [ ] \"##);
        assert_eq!(s[0].italic, None);
        assert_eq!(s[0].font_weight, None);
    }

    #[test]
    fn backslash_before_normal_char_is_literal() {
        let s = parse_inline_markdown(r##"a\b"##);
        assert_eq!(texts(&s), r##"a\b"##);
    }

    #[test]
    fn link_plain_label() {
        let s = parse_inline_markdown("[Zenith](https://example.com)");
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].text, "Zenith");
        assert_eq!(s[0].link.as_deref(), Some("https://example.com"));
    }

    #[test]
    fn link_label_with_inner_marks() {
        let s = parse_inline_markdown("[**bold** link](u)");
        assert_eq!(texts(&s), "bold link");
        for sp in &s {
            assert_eq!(sp.link.as_deref(), Some("u"));
        }
        assert_eq!(s[0].text, "bold");
        assert_eq!(s[0].font_weight, bold());
        assert_eq!(s[1].text, " link");
        assert_eq!(s[1].font_weight, None);
    }

    #[test]
    fn link_url_is_verbatim() {
        // Markdown inside the url is NOT parsed.
        let s = parse_inline_markdown("[x](http://a/**b**)");
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].link.as_deref(), Some("http://a/**b**"));
    }

    #[test]
    fn bracket_without_link_is_literal() {
        let s = parse_inline_markdown("[just text]");
        assert_eq!(texts(&s), "[just text]");
        assert!(s.iter().all(|sp| sp.link.is_none()));
    }

    #[test]
    fn bracket_with_label_but_no_paren_is_literal() {
        let s = parse_inline_markdown("[label] (noturl)");
        assert_eq!(texts(&s), "[label] (noturl)");
        assert!(s.iter().all(|sp| sp.link.is_none()));
    }

    #[test]
    fn dangling_bold_is_literal() {
        let s = parse_inline_markdown("**oops");
        assert_eq!(texts(&s), "**oops");
        assert!(s.iter().all(|sp| sp.font_weight.is_none()));
    }

    #[test]
    fn lone_star_is_literal() {
        let s = parse_inline_markdown("a * b");
        assert_eq!(texts(&s), "a * b");
        assert!(s.iter().all(|sp| sp.italic.is_none()));
    }

    #[test]
    fn unmatched_closing_underscore_is_literal() {
        let s = parse_inline_markdown("end_");
        assert_eq!(texts(&s), "end_");
        assert!(s.iter().all(|sp| sp.italic.is_none()));
    }

    #[test]
    fn whitespace_flanked_double_delim_is_literal_in_place() {
        // `a ** b` — the `**` is surrounded by spaces so it can neither open nor
        // close; it must appear as literal text AT ITS ORIGINAL POSITION.
        let s = parse_inline_markdown("a ** b");
        assert_eq!(texts(&s), "a ** b");
        assert!(s.iter().all(|sp| sp.font_weight.is_none()));
    }

    #[test]
    fn dangling_opener_emits_literal_in_original_position() {
        // The unmatched `*` sits BETWEEN "x " and " y" and must stay there, not
        // move to the end. (Regression: earlier design re-emitted at the tail.)
        let s = parse_inline_markdown("x *unclosed");
        assert_eq!(texts(&s), "x *unclosed");
        assert!(s.iter().all(|sp| sp.italic.is_none()));
        // And the literal `*` is immediately followed by "unclosed".
        let joined = texts(&s);
        let star = joined.find('*').expect("literal star present");
        assert!(joined[star + 1..].starts_with("unclosed"));
    }

    #[test]
    fn opener_needs_following_nonspace() {
        // `* a*` — first `*` is followed by a space → cannot open → literal.
        let s = parse_inline_markdown("* a*");
        assert_eq!(texts(&s), "* a*");
        assert!(s.iter().all(|sp| sp.italic.is_none()));
    }

    #[test]
    fn closer_needs_preceding_nonspace() {
        // `*a *` — closing `*` is preceded by a space → cannot close → both
        // delimiters fall through to literal text.
        let s = parse_inline_markdown("*a *");
        assert_eq!(texts(&s), "*a *");
        assert!(s.iter().all(|sp| sp.italic.is_none()));
    }

    #[test]
    fn same_delim_nested_pairs_keep_marks() {
        // `**a **b** c**` — both bold pairs resolve; every word stays bold and no
        // character is lost.
        let s = parse_inline_markdown("**a **b** c**");
        assert_eq!(texts(&s), "a b c");
        assert!(s.iter().all(|sp| sp.font_weight == bold()));
    }

    #[test]
    fn no_character_loss_consumes_only_delimiters() {
        // Concatenated span text equals input minus the markdown delimiters.
        let s = parse_inline_markdown("**a** _b_ ~~c~~ ++d++ ==e==");
        assert_eq!(texts(&s), "a b c d e");
    }

    #[test]
    fn no_character_loss_with_escapes() {
        // Escapes consume the backslash; everything else preserved.
        let s = parse_inline_markdown(r##"x \* y"##);
        assert_eq!(texts(&s), "x * y");
    }

    #[test]
    fn determinism_parse_twice_equal() {
        let src = "a **b _c_** ~~d~~ `e` [f](g) ==h== \\* ++i++";
        let a = parse_inline_markdown(src);
        let b = parse_inline_markdown(src);
        assert_eq!(a, b);
    }

    #[test]
    fn combined_all_marks() {
        let s = parse_inline_markdown("==++~~**_x_**~~++==");
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].text, "x");
        assert_eq!(s[0].highlight, hl());
        assert_eq!(s[0].underline, Some(true));
        assert_eq!(s[0].strikethrough, Some(true));
        assert_eq!(s[0].font_weight, bold());
        assert_eq!(s[0].italic, Some(true));
    }

    #[test]
    fn code_inside_text_run() {
        let s = parse_inline_markdown("use `cargo build` now");
        assert_eq!(texts(&s), "use cargo build now");
        assert_eq!(s[0].text, "use ");
        assert_eq!(s[0].code, None);
        assert_eq!(s[1].text, "cargo build");
        assert_eq!(s[1].code, Some(true));
        assert_eq!(s[2].text, " now");
    }

    #[test]
    fn unclosed_code_is_literal_backtick() {
        let s = parse_inline_markdown("a `b c");
        assert_eq!(texts(&s), "a `b c");
        assert!(s.iter().all(|sp| sp.code.is_none()));
    }

    #[test]
    fn adjacent_same_marks_coalesce() {
        // "**a****b**" → bold a then bold b; adjacent identical marks coalesce.
        let s = parse_inline_markdown("**a****b**");
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].text, "ab");
        assert_eq!(s[0].font_weight, bold());
    }
}
