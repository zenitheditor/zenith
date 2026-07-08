//! Text-bearing leaf structs: `text` and `code`.

use std::collections::BTreeMap;

use crate::ast::block_style::BlockStyle;
use crate::ast::value::{Dimension, PropertyValue};
use crate::ast::{KerningPair, Span};
use crate::tokens::SyntaxTheme;

use crate::ast::node::common::{TextSpan, UnknownProperty};

/// A `text` node.
#[derive(Debug, Clone, PartialEq)]
pub struct TextNode {
    pub id: String,
    pub name: Option<String>,
    pub role: Option<String>,
    pub x: Option<PropertyValue>,
    pub y: Option<PropertyValue>,
    pub w: Option<PropertyValue>,
    pub h: Option<PropertyValue>,
    pub align: Option<String>,
    /// Vertical text-block alignment within the box (`top`/`middle`/`bottom`,
    /// default `top` = today's behavior: no y offset applied). When the box
    /// height exceeds the laid-out text block height, the block is offset by
    /// `0` (top), `(box_h - text_h)/2` (middle), or `box_h - text_h`
    /// (bottom). Unknown values are treated as `top` (byte-identical to absent).
    pub v_align: Option<String>,
    pub direction: Option<String>,
    pub overflow: Option<String>,
    /// Overflow-wrap mode. `Some("break-word")` lets the line packer break an
    /// unbreakable token (a long URL/compound with no space or hyphen point) that
    /// is wider than the line box at a CHARACTER boundary, so it no longer
    /// overflows; a forced break emits an advisory `text.forced_break`. `None` or
    /// `"normal"` keeps the default (the overlong token overflows/clips,
    /// byte-identical to a node without the attribute). KDL:
    /// `overflow-wrap="break-word"`.
    pub overflow_wrap: Option<String>,
    pub style: Option<String>,
    pub fill: Option<PropertyValue>,
    /// Glyph outline (stroke) color. Token-required (like `fill`). When `Some`,
    /// each glyph path is filled then stroked with this color. `None` → no
    /// outline; byte-identical to a node without `stroke`. KDL:
    /// `stroke=(token)"color.ink.outline"`.
    pub stroke: Option<PropertyValue>,
    /// Glyph outline width in pixels. Token-required (like `font-size`). Only
    /// effective when `stroke` is also set. `None` / 0 → no outline.
    /// KDL: `stroke-width=(token)"size.stroke.hairline"`.
    pub stroke_width: Option<PropertyValue>,
    /// WCAG contrast hint: an explicit background color (token ref) the text
    /// visually sits ON, for nodes placed over an `image` or other non-fillable
    /// backdrop the validator cannot sample. When set, the contrast check uses
    /// THIS color as the background (highest priority, over any detected backdrop
    /// and the page background). Token-only, like `fill`. `None` → unchanged
    /// backdrop detection. KDL: `contrast-bg=(token)"color.photo.shadow"`.
    pub contrast_bg: Option<PropertyValue>,
    pub font_family: Option<PropertyValue>,
    pub font_size: Option<PropertyValue>,
    /// Floor font size for `overflow="autofit"` — the node's font shrinks no
    /// smaller than this when fitting. Token-only, like `font-size`. `None` → a
    /// default floor (`(declared * 0.5).max(8.0)`). KDL:
    /// `font-size-min=(token)"size.min"`.
    pub font_size_min: Option<PropertyValue>,
    /// Numeric font weight (100–900), usually a `fontWeight` token ref.
    pub font_weight: Option<PropertyValue>,
    /// Comma-separated OpenType feature requests applied to this text node's
    /// spans unless a span overrides them. Examples: `"liga=0,kern=1,ss01"`.
    pub font_features: Option<String>,
    /// Comma-separated alternate-selection aliases applied to this text node's
    /// spans unless a span overrides them. Examples: `"styleset(1),stylistic"`.
    pub font_alternates: Option<String>,
    /// Additional letter spacing inserted between adjacent shaped glyphs. Token or
    /// dimension, resolved to pixels by scene compilation. `None` keeps natural
    /// font spacing.
    pub letter_spacing: Option<PropertyValue>,
    /// Node-scoped manual kerning pair adjustments, applied during shaping.
    /// Empty when no `kern-pair` children are declared.
    pub kerning_pairs: Vec<KerningPair>,
    /// Drop shadow / outer glow, as a `(token)` ref to a `shadow` token.
    pub shadow: Option<PropertyValue>,
    /// Color/image filter ops, as a `(token)` ref to a `filter` token.
    pub filter: Option<PropertyValue>,
    /// Spatial coverage mask, as a `(token)` ref to a `mask` token.
    pub mask: Option<PropertyValue>,
    /// Compositing blend mode: `"normal"` (default) or one of the 11 separable
    /// blends. `None`/`"normal"` render source-over (byte-identical).
    pub blend_mode: Option<String>,
    /// Gaussian blur radius applied to the node's own rendered ink (sigma in
    /// the declared unit, resolved to pixels at compile time). `None` / 0 →
    /// no blur (byte-identical to having no attribute).
    pub blur: Option<Dimension>,
    pub opacity: Option<f64>,
    pub visible: Option<bool>,
    pub locked: Option<bool>,
    /// PDF text-extraction toggle. `None`/`Some(true)` (default) → the text is
    /// emitted as real, selectable/searchable/indexable text with a ToUnicode
    /// map, and any `link` spans become clickable. `Some(false)` → the text is
    /// drawn as filled glyph outlines instead, so it is visually identical but
    /// cannot be selected, copied, searched, or indexed. PDF-only; the raster
    /// backend renders identically either way. KDL: `selectable=#false`.
    pub selectable: Option<bool>,
    pub rotate: Option<Dimension>,
    /// Threaded-text-flow chain id. When `Some(id)`, this text node is a member
    /// of the chain named `id`; all text nodes sharing the same `chain` id form
    /// an ordered chain (ordering = document source order). A long article
    /// placed in the FIRST member's spans flows across every member's box in
    /// order: each box consumes as much text as fits, the remainder continues in
    /// the next member. Continuation members carry `chain=id` with empty spans.
    ///
    /// v0 semantics (documented):
    /// - Content source: the first member (source order) that has non-empty
    ///   spans is the sole content source; later members' spans are ignored
    ///   (no concatenation).
    /// - Shared style: all members are assumed to share font family/size/weight/
    ///   fill; the whole chain is shaped with the FIRST member's resolved style.
    ///   Each box re-wraps to its OWN width, so line height stays uniform.
    pub chain: Option<String>,
    /// Drop-cap initial: the FIRST grapheme of the paragraph is typeset large,
    /// spanning `Some(n)` body lines at the top-left, with the first `n` body
    /// lines wrapping to a narrower measure beside it and line `n+1` onward
    /// returning to the full box width. `Some(0)` or `None` disables the drop
    /// cap (rendered byte-identically to a node without the attribute). Honored
    /// only on the single-box wrap path (a box with a width whose text overflows
    /// it); chain/flow integration is a documented v0 follow-up. KDL:
    /// `drop-cap-lines=3`.
    pub drop_cap_lines: Option<u32>,
    /// Knuth–Liang hyphenation toggle. When `Some(true)`, the greedy line packer
    /// may break a word that does not fit the remaining space on a non-empty line
    /// at an embedded (en-US) hyphenation point, placing `fragment-` on the
    /// current line and carrying the remainder to the next. `None`/`Some(false)`
    /// disables hyphenation (byte-identical to a node without the attribute).
    /// KDL: `hyphenate=#true`.
    pub hyphenate: Option<bool>,
    /// Widow/orphan control: keep at least `Some(n)` lines of a paragraph
    /// together across a chain box/page break. `n=2` prevents a lone first line
    /// (orphan) from being stranded at a box bottom and a lone last line (widow)
    /// from starting the next box. Applied only at the CHAIN distribution
    /// boundary, read from the chain source node. `None` disables the control
    /// (byte-identical to a node without the attribute). KDL: `widow-orphan=2`.
    pub widow_orphan: Option<u32>,
    /// Tab-stop leader character. When `Some(s)` with a non-empty `s`, the node
    /// renders in TAB-LEADER mode (table-of-contents rows): the combined span
    /// text is split into rows on `\n`, each row is split on its FIRST `\t` into
    /// a LEFT and RIGHT segment, the LEFT segment is placed at the box left edge,
    /// the RIGHT segment is right-aligned to the box right edge, and the gap
    /// between them is filled with the leader glyph `s` (e.g. `"."`) repeated.
    /// A row with no tab renders left-aligned with no leader. `None` or an empty
    /// string disables tab-leader mode (byte-identical to a node without the
    /// attribute). KDL: `tab-leader="."`.
    pub tab_leader: Option<String>,
    /// Text-runaround exclusion: the id of ANOTHER node on the same page whose
    /// bounding box becomes an exclusion zone this text wraps around. For each
    /// wrapped line whose vertical band intersects the excluded rect, the line
    /// flows into the LARGER free horizontal segment (left or right of the rect);
    /// a line with no segment wide enough is left blank so text flows above and
    /// below a full-width exclusion ("largest-area / jump" wrap). An id naming no
    /// resolvable node yields an advisory `text-exclusion.unresolved_ref` and the
    /// text renders with no exclusion (byte-identical to a node without the
    /// attribute). Honored on the single-box wrap path; chain-member runaround is a
    /// documented v0 follow-up. KDL: `text-exclusion="author.portrait"`.
    pub text_exclusion: Option<String>,
    /// Left padding in pixels applied to EVERY wrapped line (indents the text-box
    /// left edge inward, reducing the measure). Combine with a negative
    /// [`TextNode::text_indent`] for a hanging indent (bulleted lists). `None` → 0.
    /// KDL: `padding-left=(px)44`.
    pub padding_left: Option<Dimension>,
    /// First-line horizontal offset in pixels RELATIVE to the padded left edge.
    /// May be NEGATIVE to pull the first line back out (a hanging bullet glyph sits
    /// left of the wrapped continuation lines). Applies to line 0 of the box only
    /// (per-paragraph first-line indent is a documented v0 follow-up). `None` → 0.
    /// KDL: `text-indent=(px)-44`.
    pub text_indent: Option<Dimension>,
    /// Auto-aligning list bullet. When `Some(marker)` (a non-empty string like "•",
    /// "–", "1."), the node renders as a hanging-indent list item: the marker is
    /// drawn once in the left margin at the first line's baseline, and ALL text
    /// lines (first and wrapped) are indented to a column at `marker_advance + gap`
    /// from the box left edge, so continuation lines auto-align with the text after
    /// the marker — measured from the marker shaped at the node's own font, hence
    /// font/size-independent. The span text holds only the content (no bullet glyph).
    /// `None` → not a list item (byte-identical to a node without the attribute).
    /// Honored on the plain single-box wrap path; drop-cap/runaround/chain are a
    /// documented v0 follow-up. KDL: `bullet="•"`.
    pub bullet: Option<String>,
    /// Gap between the bullet marker and the text column, in pixels. `None` → a
    /// default proportional to the font size (`0.4 × font_size`). KDL:
    /// `bullet-gap=(px)16`.
    pub bullet_gap: Option<Dimension>,
    /// Content format for this text node's span text.
    ///
    /// When `Some("markdown")`, the scene compile pass re-parses the concatenated
    /// span text (AFTER data-binding substitution) as inline markdown, replacing
    /// `node.spans` with the parsed styled spans. This enables `**bold**`,
    /// `*italic*`, `~~strike~~`, `==highlight==`, `++underline++`, `` `code` ``,
    /// and `[label](url)` in both literal text and data-bound (`data-ref`) content.
    ///
    /// `Some("plain")` or `None` keeps the current behavior — spans are used
    /// verbatim without any markdown interpretation (byte-identical to before).
    ///
    /// Any other value emits a `text.invalid_format` warning and is treated as
    /// plain (byte-identical to a node without the attribute).
    ///
    /// KDL: `format="markdown"`.
    pub content_format: Option<String>,
    /// Path to an external text or markdown file whose contents become this
    /// node's text content, relative to the document's project directory.
    ///
    /// When `Some(path)`, the CLI render layer reads the file and replaces
    /// `spans` with a single plain span carrying the file's raw UTF-8 text
    /// before compilation. When `format="markdown"` is also set, the existing
    /// `markdown_resolve` compile pass then parses the loaded text into styled
    /// spans automatically. When the file cannot be read, a `text.src_missing`
    /// Error diagnostic is emitted and the node's existing spans are left
    /// unchanged.
    ///
    /// The field is retained on the node after loading so that a future editor
    /// can write edits back to the original file.
    ///
    /// A text node WITHOUT `src` is completely unaffected by the loader
    /// (byte-identical to before).
    ///
    /// KDL: `src="copy/article.md"`.
    pub src: Option<String>,
    /// Inline text spans.
    pub spans: Vec<TextSpan>,
    /// Per-role markdown block style declarations at text-node scope. Empty when
    /// no `block role="…"` children are declared on this text node. Highest
    /// cascade precedence (text > page > document). Data-only in this unit; the
    /// layout engine consumes them later.
    pub block_styles: Vec<BlockStyle>,
    /// Page-relative placement anchor (one of the nine named positions, e.g.
    /// `"bottom-right"`). When present and recognized, the compile step derives
    /// the node's x and/or y from the page and node dimensions. An explicitly-
    /// authored x or y always wins.
    pub anchor: Option<String>,
    /// Optional safe-zone reference for the anchor. See [`RectNode::anchor_zone`](crate::ast::node::RectNode::anchor_zone).
    pub anchor_zone: Option<String>,
    /// Optional sibling node id for sibling-relative anchor positioning.
    /// See [`RectNode::anchor_sibling`](crate::ast::node::RectNode::anchor_sibling).
    pub anchor_sibling: Option<String>,
    /// Adjacent-placement edge relative to `anchor-sibling`: `above`/`below`/`before`/`after`.
    /// See [`RectNode::anchor_edge`](crate::ast::node::RectNode::anchor_edge).
    pub anchor_edge: Option<String>,
    /// Gap (px) between this node and its `anchor-sibling` edge when `anchor-edge` is set.
    /// See [`RectNode::anchor_gap`](crate::ast::node::RectNode::anchor_gap).
    pub anchor_gap: Option<Dimension>,
    /// Parent-relative anchor toggle. See [`RectNode::anchor_parent`](crate::ast::node::RectNode::anchor_parent).
    pub anchor_parent: Option<bool>,
    /// Source declaration span, when available.
    pub source_span: Option<Span>,
    /// Unknown properties preserved for forward-compat.
    pub unknown_props: BTreeMap<String, UnknownProperty>,
}

/// A `code` node — a multi-line MONOSPACE text block.
///
/// Structurally this mirrors [`TextNode`] but carries a single verbatim source
/// blob instead of styled `spans`. The blob is stored DECODED (newlines and
/// tabs are literal characters); the formatter re-encodes it with escapes.
///
/// The verbatim source is carried in the KDL as a `content` child node with one
/// escaped string argument (NOT a bare `r#"..."#` raw string): KDL v2 multi-line
/// string dedent semantics make the raw form lossy, whereas a single-line
/// escaped string round-trips `\n \t \" \\` exactly through the `kdl` crate.
/// See `transform_code` / `write_code` for the parse/format sides.
#[derive(Debug, Clone, PartialEq)]
pub struct CodeNode {
    pub id: String,
    pub name: Option<String>,
    pub role: Option<String>,
    pub x: Option<PropertyValue>,
    pub y: Option<PropertyValue>,
    pub w: Option<PropertyValue>,
    pub h: Option<PropertyValue>,
    /// "clip" (default) or "visible"; v0 does not word-wrap.
    pub overflow: Option<String>,
    /// Open string naming the source language; drives built-in syntax
    /// highlighting when the language is supported, otherwise renders as plain text.
    pub language: Option<String>,
    /// Render a line-number gutter (default false).
    pub line_numbers: Option<bool>,
    /// Rendered column width of a tab (default 4).
    pub tab_width: Option<u32>,
    pub style: Option<String>,
    pub fill: Option<PropertyValue>,
    pub font_family: Option<PropertyValue>,
    pub font_size: Option<PropertyValue>,
    /// Numeric font weight (100–900), usually a `fontWeight` token ref.
    pub font_weight: Option<PropertyValue>,
    /// Comma-separated OpenType feature requests applied to code shaping.
    pub font_features: Option<String>,
    /// Comma-separated alternate-selection aliases applied to code shaping.
    pub font_alternates: Option<String>,
    /// Additional letter spacing inserted between adjacent shaped glyphs. Token or
    /// dimension, resolved to pixels by scene compilation. `None` keeps natural
    /// monospace spacing.
    pub letter_spacing: Option<PropertyValue>,
    /// Node-scoped manual kerning pair adjustments, applied during shaping.
    /// Empty when no `kern-pair` children are declared.
    pub kerning_pairs: Vec<KerningPair>,
    /// Optional built-in syntax-highlight color theme; `None` = use default (`Dark`).
    pub syntax_theme: Option<SyntaxTheme>,
    pub opacity: Option<f64>,
    pub visible: Option<bool>,
    pub locked: Option<bool>,
    /// PDF text-extraction toggle (see [`TextNode::selectable`]). `None`/
    /// `Some(true)` (default) → real selectable/searchable text; `Some(false)` →
    /// filled glyph outlines (visually identical, not extractable). PDF-only.
    pub selectable: Option<bool>,
    pub rotate: Option<Dimension>,
    /// Verbatim source text (decoded; newlines/tabs are literal characters).
    pub content: String,
    /// Page-relative placement anchor (one of the nine named positions, e.g.
    /// `"bottom-right"`). When present and recognized, the compile step derives
    /// the node's x and/or y from the page and node dimensions. An explicitly-
    /// authored x or y always wins.
    pub anchor: Option<String>,
    /// Optional safe-zone reference for the anchor. See [`RectNode::anchor_zone`](crate::ast::node::RectNode::anchor_zone).
    pub anchor_zone: Option<String>,
    /// Optional sibling node id for sibling-relative anchor positioning.
    /// See [`RectNode::anchor_sibling`](crate::ast::node::RectNode::anchor_sibling).
    pub anchor_sibling: Option<String>,
    /// Adjacent-placement edge relative to `anchor-sibling`: `above`/`below`/`before`/`after`.
    /// See [`RectNode::anchor_edge`](crate::ast::node::RectNode::anchor_edge).
    pub anchor_edge: Option<String>,
    /// Gap (px) between this node and its `anchor-sibling` edge when `anchor-edge` is set.
    /// See [`RectNode::anchor_gap`](crate::ast::node::RectNode::anchor_gap).
    pub anchor_gap: Option<Dimension>,
    /// Parent-relative anchor toggle. See [`RectNode::anchor_parent`](crate::ast::node::RectNode::anchor_parent).
    pub anchor_parent: Option<bool>,
    /// Source declaration span, when available.
    pub source_span: Option<Span>,
    /// Unknown properties preserved for forward-compat.
    pub unknown_props: BTreeMap<String, UnknownProperty>,
}
