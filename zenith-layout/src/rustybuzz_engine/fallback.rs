//! Per-char font fallback over one face cache.

use std::collections::BTreeSet;

use zenith_core::FontProvider;

use super::face_cache::{FaceCache, FontFaceStore, parse_error};
use super::run::{FaceShapeRequest, shape_run_with_face};
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
            | 0xE0100..=0xE01EF // variation selectors supplement
        )
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

    // ── 1. Itemize text into contiguous sub-runs by chosen face slot ──────
    // For each char: prefer the primary when it covers the char; otherwise
    // the FIRST fallback face that covers it; otherwise the primary (so it
    // shapes as .notdef / tofu). Consecutive chars with the same chosen face
    // merge. Sub-run boundaries are byte ranges into `req.text` so the exact
    // substring is shaped (and reported as the run's source text). Chars no
    // face covers are recorded in `missing` (unless ignorable).
    let mut missing: BTreeSet<char> = BTreeSet::new();
    let mut pending = Some(fallbacks);
    let mut fallback_slots: Vec<usize> = Vec::new();

    // (face_slot, byte_start, byte_end) per sub-run, in text order.
    let mut segments: Vec<(usize, usize, usize)> = Vec::new();
    for (byte_off, ch) in req.text.char_indices() {
        let slot = if primary_face.glyph_index(ch).is_some() {
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
                Some(slot) => slot,
                None => {
                    if !is_ignorable_for_coverage(ch) {
                        missing.insert(ch);
                    }
                    primary
                }
            }
        };
        let ch_end = byte_off + ch.len_utf8();
        match segments.last_mut() {
            Some((last_slot, _, last_end)) if *last_slot == slot => {
                *last_end = ch_end;
            }
            _ => segments.push((slot, byte_off, ch_end)),
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
    // concatenated left-to-right by the caller, so for RTL the FIRST logical
    // segment must sit rightmost: reverse the emission order. A single
    // segment (the common all-primary case) is unaffected, and LTR keeps
    // logical order — so both the LTR path and a single-run RTL word stay
    // byte-identical.
    if req.direction == TextDirection::Rtl {
        segments.reverse();
    }
    let mut runs: Vec<ZenithGlyphRun> = Vec::with_capacity(segments.len());
    for (slot, start, end) in segments {
        let data = store.data(slot).ok_or_else(|| {
            LayoutError::new("internal: chosen face slot out of range".to_owned())
        })?;
        let face = cache.face(slot).ok_or_else(|| parse_error(data))?;
        let sub_text = req.text.get(start..end).ok_or_else(|| {
            LayoutError::new("internal: sub-run byte range out of bounds".to_owned())
        })?;
        let mut run = shape_run_with_face(
            FaceShapeRequest::from_shape_request(req, face, sub_text, data.id.clone())
                .with_plans(cache, slot),
        )?;
        if letter_spacing_px != 0.0 {
            run.advance_width += letter_spacing_px;
        }
        runs.push(run);
    }
    if letter_spacing_px != 0.0
        && let Some(last) = runs.last_mut()
    {
        last.advance_width -= letter_spacing_px;
    }

    Ok(Some(FallbackResult {
        runs,
        missing_chars: missing.into_iter().collect(),
    }))
}
