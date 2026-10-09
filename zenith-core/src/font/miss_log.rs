//! A record of the faces a document asked for and the provider could not
//! match exactly.
//!
//! A host attaches a [`FontMissLog`] to a [`super::BytesFontProvider`] to learn
//! which faces a document needs. The browser build uses it to fetch only the
//! bundled faces that the module does not embed. The log never changes
//! resolution.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use super::FontStyle;

/// One requested face that has no exact `(family, weight, style)` match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceRequest {
    /// The family as first requested (original case).
    pub family: String,
    /// The requested weight.
    pub weight: u16,
    /// The requested style.
    pub style: FontStyle,
    /// `true` when the provider holds another face of this family, so the
    /// request resolved to that face. `false` when the family is absent.
    pub family_registered: bool,
}

type MissKey = (String, u16, FontStyle);

/// A shared, thread-safe set of [`FaceRequest`]s. Cloning shares the set.
#[derive(Debug, Clone, Default)]
pub struct FontMissLog {
    inner: Arc<Mutex<BTreeMap<MissKey, FaceRequest>>>,
}

impl FontMissLog {
    /// An empty log.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one request. The first spelling of a `(family, weight, style)`
    /// wins. A poisoned lock drops the record, which only loses a hint.
    pub(super) fn record(
        &self,
        family: &str,
        weight: u16,
        style: FontStyle,
        family_registered: bool,
    ) {
        if let Ok(mut map) = self.inner.lock() {
            map.entry((family.to_lowercase(), weight, style))
                .or_insert_with(|| FaceRequest {
                    family: family.to_owned(),
                    weight,
                    style,
                    family_registered,
                });
        }
    }

    /// Every recorded request, ordered by lowercase family, weight, style.
    #[must_use]
    pub fn requests(&self) -> Vec<FaceRequest> {
        self.inner
            .lock()
            .map(|map| map.values().cloned().collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::{BytesFontProvider, FontProvider, FontSource};

    #[test]
    fn records_absent_family_and_variant_substitution_once() {
        let log = FontMissLog::new();
        let mut p = BytesFontProvider::new().with_miss_log(log.clone());
        p.register(
            "Face",
            400,
            FontStyle::Normal,
            Arc::from(vec![0u8; 4].as_slice()),
            0,
            FontSource::Project,
        );
        assert!(
            p.resolve(&["face".into()], 400, FontStyle::Normal)
                .is_some()
        );
        assert!(
            p.resolve(&["Face".into()], 700, FontStyle::Normal)
                .is_some()
        );
        assert!(
            p.resolve(&["Face".into()], 700, FontStyle::Normal)
                .is_some()
        );
        assert!(
            p.resolve(&["Gone".into()], 400, FontStyle::Italic)
                .is_none()
        );
        let got = log.requests();
        assert_eq!(got.len(), 2, "{got:?}");
        assert_eq!(got[0].family, "Face");
        assert!(got[0].family_registered);
        assert_eq!(got[1].family, "Gone");
        assert!(!got[1].family_registered);
    }
}
