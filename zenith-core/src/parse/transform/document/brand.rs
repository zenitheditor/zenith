//! The document-level `brand { … }` contract block.

use kdl::{KdlNode, KdlValue};

use crate::ast::brand::BrandContract;
use crate::error::{ParseError, ParseErrorCode};
use crate::parse::transform::helpers::node_span;

/// Transform the document-level `brand { … }` block into a [`BrandContract`].
///
/// Each child node is one category constraint:
/// - `colors "#hex1" "#hex2" …` — approved color hex strings.
/// - `fonts  "Family One" "Family Two" …` — approved font-family names.
/// - `weights 400 700 …` — approved font weight integers (100–900).
///
/// Absent child = unconstrained for that category. Unknown children are
/// silently ignored (forward-compat). Declaration order within each child
/// node's arguments is preserved.
///
/// Errors:
/// - A `colors` or `fonts` argument that is not a KDL string → [`ParseError`].
/// - A `weights` argument that is not an integer → [`ParseError`].
/// - A `weights` argument that is out of the 100–900 range → [`ParseError`].
pub(crate) fn transform_brand_contract(node: &KdlNode) -> Result<BrandContract, ParseError> {
    let source_span = node_span(node);
    let mut allowed_colors: Option<Vec<String>> = None;
    let mut allowed_fonts: Option<Vec<String>> = None;
    let mut allowed_weights: Option<Vec<u32>> = None;

    if let Some(children) = node.children() {
        for child in children.nodes() {
            match child.name().value() {
                "colors" => {
                    let mut colors: Vec<String> = Vec::new();
                    // Only positional arguments (name is None) are color values.
                    let positional: Vec<_> = child
                        .entries()
                        .iter()
                        .filter(|e| e.name().is_none())
                        .collect();
                    for (idx, entry) in positional.iter().enumerate() {
                        match entry.value() {
                            KdlValue::String(s) => {
                                // Store hex strings in lowercase so comparisons
                                // are case-insensitive without repeated conversion.
                                colors.push(s.to_lowercase());
                            }
                            _ => {
                                return Err(ParseError::spanless(
                                    ParseErrorCode::InvalidPropertyValue,
                                    format!(
                                        "brand `colors` argument {idx} must be a quoted string \
                                         (hex color), e.g. `colors \"#0b1f33\" \"#ffffff\"`"
                                    ),
                                ));
                            }
                        }
                    }
                    allowed_colors = Some(colors);
                }
                "fonts" => {
                    let mut fonts: Vec<String> = Vec::new();
                    let positional: Vec<_> = child
                        .entries()
                        .iter()
                        .filter(|e| e.name().is_none())
                        .collect();
                    for (idx, entry) in positional.iter().enumerate() {
                        match entry.value() {
                            KdlValue::String(s) => {
                                fonts.push(s.clone());
                            }
                            _ => {
                                return Err(ParseError::spanless(
                                    ParseErrorCode::InvalidPropertyValue,
                                    format!(
                                        "brand `fonts` argument {idx} must be a quoted string \
                                         (font-family name), e.g. `fonts \"Noto Sans\"`"
                                    ),
                                ));
                            }
                        }
                    }
                    allowed_fonts = Some(fonts);
                }
                "weights" => {
                    let mut weights: Vec<u32> = Vec::new();
                    let positional: Vec<_> = child
                        .entries()
                        .iter()
                        .filter(|e| e.name().is_none())
                        .collect();
                    for (idx, entry) in positional.iter().enumerate() {
                        match entry.value() {
                            KdlValue::Integer(n) => {
                                // KDL integers are i128; we need a u32 in 100..=900.
                                let n_val = *n;
                                if !(100..=900).contains(&n_val) {
                                    return Err(ParseError::spanless(
                                        ParseErrorCode::InvalidPropertyValue,
                                        format!(
                                            "brand `weights` argument {idx} must be an integer \
                                             in the range 100-900 (got {n_val})"
                                        ),
                                    ));
                                }
                                // Infallible: 100..=900 fits in u32 and is positive.
                                let w = u32::try_from(n_val).map_err(|_| {
                                    ParseError::spanless(
                                        ParseErrorCode::InvalidPropertyValue,
                                        format!(
                                            "brand `weights` argument {idx} is out of range \
                                             for a u32 weight (got {n_val})"
                                        ),
                                    )
                                })?;
                                weights.push(w);
                            }
                            _ => {
                                return Err(ParseError::spanless(
                                    ParseErrorCode::InvalidPropertyValue,
                                    format!(
                                        "brand `weights` argument {idx} must be an integer \
                                         (font weight), e.g. `weights 400 700`"
                                    ),
                                ));
                            }
                        }
                    }
                    allowed_weights = Some(weights);
                }
                // Unknown children are silently ignored (forward-compat).
                _ => {}
            }
        }
    }

    Ok(BrandContract {
        allowed_colors,
        allowed_fonts,
        allowed_weights,
        source_span,
    })
}
