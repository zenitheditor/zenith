//! Font sourcing layer for Zenith.
//!
//! Provides a deterministic, system-font-free registry for resolving font bytes
//! by family name, weight, and style. All ordering-sensitive collections use
//! `BTreeMap` for determinism. No external crate dependencies — only `std`.

use std::collections::BTreeMap;
use std::sync::Arc;

use super::miss_log::FontMissLog;

/// The style variant of a font face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FontStyle {
    Normal,
    Italic,
}

/// Where a resolved font face came from, in resolution-priority order.
///
/// This is the provenance of a registered face. It drives the `font.local`
/// advisory: a face resolved from [`FontSource::Local`] is sourced from the
/// machine running the render, so the output is NOT guaranteed deterministic
/// across machines. `Bundled` and `Project` faces travel with the engine /
/// document and are portable. `Fallback` faces are embedded too, and serve
/// only the glyphs no requested face covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontSource {
    /// Embedded in the engine binary (the bundled Noto faces). Fully portable.
    Bundled,
    /// Declared as a `font`-kind project asset and shipped with the document.
    Project,
    /// Discovered on the local/system font directories of the render machine.
    /// Non-deterministic across machines — emits a `font.local` advisory.
    Local,
    /// Embedded fallback face for glyphs the requested faces lack. Fully
    /// portable. A family name never resolves to it: shaping picks it per
    /// glyph. Registered with [`BytesFontProvider::register_fallback`].
    Fallback,
}

/// Resolved font bytes ready for shaping or outlining. Cheap to clone (`Arc`).
#[derive(Clone)]
pub struct FontData {
    /// Stable identifier, e.g. `"noto-sans-400-normal"`.
    pub id: String,
    /// Raw font file bytes.
    pub bytes: Arc<[u8]>,
    /// Face index within a font collection (0 for single-face fonts).
    pub index: u32,
    /// Provenance of this face (bundled / project / local-system).
    pub source: FontSource,
}

impl std::fmt::Debug for FontData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontData")
            .field("id", &self.id)
            .field("bytes_len", &self.bytes.len())
            .field("index", &self.index)
            .field("source", &self.source)
            .finish()
    }
}

/// Resolve font bytes by family + weight + style, or by stable id.
///
/// Implementations must never access system fonts.
pub trait FontProvider {
    /// Resolve by a priority-ordered family list, weight, and style.
    ///
    /// Iterates `families` in order. For each family:
    /// 1. Tries exact `(family, weight, style)`.
    /// 2. Falls back to the same family with any weight/style (first BTreeMap entry).
    ///
    /// Returns `None` only if no registered family matches any entry in `families`.
    /// Family comparison is case-insensitive.
    #[must_use]
    fn resolve(&self, families: &[String], weight: u16, style: FontStyle) -> Option<FontData>;

    /// Resolve by the stable id recorded on a shaped run.
    #[must_use]
    fn by_id(&self, id: &str) -> Option<FontData>;

    /// All registered faces, in a deterministic order (for building an SVG fontdb, etc.).
    #[must_use]
    fn all_faces(&self) -> Vec<FontData>;
}

// Internal key type for the primary registry map.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct FaceKey {
    family_lower: String,
    weight: u16,
    style: FontStyle,
}

/// In-memory font registry. Register bundled and project fonts up front;
/// this implementation never scans the system.
///
/// Two `BTreeMap`s and one `Vec` are maintained:
/// - `by_key`: `(family_lower, weight, style) -> FontData` for `resolve`.
/// - `by_id`: `id -> FontData` for `by_id`.
/// - `fallback`: the fallback faces, in registration order.
///
/// Cloning copies the maps and the list and shares every face's bytes (`Arc`), so a
/// clone is the cheap way to layer extra faces over a shared base.
#[derive(Clone)]
pub struct BytesFontProvider {
    by_key: BTreeMap<FaceKey, FontData>,
    by_id: BTreeMap<String, FontData>,
    /// Fallback faces in registration order. `resolve` never returns them.
    fallback: Vec<(FaceKey, FontData)>,
    /// When set, `resolve` records every request with no exact face.
    miss_log: Option<FontMissLog>,
}

impl std::fmt::Debug for BytesFontProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ids: Vec<&str> = self.by_id.keys().map(String::as_str).collect();
        let fallback: Vec<&str> = self.fallback.iter().map(|(_, d)| d.id.as_str()).collect();
        f.debug_struct("BytesFontProvider")
            .field("registered_faces", &ids)
            .field("fallback_faces", &fallback)
            .finish()
    }
}

impl BytesFontProvider {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            by_key: BTreeMap::new(),
            by_id: BTreeMap::new(),
            fallback: Vec::new(),
            miss_log: None,
        }
    }

    /// This provider with `log` recording every `resolve` request that has no
    /// exact `(family, weight, style)` face. Resolution is unchanged.
    #[must_use]
    pub fn with_miss_log(mut self, log: FontMissLog) -> Self {
        self.miss_log = Some(log);
        self
    }

    /// Register a font face and return its stable id.
    ///
    /// The id is computed as `"{family_kebab_lower}-{weight}-{style_lower}"`,
    /// e.g. `"noto-sans-400-normal"`. If the same face is registered more than
    /// once, the most recent registration wins and reuses the original id.
    /// Because kebab-casing can collapse distinct families (e.g. `"My Font"`
    /// and `"my-font"`) onto the same base id, a numeric suffix is appended
    /// when the base id is already taken by a *different* face, so every
    /// registered face keeps a unique id. Returns the assigned stable id as a
    /// convenience; callers may register purely for the side effect.
    ///
    /// `source` records the provenance of the face ([`FontSource`]); it is
    /// carried on the resolved [`FontData`] so the compile stage can emit a
    /// `font.local` advisory when a face resolves from the local system.
    pub fn register(
        &mut self,
        family: &str,
        weight: u16,
        style: FontStyle,
        bytes: Arc<[u8]>,
        index: u32,
        source: FontSource,
    ) -> String {
        let (key, base_id) = face_key(family, weight, style);

        // Re-registering the same face reuses its id; a new face whose base id
        // collides with a different face gets a numeric suffix.
        let id = match self.by_key.get(&key) {
            Some(existing) => existing.id.clone(),
            None => self.fresh_id(&base_id),
        };

        let data = FontData {
            id: id.clone(),
            bytes,
            index,
            source,
        };

        self.by_key.insert(key, data.clone());
        self.by_id.insert(id.clone(), data);

        id
    }

    /// Register a fallback face and return its stable id.
    ///
    /// [`FontProvider::all_faces`] lists fallback faces after every other
    /// face, in registration order. Shaping tries them in that order for a
    /// glyph the requested face lacks. `resolve` never returns them, so a
    /// document that does not need them renders as before.
    ///
    /// The id follows [`Self::register`]. Re-registering the same family,
    /// weight, and style replaces the bytes, keeps the id, and keeps the
    /// position in the order.
    pub fn register_fallback(
        &mut self,
        family: &str,
        weight: u16,
        style: FontStyle,
        bytes: Arc<[u8]>,
        index: u32,
    ) -> String {
        let (key, base_id) = face_key(family, weight, style);
        if let Some((_, existing)) = self.fallback.iter_mut().find(|(k, _)| *k == key) {
            existing.bytes = bytes;
            existing.index = index;
            return existing.id.clone();
        }
        let id = self.fresh_id(&base_id);
        let data = FontData {
            id: id.clone(),
            bytes,
            index,
            source: FontSource::Fallback,
        };
        self.fallback.push((key, data));
        id
    }

    /// `base_id`, or `base_id-N` for the first N from 2 that no face holds.
    fn fresh_id(&self, base_id: &str) -> String {
        let taken =
            |id: &str| self.by_id.contains_key(id) || self.fallback.iter().any(|(_, d)| d.id == id);
        let mut candidate = base_id.to_owned();
        let mut n = 2u32;
        while taken(&candidate) {
            candidate = format!("{base_id}-{n}");
            n += 1;
        }
        candidate
    }

    /// Return the lowercase family names of all registered faces (deduplicated, sorted).
    #[must_use]
    pub fn available_families(&self) -> Vec<String> {
        let mut families: Vec<String> =
            self.by_key.keys().map(|k| k.family_lower.clone()).collect();
        families.dedup();
        families
    }
}

/// The registry key and the base id (`"{family_kebab_lower}-{weight}-{style}"`)
/// of a face.
fn face_key(family: &str, weight: u16, style: FontStyle) -> (FaceKey, String) {
    let family_lower = family.to_lowercase();
    let family_kebab = family_lower.replace(' ', "-");
    let style_str = match style {
        FontStyle::Normal => "normal",
        FontStyle::Italic => "italic",
    };
    let base_id = format!("{family_kebab}-{weight}-{style_str}");
    let key = FaceKey {
        family_lower,
        weight,
        style,
    };
    (key, base_id)
}

impl Default for BytesFontProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl FontProvider for BytesFontProvider {
    fn resolve(&self, families: &[String], weight: u16, style: FontStyle) -> Option<FontData> {
        for family in families {
            let family_lower = family.to_lowercase();

            // 1. Exact match.
            let exact_key = FaceKey {
                family_lower: family_lower.clone(),
                weight,
                style,
            };
            if let Some(data) = self.by_key.get(&exact_key) {
                return Some(data.clone());
            }

            // 2. Fallback: same family, any weight/style — deterministic first entry.
            let fallback = self
                .by_key
                .range(
                    FaceKey {
                        family_lower: family_lower.clone(),
                        weight: 0,
                        style: FontStyle::Normal,
                    }..,
                )
                .find(|(k, _)| k.family_lower == family_lower)
                .map(|(_, v)| v.clone());

            if let Some(log) = &self.miss_log {
                log.record(family, weight, style, fallback.is_some());
            }
            if fallback.is_some() {
                return fallback;
            }
        }
        None
    }

    fn by_id(&self, id: &str) -> Option<FontData> {
        self.by_id
            .get(id)
            .or_else(|| {
                self.fallback
                    .iter()
                    .find(|(_, d)| d.id == id)
                    .map(|(_, d)| d)
            })
            .cloned()
    }

    /// Every face in `by_id` order, then the fallback faces in registration
    /// order.
    fn all_faces(&self) -> Vec<FontData> {
        self.by_id
            .values()
            .chain(self.fallback.iter().map(|(_, d)| d))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::default_provider;

    /// Helper: the four TrueType/OpenType magic bytes at offset 0.
    fn is_valid_tt_header(bytes: &[u8]) -> bool {
        bytes.len() > 1000 && bytes.starts_with(&[0x00, 0x01, 0x00, 0x00])
    }

    #[test]
    fn default_provider_resolves_noto_sans() {
        let p = default_provider();
        let result = p.resolve(&["Noto Sans".to_string()], 400, FontStyle::Normal);
        assert!(result.is_some(), "expected Some for Noto Sans 400 Normal");
        let data = result.unwrap();
        assert!(
            is_valid_tt_header(&data.bytes),
            "expected TrueType header and len > 1000, got len={}",
            data.bytes.len()
        );
    }

    #[test]
    #[cfg(feature = "bundled-fonts-extended")]
    fn default_provider_resolves_noto_serif_matrix() {
        let p = default_provider();
        for (weight, style) in [
            (400, FontStyle::Normal),
            (700, FontStyle::Normal),
            (400, FontStyle::Italic),
            (700, FontStyle::Italic),
        ] {
            let result = p.resolve(&["Noto Serif".to_string()], weight, style);
            assert!(
                result.is_some(),
                "expected Some for Noto Serif {weight} {style:?}"
            );
            let data = result.unwrap();
            assert_eq!(data.source, FontSource::Bundled, "serif must be bundled");
            assert!(
                is_valid_tt_header(&data.bytes),
                "expected TrueType header for Noto Serif {weight} {style:?}"
            );
        }
    }

    #[test]
    #[cfg(feature = "bundled-fonts-extended")]
    fn default_provider_resolves_noto_sans_mono() {
        let p = default_provider();
        let result = p.resolve(&["Noto Sans Mono".to_string()], 400, FontStyle::Normal);
        assert!(
            result.is_some(),
            "expected Some for Noto Sans Mono 400 Normal"
        );
        let data = result.unwrap();
        assert!(
            is_valid_tt_header(&data.bytes),
            "expected TrueType header and len > 1000, got len={}",
            data.bytes.len()
        );
        assert!(
            data.id.contains("noto-sans-mono"),
            "id should contain noto-sans-mono, got {}",
            data.id
        );
    }

    #[test]
    #[cfg(feature = "bundled-fonts-extended")]
    fn default_provider_distinguishes_sans_and_mono() {
        // The two bundled faces must be independently resolvable with distinct
        // bytes — a mono code node must not accidentally get the proportional face.
        let p = default_provider();
        let sans = p
            .resolve(&["Noto Sans".to_string()], 400, FontStyle::Normal)
            .expect("sans resolves");
        let mono = p
            .resolve(&["Noto Sans Mono".to_string()], 400, FontStyle::Normal)
            .expect("mono resolves");
        assert_ne!(sans.id, mono.id, "sans and mono must have distinct ids");
        assert_ne!(
            sans.bytes.len(),
            mono.bytes.len(),
            "sans and mono must be different font files"
        );
    }

    #[test]
    fn case_insensitive_family_lookup() {
        let p = default_provider();
        let lower = p.resolve(&["noto sans".to_string()], 400, FontStyle::Normal);
        let mixed = p.resolve(&["Noto Sans".to_string()], 400, FontStyle::Normal);
        assert!(lower.is_some(), "lowercase family should resolve");
        assert!(mixed.is_some(), "mixed-case family should resolve");
        assert_eq!(lower.unwrap().id, mixed.unwrap().id);
    }

    #[test]
    fn weight_fallback_resolves_unregistered_weight() {
        let p = default_provider();
        // weight 900 is not registered — should fall back to a registered face.
        let result = p.resolve(&["Noto Sans".to_string()], 900, FontStyle::Normal);
        assert!(
            result.is_some(),
            "weight 900 should fall back to a registered face"
        );
        let data = result.unwrap();
        assert!(data.id.contains("noto-sans"), "id should contain noto-sans");
    }

    #[test]
    #[cfg(feature = "bundled-fonts-extended")]
    fn bold_italic_resolves_distinct_combined_face() {
        // Weight 700 + Italic must resolve EXACTLY to the bold-italic face — not
        // fall back to bold-upright or regular-italic.
        let p = default_provider();
        let bold = p
            .resolve(&["Noto Sans".to_string()], 700, FontStyle::Normal)
            .expect("bold resolves");
        let italic = p
            .resolve(&["Noto Sans".to_string()], 400, FontStyle::Italic)
            .expect("italic resolves");
        let bold_italic = p
            .resolve(&["Noto Sans".to_string()], 700, FontStyle::Italic)
            .expect("bold-italic resolves");
        assert!(
            bold_italic.id.contains("700") && bold_italic.id.contains("italic"),
            "bold-italic id should encode both 700 and italic, got {}",
            bold_italic.id
        );
        assert_ne!(bold_italic.id, bold.id, "must differ from bold-upright");
        assert_ne!(bold_italic.id, italic.id, "must differ from regular-italic");
    }

    #[test]
    #[cfg(feature = "bundled-fonts-extended")]
    fn italic_style_resolves_distinct_italic_face() {
        // The bundled italic face (Noto Sans 400 Italic) must resolve EXACTLY
        // and be a different file than the regular Normal face.
        let p = default_provider();
        let normal = p
            .resolve(&["Noto Sans".to_string()], 400, FontStyle::Normal)
            .expect("normal resolves");
        let italic = p
            .resolve(&["Noto Sans".to_string()], 400, FontStyle::Italic)
            .expect("italic resolves");
        assert!(
            italic.id.contains("italic"),
            "italic id should encode the italic style, got {}",
            italic.id
        );
        assert_ne!(
            normal.id, italic.id,
            "normal and italic must have distinct ids"
        );
        assert_ne!(
            normal.bytes.len(),
            italic.bytes.len(),
            "normal and italic must be different font files"
        );
    }

    #[test]
    fn bold_weight_resolves_distinct_bold_face() {
        // The bundled bold face (Noto Sans 700) must resolve EXACTLY and be a
        // different file than the regular 400 face.
        let p = default_provider();
        let regular = p
            .resolve(&["Noto Sans".to_string()], 400, FontStyle::Normal)
            .expect("regular resolves");
        let bold = p
            .resolve(&["Noto Sans".to_string()], 700, FontStyle::Normal)
            .expect("bold resolves");
        assert!(
            bold.id.contains("noto-sans-700"),
            "bold id should encode weight 700, got {}",
            bold.id
        );
        assert_ne!(
            regular.id, bold.id,
            "regular and bold must have distinct ids"
        );
        assert_ne!(
            regular.bytes.len(),
            bold.bytes.len(),
            "regular and bold must be different font files"
        );
    }

    #[test]
    #[cfg(feature = "bundled-fonts-extended")]
    fn mono_bold_weight_resolves_distinct_bold_face() {
        // The bundled Noto Sans Mono Bold face (weight 700) must resolve EXACTLY
        // and be a different file than the Mono Regular (weight 400) face.
        let p = default_provider();
        let mono_regular = p
            .resolve(&["Noto Sans Mono".to_string()], 400, FontStyle::Normal)
            .expect("mono regular resolves");
        let mono_bold = p
            .resolve(&["Noto Sans Mono".to_string()], 700, FontStyle::Normal)
            .expect("mono bold resolves");
        assert!(
            mono_bold.id.contains("noto-sans-mono-700"),
            "mono bold id should encode weight 700, got {}",
            mono_bold.id
        );
        assert_ne!(
            mono_regular.id, mono_bold.id,
            "mono regular and mono bold must have distinct ids"
        );
        assert_ne!(
            mono_regular.bytes.len(),
            mono_bold.bytes.len(),
            "mono regular and mono bold must be different font files"
        );
    }

    #[test]
    fn unknown_family_returns_none() {
        let p = default_provider();
        let result = p.resolve(&["Nonexistent".to_string()], 400, FontStyle::Normal);
        assert!(result.is_none(), "unknown family must return None");
    }

    #[test]
    fn by_id_roundtrip() {
        let p = default_provider();
        let resolved = p
            .resolve(&["Noto Sans".to_string()], 400, FontStyle::Normal)
            .expect("should resolve");
        let by_id = p
            .by_id(&resolved.id)
            .expect("by_id should return same face");
        assert_eq!(resolved.id, by_id.id);
        assert_eq!(resolved.bytes.len(), by_id.bytes.len());
    }

    #[test]
    fn by_id_unknown_returns_none() {
        let p = default_provider();
        assert!(p.by_id("no-such-font-0-normal").is_none());
    }

    #[test]
    fn manual_register_and_resolve() {
        let mut p = BytesFontProvider::new();
        let dummy_bytes: Arc<[u8]> = Arc::from(vec![0u8; 64].as_slice());
        let id = p.register(
            "Test Family",
            400,
            FontStyle::Normal,
            dummy_bytes.clone(),
            0,
            FontSource::Project,
        );
        assert_eq!(id, "test-family-400-normal");

        let resolved = p.resolve(&["Test Family".to_string()], 400, FontStyle::Normal);
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap().id, "test-family-400-normal");
    }

    #[test]
    fn stable_id_format() {
        let mut p = BytesFontProvider::new();
        let bytes: Arc<[u8]> = Arc::from(vec![0u8; 4].as_slice());
        let id = p.register(
            "My Font",
            700,
            FontStyle::Italic,
            bytes,
            0,
            FontSource::Local,
        );
        assert_eq!(id, "my-font-700-italic");
    }

    #[test]
    fn re_registering_same_face_reuses_id() {
        let mut p = BytesFontProvider::new();
        let bytes: Arc<[u8]> = Arc::from(vec![1u8; 8].as_slice());
        let id1 = p.register(
            "Inter",
            400,
            FontStyle::Normal,
            bytes.clone(),
            0,
            FontSource::Project,
        );
        let id2 = p.register(
            "Inter",
            400,
            FontStyle::Normal,
            bytes,
            0,
            FontSource::Project,
        );
        assert_eq!(id1, id2, "same face re-registration keeps a stable id");
    }

    #[test]
    fn kebab_colliding_families_get_distinct_ids() {
        // "My Font" and "my-font" both kebab to "my-font-400-normal"; the second
        // must get a distinct id so it remains independently resolvable by id.
        let mut p = BytesFontProvider::new();
        let a: Arc<[u8]> = Arc::from(vec![0xAAu8; 4].as_slice());
        let b: Arc<[u8]> = Arc::from(vec![0xBBu8; 4].as_slice());
        let id_a = p.register("My Font", 400, FontStyle::Normal, a, 0, FontSource::Local);
        let id_b = p.register("my-font", 400, FontStyle::Normal, b, 0, FontSource::Local);
        assert_eq!(id_a, "my-font-400-normal");
        assert_ne!(id_a, id_b, "colliding families must not share an id");
        // Both remain resolvable by their distinct ids, with their own bytes.
        assert_eq!(p.by_id(&id_a).unwrap().bytes[0], 0xAA);
        assert_eq!(p.by_id(&id_b).unwrap().bytes[0], 0xBB);
    }

    #[test]
    fn resolve_carries_registered_source() {
        // A face registered as Local resolves with source == Local; a bundled
        // face resolves with source == Bundled. This provenance drives the
        // `font.local` advisory at compile time.
        let mut p = BytesFontProvider::new();
        let bytes: Arc<[u8]> = Arc::from(vec![0u8; 8].as_slice());
        p.register(
            "Local Face",
            400,
            FontStyle::Normal,
            bytes,
            0,
            FontSource::Local,
        );
        let local = p
            .resolve(&["Local Face".to_string()], 400, FontStyle::Normal)
            .expect("local face resolves");
        assert_eq!(local.source, FontSource::Local);

        let bundled = default_provider()
            .resolve(&["Noto Sans".to_string()], 400, FontStyle::Normal)
            .expect("bundled face resolves");
        assert_eq!(bundled.source, FontSource::Bundled);
    }

    fn fallback_bytes(tag: u8) -> Arc<[u8]> {
        Arc::from(vec![tag; 4].as_slice())
    }

    #[test]
    fn all_faces_lists_fallback_last_in_registration_order() {
        let mut p = BytesFontProvider::new();
        p.register_fallback("Zeta", 400, FontStyle::Normal, fallback_bytes(1), 0);
        p.register(
            "Inter",
            400,
            FontStyle::Normal,
            fallback_bytes(2),
            0,
            FontSource::Project,
        );
        p.register_fallback("Alpha", 400, FontStyle::Normal, fallback_bytes(3), 0);
        let ids: Vec<String> = p.all_faces().into_iter().map(|f| f.id).collect();
        assert_eq!(
            ids,
            ["inter-400-normal", "zeta-400-normal", "alpha-400-normal"]
        );
        let sources: Vec<FontSource> = p.all_faces().into_iter().map(|f| f.source).collect();
        assert_eq!(
            sources,
            [
                FontSource::Project,
                FontSource::Fallback,
                FontSource::Fallback
            ]
        );
    }

    #[test]
    fn fallback_by_id_round_trips_and_never_resolves_by_family() {
        let mut p = BytesFontProvider::new();
        let id = p.register_fallback("Noto Emoji", 400, FontStyle::Normal, fallback_bytes(7), 0);
        assert_eq!(id, "noto-emoji-400-normal");
        let face = p.by_id(&id).expect("fallback face by id");
        assert_eq!(face.id, id);
        assert_eq!(face.source, FontSource::Fallback);
        assert_eq!(face.bytes[0], 7);
        assert!(
            p.resolve(&["Noto Emoji".to_owned()], 400, FontStyle::Normal)
                .is_none()
        );
        assert!(p.available_families().is_empty());
    }

    #[test]
    fn re_registering_a_fallback_keeps_id_and_position() {
        let mut p = BytesFontProvider::new();
        let a = p.register_fallback("A", 400, FontStyle::Normal, fallback_bytes(1), 0);
        p.register_fallback("B", 400, FontStyle::Normal, fallback_bytes(2), 0);
        let again = p.register_fallback("a", 400, FontStyle::Normal, fallback_bytes(9), 0);
        assert_eq!(a, again);
        let faces = p.all_faces();
        assert_eq!(faces.len(), 2);
        assert_eq!(faces[0].id, a);
        assert_eq!(faces[0].bytes[0], 9);
    }

    #[test]
    fn fallback_and_regular_faces_never_share_an_id() {
        let mut p = BytesFontProvider::new();
        let fallback = p.register_fallback("X", 400, FontStyle::Normal, fallback_bytes(1), 0);
        let regular = p.register(
            "X",
            400,
            FontStyle::Normal,
            fallback_bytes(2),
            0,
            FontSource::Project,
        );
        assert_ne!(fallback, regular);
        assert_eq!(p.by_id(&fallback).expect("fallback").bytes[0], 1);
        assert_eq!(p.by_id(&regular).expect("regular").bytes[0], 2);
    }

    #[test]
    fn default_provider_faces_are_all_bundled() {
        // Every face the default provider exposes must be Bundled — the
        // byte-identical-when-bundled invariant relies on this.
        let p = default_provider();
        for face in p.all_faces() {
            assert_eq!(
                face.source,
                FontSource::Bundled,
                "default provider face {} must be Bundled",
                face.id
            );
        }
    }
}
