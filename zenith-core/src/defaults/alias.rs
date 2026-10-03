//! Internal ids of per-page lowered copies, and the map back to authored ids.
//!
//! A local component placed on a page, and the page's master, lower into a
//! copy with an internal id so each page (and ambient pair) gets its own
//! lowered subtree. The id is an opaque key: nothing parses it. [`IdAliases`]
//! records each copy id with the authored id it stands for, and every
//! user-facing surface maps through it.

use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostics::Diagnostic;

/// Copy id → authored id, for the copies one lowering made.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct IdAliases {
    authored: BTreeMap<String, String>,
}

impl IdAliases {
    /// Whether no copy was made.
    pub fn is_empty(&self) -> bool {
        self.authored.is_empty()
    }

    /// The authored id behind `id`: the component or master a copy stands
    /// for, else `id` itself.
    pub fn authored_id<'a>(&'a self, id: &'a str) -> &'a str {
        self.authored.get(id).map_or(id, String::as_str)
    }

    /// Add every alias of `other`.
    pub fn extend(&mut self, other: &IdAliases) {
        for (copy, authored) in &other.authored {
            self.authored
                .entry(copy.clone())
                .or_insert_with(|| authored.clone());
        }
    }

    /// Rewrite copy ids to authored ids in the subject, message, and cause
    /// of each diagnostic.
    pub fn scrub(&self, diagnostics: &mut [Diagnostic]) {
        if self.authored.is_empty() {
            return;
        }
        for diagnostic in diagnostics {
            if let Some(subject) = diagnostic.subject_id.as_mut() {
                let authored = self.scrub_text(subject);
                *subject = authored;
            }
            diagnostic.message = self.scrub_text(&diagnostic.message);
            if let Some(cause) = diagnostic.cause()
                && self.mentions(cause)
            {
                let cause = self.scrub_text(cause);
                *diagnostic = diagnostic.clone().with_cause(cause);
            }
        }
    }

    fn mentions(&self, text: &str) -> bool {
        self.authored
            .keys()
            .any(|copy| text.contains(copy.as_str()))
    }

    /// `text` with every copy id replaced by its authored id. Longer copy ids
    /// go first, so a copy id that prefixes another never splits it.
    fn scrub_text(&self, text: &str) -> String {
        if !self.mentions(text) {
            return text.to_owned();
        }
        let mut copies: Vec<(&String, &String)> = self.authored.iter().collect();
        copies.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then_with(|| a.0.cmp(b.0)));
        let mut out = text.to_owned();
        for (copy, authored) in copies {
            if out.contains(copy.as_str()) {
                out = out.replace(copy.as_str(), authored);
            }
        }
        out
    }
}

/// Allocates copy ids that collide with no declared id.
pub(super) struct CopyIds {
    taken: BTreeSet<String>,
    pub(super) aliases: IdAliases,
}

impl CopyIds {
    /// `taken`: every component and master id the document declares.
    pub(super) fn new(taken: BTreeSet<String>) -> Self {
        Self {
            taken,
            aliases: IdAliases::default(),
        }
    }

    /// A fresh copy id for `authored` keyed by `key`, recorded as its alias.
    /// The same `(authored, key)` returns the same id.
    pub(super) fn id(&mut self, authored: &str, key: &str) -> String {
        let base = format!("{authored}@defaults:{key}");
        let mut candidate = base.clone();
        let mut n: u32 = 1;
        loop {
            match self.aliases.authored.get(&candidate) {
                Some(existing) if existing == authored => return candidate,
                Some(_) => {}
                None if !self.taken.contains(&candidate) => {
                    self.aliases
                        .authored
                        .insert(candidate.clone(), authored.to_owned());
                    return candidate;
                }
                None => {}
            }
            n += 1;
            candidate = format!("{base}~{n}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_ids_avoid_declared_ids_and_map_back() {
        let mut ids = CopyIds::new(["card@defaults:pg".to_owned()].into_iter().collect());
        let id = ids.id("card", "pg");
        assert_eq!(id, "card@defaults:pg~2");
        assert_eq!(ids.id("card", "pg"), id, "stable per key");
        assert_eq!(ids.aliases.authored_id(&id), "card");
        assert_eq!(ids.aliases.authored_id("other"), "other");
    }

    #[test]
    fn scrub_rewrites_subject_message_and_cause() {
        let mut ids = CopyIds::new(BTreeSet::new());
        let short = ids.id("card", "pg:color.a");
        let long = ids.id("card", "pg:color.a.content");
        let mut diags = vec![
            Diagnostic::warning(
                "x.y",
                format!("component '{long}' and '{short}' failed"),
                None,
                Some(long.clone()),
            )
            .with_cause(format!("in '{short}'")),
        ];
        ids.aliases.scrub(&mut diags);
        assert_eq!(diags[0].message, "component 'card' and 'card' failed");
        assert_eq!(diags[0].subject_id.as_deref(), Some("card"));
        assert_eq!(diags[0].cause(), Some("in 'card'"));
    }
}
