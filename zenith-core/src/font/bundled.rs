//! The bundled Noto faces: the manifest of every face the project ships, and
//! the cached default provider built from the faces this build embeds.
//!
//! The manifest lists all ten faces in every build, with `bytes: None` for a
//! face the build drops (see the `bundled-fonts-extended` feature). A host
//! reads the manifest to learn which file to fetch and registers it with
//! [`BundledFace::register`]. The registered face is identical to one the
//! default build embeds, so output does not change.

use std::sync::{Arc, OnceLock};

use super::embedded;
use super::{BytesFontProvider, FontSource, FontStyle};

/// One face of the bundled set.
#[derive(Debug, Clone, Copy)]
pub struct BundledFace {
    /// The file name under `zenith-core/assets/fonts/`.
    pub file: &'static str,
    /// The family name the face registers under.
    pub family: &'static str,
    /// The face weight.
    pub weight: u16,
    /// The face style.
    pub style: FontStyle,
    /// The embedded bytes, or `None` when this build drops the face.
    pub bytes: Option<&'static [u8]>,
}

/// Expand to `Some(embedded::$name)` when the extended set is embedded, and
/// to `None` when it is not.
macro_rules! extended {
    ($name:ident) => {{
        #[cfg(feature = "bundled-fonts-extended")]
        let bytes = Some(embedded::$name);
        #[cfg(not(feature = "bundled-fonts-extended"))]
        let bytes = None;
        bytes
    }};
}

const fn face(
    file: &'static str,
    family: &'static str,
    weight: u16,
    style: FontStyle,
    bytes: Option<&'static [u8]>,
) -> BundledFace {
    BundledFace {
        file,
        family,
        weight,
        style,
        bytes,
    }
}

/// All ten bundled faces, in registration order.
///
/// A `static`, not a `const`: every use of a `const` copies the array, and
/// that duplicated the embedded font data in the binary.
static FACES: [BundledFace; 10] = [
    face(
        "NotoSans-Regular.ttf",
        "Noto Sans",
        400,
        FontStyle::Normal,
        Some(embedded::NOTO_SANS_REGULAR),
    ),
    face(
        "NotoSans-Bold.ttf",
        "Noto Sans",
        700,
        FontStyle::Normal,
        Some(embedded::NOTO_SANS_BOLD),
    ),
    face(
        "NotoSans-Italic.ttf",
        "Noto Sans",
        400,
        FontStyle::Italic,
        extended!(NOTO_SANS_ITALIC),
    ),
    face(
        "NotoSans-BoldItalic.ttf",
        "Noto Sans",
        700,
        FontStyle::Italic,
        extended!(NOTO_SANS_BOLD_ITALIC),
    ),
    face(
        "NotoSerif-Regular.ttf",
        "Noto Serif",
        400,
        FontStyle::Normal,
        extended!(NOTO_SERIF_REGULAR),
    ),
    face(
        "NotoSerif-Bold.ttf",
        "Noto Serif",
        700,
        FontStyle::Normal,
        extended!(NOTO_SERIF_BOLD),
    ),
    face(
        "NotoSerif-Italic.ttf",
        "Noto Serif",
        400,
        FontStyle::Italic,
        extended!(NOTO_SERIF_ITALIC),
    ),
    face(
        "NotoSerif-BoldItalic.ttf",
        "Noto Serif",
        700,
        FontStyle::Italic,
        extended!(NOTO_SERIF_BOLD_ITALIC),
    ),
    face(
        "NotoSansMono-Regular.ttf",
        "Noto Sans Mono",
        400,
        FontStyle::Normal,
        extended!(NOTO_SANS_MONO_REGULAR),
    ),
    face(
        "NotoSansMono-Bold.ttf",
        "Noto Sans Mono",
        700,
        FontStyle::Normal,
        extended!(NOTO_SANS_MONO_BOLD),
    ),
];

/// The manifest of all ten bundled faces, whether or not this build embeds
/// them.
#[must_use]
pub fn bundled_faces() -> &'static [BundledFace] {
    &FACES
}

/// The bundled face named `file`, with or without the `.ttf` extension, in
/// any case. `None` when `file` is not a bundled file name.
#[must_use]
pub fn bundled_face_by_file(file: &str) -> Option<&'static BundledFace> {
    let wanted = file.strip_suffix(".ttf").unwrap_or(file);
    FACES.iter().find(|f| {
        f.file
            .strip_suffix(".ttf")
            .unwrap_or(f.file)
            .eq_ignore_ascii_case(wanted)
    })
}

/// The bundled face with exactly this family (any case), weight, and style.
#[must_use]
pub fn bundled_face_for(
    family: &str,
    weight: u16,
    style: FontStyle,
) -> Option<&'static BundledFace> {
    FACES
        .iter()
        .find(|f| f.family.eq_ignore_ascii_case(family) && f.weight == weight && f.style == style)
}

impl BundledFace {
    /// `true` when this build embeds the face.
    #[must_use]
    pub fn is_embedded(&self) -> bool {
        self.bytes.is_some()
    }

    /// Register `bytes` (the content of [`Self::file`]) into `provider` as
    /// this face, with [`FontSource::Bundled`]. The registration equals the
    /// one an embedding build makes, so rendering does not change.
    pub fn register(&self, provider: &mut BytesFontProvider, bytes: Arc<[u8]>) -> String {
        provider.register(
            self.family,
            self.weight,
            self.style,
            bytes,
            0,
            FontSource::Bundled,
        )
    }
}

static BASE: OnceLock<BytesFontProvider> = OnceLock::new();

/// The shared base provider: every embedded face, built once per process.
fn base() -> &'static BytesFontProvider {
    BASE.get_or_init(|| {
        let mut provider = BytesFontProvider::new();
        for face in &FACES {
            if let Some(bytes) = face.bytes {
                face.register(&mut provider, Arc::from(bytes));
            }
        }
        provider
    })
}

/// A [`BytesFontProvider`] preloaded with the bundled default fonts.
///
/// The faces are built once per process. Each call clones the registry and
/// shares the font bytes (`Arc`), so a caller can register project faces on
/// the result without touching the shared base and without copying any font.
///
/// Noto Sans Regular (400) and Bold (700), both Normal, are always present.
/// With the default `bundled-fonts-extended` feature the provider also holds
/// Noto Sans Italic and Bold Italic, the four Noto Serif faces (400 and 700,
/// Normal and Italic), and Noto Sans Mono Regular and Bold. All are
/// Apache-2.0. [`bundled_faces`] lists which a build embeds.
#[must_use]
pub fn default_provider() -> BytesFontProvider {
    base().clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::FontProvider;

    #[test]
    fn manifest_lists_ten_unique_faces() {
        let faces = bundled_faces();
        assert_eq!(faces.len(), 10);
        let mut files: Vec<_> = faces.iter().map(|f| f.file).collect();
        files.sort_unstable();
        files.dedup();
        assert_eq!(files.len(), 10);
    }

    #[test]
    fn sans_regular_and_bold_are_always_embedded() {
        for file in ["NotoSans-Regular.ttf", "NotoSans-Bold.ttf"] {
            let face = bundled_face_by_file(file).expect("in manifest");
            assert!(face.is_embedded(), "{file}");
        }
    }

    #[test]
    fn lookup_by_file_ignores_extension_and_case() {
        let a = bundled_face_by_file("notoserif-bold").expect("found");
        assert_eq!(a.file, "NotoSerif-Bold.ttf");
        assert!(bundled_face_by_file("Inter-Regular.ttf").is_none());
        let b = bundled_face_for("noto sans mono", 700, FontStyle::Normal).expect("found");
        assert_eq!(b.file, "NotoSansMono-Bold.ttf");
    }

    #[test]
    fn default_provider_shares_bytes_between_calls() {
        let a = default_provider()
            .resolve(&["Noto Sans".to_owned()], 400, FontStyle::Normal)
            .expect("sans");
        let b = default_provider()
            .resolve(&["Noto Sans".to_owned()], 400, FontStyle::Normal)
            .expect("sans");
        assert!(Arc::ptr_eq(&a.bytes, &b.bytes), "calls must share bytes");
    }

    #[test]
    fn layering_a_face_leaves_the_shared_base_unchanged() {
        let mut layered = default_provider();
        layered.register(
            "Extra",
            400,
            FontStyle::Normal,
            Arc::from(vec![1u8; 4].as_slice()),
            0,
            FontSource::Project,
        );
        assert!(
            layered
                .resolve(&["Extra".to_owned()], 400, FontStyle::Normal)
                .is_some()
        );
        assert!(
            default_provider()
                .resolve(&["Extra".to_owned()], 400, FontStyle::Normal)
                .is_none(),
            "the shared base must not gain the layered face"
        );
    }

    #[test]
    fn default_provider_holds_exactly_the_embedded_faces() {
        let embedded_count = bundled_faces().iter().filter(|f| f.is_embedded()).count();
        assert_eq!(default_provider().all_faces().len(), embedded_count);
    }

    #[test]
    #[cfg(not(feature = "bundled-fonts-extended"))]
    fn minimal_build_embeds_only_sans_regular_and_bold() {
        let kept: Vec<_> = bundled_faces()
            .iter()
            .filter(|f| f.is_embedded())
            .map(|f| f.file)
            .collect();
        assert_eq!(kept, ["NotoSans-Regular.ttf", "NotoSans-Bold.ttf"]);
        let p = default_provider();
        assert!(
            p.resolve(&["Noto Sans".to_owned()], 700, FontStyle::Normal)
                .is_some()
        );
        assert!(
            p.resolve(&["Noto Serif".to_owned()], 400, FontStyle::Normal)
                .is_none()
        );
        assert!(
            p.resolve(&["Noto Sans Mono".to_owned()], 400, FontStyle::Normal)
                .is_none()
        );
    }

    #[test]
    #[cfg(feature = "bundled-fonts-extended")]
    fn default_build_embeds_all_ten_faces() {
        assert!(bundled_faces().iter().all(BundledFace::is_embedded));
    }
}
