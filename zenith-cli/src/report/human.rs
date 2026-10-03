//! Human diagnostic lines with shared-cause grouping.

use std::collections::{BTreeMap, BTreeSet};

use zenith_core::{Diagnostic, Severity};

use super::Locator;
use crate::commands::format_located_diagnostic_line;
use crate::json_types::severity_str;

/// Smallest group of same-cause diagnostics printed as one line.
const MIN_GROUP: usize = 3;

/// Subject ids listed in a grouped line before the `(+N more)` tail.
const LISTED_SUBJECTS: usize = 8;

/// Grouping key: code, severity, and shared cause.
type GroupKey<'a> = (&'a str, Severity, &'a str);

/// Format `diagnostics` as human lines, one per diagnostic, in input order.
///
/// Non-error diagnostics that share `(code, severity, cause)` with at least
/// two others collapse into one line at the first member's position:
/// `advisory[font.unresolved]: <cause> — 31 nodes: a, b, … (+N more)`.
/// Errors are never grouped. A diagnostic without a cause is never grouped.
///
/// A diagnostic line shows `line:col` after its code and subject when
/// `locator` resolves its span. A grouped line has no location.
pub(crate) fn human_diagnostic_lines(
    diagnostics: &[Diagnostic],
    locator: &mut Locator<'_>,
) -> Vec<String> {
    let mut groups: BTreeMap<GroupKey<'_>, Vec<&Diagnostic>> = BTreeMap::new();
    for d in diagnostics {
        if let Some(key) = group_key(d) {
            groups.entry(key).or_default().push(d);
        }
    }
    let mut printed: BTreeSet<GroupKey<'_>> = BTreeSet::new();
    let mut lines = Vec::with_capacity(diagnostics.len());
    for d in diagnostics {
        let members = group_key(d).and_then(|key| {
            groups
                .get(&key)
                .filter(|m| m.len() >= MIN_GROUP)
                .map(|m| (key, m))
        });
        match members {
            Some((key, members)) => {
                if printed.insert(key) {
                    lines.push(group_line(key, members));
                }
            }
            None => {
                let at = locator.locate(d).display();
                lines.push(format_located_diagnostic_line(d, at.as_deref()));
            }
        }
    }
    lines
}

fn group_key(d: &Diagnostic) -> Option<GroupKey<'_>> {
    match d.severity {
        Severity::Error => None,
        Severity::Warning | Severity::Advisory => {
            d.cause().map(|cause| (d.code.as_str(), d.severity, cause))
        }
    }
}

fn group_line(key: GroupKey<'_>, members: &[&Diagnostic]) -> String {
    let (code, severity, cause) = key;
    let subjects: Vec<&str> = members
        .iter()
        .filter_map(|d| d.subject_id.as_deref())
        .collect();
    let listed: Vec<&str> = subjects.iter().take(LISTED_SUBJECTS).copied().collect();
    let rest = members.len().saturating_sub(listed.len());
    let mut ids = listed.join(", ");
    if rest > 0 {
        ids.push_str(&format!(", … (+{rest} more)"));
    }
    format!(
        "{}[{}]: {} — {} nodes: {}",
        severity_str(&severity),
        code,
        cause,
        members.len(),
        ids
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unresolved(id: &str) -> Diagnostic {
        Diagnostic::advisory(
            "font.unresolved",
            format!("text node '{id}': font family 'Foo' not available"),
            None,
            Some(id.to_owned()),
        )
        .with_cause("font family 'Foo' not available, falling back to 'Noto Sans'")
    }

    #[test]
    fn three_or_more_same_cause_collapse_into_one_line() {
        let mut diags: Vec<Diagnostic> = (0..10).map(|i| unresolved(&format!("n{i}"))).collect();
        diags.insert(
            1,
            Diagnostic::warning("text.overflow", "overflow", None, Some("t".into())),
        );
        let lines = human_diagnostic_lines(&diags, &mut Locator::new(""));
        assert_eq!(lines.len(), 2, "lines: {lines:?}");
        assert_eq!(
            lines[0],
            "advisory[font.unresolved]: font family 'Foo' not available, falling back to \
             'Noto Sans' — 10 nodes: n0, n1, n2, n3, n4, n5, n6, n7, … (+2 more)"
        );
        assert!(lines[1].starts_with("warning[text.overflow] (t)"));
    }

    #[test]
    fn two_same_cause_stay_separate() {
        let lines =
            human_diagnostic_lines(&[unresolved("a"), unresolved("b")], &mut Locator::new(""));
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("(a)"));
    }

    #[test]
    fn errors_are_never_grouped() {
        let diags: Vec<Diagnostic> = (0..4)
            .map(|i| {
                Diagnostic::error("x.bad", format!("bad {i}"), None, Some(format!("e{i}")))
                    .with_cause("shared")
            })
            .collect();
        assert_eq!(
            human_diagnostic_lines(&diags, &mut Locator::new("")).len(),
            4
        );
    }

    #[test]
    fn short_group_lists_every_subject_without_tail() {
        let diags = [unresolved("a"), unresolved("b"), unresolved("c")];
        let lines = human_diagnostic_lines(&diags, &mut Locator::new(""));
        assert_eq!(lines.len(), 1);
        assert!(lines[0].ends_with("3 nodes: a, b, c"), "{}", lines[0]);
    }

    #[test]
    fn spanned_diagnostic_shows_location_after_subject() {
        let d = Diagnostic::warning(
            "text.overflow",
            "overflow",
            Some(zenith_core::Span { start: 3, end: 4 }),
            Some("t".into()),
        );
        let lines = human_diagnostic_lines(&[d], &mut Locator::new("ab\ncd"));
        assert_eq!(lines, ["warning[text.overflow] (t) 2:1: overflow"]);
    }

    #[test]
    fn spanless_diagnostic_prints_without_location() {
        let d = Diagnostic::warning("text.overflow", "overflow", None, Some("t".into()));
        let lines = human_diagnostic_lines(&[d], &mut Locator::new("ab\ncd"));
        assert_eq!(lines, ["warning[text.overflow] (t): overflow"]);
    }
}
