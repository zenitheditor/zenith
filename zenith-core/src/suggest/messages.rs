//! Diagnostic message builders for "did you mean?" hints.

use super::distance::find_suggestion;

/// Message for a known property whose enum value is not allowed.
///
/// `subject` names the owner (for example `polygon 'p1'`). The message names
/// the property, the bad value, a did-you-mean when `value` is within edit
/// distance ≤ 2 of an allowed value, and every allowed value.
pub(crate) fn invalid_value_message(
    subject: &str,
    prop: &str,
    value: &str,
    allowed: &[&str],
) -> String {
    let list = allowed.join(", ");
    match find_suggestion(value, allowed.iter().copied(), 2) {
        Some(s) => format!(
            "{subject}: invalid {prop} '{value}' — did you mean '{s}'? Allowed values: {list}"
        ),
        None => format!("{subject}: invalid {prop} '{value}' — allowed values: {list}"),
    }
}

/// Message for an unknown property name on `kind`.
///
/// `subject` names the owner (for example `asset 'a1'`). The message suggests
/// the closest entry of `known` within edit distance ≤ 2, otherwise it points
/// at `zenith schema`.
pub(crate) fn unknown_property_message(
    subject: &str,
    kind: &str,
    name: &str,
    known: &[&str],
) -> String {
    match find_suggestion(name, known.iter().copied(), 2) {
        Some(s) => format!("{subject}: unknown property '{name}' — did you mean '{s}'?"),
        None => format!(
            "{subject}: unknown property '{name}' \
             — remove it or run `zenith schema` to list the {kind} properties"
        ),
    }
}

/// Message for an unknown child node inside a structural block.
///
/// `subject` names the parent (for example `brand` or `variant 'v1'`). The
/// message names the child, a did-you-mean when `child` is within edit distance
/// ≤ 2 of an allowed name, and every allowed child. A block that takes no
/// children says so and tells the author to remove the child.
pub(crate) fn unknown_child_message(subject: &str, child: &str, allowed: &[&str]) -> String {
    if allowed.is_empty() {
        return format!(
            "{subject}: unknown child '{child}' — {subject} takes no child nodes; remove it"
        );
    }
    let list = allowed.join(", ");
    match find_suggestion(child, allowed.iter().copied(), 2) {
        Some(s) => format!(
            "{subject}: unknown child '{child}' — did you mean '{s}'? Allowed children: {list}"
        ),
        None => format!("{subject}: unknown child '{child}' — allowed children: {list}"),
    }
}

/// Message for a child node a renderable node kind does not consume.
///
/// `subject` names the parent (for example `ellipse 'e1'`). When the kind has an
/// accepted-child list the message adds a did-you-mean and that list. Otherwise
/// it tells the author to place the child as a sibling inside a group.
pub(crate) fn unsupported_child_message(
    subject: &str,
    child: &str,
    parent: &str,
    allowed: &[&str],
) -> String {
    let head = format!("{subject}: child node '{child}' is not supported by '{parent}'");
    if allowed.is_empty() {
        return format!("{head} and is discarded; place it as a sibling inside a group");
    }
    let list = allowed.join(", ");
    match find_suggestion(child, allowed.iter().copied(), 2) {
        Some(s) => format!("{head} — did you mean '{s}'? Allowed children: {list}"),
        None => format!("{head} — allowed children: {list}"),
    }
}

/// Message for `token.raw_visual_literal`: `prop` on `node_id` holds a raw
/// value. `hint` names the next action.
pub(crate) fn raw_literal_message(node_id: &str, prop: &str, hint: &str) -> String {
    format!(
        "node '{node_id}': visual property '{prop}' has a raw literal value; \
         visual properties must reference design tokens — {hint}"
    )
}

/// Message for `token.unknown_reference` on a node property.
pub(crate) fn unknown_reference_message(
    node_id: &str,
    prop: &str,
    token_id: &str,
    hint: &str,
) -> String {
    format!(
        "node '{node_id}': property '{prop}' references token '{token_id}' which \
         does not exist — {hint}"
    )
}

/// Maximum number of names listed by [`format_candidate_list`].
const MAX_LISTED: usize = 8;

/// Format a bounded candidate list for a diagnostic.
///
/// `label` names the kind of candidate (for example `"color tokens"`).
/// Returns `declared <label>: a, b, … (+N more)` with the first 8 names in
/// sorted order, or `no <label> declared` when `names` is empty.
pub(crate) fn format_candidate_list<'a>(
    label: &str,
    names: impl IntoIterator<Item = &'a str>,
) -> String {
    let mut sorted: Vec<&str> = names.into_iter().collect();
    sorted.sort_unstable();
    sorted.dedup();
    if sorted.is_empty() {
        return format!("no {label} declared");
    }
    let total = sorted.len();
    let shown = sorted
        .iter()
        .take(MAX_LISTED)
        .copied()
        .collect::<Vec<_>>()
        .join(", ");
    if total > MAX_LISTED {
        format!("declared {label}: {shown} (+{} more)", total - MAX_LISTED)
    } else {
        format!("declared {label}: {shown}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_child_message_suggests_close_name() {
        let msg = unknown_child_message("brand", "color", &["colors", "fonts", "weights"]);
        assert!(msg.contains("did you mean 'colors'?"), "{msg}");
        assert!(
            msg.contains("Allowed children: colors, fonts, weights"),
            "{msg}"
        );
    }

    #[test]
    fn unknown_child_message_lists_allowed_without_suggestion() {
        let msg = unknown_child_message("brand", "zzzzzz", &["colors", "fonts"]);
        assert!(!msg.contains("did you mean"), "{msg}");
        assert!(msg.contains("allowed children: colors, fonts"), "{msg}");
    }

    #[test]
    fn unknown_child_message_for_childless_block() {
        let msg = unknown_child_message("section 's1'", "x", &[]);
        assert!(msg.contains("takes no child nodes; remove it"), "{msg}");
    }

    #[test]
    fn candidate_list_empty() {
        assert_eq!(
            format_candidate_list("color tokens", []),
            "no color tokens declared"
        );
    }

    #[test]
    fn candidate_list_truncates_with_more_count() {
        let names: Vec<String> = (0..11).map(|i| format!("t{i:02}")).collect();
        let out = format_candidate_list("color tokens", names.iter().map(String::as_str));
        assert_eq!(
            out,
            "declared color tokens: t00, t01, t02, t03, t04, t05, t06, t07 (+3 more)"
        );
    }
}
