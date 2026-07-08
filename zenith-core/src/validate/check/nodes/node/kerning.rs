//! Validation for authored manual kerning pair children.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::value::{PropertyValue, dim_to_px};
use crate::ast::{KerningPair, Span};
use crate::diagnostics::Diagnostic;
use crate::tokens::{ResolvedToken, ResolvedValue};

pub(super) fn check_kerning_pairs(
    node_kind: &str,
    node_id: &str,
    pairs: &[KerningPair],
    source_span: Option<Span>,
    referenced_token_ids: &mut BTreeSet<String>,
    resolved_tokens: &BTreeMap<String, ResolvedToken>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut seen = BTreeSet::new();
    for pair in pairs {
        if pair.left.is_empty() {
            diagnostics.push(Diagnostic::error(
                "kerning.empty_pair",
                format!("{node_kind} '{node_id}': kern-pair left string must not be empty"),
                source_span,
                Some(node_id.to_owned()),
            ));
        }
        if pair.right.is_empty() {
            diagnostics.push(Diagnostic::error(
                "kerning.empty_pair",
                format!("{node_kind} '{node_id}': kern-pair right string must not be empty"),
                source_span,
                Some(node_id.to_owned()),
            ));
        }

        if !seen.insert((pair.left.as_str(), pair.right.as_str())) {
            diagnostics.push(Diagnostic::warning(
                "kerning.duplicate_pair",
                format!(
                    "{node_kind} '{node_id}': duplicate kern-pair \"{}\" \"{}\"",
                    pair.left, pair.right
                ),
                source_span,
                Some(node_id.to_owned()),
            ));
        }

        check_kerning_by(
            node_kind,
            node_id,
            &pair.by,
            referenced_token_ids,
            resolved_tokens,
            diagnostics,
        );
    }
}

fn check_kerning_by(
    node_kind: &str,
    node_id: &str,
    by: &PropertyValue,
    referenced_token_ids: &mut BTreeSet<String>,
    resolved_tokens: &BTreeMap<String, ResolvedToken>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match by {
        PropertyValue::Dimension(dim) => {
            if dim_to_px(dim.value, &dim.unit).is_none() {
                diagnostics.push(Diagnostic::error(
                    "token.incompatible_property",
                    format!(
                        "{node_kind} '{node_id}': kern-pair by must use a pixel-convertible dimension"
                    ),
                    None,
                    Some(node_id.to_owned()),
                ));
            }
        }
        PropertyValue::TokenRef(token_id) => {
            referenced_token_ids.insert(token_id.clone());
            let Some(resolved) = resolved_tokens.get(token_id.as_str()) else {
                diagnostics.push(Diagnostic::error(
                    "token.unknown_reference",
                    format!(
                        "{node_kind} '{node_id}': kern-pair by references token '{token_id}' which does not exist or failed resolution"
                    ),
                    None,
                    Some(node_id.to_owned()),
                ));
                return;
            };
            match &resolved.value {
                ResolvedValue::Dimension(dim) if dim_to_px(dim.value, &dim.unit).is_some() => {}
                ResolvedValue::Dimension(_) => {
                    diagnostics.push(Diagnostic::error(
                        "token.incompatible_property",
                        format!(
                            "{node_kind} '{node_id}': kern-pair by token '{token_id}' must resolve to a pixel-convertible dimension"
                        ),
                        None,
                        Some(node_id.to_owned()),
                    ));
                }
                ResolvedValue::Color(_)
                | ResolvedValue::CmykColor { .. }
                | ResolvedValue::Number(_)
                | ResolvedValue::FontFamily(_)
                | ResolvedValue::FontWeight(_)
                | ResolvedValue::Gradient(_)
                | ResolvedValue::Shadow(_)
                | ResolvedValue::Filter(_)
                | ResolvedValue::Mask(_) => {
                    diagnostics.push(Diagnostic::error(
                        "token.incompatible_property",
                        format!(
                            "{node_kind} '{node_id}': kern-pair by expects a dimension token but '{token_id}' is not dimension-valued"
                        ),
                        None,
                        Some(node_id.to_owned()),
                    ));
                }
            }
        }
        PropertyValue::Literal(_) => diagnostics.push(Diagnostic::error(
            "token.raw_visual_literal",
            format!(
                "{node_kind} '{node_id}': kern-pair by must be a dimension literal or dimension token"
            ),
            None,
            Some(node_id.to_owned()),
        )),
        PropertyValue::DataRef(_) => diagnostics.push(Diagnostic::error(
            "token.incompatible_property",
            format!("{node_kind} '{node_id}': kern-pair by does not support data references"),
            None,
            Some(node_id.to_owned()),
        )),
    }
}
