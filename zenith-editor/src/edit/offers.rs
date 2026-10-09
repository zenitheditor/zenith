//! The follow-up commands a rejection offers.

use serde_json::{Value, json};
use zenith_core::Diagnostic;

use crate::error::Offer;

/// The same command with `flag: true` added to its params.
pub(crate) fn with_flag(command: &str, raw: &Value, id: &str, flag: &str, label: &str) -> Offer {
    let mut params = match raw {
        Value::Object(map) => map.clone(),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => {
            serde_json::Map::new()
        }
    };
    params.insert(flag.to_owned(), Value::Bool(true));
    Offer {
        id: id.to_owned(),
        label: label.to_owned(),
        command: command.to_owned(),
        params: Value::Object(params),
    }
}

/// `tx.apply` that unlocks `node`.
pub(crate) fn unlock(node: &str) -> Offer {
    Offer {
        id: "unlock".to_owned(),
        label: format!("Unlock {node}"),
        command: "tx.apply".to_owned(),
        params: json!({ "ops": [{ "op": "set_locked", "node": node, "locked": false }] }),
    }
}

/// `tx.apply` that shows `node`.
pub(crate) fn show(node: &str) -> Offer {
    Offer {
        id: "show".to_owned(),
        label: format!("Show {node}"),
        command: "tx.apply".to_owned(),
        params: json!({ "ops": [{ "op": "set_visible", "node": node, "visible": true }] }),
    }
}

/// The offers for the rejection `diagnostics` of `command` sent with
/// `raw` params. Gesture flags (`detach`, `detach_anchor`, `confirm_size`,
/// `replace`, `reorder`, `absolute`) are offered only when `gesture`
/// (`reorder` only to `gesture.*` commands);
/// every command gets `unlock` for a `node.locked` subject.
pub(crate) fn for_rejection(
    command: &str,
    raw: &Value,
    diagnostics: &[Diagnostic],
    gesture: bool,
) -> Vec<Offer> {
    let mut out: Vec<Offer> = Vec::new();
    let mut push = |offer: Offer| {
        if !out
            .iter()
            .any(|o| o.id == offer.id && o.params == offer.params)
        {
            out.push(offer);
        }
    };
    for d in diagnostics.iter().filter(|d| d.is_error()) {
        match d.code.as_str() {
            "node.locked" => {
                if let Some(node) = d.subject_id.as_deref() {
                    push(unlock(node));
                }
            }
            "tx.token_bound" if gesture => {
                push(with_flag(
                    command,
                    raw,
                    "detach",
                    "detach",
                    "Detach from token",
                ));
            }
            "tx.anchored" if gesture => push(with_flag(
                command,
                raw,
                "detach_anchor",
                "detach_anchor",
                "Detach from anchor",
            )),
            "tx.computed_size" if gesture => push(with_flag(
                command,
                raw,
                "set_size",
                "confirm_size",
                "Set a fixed size",
            )),
            "tx.value_unresolved" if gesture => push(with_flag(
                command,
                raw,
                "replace_value",
                "replace",
                "Replace with px",
            )),
            "tx.layout_managed" if gesture => {
                // A flow-slot move needs a pointer delta: gestures only.
                if command.starts_with("gesture.") {
                    push(with_flag(
                        command,
                        raw,
                        "reorder",
                        "reorder",
                        "Reorder in layout",
                    ));
                }
                push(with_flag(
                    command,
                    raw,
                    "absolute",
                    "absolute",
                    "Take out of layout",
                ));
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gesture_rejections_offer_flags_once() {
        let raw = json!({"node": "r", "dx": 3});
        let d = |code: &str| Diagnostic::error(code, "x", None, Some("r".to_owned()));
        let offers = for_rejection(
            "gesture.commit",
            &raw,
            &[d("tx.token_bound"), d("tx.token_bound"), d("node.locked")],
            true,
        );
        let ids: Vec<&str> = offers.iter().map(|o| o.id.as_str()).collect();
        assert_eq!(ids, vec!["detach", "unlock"]);
        let detach = offers.first().expect("detach");
        assert_eq!(detach.params, json!({"node": "r", "dx": 3, "detach": true}));
        assert!(for_rejection("tx.apply", &raw, &[d("tx.token_bound")], false).is_empty());
    }
}
