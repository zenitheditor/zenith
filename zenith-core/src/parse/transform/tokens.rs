//! Transform of the document-level `tokens { … }` and `styles { … }` blocks.

use std::collections::BTreeMap;

use kdl::{KdlNode, KdlValue};

use crate::ast::{
    style::{Style, StyleBlock, UnknownStyleProp, canonicalize_style_key},
    token::{
        FilterKind, FilterLiteral, FilterOp, GradientKind, GradientLiteral, GradientStopRef,
        MaskLiteral, MaskShape, ShadowLayerRef, ShadowLiteral, Token, TokenBlock, TokenLiteral,
        TokenType, TokenValue,
    },
    value::{Dimension, PropertyValue, Unit},
};
use crate::error::{ParseError, ParseErrorCode};

use super::helpers::{
    entry_annotation, entry_to_property_value, kdl_value_to_literal_string, node_span,
    optional_bool_prop, optional_dimension_prop, optional_f64_prop, optional_i64_prop,
    optional_token_ref_prop, required_string_prop,
};

pub(super) fn transform_tokens(node: &KdlNode) -> Result<TokenBlock, ParseError> {
    let format = required_string_prop(node, "format")?.to_owned();

    let mut token_list: Vec<Token> = Vec::new();
    if let Some(children) = node.children() {
        for child in children.nodes() {
            if child.name().value() == "token" {
                token_list.push(transform_token(child)?);
            }
        }
    }

    Ok(TokenBlock {
        format,
        tokens: token_list,
    })
}

fn transform_token(node: &KdlNode) -> Result<Token, ParseError> {
    let id = required_string_prop(node, "id")?.to_owned();
    let type_str = required_string_prop(node, "type")?;
    let token_type = TokenType::from_type_name(type_str);

    // Gradient tokens carry no scalar `value=`; they are built from an optional
    // `angle=(deg)N` prop plus child `stop` nodes. Prefer this child-node form
    // even if a stray `value=` entry is also present.
    if token_type == TokenType::Gradient {
        let token_value = transform_gradient(node);
        let source_span = node_span(node);
        return Ok(Token {
            id,
            token_type,
            value: token_value,
            source_span,
        });
    }

    // Shadow tokens carry no scalar `value=`; they are built from child `layer`
    // nodes. Prefer this child-node form even if a stray `value=` is present.
    if token_type == TokenType::Shadow {
        let token_value = transform_shadow(node);
        let source_span = node_span(node);
        return Ok(Token {
            id,
            token_type,
            value: token_value,
            source_span,
        });
    }

    // Filter tokens carry no scalar `value=`; they are built from child op
    // nodes. Prefer this child-node form even if a stray `value=` is present.
    if token_type == TokenType::Filter {
        let token_value = transform_filter(node);
        let source_span = node_span(node);
        return Ok(Token {
            id,
            token_type,
            value: token_value,
            source_span,
        });
    }

    // Mask tokens carry no scalar `value=`; they are built from a single shape
    // child node. Prefer this child-node form even if a stray `value=` is present.
    if token_type == TokenType::Mask {
        let token_value = transform_mask(node);
        let source_span = node_span(node);
        return Ok(Token {
            id,
            token_type,
            value: token_value,
            source_span,
        });
    }

    let value_entry = node.entry("value").ok_or_else(|| {
        ParseError::spanless(
            ParseErrorCode::InvalidPropertyValue,
            format!("token `{id}` is missing required property `value`"),
        )
    })?;

    let token_value = match entry_annotation(value_entry) {
        Some("token") => match value_entry.value() {
            KdlValue::String(s) => TokenValue::Reference {
                token_id: s.clone(),
            },
            other => {
                return Err(ParseError::spanless(
                    ParseErrorCode::InvalidPropertyValue,
                    format!("token `{id}` has (token) annotation but non-string value: {other:?}"),
                ));
            }
        },
        Some(unit_str) => {
            // Annotated number → dimension literal.
            let unit = Unit::from_annotation(unit_str);
            let numeric = match value_entry.value() {
                KdlValue::Integer(n) => *n as f64,
                KdlValue::Float(f) => *f,
                other => {
                    return Err(ParseError::spanless(
                        ParseErrorCode::InvalidPropertyValue,
                        format!(
                            "token `{id}` has unit annotation but non-numeric value: {other:?}"
                        ),
                    ));
                }
            };
            TokenValue::Literal(TokenLiteral::Dimension(Dimension {
                value: numeric,
                unit,
            }))
        }
        None => {
            let literal = match value_entry.value() {
                KdlValue::String(s) => TokenLiteral::String(s.clone()),
                KdlValue::Integer(n) => TokenLiteral::Number(*n as f64),
                KdlValue::Float(f) => TokenLiteral::Number(*f),
                other => {
                    return Err(ParseError::spanless(
                        ParseErrorCode::InvalidPropertyValue,
                        format!("token `{id}` has unsupported value type: {other:?}"),
                    ));
                }
            };
            TokenValue::Literal(literal)
        }
    };

    let source_span = node_span(node);
    Ok(Token {
        id,
        token_type,
        value: token_value,
        source_span,
    })
}

/// Default gradient angle (degrees, clockwise from +x) when `angle=` is absent
/// or cannot be read as a dimension: 90 = top→bottom.
const DEFAULT_GRADIENT_ANGLE_DEG: f64 = 90.0;

/// Build a gradient `TokenValue` from a `token` node's props and `stop`
/// children. Infallible: a malformed gradient simply yields fewer/zero stops,
/// which the resolver later reports via `gradient.too_few_stops`.
///
/// KDL forms:
/// - Linear: `token … type="gradient" angle=(deg)90 { stop … }`
/// - Radial:  `token … type="gradient" radial=#true center-x=0.5 center-y=0.5 radius=0.7 { stop … }`
fn transform_gradient(node: &KdlNode) -> TokenValue {
    // `radial=#true` → radial gradient; absent or `#false` → linear.
    let kind = if optional_bool_prop(node, "radial").unwrap_or(false) {
        GradientKind::Radial
    } else {
        GradientKind::Linear
    };

    // `angle=(deg)N` is read like other `(deg)` dimensions: take the dimension
    // `.value` directly as degrees (no dim_to_px conversion). Absent or
    // unparseable → default.
    let angle_deg =
        optional_dimension_prop(node, "angle").map_or(DEFAULT_GRADIENT_ANGLE_DEG, |d| d.value);

    // Radial-specific geometry params as bare f64 fractions.
    let center_x = optional_f64_prop(node, "center-x");
    let center_y = optional_f64_prop(node, "center-y");
    let radius = optional_f64_prop(node, "radius");

    let mut stops: Vec<GradientStopRef> = Vec::new();
    if let Some(children) = node.children() {
        for child in children.nodes() {
            if child.name().value() != "stop" {
                continue;
            }
            // A stop without a usable color token ref is meaningless; skip it.
            let Some(color_token) = optional_token_ref_prop(child, "color") else {
                continue;
            };
            let offset = optional_f64_prop(child, "offset").unwrap_or(0.0);
            stops.push(GradientStopRef {
                offset,
                color_token,
            });
        }
    }

    TokenValue::Literal(TokenLiteral::Gradient(GradientLiteral {
        kind,
        angle_deg,
        center_x,
        center_y,
        radius,
        stops,
    }))
}

/// Build a shadow `TokenValue` from a `token` node's `layer` children. Each
/// layer reads `dx`/`dy`/`blur` as `(px)` dimensions (pixel value taken
/// directly; absent → 0) plus a `color=(token)"id"` color token id. Infallible:
/// a malformed shadow simply yields fewer/zero layers, which the resolver later
/// reports via `shadow.no_layers`.
fn transform_shadow(node: &KdlNode) -> TokenValue {
    let mut layers: Vec<ShadowLayerRef> = Vec::new();
    if let Some(children) = node.children() {
        for child in children.nodes() {
            if child.name().value() != "layer" {
                continue;
            }
            // A layer without a usable color token ref is meaningless; skip it.
            let Some(color_token) = optional_token_ref_prop(child, "color") else {
                continue;
            };
            let dx = optional_dimension_prop(child, "dx")
                .map(|d| d.value)
                .unwrap_or(0.0);
            let dy = optional_dimension_prop(child, "dy")
                .map(|d| d.value)
                .unwrap_or(0.0);
            let blur = optional_dimension_prop(child, "blur")
                .map(|d| d.value)
                .unwrap_or(0.0);
            layers.push(ShadowLayerRef {
                dx,
                dy,
                blur,
                color_token,
            });
        }
    }

    TokenValue::Literal(TokenLiteral::Shadow(ShadowLiteral { layers }))
}

/// Build a filter `TokenValue` from a `token` node's op children. Each child
/// node name is mapped via [`FilterKind::from_op_name`]; unrecognized names are
/// skipped. An optional unitless `amount` prop is read per op. Infallible: a
/// malformed filter simply yields fewer/zero ops, which the resolver later
/// reports via `filter.no_ops`.
fn transform_filter(node: &KdlNode) -> TokenValue {
    let mut ops: Vec<FilterOp> = Vec::new();
    if let Some(children) = node.children() {
        for child in children.nodes() {
            // An unrecognized op name is meaningless; skip it.
            let Some(kind) = FilterKind::from_op_name(child.name().value()) else {
                continue;
            };
            let amount = optional_f64_prop(child, "amount");
            // `shadow`/`highlight` are color token refs, only meaningful for a
            // `duotone` op. Non-duotone ops simply won't carry them → `None`.
            let shadow = optional_token_ref_prop(child, "shadow");
            let highlight = optional_token_ref_prop(child, "highlight");
            // `seed`/`scale` are only meaningful for a `noise` op; other ops
            // simply won't carry them → `None`.
            let seed = optional_i64_prop(child, "seed");
            let scale = optional_f64_prop(child, "scale");
            ops.push(FilterOp {
                kind,
                amount,
                shadow,
                highlight,
                seed,
                scale,
            });
        }
    }

    TokenValue::Literal(TokenLiteral::Filter(FilterLiteral { ops }))
}

/// Build a mask `TokenValue` from a `token` node's single shape child. The first
/// child whose name maps via [`MaskShape::from_shape_name`] picks the shape
/// (rect/rounded/ellipse); `radius` (optional number), `feather` (optional
/// number, default 0.0), and `invert` (optional bool, default false) are read
/// off that child. Infallible: a `mask {}` with no recognized shape child
/// defaults to a full-box `Rect` cover.
fn transform_mask(node: &KdlNode) -> TokenValue {
    let mut shape = MaskShape::Rect;
    let mut radius: Option<f64> = None;
    let mut feather = 0.0;
    let mut invert = false;

    if let Some(children) = node.children() {
        for child in children.nodes() {
            // The first child whose name maps to a shape wins; others ignored.
            let Some(kind) = MaskShape::from_shape_name(child.name().value()) else {
                continue;
            };
            shape = kind;
            radius = optional_f64_prop(child, "radius");
            feather = optional_f64_prop(child, "feather").unwrap_or(0.0);
            invert = optional_bool_prop(child, "invert").unwrap_or(false);
            break;
        }
    }

    TokenValue::Literal(TokenLiteral::Mask(MaskLiteral {
        shape,
        radius,
        feather,
        invert,
    }))
}

// ---------------------------------------------------------------------------
// Styles
// ---------------------------------------------------------------------------

pub(super) fn transform_styles(node: &KdlNode) -> Result<StyleBlock, ParseError> {
    let source_span = node_span(node);
    let mut style_list: Vec<Style> = Vec::new();

    if let Some(children) = node.children() {
        for child in children.nodes() {
            if child.name().value() == "style" {
                let id = required_string_prop(child, "id")?.to_owned();
                let style_source_span = node_span(child);

                let mut properties: BTreeMap<String, PropertyValue> = BTreeMap::new();
                let mut unknown_props: BTreeMap<String, UnknownStyleProp> = BTreeMap::new();

                // Each child node of the `style` node is a property declaration.
                // Its NAME is the property key; its FIRST positional argument
                // is the value (e.g. `fill (token)"color.text.primary"`).
                if let Some(prop_nodes) = child.children() {
                    for prop_node in prop_nodes.nodes() {
                        let prop_name = prop_node.name().value();
                        if let Some(canonical) = canonicalize_style_key(prop_name) {
                            // Read the first positional (unnamed) entry as a PropertyValue.
                            let first_positional =
                                prop_node.entries().iter().find(|e| e.name().is_none());
                            if let Some(entry) = first_positional
                                && let Ok(pv) = entry_to_property_value(entry)
                            {
                                properties.insert(canonical.to_owned(), pv);
                            }
                        } else {
                            // Unrecognized property: preserve for validator warnings.
                            let raw = prop_node
                                .entries()
                                .iter()
                                .find(|e| e.name().is_none())
                                .map(|e| kdl_value_to_literal_string(e.value()))
                                .unwrap_or_default();
                            unknown_props.insert(prop_name.to_owned(), UnknownStyleProp { raw });
                        }
                    }
                }

                style_list.push(Style {
                    id,
                    properties,
                    unknown_props,
                    source_span: style_source_span,
                });
            }
        }
    }

    Ok(StyleBlock {
        styles: style_list,
        source_span,
    })
}
