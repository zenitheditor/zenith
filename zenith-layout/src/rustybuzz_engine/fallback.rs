//! Per-char font fallback over one face cache.

use std::collections::BTreeSet;

use zenith_core::{FontProvider, FontSource};

use super::face_cache::{FaceCache, FontFaceStore, parse_error};
use super::run::{FaceShapeRequest, shape_run_with_face};
use crate::bidi::{base_level, bidi_levels, level_direction, level_runs, visual_order};
use crate::engine::{FallbackResult, ShapeRequest, TextDirection, ZenithGlyphRun};
use crate::error::LayoutError;

/// Code points that legitimately have no standalone glyph (consumed during
/// shaping) and must NOT be reported as missing: control/whitespace and the
/// Unicode default-ignorable ranges (joiners, bidi marks, variation selectors,
/// BOM, soft hyphen, word joiner, etc.).
fn is_ignorable_for_coverage(ch: char) -> bool {
    ch.is_control()
        || ch.is_whitespace()
        || matches!(
            ch as u32,
            0x00AD            // soft hyphen
            | 0x200B..=0x200F // ZWSP, ZWNJ, ZWJ, LRM, RLM
            | 0x202A..=0x202E // bidi embeddings/overrides
            | 0x2060..=0x206F // word joiner, invisible operators, deprecated format
            | 0xFEFF          // BOM / ZWNBSP
            | 0xFE00..=0xFE0F // variation selectors
            | 0xE0000..=0xE007F // tags (emoji subdivision flags)
            | 0xE0100..=0xE01EF // variation selectors supplement
        )
}

/// Whether `ch` extends the cluster before it: a combining mark, a joiner,
/// a variation selector, an emoji skin-tone modifier, or a tag.
///
/// An extender stays in the face of the char before it, so an emoji ZWJ
/// sequence or a base letter with its marks shapes as one cluster.
fn extends_cluster(ch: char) -> bool {
    matches!(ch as u32, 0x200C | 0x200D | 0x1F3FB..=0x1F3FF | 0xE0020..=0xE007F)
        || unicode_bidi::bidi_class(ch) == unicode_bidi::BidiClass::NSM
}

/// Fallback faces for one call, in priority order.
pub(super) enum FallbackSlots<'p> {
    /// Store slots known up front.
    Ready(Vec<usize>),
    /// `provider.all_faces()` order without the primary id, read on first need.
    Provider {
        provider: &'p dyn FontProvider,
        primary_id: &'p str,
    },
}

impl FallbackSlots<'_> {
    /// Map the fallback faces to slots in `store`.
    ///
    /// Returns `None` when a provider face is not held in `store`.
    fn resolve(self, store: &FontFaceStore) -> Option<Vec<usize>> {
        match self {
            Self::Ready(slots) => Some(slots),
            Self::Provider {
                provider,
                primary_id,
            } => provider
                .all_faces()
                .iter()
                .filter(|data| data.id != primary_id)
                .map(|data| store.slot_of(data))
                .collect(),
        }
    }
}

/// Shape `req` with per-char fallback over the faces in `cache`.
///
/// `primary` is the primary face slot. Fallback faces parse only when a char
/// is not covered by the primary or an earlier fallback face.
///
/// Each char that goes to a fallback face, or to no face, is noted on
/// `notes` ([`FontProvider::note_fallback_glyphs`]) once shaping succeeds.
///
/// Returns `Ok(None)` when a needed fallback face is not in the cache's store.
/// The caller then retries with a store built for this call.
///
/// # Errors
///
/// Returns `LayoutError` if the primary face fails to parse or a face reports
/// `units_per_em <= 0`.
pub(super) fn shape_with_fallback_in(
    cache: &FaceCache<'_>,
    primary: usize,
    fallbacks: FallbackSlots<'_>,
    req: &ShapeRequest<'_>,
    notes: &dyn FontProvider,
) -> Result<Option<FallbackResult>, LayoutError> {
    let store = cache.store();
    let primary_data = store
        .data(primary)
        .ok_or_else(|| LayoutError::new("internal: primary face slot out of range".to_owned()))?;
    let primary_face = cache
        .face(primary)
        .ok_or_else(|| parse_error(primary_data))?;

    let letter_spacing_px = if req.letter_spacing_px.is_finite() {
        req.letter_spacing_px
    } else {
        0.0
    };

    // ── 1. Itemize text into sub-runs by bidi level, then by face slot ────
    // Bidi first: `req.direction` is the paragraph direction, and each
    // maximal same-level byte range shapes in its own direction. Latin text
    // in an LTR request is one level-0 range, so it itemizes as before.
    //
    // Then per char: prefer the primary when it covers the char; otherwise
    // the FIRST fallback face that covers it; otherwise the primary (so it
    // shapes as .notdef / tofu). A cluster extender stays in the face of the
    // char before it. Consecutive chars with the same chosen face merge.
    // Sub-run boundaries are byte ranges into `req.text` so the exact
    // substring is shaped (and reported as the run's source text). Chars no
    // face covers are recorded in `missing` (unless ignorable).
    let mut missing: BTreeSet<char> = BTreeSet::new();
    // Chars given to a fallback face (its slot) or to no face (`None`).
    let mut fallback_glyphs: BTreeSet<(char, Option<usize>)> = BTreeSet::new();
    let mut pending = Some(fallbacks);
    let mut fallback_slots: Vec<usize> = Vec::new();

    let base = base_level(req.direction);
    let levels = bidi_levels(req.text, req.direction);
    let level_ranges = level_runs(req.text, &levels, base);

    // (face_slot, byte_start, byte_end) per sub-run, in text order, and the
    // index into `level_ranges` of the level range each sub-run belongs to.
    let mut segments: Vec<(usize, usize, usize)> = Vec::new();
    let mut segment_range: Vec<usize> = Vec::new();
    for (range_idx, &(range_start, range_end, _)) in level_ranges.iter().enumerate() {
        let range_text = req.text.get(range_start..range_end).unwrap_or("");
        let first_segment = segments.len();
        for (rel_off, ch) in range_text.char_indices() {
            let byte_off = range_start + rel_off;
            let sticky = segments
                .get(first_segment..)
                .and_then(<[_]>::last)
                .map(|&(slot, _, _)| slot)
                .filter(|&slot| {
                    extends_cluster(ch)
                        && (is_ignorable_for_coverage(ch)
                            || cache
                                .face(slot)
                                .is_some_and(|face| face.glyph_index(ch).is_some()))
                });
            let slot = if let Some(slot) = sticky {
                slot
            } else if primary_face.glyph_index(ch).is_some() {
                primary
            } else {
                // The fallback list is read once, on the first uncovered char.
                if let Some(source) = pending.take() {
                    match source.resolve(store) {
                        Some(slots) => fallback_slots = slots,
                        None => return Ok(None),
                    }
                }
                // Coverage uses ttf-parser's `Face::glyph_index`, exposed on
                // `rustybuzz::Face` via Deref. Each probe parses at most once.
                let covering = fallback_slots.iter().copied().find(|&slot| {
                    cache
                        .face(slot)
                        .is_some_and(|face| face.glyph_index(ch).is_some())
                });
                match covering {
                    Some(slot) => {
                        if store
                            .data(slot)
                            .is_some_and(|data| data.source == FontSource::Fallback)
                        {
                            fallback_glyphs.insert((ch, Some(slot)));
                        }
                        slot
                    }
                    None => {
                        if !is_ignorable_for_coverage(ch) {
                            missing.insert(ch);
                            fallback_glyphs.insert((ch, None));
                        }
                        primary
                    }
                }
            };
            let ch_end = byte_off + ch.len_utf8();
            match segments.get_mut(first_segment..).and_then(<[_]>::last_mut) {
                Some((last_slot, _, last_end)) if *last_slot == slot => {
                    *last_end = ch_end;
                }
                _ => {
                    segments.push((slot, byte_off, ch_end));
                    segment_range.push(range_idx);
                }
            }
        }
    }

    // Empty text → no segments; shape the empty string with the primary so
    // a (degenerate but valid) run with primary metrics is still returned,
    // matching `shape("")`.
    if segments.is_empty() {
        let run = shape_run_with_face(
            FaceShapeRequest::from_shape_request(
                req,
                primary_face,
                req.text,
                primary_data.id.clone(),
            )
            .with_plans(cache, primary),
        )?;
        return Ok(Some(FallbackResult {
            runs: vec![run],
            missing_chars: missing.into_iter().collect(),
        }));
    }

    // ── 2. Shape each sub-run with its chosen face ────────────────────────
    // The all-primary case is exactly one segment spanning the whole text →
    // a single run byte-identical to `shape`, because both call
    // `shape_run_with_face` with the same face, text, id, and size.
    //
    // Segments are itemized in LOGICAL (text) order. The returned runs are
    // concatenated left to right by the caller. Inside one RTL level range
    // the first logical segment sits rightmost, so that range reverses. The
    // level ranges then take their visual order (UAX #9 rule L2). One level
    // range at the base level (all Latin LTR text, or all-RTL text in an RTL
    // request) keeps the order it had before bidi itemization.
    let range_levels: Vec<u8> = level_ranges.iter().map(|&(_, _, level)| level).collect();
    let mut runs: Vec<ZenithGlyphRun> = Vec::with_capacity(segments.len());
    for range_idx in visual_order(&range_levels) {
        let level = range_levels.get(range_idx).copied().unwrap_or(base);
        let direction = level_direction(level);
        let mut in_range: Vec<(usize, usize, usize)> = segments
            .iter()
            .zip(&segment_range)
            .filter(|&(_, &r)| r == range_idx)
            .map(|(&segment, _)| segment)
            .collect();
        if direction == TextDirection::Rtl {
            in_range.reverse();
        }
        for (slot, start, end) in in_range {
            let data = store.data(slot).ok_or_else(|| {
                LayoutError::new("internal: chosen face slot out of range".to_owned())
            })?;
            let face = cache.face(slot).ok_or_else(|| parse_error(data))?;
            let sub_text = req.text.get(start..end).ok_or_else(|| {
                LayoutError::new("internal: sub-run byte range out of bounds".to_owned())
            })?;
            let mut run = shape_run_with_face(
                FaceShapeRequest::from_shape_request(req, face, sub_text, data.id.clone())
                    .with_plans(cache, slot)
                    .with_direction(direction),
            )?;
            if letter_spacing_px != 0.0 {
                run.advance_width += letter_spacing_px;
            }
            runs.push(run);
        }
    }
    if letter_spacing_px != 0.0
        && let Some(last) = runs.last_mut()
    {
        last.advance_width -= letter_spacing_px;
    }

    if !fallback_glyphs.is_empty() {
        let noted: Vec<(char, Option<&str>)> = fallback_glyphs
            .iter()
            .map(|&(ch, slot)| {
                let id = slot
                    .and_then(|slot| store.data(slot))
                    .map(|d| d.id.as_str());
                (ch, id)
            })
            .collect();
        notes.note_fallback_glyphs(&noted);
    }

    Ok(Some(FallbackResult {
        runs,
        missing_chars: missing.into_iter().collect(),
    }))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use zenith_core::{BytesFontProvider, FontStyle, default_provider};

    use crate::engine::{FallbackResult, ShapeRequest, TextDirection, TextLayoutEngine};
    use crate::rustybuzz_engine::{FontFaceStore, RustybuzzEngine};

    use super::extends_cluster;

    /// The bundled faces plus the embedded fallback chain.
    fn provider() -> BytesFontProvider {
        let mut provider = default_provider();
        for face in zenith_fonts_fallback::fallback_faces() {
            if let Some(bytes) = face.bytes {
                provider.register_fallback(
                    face.family,
                    face.weight,
                    FontStyle::Normal,
                    Arc::from(bytes),
                    0,
                );
            }
        }
        provider
    }

    fn shape(text: &str, direction: TextDirection) -> FallbackResult {
        let provider = provider();
        let store = FontFaceStore::new(&provider);
        let engine = RustybuzzEngine::new(&store);
        let families = vec!["Noto Sans".to_owned()];
        let req = ShapeRequest {
            text,
            families: &families,
            weight: 400,
            style: FontStyle::Normal,
            font_size: 20.0,
            direction,
            features: &[],
            kerning_pairs: &[],
            letter_spacing_px: 0.0,
        };
        engine
            .shape_with_fallback(&req, &provider)
            .expect("fallback shape")
    }

    /// Each run's source text, in visual order.
    fn run_texts(result: &FallbackResult) -> Vec<String> {
        result
            .runs
            .iter()
            .map(|run| run.glyphs.iter().map(|g| g.text.as_str()).collect())
            .collect()
    }

    #[test]
    fn hebrew_inside_ltr_shapes_rtl() {
        let result = shape("ab שלום", TextDirection::Ltr);
        let texts = run_texts(&result);
        assert_eq!(texts.first().map(String::as_str), Some("ab "));
        // The RTL run lists its glyphs right to left: the last letter first.
        assert_eq!(texts.get(1).map(String::as_str), Some("םולש"));
        assert!(result.missing_chars.is_empty());
    }

    #[test]
    fn latin_inside_rtl_shapes_ltr_and_sits_left() {
        let result = shape("שלום abc", TextDirection::Rtl);
        let texts = run_texts(&result);
        // Visual order: the LTR word on the left, then the space and Hebrew.
        assert_eq!(texts.first().map(String::as_str), Some("abc"));
        assert_eq!(texts.last().map(String::as_str), Some("םולש"));
    }

    #[test]
    fn digits_inside_rtl_keep_their_order() {
        let result = shape("السعر 25", TextDirection::Rtl);
        let texts = run_texts(&result);
        assert!(texts.iter().any(|t| t == "25"), "{texts:?}");
    }

    #[test]
    fn emoji_zwj_sequence_is_one_run() {
        let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
        let result = shape(family, TextDirection::Ltr);
        assert_eq!(
            result.runs.len(),
            1,
            "the ZWJ joins stay with the emoji face"
        );
        assert!(result.missing_chars.is_empty());
    }

    #[test]
    fn cluster_extenders_are_marks_joiners_and_modifiers() {
        assert!(extends_cluster('\u{200D}'));
        assert!(extends_cluster('\u{FE0F}'));
        assert!(extends_cluster('\u{1F3FD}'));
        assert!(extends_cluster('\u{0301}'));
        assert!(extends_cluster('\u{064E}'), "Arabic fatha is a mark");
        assert!(!extends_cluster('a'));
        assert!(!extends_cluster(' '));
    }

    #[test]
    fn fallback_and_missing_chars_are_noted_on_the_provider() {
        let log = zenith_core::FontMissLog::new();
        let provider = provider().with_miss_log(log.clone());
        let store = FontFaceStore::new(&provider);
        let engine = RustybuzzEngine::new(&store);
        let families = vec!["Noto Sans".to_owned()];
        let req = ShapeRequest {
            text: "a \u{0639}\u{1F600}\u{E000}",
            families: &families,
            weight: 400,
            style: FontStyle::Normal,
            font_size: 20.0,
            direction: TextDirection::Ltr,
            features: &[],
            kerning_pairs: &[],
            letter_spacing_px: 0.0,
        };
        engine
            .shape_with_fallback(&req, &provider)
            .expect("fallback shape");
        let got: Vec<(char, Option<String>)> = log
            .fallback_glyphs()
            .into_iter()
            .map(|g| (g.ch, g.face_family))
            .collect();
        assert_eq!(
            got,
            vec![
                ('\u{0639}', Some("noto sans arabic".to_owned())),
                ('\u{E000}', None),
                ('\u{1F600}', Some("noto emoji".to_owned())),
            ]
        );
    }

    #[test]
    fn latin_ltr_output_is_unchanged_by_bidi() {
        let result = shape("Hello, world 42!", TextDirection::Ltr);
        assert_eq!(result.runs.len(), 1);
        assert_eq!(run_texts(&result), vec!["Hello, world 42!".to_owned()]);
    }
}
