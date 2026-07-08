//! Transforms for the text-bearing leaf nodes: text and code.

use kdl::{KdlNode, KdlValue};

use crate::ast::block_style::BlockStyle;
use crate::ast::node::{CodeNode, TextNode, TextSpan};
use crate::error::ParseError;
use crate::tokens::SyntaxTheme;

use super::span::transform_span;
use crate::parse::transform::block_style::transform_block_style;
use crate::parse::transform::helpers::{
    collect_unknown_props, node_span, optional_bool_prop, optional_dimension_prop,
    optional_f64_prop, optional_property_value, optional_property_value_aliased,
    optional_string_prop, optional_string_prop_aliased, optional_u32_prop, required_string_prop,
};
use crate::parse::transform::kerning::transform_kerning_pair;

pub(crate) const TEXT_KNOWN_PROPS: &[&str] = &[
    "id",
    "name",
    "role",
    "x",
    "y",
    "w",
    "h",
    "align",
    "v-align",
    "v_align",
    "direction",
    "overflow",
    "overflow-wrap",
    "overflow_wrap",
    "style",
    "fill",
    "stroke",
    "stroke-width",
    "stroke_width",
    "contrast-bg",
    "contrast_bg",
    "font-family",
    "font_family",
    "font-size",
    "font_size",
    "font-size-min",
    "font_size_min",
    "font-weight",
    "font_weight",
    "font-features",
    "font_features",
    "font-alternates",
    "font_alternates",
    "letter-spacing",
    "letter_spacing",
    "tracking",
    "shadow",
    "filter",
    "mask",
    "blend-mode",
    "blend_mode",
    "blur",
    "opacity",
    "visible",
    "locked",
    "selectable",
    "rotate",
    "chain",
    "drop-cap-lines",
    "drop_cap_lines",
    "hyphenate",
    "widow-orphan",
    "widow_orphan",
    "tab-leader",
    "tab_leader",
    "text-exclusion",
    "text_exclusion",
    "padding-left",
    "padding_left",
    "text-indent",
    "text_indent",
    "bullet",
    "bullet-gap",
    "bullet_gap",
    "anchor",
    "anchor-zone",
    "anchor_zone",
    "anchor-sibling",
    "anchor_sibling",
    "anchor-edge",
    "anchor_edge",
    "anchor-gap",
    "anchor_gap",
    "anchor-parent",
    "anchor_parent",
    // content format: "markdown" opts into inline-markdown parsing of span text.
    "format",
    // external file source: `src="path"` loads the file's text as the node content.
    "src",
    // span data-binding attrs (carried on `span` children, listed here so the
    // family of recognized text/span props is documented in one place).
    "data-ref",
    "data_ref",
    "precision",
    "locale",
];

pub(in crate::parse::transform) fn transform_text(node: &KdlNode) -> Result<TextNode, ParseError> {
    let id = required_string_prop(node, "id")?.to_owned();

    let font_family = optional_property_value_aliased(node, "font-family", "font_family");
    let font_size = optional_property_value_aliased(node, "font-size", "font_size");
    let font_size_min = optional_property_value_aliased(node, "font-size-min", "font_size_min");
    let font_weight = optional_property_value_aliased(node, "font-weight", "font_weight");
    let font_features =
        optional_string_prop_aliased(node, "font-features", "font_features").map(str::to_owned);
    let font_alternates =
        optional_string_prop_aliased(node, "font-alternates", "font_alternates").map(str::to_owned);
    let letter_spacing = optional_property_value_aliased(node, "letter-spacing", "letter_spacing")
        .or_else(|| optional_property_value(node, "tracking"));
    let drop_cap_lines = optional_u32_prop(node, "drop-cap-lines")
        .or_else(|| optional_u32_prop(node, "drop_cap_lines"));
    let hyphenate = optional_bool_prop(node, "hyphenate");
    let widow_orphan =
        optional_u32_prop(node, "widow-orphan").or_else(|| optional_u32_prop(node, "widow_orphan"));
    let tab_leader = optional_string_prop(node, "tab-leader")
        .or_else(|| optional_string_prop(node, "tab_leader"))
        .map(str::to_owned);
    let text_exclusion = optional_string_prop(node, "text-exclusion")
        .or_else(|| optional_string_prop(node, "text_exclusion"))
        .map(str::to_owned);
    // Optional signed geometry dimensions (text-indent may be NEGATIVE for a
    // hanging indent). Accept both hyphenated and underscored spellings.
    let padding_left = optional_dimension_prop(node, "padding-left")
        .or_else(|| optional_dimension_prop(node, "padding_left"));
    let text_indent = optional_dimension_prop(node, "text-indent")
        .or_else(|| optional_dimension_prop(node, "text_indent"));
    let stroke = optional_property_value(node, "stroke");
    let stroke_width = optional_property_value_aliased(node, "stroke-width", "stroke_width");
    let bullet = optional_string_prop(node, "bullet").map(str::to_owned);
    let bullet_gap = optional_dimension_prop(node, "bullet-gap")
        .or_else(|| optional_dimension_prop(node, "bullet_gap"));
    // Content format: `format="markdown"` opts into inline-markdown span parsing.
    // `format="plain"` or absent = literal (byte-identical to current behavior).
    let content_format = optional_string_prop(node, "format").map(str::to_owned);
    // External file source: `src="path"` causes the CLI render layer to read the
    // file and replace the node's spans with the file's raw text before compile.
    let src = optional_string_prop(node, "src").map(str::to_owned);

    let mut spans: Vec<TextSpan> = Vec::new();
    let mut block_styles: Vec<BlockStyle> = Vec::new();
    let mut kerning_pairs = Vec::new();
    if let Some(children) = node.children() {
        for child in children.nodes() {
            match child.name().value() {
                "block" => block_styles.push(transform_block_style(child)?),
                "kern-pair" => kerning_pairs.push(transform_kerning_pair(child)?),
                "span" => spans.push(transform_span(child)?),
                _ => {}
            }
        }
    }

    let unknown_props = collect_unknown_props(node, TEXT_KNOWN_PROPS);

    Ok(TextNode {
        id,
        name: optional_string_prop(node, "name").map(str::to_owned),
        role: optional_string_prop(node, "role").map(str::to_owned),
        x: optional_property_value(node, "x"),
        y: optional_property_value(node, "y"),
        w: optional_property_value(node, "w"),
        h: optional_property_value(node, "h"),
        align: optional_string_prop(node, "align").map(str::to_owned),
        v_align: optional_string_prop_aliased(node, "v-align", "v_align").map(str::to_owned),
        direction: optional_string_prop(node, "direction").map(str::to_owned),
        overflow: optional_string_prop(node, "overflow").map(str::to_owned),
        overflow_wrap: optional_string_prop(node, "overflow-wrap")
            .or_else(|| optional_string_prop(node, "overflow_wrap"))
            .map(str::to_owned),
        style: optional_string_prop(node, "style").map(str::to_owned),
        fill: optional_property_value(node, "fill"),
        stroke,
        stroke_width,
        contrast_bg: optional_property_value_aliased(node, "contrast-bg", "contrast_bg"),
        font_family,
        font_size,
        font_size_min,
        font_weight,
        font_features,
        font_alternates,
        letter_spacing,
        kerning_pairs,
        shadow: optional_property_value(node, "shadow"),
        filter: optional_property_value(node, "filter"),
        mask: optional_property_value(node, "mask"),
        blend_mode: optional_string_prop_aliased(node, "blend-mode", "blend_mode")
            .map(str::to_owned),
        blur: optional_dimension_prop(node, "blur"),
        opacity: optional_f64_prop(node, "opacity"),
        visible: optional_bool_prop(node, "visible"),
        locked: optional_bool_prop(node, "locked"),
        selectable: optional_bool_prop(node, "selectable"),
        rotate: optional_dimension_prop(node, "rotate"),
        chain: optional_string_prop(node, "chain").map(str::to_owned),
        drop_cap_lines,
        hyphenate,
        widow_orphan,
        tab_leader,
        text_exclusion,
        padding_left,
        text_indent,
        content_format,
        src,
        bullet,
        bullet_gap,
        spans,
        block_styles,
        anchor: optional_string_prop(node, "anchor").map(str::to_owned),
        anchor_zone: optional_string_prop(node, "anchor-zone")
            .or_else(|| optional_string_prop(node, "anchor_zone"))
            .map(str::to_owned),
        anchor_sibling: optional_string_prop(node, "anchor-sibling")
            .or_else(|| optional_string_prop(node, "anchor_sibling"))
            .map(str::to_owned),
        anchor_edge: optional_string_prop(node, "anchor-edge")
            .or_else(|| optional_string_prop(node, "anchor_edge"))
            .map(str::to_owned),
        anchor_gap: optional_dimension_prop(node, "anchor-gap")
            .or_else(|| optional_dimension_prop(node, "anchor_gap")),
        anchor_parent: optional_bool_prop(node, "anchor-parent")
            .or_else(|| optional_bool_prop(node, "anchor_parent")),
        source_span: node_span(node),
        unknown_props,
    })
}

pub(crate) const CODE_KNOWN_PROPS: &[&str] = &[
    "id",
    "name",
    "role",
    "x",
    "y",
    "w",
    "h",
    "overflow",
    "language",
    "line-numbers",
    "line_numbers",
    "tab-width",
    "tab_width",
    "style",
    "fill",
    "font-family",
    "font_family",
    "font-size",
    "font_size",
    "font-weight",
    "font_weight",
    "font-features",
    "font_features",
    "font-alternates",
    "font_alternates",
    "letter-spacing",
    "letter_spacing",
    "tracking",
    "syntax-theme",
    "syntax_theme",
    "opacity",
    "visible",
    "locked",
    "selectable",
    "rotate",
    "anchor",
    "anchor-zone",
    "anchor_zone",
    "anchor-sibling",
    "anchor_sibling",
    "anchor-edge",
    "anchor_edge",
    "anchor-gap",
    "anchor_gap",
    "anchor-parent",
    "anchor_parent",
];

pub(in crate::parse::transform) fn transform_code(node: &KdlNode) -> Result<CodeNode, ParseError> {
    let id = required_string_prop(node, "id")?.to_owned();

    let font_family = optional_property_value_aliased(node, "font-family", "font_family");
    let font_size = optional_property_value_aliased(node, "font-size", "font_size");
    let font_weight = optional_property_value_aliased(node, "font-weight", "font_weight");
    let font_features =
        optional_string_prop_aliased(node, "font-features", "font_features").map(str::to_owned);
    let font_alternates =
        optional_string_prop_aliased(node, "font-alternates", "font_alternates").map(str::to_owned);
    let letter_spacing = optional_property_value_aliased(node, "letter-spacing", "letter_spacing")
        .or_else(|| optional_property_value(node, "tracking"));
    let line_numbers = optional_bool_prop(node, "line-numbers")
        .or_else(|| optional_bool_prop(node, "line_numbers"));
    let tab_width =
        optional_u32_prop(node, "tab-width").or_else(|| optional_u32_prop(node, "tab_width"));
    let syntax_theme = optional_string_prop(node, "syntax-theme")
        .or_else(|| optional_string_prop(node, "syntax_theme"))
        .and_then(SyntaxTheme::from_name);

    let mut kerning_pairs = Vec::new();

    // The verbatim source is carried by a `content` child node whose first
    // positional argument is the DECODED string. KDL v2 multi-line string
    // dedent rules make a bare `r#"..."#` form lossy, so the carrier uses a
    // single-line escaped string which round-trips `\n \t \" \\` exactly.
    // Stored decoded here; `write_code` re-encodes the escapes.
    let mut content = String::new();
    let mut content_seen = false;
    if let Some(children) = node.children() {
        for child in children.nodes() {
            match child.name().value() {
                "content" => {
                    if !content_seen && let Some(KdlValue::String(s)) = child.get(0) {
                        content = s.clone();
                        content_seen = true;
                    }
                }
                "kern-pair" => {
                    kerning_pairs.push(transform_kerning_pair(child)?);
                }
                _ => {}
            }
        }
    }

    let unknown_props = collect_unknown_props(node, CODE_KNOWN_PROPS);

    Ok(CodeNode {
        id,
        name: optional_string_prop(node, "name").map(str::to_owned),
        role: optional_string_prop(node, "role").map(str::to_owned),
        x: optional_property_value(node, "x"),
        y: optional_property_value(node, "y"),
        w: optional_property_value(node, "w"),
        h: optional_property_value(node, "h"),
        overflow: optional_string_prop(node, "overflow").map(str::to_owned),
        language: optional_string_prop(node, "language").map(str::to_owned),
        line_numbers,
        tab_width,
        style: optional_string_prop(node, "style").map(str::to_owned),
        fill: optional_property_value(node, "fill"),
        font_family,
        font_size,
        font_weight,
        font_features,
        font_alternates,
        letter_spacing,
        kerning_pairs,
        syntax_theme,
        opacity: optional_f64_prop(node, "opacity"),
        visible: optional_bool_prop(node, "visible"),
        locked: optional_bool_prop(node, "locked"),
        selectable: optional_bool_prop(node, "selectable"),
        rotate: optional_dimension_prop(node, "rotate"),
        content,
        anchor: optional_string_prop(node, "anchor").map(str::to_owned),
        anchor_zone: optional_string_prop(node, "anchor-zone")
            .or_else(|| optional_string_prop(node, "anchor_zone"))
            .map(str::to_owned),
        anchor_sibling: optional_string_prop(node, "anchor-sibling")
            .or_else(|| optional_string_prop(node, "anchor_sibling"))
            .map(str::to_owned),
        anchor_edge: optional_string_prop(node, "anchor-edge")
            .or_else(|| optional_string_prop(node, "anchor_edge"))
            .map(str::to_owned),
        anchor_gap: optional_dimension_prop(node, "anchor-gap")
            .or_else(|| optional_dimension_prop(node, "anchor_gap")),
        anchor_parent: optional_bool_prop(node, "anchor-parent")
            .or_else(|| optional_bool_prop(node, "anchor_parent")),
        source_span: node_span(node),
        unknown_props,
    })
}
