//! The fallback chain manifest.

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
}

impl FallbackFace {
    /// `true` when this build embeds the face.
    #[must_use]
    pub fn is_embedded(&self) -> bool {
        self.bytes.is_some()
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
    },
    FallbackFace {
        id: "noto-sans-hebrew-400-normal",
        file: "NotoSansHebrew-Regular.ttf",
        family: "Noto Sans Hebrew",
        weight: 400,
        sha256: "b2af525b860f344139e1fca57fe545f375a8ef3c6b1b0728bb51c7c86c870163",
        bytes: embedded!(NOTO_SANS_HEBREW),
    },
    FallbackFace {
        id: "noto-sans-cjk-sc-400-normal",
        file: "NotoSansCJKsc-Fallback.otf",
        family: "Noto Sans CJK SC",
        weight: 400,
        sha256: "2e827dfaab2a1f3195c09fab6efd20e78f087069bb3f6bbaeac3692fde3ac128",
        bytes: embedded!(NOTO_SANS_CJK_SC),
    },
    FallbackFace {
        id: "noto-emoji-400-normal",
        file: "NotoEmoji-Regular.ttf",
        family: "Noto Emoji",
        weight: 400,
        sha256: "73cd897c832fa39bb7f78f8aa3f039e298734873653b3351de49c90839f976db",
        bytes: embedded!(NOTO_EMOJI),
    },
];

/// Every fallback face, in chain order, whether or not this build embeds it.
#[must_use]
pub fn fallback_faces() -> &'static [FallbackFace] {
    &FACES
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
    #[cfg(not(feature = "embed"))]
    fn no_face_is_embedded_without_the_feature() {
        assert!(fallback_faces().iter().all(|f| !f.is_embedded()));
    }
}
