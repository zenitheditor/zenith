//! Transform for the shared `span` child of text-bearing nodes.

use kdl::{KdlNode, KdlValue};

use crate::ast::node::TextSpan;
use crate::data::DataFormat;
use crate::error::{ParseError, ParseErrorCode};

use crate::parse::transform::helpers::{
    entry_to_property_value, optional_bool_prop, optional_property_value,
    optional_property_value_aliased, optional_string_prop, optional_string_prop_aliased,
    optional_u32_prop,
};

pub(in crate::parse::transform) fn transform_span(node: &KdlNode) -> Result<TextSpan, ParseError> {
    // First positional argument is the text content.
    let text = node
        .get(0)
        .and_then(|v| {
            if let KdlValue::String(s) = v {
                Some(s.clone())
            } else {
                None
            }
        })
        .ok_or_else(|| {
            ParseError::spanless(
                ParseErrorCode::InvalidPropertyValue,
                "`span` node must have a string argument as its first value",
            )
        })?;

    let fill = node
        .entry("fill")
        .and_then(|e| entry_to_property_value(e).ok());
    let font_weight = optional_property_value_aliased(node, "font-weight", "font_weight");
    let font_features =
        optional_string_prop_aliased(node, "font-features", "font_features").map(str::to_owned);
    let font_alternates =
        optional_string_prop_aliased(node, "font-alternates", "font_alternates").map(str::to_owned);
    let letter_spacing = optional_property_value_aliased(node, "letter-spacing", "letter_spacing")
        .or_else(|| optional_property_value(node, "tracking"));
    let italic = optional_bool_prop(node, "italic");
    let underline = optional_bool_prop(node, "underline");
    let strikethrough = optional_bool_prop(node, "strikethrough");
    let vertical_align =
        optional_string_prop_aliased(node, "vertical-align", "vertical_align").map(str::to_owned);
    let footnote_ref =
        optional_string_prop_aliased(node, "footnote-ref", "footnote_ref").map(str::to_owned);

    // Data-binding: a `data-ref="path"` ties the span's TEXT CONTENT to a runtime
    // field; an optional `format` (+ `precision` / `locale`) styles the resolved
    // value. Both spellings (`data-ref`/`data_ref`) are accepted.
    let data_ref = optional_string_prop_aliased(node, "data-ref", "data_ref").map(str::to_owned);
    let data_format = parse_span_data_format(node);

    // Per-span highlight background color (token ref or raw color string),
    // mirroring the `fill` read path. Absent → `None` (no highlight).
    let highlight = node
        .entry("highlight")
        .and_then(|e| entry_to_property_value(e).ok());

    // Inline code mark: `code=#true` shapes this span in monospace + emits a
    // subtle background rect. Absent → `None` (byte-identical).
    let code = optional_bool_prop(node, "code");

    // Hyperlink URL: `link="https://…"` renders the span underlined in the
    // internal link color (unless `fill` is set) and becomes a clickable PDF
    // `/Link` annotation on selectable text. Absent → `None` (byte-identical).
    let link = optional_string_prop(node, "link").map(str::to_owned);

    Ok(TextSpan {
        text,
        fill,
        font_weight,
        font_features,
        font_alternates,
        letter_spacing,
        italic,
        underline,
        strikethrough,
        vertical_align,
        footnote_ref,
        data_ref,
        data_format,
        highlight,
        code,
        link,
    })
}

/// Build a [`DataFormat`] for a `span` from its `format` / `precision` / `locale`
/// attributes. Returns `None` when no `format` attribute is present (the span
/// substitutes the raw data value verbatim). `precision` is an optional integer
/// clamped into `u8`; `locale` is an optional string (only carried by currency).
/// An unrecognized `format` value yields `None`.
fn parse_span_data_format(node: &KdlNode) -> Option<DataFormat> {
    let format = optional_string_prop(node, "format")?;
    let precision: Option<u8> =
        optional_u32_prop(node, "precision").and_then(|n| u8::try_from(n).ok());
    let locale = optional_string_prop(node, "locale").map(str::to_owned);
    match format {
        "currency" => Some(DataFormat::Currency { locale, precision }),
        "percent" => Some(DataFormat::Percent { precision }),
        "number" => Some(DataFormat::Number { precision }),
        _ => None,
    }
}
