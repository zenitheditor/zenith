//! Diagnostics an edit introduces: what `zenith validate` reports on the
//! result and did not report on the input. A text edit that overflows its box
//! shows up in the `tx` output, so an agent need not run `validate` to find it.

use std::collections::BTreeMap;
use std::path::Path;

use zenith_core::{Diagnostic, Severity};
use zenith_pipeline::PolicyFlags;

use crate::commands::validate;

/// Identity of a diagnostic across the two validate runs. The message is left
/// out: it carries measured values that an unrelated edit can change.
fn key(d: &Diagnostic) -> (String, Option<String>) {
    (d.code.clone(), d.subject_id.clone())
}

/// The Error and Warning diagnostics of `after` that `before` lacks, in
/// report order. A diagnostic matches one of `before`'s with the same code
/// and subject, once each, so a second overflow on a node that already
/// overflowed still counts. Advisories are left out: they are noise next to
/// an edit.
///
/// Empty when `before` has an error. A document that does not validate skips
/// the compile checks, so its diagnostics are no baseline for `after`.
pub(super) fn introduced(before: &str, after: &str, project_dir: Option<&Path>) -> Vec<Diagnostic> {
    let flags = PolicyFlags::default();
    let before = validate::collect(before, project_dir, &flags).diagnostics;
    if Diagnostic::has_errors(&before) {
        return Vec::new();
    }
    let mut known: BTreeMap<_, usize> = BTreeMap::new();
    for d in &before {
        *known.entry(key(d)).or_default() += 1;
    }
    validate::collect(after, project_dir, &flags)
        .diagnostics
        .into_iter()
        .filter(|d| match d.severity {
            Severity::Error | Severity::Warning => match known.get_mut(&key(d)) {
                Some(left) if *left > 0 => {
                    *left -= 1;
                    false
                }
                _ => true,
            },
            Severity::Advisory => false,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = r##"zenith version=1 {
  project id="p" name="P"
  tokens format="zenith-token-v1" {
    token id="color.k" type="color" value="#000000"
    token id="font.body" type="fontFamily" value="Noto Sans"
    token id="size.big" type="dimension" value=(px)44
  }
  styles {}
  document id="d" title="T" {
    page id="pg" w=(px)400 h=(px)200 {
      text id="t" x=(px)10 y=(px)10 w=(px)120 h=(px)60 fill=(token)"color.k" font-family=(token)"font.body" font-size=(token)"size.big" { span "Hi" }
    }
  }
}
"##;

    #[test]
    fn an_edit_that_overflows_its_box_is_reported() {
        let after = DOC.replace("span \"Hi\"", "span \"A much longer headline that wraps\"");
        let found = introduced(DOC, &after, None);
        assert!(found.iter().any(|d| d.code == "text.overflow"), "{found:?}");
    }

    #[test]
    fn a_diagnostic_already_on_the_input_is_not_reported_again() {
        let long = DOC.replace("span \"Hi\"", "span \"A much longer headline that wraps\"");
        assert!(introduced(&long, &long, None).is_empty());
    }

    #[test]
    fn an_edit_that_only_changes_a_measured_value_reports_nothing() {
        let long = DOC.replace("span \"Hi\"", "span \"A much longer headline that wraps\"");
        let taller = long.replace("value=(px)44", "value=(px)46");
        assert!(introduced(&long, &taller, None).is_empty());
    }

    #[test]
    fn a_document_with_an_error_gives_no_baseline() {
        let broken = DOC.replace("fill=(token)\"color.k\"", "fill=(token)\"color.missing\"");
        let long = DOC.replace("span \"Hi\"", "span \"A much longer headline that wraps\"");
        assert!(introduced(&broken, &long, None).is_empty());
    }

    #[test]
    fn a_clean_edit_reports_nothing() {
        let after = DOC.replace("span \"Hi\"", "span \"Yo\"");
        assert!(introduced(DOC, &after, None).is_empty());
    }
}
