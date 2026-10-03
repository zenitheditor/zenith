//! The public engine and the cached shaper behind it.

use std::cell::RefCell;
use std::collections::BTreeMap;

use zenith_core::{FontData, FontProvider};

use super::face_cache::{FaceCache, FontFaceStore, parse_error};
use super::fallback::{FallbackSlots, shape_with_fallback_in};
use super::run::{FaceShapeRequest, shape_run_with_face};
use crate::engine::{FallbackResult, ShapeRequest, TextLayoutEngine, ZenithGlyphRun};
use crate::error::LayoutError;
use crate::ink::{GlyphInkBox, ink_box_with_face};

/// HarfBuzz-port shaping engine backed by `rustybuzz` and `rustybuzz::ttf_parser`.
///
/// Build one engine per render over a [`FontFaceStore`] and reuse it for every
/// shape call. Each stored face parses at most once per engine. A fallback
/// face parses only when a char needs it.
pub struct RustybuzzEngine<'a> {
    /// The face cache uses interior mutability, so its lifetime is invariant.
    /// The boxed trait object hides it. `&'a RustybuzzEngine<'a>` then
    /// shortens like a plain reference.
    shaper: Box<dyn TextLayoutEngine + 'a>,
}

impl<'a> RustybuzzEngine<'a> {
    /// Create an engine that parses faces from `store` on first use.
    #[must_use]
    pub fn new(store: &'a FontFaceStore) -> Self {
        Self {
            shaper: Box::new(CachedShaper::new(store)),
        }
    }
}

impl std::fmt::Debug for RustybuzzEngine<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RustybuzzEngine").finish_non_exhaustive()
    }
}

impl TextLayoutEngine for RustybuzzEngine<'_> {
    fn shape(
        &self,
        req: &ShapeRequest<'_>,
        provider: &dyn FontProvider,
    ) -> Result<ZenithGlyphRun, LayoutError> {
        self.shaper.shape(req, provider)
    }

    fn shape_with_fallback(
        &self,
        req: &ShapeRequest<'_>,
        provider: &dyn FontProvider,
    ) -> Result<FallbackResult, LayoutError> {
        self.shaper.shape_with_fallback(req, provider)
    }

    fn glyph_ink_box(
        &self,
        font_id: &str,
        glyph_id: u16,
        font_size: f32,
        provider: &dyn FontProvider,
    ) -> Option<GlyphInkBox> {
        self.shaper
            .glyph_ink_box(font_id, glyph_id, font_size, provider)
    }
}

/// Shaper that reads parsed faces from a [`FaceCache`].
///
/// A face the provider resolves outside the store parses per call. Output is
/// the same either way.
struct CachedShaper<'s> {
    faces: FaceCache<'s>,
    /// Ink boxes of stored faces, by `(slot, glyph id, font size bits)`.
    /// The page lint and the box recorder ask for the same glyph many times.
    ink_boxes: RefCell<BTreeMap<(usize, u16, u32), Option<GlyphInkBox>>>,
}

impl<'s> CachedShaper<'s> {
    fn new(store: &'s FontFaceStore) -> Self {
        Self {
            faces: FaceCache::new(store),
            ink_boxes: RefCell::new(BTreeMap::new()),
        }
    }
}

/// Resolve the primary face for `req`.
fn resolve_primary(
    req: &ShapeRequest<'_>,
    provider: &dyn FontProvider,
) -> Result<FontData, LayoutError> {
    provider
        .resolve(req.families, req.weight, req.style)
        .ok_or_else(|| {
            LayoutError::new(format!("no font resolved for families {:?}", req.families))
        })
}

impl TextLayoutEngine for CachedShaper<'_> {
    fn shape(
        &self,
        req: &ShapeRequest<'_>,
        provider: &dyn FontProvider,
    ) -> Result<ZenithGlyphRun, LayoutError> {
        let font_data = resolve_primary(req, provider)?;

        if let Some(slot) = self.faces.store().slot_of(&font_data) {
            let face = self
                .faces
                .face(slot)
                .ok_or_else(|| parse_error(&font_data))?;
            return shape_run_with_face(FaceShapeRequest::from_shape_request(
                req,
                face,
                req.text,
                font_data.id,
            ));
        }

        // The face is outside the store: parse it for this call only.
        let face = rustybuzz::Face::from_slice(&font_data.bytes, font_data.index)
            .ok_or_else(|| parse_error(&font_data))?;
        shape_run_with_face(FaceShapeRequest::from_shape_request(
            req,
            &face,
            req.text,
            font_data.id.clone(),
        ))
    }

    fn shape_with_fallback(
        &self,
        req: &ShapeRequest<'_>,
        provider: &dyn FontProvider,
    ) -> Result<FallbackResult, LayoutError> {
        let primary = resolve_primary(req, provider)?;

        if let Some(slot) = self.faces.store().slot_of(&primary) {
            let fallbacks = FallbackSlots::Provider {
                provider,
                primary_id: primary.id.as_str(),
            };
            if let Some(result) = shape_with_fallback_in(&self.faces, slot, fallbacks, req)? {
                return Ok(result);
            }
        }

        // A needed face is outside the store: build a store for this call.
        // The primary takes slot 0. The other faces follow in the order
        // `provider.all_faces()` returns them, without the primary id.
        let primary_id = primary.id.clone();
        let mut faces = vec![primary];
        faces.extend(
            provider
                .all_faces()
                .into_iter()
                .filter(|data| data.id != primary_id),
        );
        let store = FontFaceStore::from_faces(faces);
        let cache = FaceCache::new(&store);
        let fallbacks = FallbackSlots::Ready((1..store.face_count()).collect());
        shape_with_fallback_in(&cache, 0, fallbacks, req)?.ok_or_else(|| {
            LayoutError::new("internal: call-local face store is missing a face".to_owned())
        })
    }

    fn glyph_ink_box(
        &self,
        font_id: &str,
        glyph_id: u16,
        font_size: f32,
        provider: &dyn FontProvider,
    ) -> Option<GlyphInkBox> {
        let font = provider.by_id(font_id)?;
        match self.faces.store().slot_of(&font) {
            // A stored face: read the memo, else the cached parse.
            Some(slot) => {
                let key = (slot, glyph_id, font_size.to_bits());
                if let Some(hit) = self.ink_boxes.borrow().get(&key) {
                    return *hit;
                }
                let ink = ink_box_with_face(self.faces.face(slot)?, glyph_id, font_size);
                self.ink_boxes.borrow_mut().insert(key, ink);
                ink
            }
            // A face outside the store: parse it for this call only.
            None => crate::ink::glyph_ink_box(&font.bytes, font.index, glyph_id, font_size),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use zenith_core::{BytesFontProvider, FontStyle, default_provider};

    use super::*;
    use crate::engine::{KerningPairAdjustment, TextDirection};

    fn noto_sans() -> Vec<String> {
        vec!["Noto Sans".to_string()]
    }

    fn request<'a>(
        text: &'a str,
        families: &'a [String],
        font_size: f32,
        direction: TextDirection,
    ) -> ShapeRequest<'a> {
        ShapeRequest {
            text,
            families,
            weight: 400,
            style: FontStyle::Normal,
            font_size,
            direction,
            features: &[],
            kerning_pairs: &[],
            letter_spacing_px: 0.0,
        }
    }

    fn shape_at(font_size: f32) -> Result<ZenithGlyphRun, LayoutError> {
        let families = noto_sans();
        let req = request("Hello Zenith", &families, font_size, TextDirection::Ltr);
        let provider = default_provider();
        let store = FontFaceStore::new(&provider);
        RustybuzzEngine::new(&store).shape(&req, &provider)
    }

    #[test]
    fn shape_hello_zenith_at_24px() {
        let run = shape_at(24.0).expect("shaping should succeed");

        // font_id matches the registered stable id.
        assert_eq!(run.font_id, "noto-sans-400-normal");

        // Glyph count: "Hello Zenith" = 12 characters including the space.
        assert!(
            run.glyphs.len() >= 10,
            "expected >= 10 glyphs, got {}",
            run.glyphs.len()
        );

        // Metrics sanity.
        assert!(
            run.ascent > 0.0,
            "ascent must be positive, got {}",
            run.ascent
        );
        assert!(
            run.advance_width > 0.0,
            "advance_width must be positive, got {}",
            run.advance_width
        );

        // Glyph x positions must be non-decreasing (monotonic pen advance).
        let mut prev_x = f32::NEG_INFINITY;
        for g in &run.glyphs {
            assert!(
                g.x >= prev_x - 1e-4,
                "x positions must be non-decreasing: {} < {}",
                g.x,
                prev_x
            );
            prev_x = g.x;
        }
    }

    #[test]
    fn shaping_is_deterministic() {
        let run1 = shape_at(24.0).expect("first shape");
        let run2 = shape_at(24.0).expect("second shape");
        assert_eq!(run1, run2, "shaping must be deterministic");
    }

    #[test]
    fn unknown_family_returns_error() {
        let families = vec!["Nonexistent".to_string()];
        let req = request("test", &families, 16.0, TextDirection::Ltr);
        let provider = default_provider();
        let store = FontFaceStore::new(&provider);
        let result = RustybuzzEngine::new(&store).shape(&req, &provider);
        assert!(result.is_err(), "unknown family must return Err");
        let msg = result.unwrap_err().message;
        assert!(
            msg.contains("no font resolved"),
            "error message should mention unresolved font, got: {msg}"
        );
    }

    #[test]
    fn letter_spacing_adds_one_gap_per_cluster_boundary() {
        let families = noto_sans();
        let provider = default_provider();
        let store = FontFaceStore::new(&provider);
        let engine = RustybuzzEngine::new(&store);
        let base_req = request("ABC", &families, 24.0, TextDirection::Ltr);
        let spaced_req = ShapeRequest {
            letter_spacing_px: 3.0,
            ..base_req.clone()
        };

        let base = engine.shape(&base_req, &provider).expect("base shape");
        let spaced = engine.shape(&spaced_req, &provider).expect("spaced shape");

        assert_eq!(base.glyphs.len(), spaced.glyphs.len());
        assert!(
            (spaced.advance_width - base.advance_width - 6.0).abs() < 0.001,
            "three clusters should add two letter-spacing gaps"
        );
        assert!(
            (spaced.glyphs[1].x - base.glyphs[1].x - 3.0).abs() < 0.001,
            "second cluster should shift by one gap"
        );
        assert!(
            (spaced.glyphs[2].x - base.glyphs[2].x - 6.0).abs() < 0.001,
            "third cluster should shift by two gaps"
        );
    }

    #[test]
    fn kerning_pair_adjustment_shifts_next_cluster_and_advance() {
        let families = noto_sans();
        let provider = default_provider();
        let store = FontFaceStore::new(&provider);
        let engine = RustybuzzEngine::new(&store);
        let base_req = request("AV", &families, 24.0, TextDirection::Ltr);
        let tight_pair = [KerningPairAdjustment {
            left: "A".to_string(),
            right: "V".to_string(),
            adjustment_px: -4.0,
        }];
        let loose_pair = [KerningPairAdjustment {
            left: "A".to_string(),
            right: "V".to_string(),
            adjustment_px: 5.0,
        }];
        let tight_req = ShapeRequest {
            kerning_pairs: &tight_pair,
            ..base_req.clone()
        };
        let loose_req = ShapeRequest {
            kerning_pairs: &loose_pair,
            ..base_req.clone()
        };

        let base = engine.shape(&base_req, &provider).expect("base shape");
        let tight = engine.shape(&tight_req, &provider).expect("tight shape");
        let loose = engine.shape(&loose_req, &provider).expect("loose shape");

        assert_eq!(base.glyphs.len(), tight.glyphs.len());
        assert_eq!(base.glyphs.len(), loose.glyphs.len());
        assert!(
            (tight.glyphs[1].x - base.glyphs[1].x + 4.0).abs() < 0.001,
            "negative adjustment should shift the second cluster left"
        );
        assert!(
            (tight.advance_width - base.advance_width + 4.0).abs() < 0.001,
            "negative adjustment should reduce advance"
        );
        assert!(
            (loose.glyphs[1].x - base.glyphs[1].x - 5.0).abs() < 0.001,
            "positive adjustment should shift the second cluster right"
        );
        assert!(
            (loose.advance_width - base.advance_width - 5.0).abs() < 0.001,
            "positive adjustment should increase advance"
        );
    }

    #[test]
    fn kerning_pair_adjustment_combines_with_letter_spacing() {
        let families = noto_sans();
        let provider = default_provider();
        let store = FontFaceStore::new(&provider);
        let engine = RustybuzzEngine::new(&store);
        let pair = [KerningPairAdjustment {
            left: "A".to_string(),
            right: "V".to_string(),
            adjustment_px: -1.25,
        }];
        let base_req = request("AV", &families, 24.0, TextDirection::Ltr);
        let adjusted_req = ShapeRequest {
            kerning_pairs: &pair,
            letter_spacing_px: 3.0,
            ..base_req.clone()
        };

        let base = engine.shape(&base_req, &provider).expect("base shape");
        let adjusted = engine
            .shape(&adjusted_req, &provider)
            .expect("adjusted shape");

        assert!(
            (adjusted.glyphs[1].x - base.glyphs[1].x - 1.75).abs() < 0.001,
            "manual pair and letter spacing should add at the cluster boundary"
        );
        assert!(
            (adjusted.advance_width - base.advance_width - 1.75).abs() < 0.001,
            "advance should include both deltas"
        );
    }

    #[test]
    fn absent_kerning_pairs_preserve_output() {
        let families = noto_sans();
        let provider = default_provider();
        let store = FontFaceStore::new(&provider);
        let engine = RustybuzzEngine::new(&store);
        let ignored_pairs = [
            KerningPairAdjustment {
                left: "A".to_string(),
                right: "V".to_string(),
                adjustment_px: f32::NAN,
            },
            KerningPairAdjustment {
                left: "V".to_string(),
                right: "A".to_string(),
                adjustment_px: -8.0,
            },
        ];
        let base_req = request("AV", &families, 24.0, TextDirection::Ltr);
        let ignored_req = ShapeRequest {
            kerning_pairs: &ignored_pairs,
            ..base_req.clone()
        };

        let base = engine.shape(&base_req, &provider).expect("base shape");
        let ignored = engine
            .shape(&ignored_req, &provider)
            .expect("ignored pair shape");

        assert_eq!(ignored, base);
    }

    #[test]
    fn fallback_all_primary_matches_single_shape() {
        // CRITICAL byte-identity guarantee: text fully covered by the primary
        // face must yield exactly ONE run identical to `shape()`.
        let families = noto_sans();
        let req = request("Hello Zenith 123!", &families, 24.0, TextDirection::Ltr);
        let provider = default_provider();
        let store = FontFaceStore::new(&provider);
        let engine = RustybuzzEngine::new(&store);

        let single = engine.shape(&req, &provider).expect("single-run shape");
        let result = engine
            .shape_with_fallback(&req, &provider)
            .expect("fallback shape");

        assert_eq!(
            result.runs.len(),
            1,
            "all-primary text must produce exactly one run"
        );
        assert_eq!(
            result.runs.first().expect("one run"),
            &single,
            "all-primary fallback run must be byte-identical to shape()"
        );
        assert!(
            result.missing_chars.is_empty(),
            "fully-covered ASCII must have no missing chars"
        );
    }

    #[test]
    fn fallback_empty_text_matches_single_shape() {
        // Degenerate empty input must still match `shape("")` (one run).
        let families = noto_sans();
        let req = request("", &families, 16.0, TextDirection::Ltr);
        let provider = default_provider();
        let store = FontFaceStore::new(&provider);
        let engine = RustybuzzEngine::new(&store);

        let single = engine.shape(&req, &provider).expect("single empty shape");
        let result = engine
            .shape_with_fallback(&req, &provider)
            .expect("fallback empty shape");
        assert_eq!(
            result.runs.len(),
            1,
            "empty text still yields one (degenerate) run"
        );
        assert_eq!(result.runs.first().expect("one run"), &single);
    }

    #[test]
    fn fallback_unknown_primary_returns_error() {
        // No resolvable primary → Err, exactly like `shape`.
        let families = vec!["Nonexistent".to_string()];
        let req = request("test", &families, 16.0, TextDirection::Ltr);
        let provider = default_provider();
        let store = FontFaceStore::new(&provider);
        let result = RustybuzzEngine::new(&store).shape_with_fallback(&req, &provider);
        assert!(result.is_err(), "unknown primary family must return Err");
    }

    #[test]
    fn fallback_is_deterministic() {
        let families = noto_sans();
        let req = request("Hi there", &families, 18.0, TextDirection::Ltr);
        let provider = default_provider();
        let store = FontFaceStore::new(&provider);
        let engine = RustybuzzEngine::new(&store);
        let a = engine.shape_with_fallback(&req, &provider).expect("a");
        let b = engine.shape_with_fallback(&req, &provider).expect("b");
        assert_eq!(a.runs, b.runs, "fallback shaping must be deterministic");
        assert_eq!(
            a.missing_chars, b.missing_chars,
            "missing_chars must be deterministic"
        );
    }

    #[test]
    fn rtl_reverses_visual_glyph_order() {
        // For a non-joining script (Latin), RTL shaping reorders glyphs to
        // visual (right-to-left) order: the RTL glyph_id sequence is the reverse
        // of the LTR one, while the total advance stays positive and equal.
        let families = noto_sans();
        let provider = default_provider();
        let store = FontFaceStore::new(&provider);
        let engine = RustybuzzEngine::new(&store);

        let ltr = engine
            .shape(
                &request("ABC", &families, 24.0, TextDirection::Ltr),
                &provider,
            )
            .expect("ltr shape");
        let rtl = engine
            .shape(
                &request("ABC", &families, 24.0, TextDirection::Rtl),
                &provider,
            )
            .expect("rtl shape");

        let ltr_ids: Vec<u16> = ltr.glyphs.iter().map(|g| g.glyph_id).collect();
        let mut rtl_ids: Vec<u16> = rtl.glyphs.iter().map(|g| g.glyph_id).collect();
        rtl_ids.reverse();
        assert_eq!(
            ltr_ids, rtl_ids,
            "RTL glyph order must be the visual reverse of LTR"
        );
        assert!(rtl.advance_width > 0.0, "RTL advance must be positive");
        assert!(
            (rtl.advance_width - ltr.advance_width).abs() < 1e-3,
            "RTL and LTR total advance must match"
        );
    }

    #[test]
    fn rtl_shaping_is_deterministic() {
        let families = noto_sans();
        let provider = default_provider();
        let store = FontFaceStore::new(&provider);
        let engine = RustybuzzEngine::new(&store);
        let req = request("Shalom", &families, 20.0, TextDirection::Rtl);
        let a = engine.shape(&req, &provider).expect("a");
        let b = engine.shape(&req, &provider).expect("b");
        assert_eq!(a, b, "RTL shaping must be deterministic");
    }

    #[test]
    fn font_size_scaling_proportional() {
        let run24 = shape_at(24.0).expect("24px");
        let run48 = shape_at(48.0).expect("48px");

        // Ascent doubles (~2×) when font_size doubles.
        let ratio_ascent = run48.ascent / run24.ascent;
        assert!(
            (ratio_ascent - 2.0).abs() < 0.01,
            "ascent ratio should be ~2.0, got {ratio_ascent}"
        );

        // advance_width also doubles (~2×).
        let ratio_adv = run48.advance_width / run24.advance_width;
        assert!(
            (ratio_adv - 2.0).abs() < 0.01,
            "advance_width ratio should be ~2.0, got {ratio_adv}"
        );
    }

    // ── Face cache ───────────────────────────────────────────────────────────

    /// Text with one char no bundled face covers (plane-16 private use).
    const UNCOVERED: &str = "A\u{10FFFD}B";

    /// Shape `text` through `shaper` with both trait methods.
    fn shape_both(
        shaper: &dyn TextLayoutEngine,
        provider: &dyn FontProvider,
        text: &str,
        direction: TextDirection,
        letter_spacing_px: f32,
    ) -> (ZenithGlyphRun, Vec<ZenithGlyphRun>, Vec<char>) {
        let families = noto_sans();
        let req = ShapeRequest {
            letter_spacing_px,
            ..request(text, &families, 18.0, direction)
        };
        let single = shaper.shape(&req, provider).expect("shape");
        let fallback = shaper
            .shape_with_fallback(&req, provider)
            .expect("fallback shape");
        (single, fallback.runs, fallback.missing_chars)
    }

    #[test]
    fn cached_shaping_matches_per_call_parsing() {
        let provider = default_provider();
        let store = FontFaceStore::new(&provider);
        let cached = CachedShaper::new(&store);
        // An empty store parses every face per call, as before the cache.
        let empty = FontFaceStore::from_faces(Vec::new());
        let per_call = CachedShaper::new(&empty);

        for text in ["Hello Zenith", "", UNCOVERED, "Shalom 123"] {
            for direction in [TextDirection::Ltr, TextDirection::Rtl] {
                for spacing in [0.0, 2.5] {
                    let a = shape_both(&cached, &provider, text, direction, spacing);
                    let b = shape_both(&per_call, &provider, text, direction, spacing);
                    assert_eq!(a, b, "cached output differs for {text:?} {direction:?}");
                }
            }
        }
        assert_eq!(
            per_call.faces.parsed_count(),
            0,
            "empty store caches nothing"
        );
    }

    #[test]
    fn fallback_faces_parse_only_when_needed() {
        let provider = default_provider();
        let store = FontFaceStore::new(&provider);
        let shaper = CachedShaper::new(&store);

        let _ = shape_both(&shaper, &provider, "Hello", TextDirection::Ltr, 0.0);
        assert_eq!(
            shaper.faces.parsed_count(),
            1,
            "covered text parses only the primary"
        );

        let (_, runs, missing) = shape_both(&shaper, &provider, UNCOVERED, TextDirection::Ltr, 0.0);
        assert_eq!(missing, vec!['\u{10FFFD}']);
        assert_eq!(runs.len(), 1, "an uncovered char stays with the primary");
        assert_eq!(
            shaper.faces.parsed_count(),
            store.face_count(),
            "an uncovered char probes every fallback face"
        );
    }

    #[test]
    fn repeated_shaping_does_not_reparse() {
        let provider = default_provider();
        let store = FontFaceStore::new(&provider);
        let shaper = CachedShaper::new(&store);
        let primary = provider
            .by_id("noto-sans-400-normal")
            .and_then(|data| store.slot_of(&data))
            .expect("primary is in the store");

        let first = shape_both(&shaper, &provider, "Hello", TextDirection::Ltr, 0.0);
        let face_before: *const rustybuzz::Face<'_> =
            shaper.faces.face(primary).expect("primary parsed");
        for _ in 0..5 {
            let again = shape_both(&shaper, &provider, "Hello", TextDirection::Ltr, 0.0);
            assert_eq!(again, first);
        }
        let face_after: *const rustybuzz::Face<'_> =
            shaper.faces.face(primary).expect("primary parsed");
        assert!(std::ptr::eq(face_before, face_after), "the face is reused");
        assert_eq!(shaper.faces.parsed_count(), 1);
    }

    #[test]
    fn store_from_other_provider_still_shapes_correctly() {
        // A store over different allocations never serves this provider.
        let provider = default_provider();
        let other = default_provider();
        let foreign = FontFaceStore::new(&other);
        let shaper = CachedShaper::new(&foreign);
        let own = FontFaceStore::new(&provider);
        let cached = CachedShaper::new(&own);

        for text in ["Hello", UNCOVERED] {
            let a = shape_both(&shaper, &provider, text, TextDirection::Ltr, 0.0);
            let b = shape_both(&cached, &provider, text, TextDirection::Ltr, 0.0);
            assert_eq!(a, b, "foreign store output differs for {text:?}");
        }
        assert_eq!(shaper.faces.parsed_count(), 0, "foreign faces never parse");
    }

    #[test]
    fn primary_parse_error_is_reported() {
        let bad: std::sync::Arc<[u8]> = std::sync::Arc::from(vec![0u8; 16].as_slice());
        let mut provider = BytesFontProvider::new();
        provider.register(
            "Broken",
            400,
            FontStyle::Normal,
            bad,
            0,
            zenith_core::FontSource::Project,
        );
        let store = FontFaceStore::new(&provider);
        let engine = RustybuzzEngine::new(&store);
        let families = vec!["Broken".to_string()];
        let req = request("x", &families, 12.0, TextDirection::Ltr);

        let single = engine.shape(&req, &provider).expect_err("bad bytes");
        let Err(fallback) = engine.shape_with_fallback(&req, &provider) else {
            panic!("bad bytes must fail fallback shaping");
        };
        let expected = "failed to parse font face for 'broken-400-normal' (index 0)";
        assert_eq!(single.message, expected);
        assert_eq!(fallback.message, expected);
    }
}
