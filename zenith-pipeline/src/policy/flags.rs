//! Caller policy overrides ([`PolicyFlags`]) and the four-tier policy merge.

use zenith_core::{DiagnosticPolicy, PolicyEntry, PolicyVerb};

/// Caller-supplied policy overrides, one bucket per verb. Each `String` is a
/// diagnostic code. Buckets apply allow → warn → deny at the highest
/// precedence, so a later verb for the same code wins (last-wins).
///
/// The CLI fills these from `--allow` / `--warn` / `--deny`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PolicyFlags {
    /// Codes to suppress.
    pub allow: Vec<String>,
    /// Codes to force to Warning.
    pub warn: Vec<String>,
    /// Codes to elevate to Error.
    pub deny: Vec<String>,
}

impl PolicyFlags {
    /// Whether any flag was supplied. An all-empty set contributes no entries,
    /// keeping the merged policy byte-identical to the no-flags case.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.allow.is_empty() && self.warn.is_empty() && self.deny.is_empty()
    }

    /// The flag buckets as [`PolicyEntry`] records, allow → warn → deny, with
    /// no source span.
    fn entries(&self) -> Vec<PolicyEntry> {
        let entry = |verb: PolicyVerb, code: &String| PolicyEntry {
            verb,
            code: code.clone(),
            subjects: Vec::new(),
            source_span: None,
        };
        let mut entries = Vec::with_capacity(self.allow.len() + self.warn.len() + self.deny.len());
        entries.extend(self.allow.iter().map(|c| entry(PolicyVerb::Allow, c)));
        entries.extend(self.warn.iter().map(|c| entry(PolicyVerb::Warn, c)));
        entries.extend(self.deny.iter().map(|c| entry(PolicyVerb::Deny, c)));
        entries
    }
}

/// Merge the four policy tiers into one [`DiagnosticPolicy`].
///
/// The tiers concatenate low → high (`global ++ local ++ in_file ++ flags`),
/// so last-wins resolution yields `flags > in-file > local > global`. The
/// result applies once at the validation choke point. When every tier is
/// empty the merged policy is empty (an identity pass).
#[must_use]
pub fn merge_policy(
    global: &DiagnosticPolicy,
    local: &DiagnosticPolicy,
    in_file: &DiagnosticPolicy,
    flags: &PolicyFlags,
) -> DiagnosticPolicy {
    let flag_entries = flags.entries();
    let mut entries = Vec::with_capacity(
        global.entries.len() + local.entries.len() + in_file.entries.len() + flag_entries.len(),
    );
    entries.extend(global.entries.iter().cloned());
    entries.extend(local.entries.iter().cloned());
    entries.extend(in_file.entries.iter().cloned());
    entries.extend(flag_entries);
    DiagnosticPolicy { entries }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(verb: PolicyVerb, code: &str) -> DiagnosticPolicy {
        DiagnosticPolicy {
            entries: vec![PolicyEntry {
                verb,
                code: code.to_owned(),
                subjects: Vec::new(),
                source_span: None,
            }],
        }
    }

    #[test]
    fn empty_everything_is_identity() {
        let empty = DiagnosticPolicy::default();
        let merged = merge_policy(&empty, &empty, &empty, &PolicyFlags::default());
        assert!(merged.entries.is_empty());
        assert!(PolicyFlags::default().is_empty());
    }

    #[test]
    fn flags_beat_in_file_beat_local_beat_global() {
        let flags = PolicyFlags {
            deny: vec!["a".to_owned()],
            ..Default::default()
        };
        let merged = merge_policy(
            &one(PolicyVerb::Allow, "a"),
            &one(PolicyVerb::Deny, "a"),
            &one(PolicyVerb::Allow, "a"),
            &flags,
        );
        assert_eq!(merged.verb_for("a", None), Some(&PolicyVerb::Deny));
    }

    #[test]
    fn in_file_beats_config_when_no_flag() {
        let merged = merge_policy(
            &one(PolicyVerb::Deny, "a"),
            &one(PolicyVerb::Deny, "a"),
            &one(PolicyVerb::Allow, "a"),
            &PolicyFlags::default(),
        );
        assert_eq!(merged.verb_for("a", None), Some(&PolicyVerb::Allow));
    }

    #[test]
    fn deny_flag_wins_over_allow_flag_for_same_code() {
        let flags = PolicyFlags {
            allow: vec!["a".to_owned()],
            deny: vec!["a".to_owned()],
            ..Default::default()
        };
        let empty = DiagnosticPolicy::default();
        let merged = merge_policy(&empty, &empty, &empty, &flags);
        assert_eq!(merged.verb_for("a", None), Some(&PolicyVerb::Deny));
    }
}
