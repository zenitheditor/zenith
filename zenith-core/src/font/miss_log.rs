//! A record of the faces a document asked for and the provider could not
//! match exactly, and of the chars the shaper gave to a fallback face.
//!
//! A host attaches a [`FontMissLog`] to a [`super::BytesFontProvider`] to learn
//! which faces a document needs. The browser build uses it to fetch only the
//! bundled and fallback faces that the module does not embed. The log never
//! changes resolution or shaping.

use std::collections::{BTreeMap, BTreeSet};
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

/// One char that no requested face covers, and the fallback face that
/// shaped it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FallbackGlyph {
    /// The char.
    pub ch: char,
    /// The lowercase family of the fallback face that shaped `ch`. `None`
    /// when no face covers it, so it shaped as `.notdef`.
    pub face_family: Option<String>,
}

type MissKey = (String, u16, FontStyle);

/// The records behind one log.
#[derive(Debug, Default)]
struct Records {
    faces: BTreeMap<MissKey, FaceRequest>,
    glyphs: BTreeSet<FallbackGlyph>,
}

/// A shared, thread-safe set of [`FaceRequest`]s and [`FallbackGlyph`]s.
/// Cloning shares the sets.
#[derive(Debug, Clone, Default)]
pub struct FontMissLog {
    inner: Arc<Mutex<Records>>,
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
        if let Ok(mut records) = self.inner.lock() {
            records
                .faces
                .entry((family.to_lowercase(), weight, style))
                .or_insert_with(|| FaceRequest {
                    family: family.to_owned(),
                    weight,
                    style,
                    family_registered,
                });
        }
    }

    /// Record that the shaper gave `ch` to the fallback face of family
    /// `face_family`, or to no face (`None`). A poisoned lock drops the
    /// record, which only loses a hint.
    pub(super) fn record_glyph(&self, ch: char, face_family: Option<&str>) {
        if let Ok(mut records) = self.inner.lock() {
            records.glyphs.insert(FallbackGlyph {
                ch,
                face_family: face_family.map(str::to_lowercase),
            });
        }
    }

    /// Every recorded request, ordered by lowercase family, weight, style.
    #[must_use]
    pub fn requests(&self) -> Vec<FaceRequest> {
        self.inner
            .lock()
            .map(|records| records.faces.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Every recorded fallback glyph, ordered by char, then face family.
    #[must_use]
    pub fn fallback_glyphs(&self) -> Vec<FallbackGlyph> {
        self.inner
            .lock()
            .map(|records| records.glyphs.iter().cloned().collect())
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
        assert!(log.fallback_glyphs().is_empty());
    }

    #[test]
    fn records_fallback_glyphs_through_the_provider() {
        let log = FontMissLog::new();
        let mut p = BytesFontProvider::new().with_miss_log(log.clone());
        let id = p.register_fallback(
            "Noto Sans Arabic",
            400,
            FontStyle::Normal,
            Arc::from(vec![0u8; 4].as_slice()),
            0,
        );
        p.note_fallback_glyphs(&[
            ('ع', Some(id.as_str())),
            ('😀', None),
            ('ع', Some(id.as_str())),
        ]);
        let got = log.fallback_glyphs();
        assert_eq!(
            got,
            vec![
                FallbackGlyph {
                    ch: 'ع',
                    face_family: Some("noto sans arabic".to_owned()),
                },
                FallbackGlyph {
                    ch: '😀',
                    face_family: None,
                },
            ]
        );
    }
}
