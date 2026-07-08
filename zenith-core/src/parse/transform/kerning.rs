//! Transform helpers for authored manual kerning pair child nodes.

use kdl::{KdlNode, KdlValue};

use crate::ast::KerningPair;
use crate::error::{ParseError, ParseErrorCode};

use super::helpers::entry_to_property_value;

pub(super) fn transform_kerning_pair(node: &KdlNode) -> Result<KerningPair, ParseError> {
    let left = required_string_arg(node, 0, "left")?.to_owned();
    let right = required_string_arg(node, 1, "right")?.to_owned();
    let by = node.entry("by").ok_or_else(|| {
        ParseError::spanless(
            ParseErrorCode::InvalidPropertyValue,
            "`kern-pair` requires a `by` property".to_owned(),
        )
    })?;

    Ok(KerningPair {
        left,
        right,
        by: entry_to_property_value(by)?,
    })
}

fn required_string_arg<'a>(
    node: &'a KdlNode,
    index: usize,
    label: &str,
) -> Result<&'a str, ParseError> {
    match node.get(index) {
        Some(KdlValue::String(value)) => Ok(value.as_str()),
        Some(other) => Err(ParseError::spanless(
            ParseErrorCode::InvalidPropertyValue,
            format!("`kern-pair` {label} argument must be a string, got: {other:?}"),
        )),
        None => Err(ParseError::spanless(
            ParseErrorCode::InvalidPropertyValue,
            format!("`kern-pair` requires a {label} string argument"),
        )),
    }
}
