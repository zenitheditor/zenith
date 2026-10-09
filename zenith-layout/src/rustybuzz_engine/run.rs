//! Single-face shaping: one parsed face and one text run in, one glyph run out.

use crate::engine::{
    FontFeature, KerningPairAdjustment, PositionedGlyph, ShapeRequest, TextDirection,
    ZenithGlyphRun,
};
use crate::error::LayoutError;

use super::face_cache::{FaceCache, Segment};

/// One shaping call against an already-parsed face.
///
/// `'r` is the request borrow. `'f` is the font-bytes borrow inside the face.
pub(super) struct FaceShapeRequest<'r, 'f> {
    face: &'r rustybuzz::Face<'f>,
    text: &'r str,
    font_id: String,
    font_size: f32,
    direction: TextDirection,
    features: &'r [FontFeature],
    kerning_pairs: &'r [KerningPairAdjustment],
    letter_spacing_px: f32,
    /// The face cache and slot `face` came from, for cached shape plans.
    plans: Option<(&'r FaceCache<'f>, usize)>,
}

impl<'r, 'f> FaceShapeRequest<'r, 'f> {
    /// Build a face request that shapes `text` with every other field from `req`.
    pub(super) fn from_shape_request(
        req: &'r ShapeRequest<'_>,
        face: &'r rustybuzz::Face<'f>,
        text: &'r str,
        font_id: String,
    ) -> Self {
        Self {
            face,
            text,
            font_id,
            font_size: req.font_size,
            direction: req.direction,
            features: req.features,
            kerning_pairs: req.kerning_pairs,
            letter_spacing_px: req.letter_spacing_px,
            plans: None,
        }
    }

    /// This request with `face` known as slot `slot` of `cache`: shape
    /// plans come from the cache.
    pub(super) fn with_plans(mut self, cache: &'r FaceCache<'f>, slot: usize) -> Self {
        self.plans = Some((cache, slot));
        self
    }
}

/// Shape `text` with an already-parsed `face` and produce a single
/// [`ZenithGlyphRun`] tagged with `font_id`.
///
/// This is the one place shaping, scaling, and metric derivation live, so
/// `shape` (single-face) and `shape_with_fallback` (per-glyph fallback)
/// cannot diverge: both route every run through here. Glyphs are positioned
/// from `x = 0` within the run.
///
/// # Errors
///
/// Returns `LayoutError` if the face reports `units_per_em <= 0`.
pub(super) fn shape_run_with_face(
    req: FaceShapeRequest<'_, '_>,
) -> Result<ZenithGlyphRun, LayoutError> {
    // ── Compute pixel scale ───────────────────────────────────────────────
    // `units_per_em` comes from the `ttf_parser::Face` trait exposed by
    // `rustybuzz::Face` via Deref.
    let units_per_em = req.face.units_per_em();
    if units_per_em <= 0 {
        return Err(LayoutError::new(format!(
            "font '{}' reports units_per_em = {units_per_em}",
            req.font_id
        )));
    }
    // `units_per_em` is a positive `i32` (guarded above); the OTF spec
    // range (16–16384) is exactly representable as `f32`.
    let scale = req.font_size / units_per_em as f32;

    // ── Derive line metrics ───────────────────────────────────────────────
    // `ascender` and `descender` are in font units; descender is negative.
    let ascent = f32::from(req.face.ascender()) * scale;
    let descent = -(f32::from(req.face.descender()) * scale); // store positive magnitude
    let line_gap = f32::from(req.face.line_gap()) * scale;
    let line_height = ascent + descent + line_gap;

    // ── Shape the text ────────────────────────────────────────────────────
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(req.text);
    // RTL sets the buffer direction so rustybuzz reorders glyphs to visual
    // order and applies RTL-correct joining (Arabic, Hebrew). The run's
    // advance + glyph pen positions stay left-to-right, so a word emitted
    // at its left x renders correctly; LTR is the default (unchanged).
    buffer.set_direction(match req.direction {
        TextDirection::Ltr => rustybuzz::Direction::LeftToRight,
        TextDirection::Rtl => rustybuzz::Direction::RightToLeft,
    });

    let features = rustybuzz_features(req.features);
    // `rustybuzz::shape` guesses the segment properties, builds a plan from
    // them, and shapes with it. A stored face reuses the plan of equal
    // properties. A buffer with no script keeps the plain call: the plan
    // then gets no script, which `UnicodeBuffer::script` cannot express.
    buffer.guess_segment_properties();
    let script = buffer.script();
    let language = buffer.language();
    let plan = match req.plans {
        Some((cache, slot)) if script != rustybuzz::script::UNKNOWN => cache.plan(
            slot,
            &Segment {
                direction: buffer.direction(),
                script,
                language: language.as_ref(),
                features: &features,
            },
        ),
        Some(_) | None => None,
    };
    let glyph_buffer = match plan {
        Some(plan) => rustybuzz::shape_with_plan(req.face, &plan, buffer),
        None => rustybuzz::shape(req.face, &features, buffer),
    };

    let infos = glyph_buffer.glyph_infos();
    let positions = glyph_buffer.glyph_positions();

    // ── Cluster → source-text boundaries ──────────────────────────────────
    // Each glyph carries `cluster`: the byte offset into `text` it derives
    // from. The sorted, deduplicated set of cluster offsets gives the source
    // substring boundaries: a cluster starting at offset `c` spans up to the
    // next greater offset (or `text.len()`). The FIRST glyph of each cluster
    // carries that whole substring (so a ligature's single glyph maps to all
    // its chars); later glyphs of the same cluster carry the empty string (so
    // a one-char→many-glyph decomposition is not duplicated). This per-glyph
    // Unicode mapping is what the PDF backend turns into a ToUnicode CMap.
    let mut boundaries: Vec<u32> = infos.iter().map(|i| i.cluster).collect();
    boundaries.sort_unstable();
    boundaries.dedup();
    let cluster_text = |cluster: u32| -> &str {
        let start = cluster as usize;
        let end = match boundaries.binary_search(&cluster) {
            Ok(i) => boundaries
                .get(i + 1)
                .map_or(req.text.len(), |&b| b as usize),
            // A cluster value not in the set cannot happen (it was collected
            // from the same infos); fall back to a single source char span.
            Err(_) => req.text.len(),
        };
        req.text.get(start..end).unwrap_or("")
    };

    // ── Build glyph list ──────────────────────────────────────────────────
    let mut glyphs: Vec<PositionedGlyph> = Vec::with_capacity(infos.len());
    let mut pen_x: f32 = 0.0;
    let mut pen_y: f32 = 0.0;
    let mut prev_cluster: Option<u32> = None;

    let letter_spacing_px = if req.letter_spacing_px.is_finite() {
        req.letter_spacing_px
    } else {
        0.0
    };
    let mut letter_spacing_offset: f32 = 0.0;
    let mut kerning_offset: f32 = 0.0;
    for (glyph_index, (info, pos)) in infos.iter().zip(positions.iter()).enumerate() {
        // glyph_id is u32 in rustybuzz; OTF glyph IDs fit in u16 (max 65535).
        // A value above u16::MAX indicates a malformed font — map it to the
        // .notdef glyph (0) rather than silently truncating.
        let glyph_id = u16::try_from(info.glyph_id).unwrap_or(0);

        let mut x = pen_x + letter_spacing_offset + pos.x_offset as f32 * scale;
        if kerning_offset != 0.0 {
            x += kerning_offset;
        }
        // y_offset is in font units; positive = up in font coords → negative screen y.
        let y = pen_y - pos.y_offset as f32 * scale;

        // First glyph of a new cluster carries the source text; repeats are empty.
        let glyph_text = if prev_cluster == Some(info.cluster) {
            String::new()
        } else {
            cluster_text(info.cluster).to_string()
        };
        prev_cluster = Some(info.cluster);

        glyphs.push(PositionedGlyph {
            glyph_id,
            x,
            y,
            text: glyph_text,
        });

        pen_x += pos.x_advance as f32 * scale;
        pen_y += pos.y_advance as f32 * scale;
        if let Some(next) = infos.get(glyph_index + 1)
            && next.cluster != info.cluster
        {
            if !req.kerning_pairs.is_empty() {
                let left = cluster_text(info.cluster);
                let right = cluster_text(next.cluster);
                kerning_offset += kerning_pair_adjustment(left, right, req.kerning_pairs);
            }
            letter_spacing_offset += letter_spacing_px;
        }
    }

    let mut advance_width = pen_x + letter_spacing_offset;
    if kerning_offset != 0.0 {
        advance_width += kerning_offset;
    }

    Ok(ZenithGlyphRun {
        font_id: req.font_id,
        font_size: req.font_size,
        ascent,
        descent,
        line_height,
        advance_width,
        glyphs,
    })
}

/// Sum of the finite adjustments of every pair that matches `left` and `right`.
fn kerning_pair_adjustment(
    left: &str,
    right: &str,
    kerning_pairs: &[KerningPairAdjustment],
) -> f32 {
    kerning_pairs
        .iter()
        .filter(|pair| pair.adjustment_px.is_finite() && pair.left == left && pair.right == right)
        .map(|pair| pair.adjustment_px)
        .sum()
}

/// Convert Zenith features to `rustybuzz` features, skipping any that fail to parse.
fn rustybuzz_features(features: &[FontFeature]) -> Vec<rustybuzz::Feature> {
    let mut shaped_features = Vec::with_capacity(features.len());
    for feature in features {
        let tag = feature.tag();
        let Ok(tag) = std::str::from_utf8(&tag) else {
            continue;
        };
        let spec = format!("{}={}", tag, feature.value());
        if let Ok(feature) = spec.parse::<rustybuzz::Feature>() {
            shaped_features.push(feature);
        }
    }

    shaped_features
}
