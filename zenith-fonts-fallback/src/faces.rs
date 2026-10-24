//! The fallback chain manifest.

use crate::coverage;

/// One face of the fallback chain. Every face is upright (normal style).
#[derive(Debug, Clone, Copy)]
pub struct FallbackFace {
    /// The stable font id a Zenith provider assigns to the face:
    /// `"{family_kebab_lower}-{weight}-normal"`.
    pub id: &'static str,
    /// The file name under `fonts/`.
    pub file: &'static str,
    /// The family name in the face's name table.
    pub family: &'static str,
    /// The face weight.
    pub weight: u16,
    /// The SHA-256 hex digest of the file.
    pub sha256: &'static str,
    /// The embedded bytes, or `None` without the `embed` feature.
    pub bytes: Option<&'static [u8]>,
    /// The code points the face maps, as sorted inclusive ranges. Every
    /// build holds it, with or without the `embed` feature.
    coverage: &'static [(u32, u32)],
}

impl FallbackFace {
    /// `true` when this build embeds the face.
    #[must_use]
    pub fn is_embedded(&self) -> bool {
        self.bytes.is_some()
    }

    /// `true` when the face maps `ch` to a glyph. The test equals the one
    /// the shaper runs on the parsed face.
    #[must_use]
    pub fn covers(&self, ch: char) -> bool {
        let cp = u32::from(ch);
        self.coverage
            .binary_search_by(|&(start, end)| {
                if end < cp {
                    std::cmp::Ordering::Less
                } else if start > cp {
                    std::cmp::Ordering::Greater
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .is_ok()
    }
}

/// Expand to `Some(embedded::$name)` with the `embed` feature, else `None`.
macro_rules! embedded {
    ($name:ident) => {{
        #[cfg(feature = "embed")]
        let bytes = Some(crate::embedded::$name);
        #[cfg(not(feature = "embed"))]
        let bytes = None;
        bytes
    }};
}

/// The chain, in fallback order. Emoji is last, so a CJK or script face
/// wins for a symbol both cover.
///
/// A `static`, not a `const`: every use of a `const` copies the array.
static FACES: [FallbackFace; 4] = [
    FallbackFace {
        id: "noto-sans-arabic-400-normal",
        file: "NotoSansArabic-Regular.ttf",
        family: "Noto Sans Arabic",
        weight: 400,
        sha256: "bdca411e84c02a4fc78337f06babbb9384a4c125a49cd288bc10ec3711c5fe8b",
        bytes: embedded!(NOTO_SANS_ARABIC),
        coverage: &coverage::ARABIC,
    },
    FallbackFace {
        id: "noto-sans-hebrew-400-normal",
        file: "NotoSansHebrew-Regular.ttf",
        family: "Noto Sans Hebrew",
        weight: 400,
        sha256: "b2af525b860f344139e1fca57fe545f375a8ef3c6b1b0728bb51c7c86c870163",
        bytes: embedded!(NOTO_SANS_HEBREW),
        coverage: &coverage::HEBREW,
    },
    FallbackFace {
        id: "noto-sans-cjk-sc-400-normal",
        file: "NotoSansCJKsc-Fallback.otf",
        family: "Noto Sans CJK SC",
        weight: 400,
        sha256: "2e827dfaab2a1f3195c09fab6efd20e78f087069bb3f6bbaeac3692fde3ac128",
        bytes: embedded!(NOTO_SANS_CJK_SC),
        coverage: &coverage::CJK_SC,
    },
    FallbackFace {
        id: "noto-emoji-400-normal",
        file: "NotoEmoji-Regular.ttf",
        family: "Noto Emoji",
        weight: 400,
        sha256: "73cd897c832fa39bb7f78f8aa3f039e298734873653b3351de49c90839f976db",
        bytes: embedded!(NOTO_EMOJI),
        coverage: &coverage::EMOJI,
    },
];

/// Every fallback face, in chain order, whether or not this build embeds it.
#[must_use]
pub fn fallback_faces() -> &'static [FallbackFace] {
    &FACES
}

/// The face the shaper picks for `ch` when no other face covers it: the
/// first face in chain order that covers `ch`. `None` when no face does.
///
/// A host that does not embed the chain fetches this face's file. With
/// the faces registered in chain order, it then shapes `ch` as a build
/// that embeds every face.
#[must_use]
pub fn fallback_face_for(ch: char) -> Option<&'static FallbackFace> {
    FACES.iter().find(|f| f.covers(ch))
}

/// The fallback face whose file name is `file`, in any case.
#[must_use]
pub fn fallback_face_by_file(file: &str) -> Option<&'static FallbackFace> {
    FACES.iter().find(|f| f.file.eq_ignore_ascii_case(file))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_follow_the_family_weight_scheme() {
        for face in fallback_faces() {
            let base = face.family.to_lowercase().replace(' ', "-");
            assert_eq!(face.id, format!("{base}-{}-normal", face.weight));
        }
    }

    #[test]
    fn lookup_by_file_ignores_case() {
        let face = fallback_face_by_file("notoemoji-regular.ttf").expect("found");
        assert_eq!(face.family, "Noto Emoji");
        assert!(fallback_face_by_file("NotoColorEmoji.ttf").is_none());
    }

    #[test]
    fn each_script_maps_to_its_face() {
        let cases = [
            ('ع', "NotoSansArabic-Regular.ttf"),
            ('ש', "NotoSansHebrew-Regular.ttf"),
            ('你', "NotoSansCJKsc-Fallback.otf"),
            ('안', "NotoSansCJKsc-Fallback.otf"),
            ('こ', "NotoSansCJKsc-Fallback.otf"),
            ('😀', "NotoEmoji-Regular.ttf"),
        ];
        for (ch, file) in cases {
            let face = fallback_face_for(ch).map(|f| f.file);
            assert_eq!(face, Some(file), "{ch}");
        }
    }

    #[test]
    fn uncovered_code_points_map_to_no_face() {
        // A private-use code point and an unassigned plane-3 code point.
        assert!(fallback_face_for('\u{E000}').is_none());
        assert!(fallback_face_for('\u{3FFFD}').is_none());
    }

    #[test]
    fn range_ends_are_covered_and_gaps_are_not() {
        for face in fallback_faces() {
            assert!(!face.coverage.is_empty(), "{}", face.file);
            for pair in face.coverage.windows(2) {
                let (s0, e0) = pair[0];
                let (s1, _) = pair[1];
                assert!(
                    s0 <= e0 && e0 + 1 < s1,
                    "{}: unsorted or touching",
                    face.file
                );
                // The code point after a range is not covered (a surrogate is no char).
                if let Some(gap) = char::from_u32(e0 + 1) {
                    assert!(!face.covers(gap), "{} U+{:04X}", face.file, e0 + 1);
                }
            }
            for &(start, end) in face.coverage {
                for cp in [start, end] {
                    let ch = char::from_u32(cp).expect("range end is a char");
                    assert!(face.covers(ch), "{} U+{cp:04X}", face.file);
                }
            }
        }
    }

    #[test]
    #[cfg(not(feature = "embed"))]
    fn no_face_is_embedded_without_the_feature() {
        assert!(fallback_faces().iter().all(|f| !f.is_embedded()));
    }
}
