//! Public bidi, line-break, and fallback-shaping API over real fonts.

use std::sync::Arc;

use zenith_core::{BytesFontProvider, FontStyle, default_provider};
use zenith_layout::{
    FontFaceStore, RustybuzzEngine, ShapeRequest, TextDirection, TextLayoutEngine, bidi_levels,
    detect_direction, ideographic_breaks, level_runs, visual_order,
};

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

/// Each run's source text, left to right, for `text` shaped from `direction`.
fn shaped_runs(text: &str, direction: TextDirection) -> Vec<(String, String)> {
    let provider = provider();
    let store = FontFaceStore::new(&provider);
    let engine = RustybuzzEngine::new(&store);
    let families = vec!["Noto Sans".to_owned()];
    let req = ShapeRequest {
        text,
        families: &families,
        weight: 400,
        style: FontStyle::Normal,
        font_size: 16.0,
        direction,
        features: &[],
        kerning_pairs: &[],
        letter_spacing_px: 0.0,
    };
    let result = engine
        .shape_with_fallback(&req, &provider)
        .expect("fallback shape");
    assert!(
        result.missing_chars.is_empty(),
        "{:?}",
        result.missing_chars
    );
    result
        .runs
        .into_iter()
        .map(|run| {
            let text = run.glyphs.iter().map(|g| g.text.as_str()).collect();
            (run.font_id, text)
        })
        .collect()
}

#[test]
fn base_direction_comes_from_the_first_strong_char() {
    assert_eq!(detect_direction("— مرحبا"), Some(TextDirection::Rtl));
    assert_eq!(detect_direction("Hi שלום"), Some(TextDirection::Ltr));
    assert_eq!(detect_direction("123"), None);
}

#[test]
fn mixed_paragraph_resolves_three_levels() {
    let text = "שלום abc עולם 12";
    let levels = bidi_levels(text, TextDirection::Rtl);
    let runs: Vec<u8> = level_runs(text, &levels, 1)
        .into_iter()
        .map(|(_, _, level)| level)
        .collect();
    assert_eq!(runs, vec![1, 2, 1, 2]);
    // Visual order: the LTR runs swap places with the RTL ones around them.
    assert_eq!(visual_order(&runs), vec![3, 2, 1, 0]);
}

#[test]
fn hebrew_word_in_latin_text_shapes_right_to_left() {
    let runs = shaped_runs("say שלום now", TextDirection::Ltr);
    let texts: Vec<&str> = runs.iter().map(|(_, t)| t.as_str()).collect();
    assert_eq!(texts, vec!["say ", "םולש", " now"]);
    assert_eq!(runs[1].0, "noto-sans-hebrew-400-normal");
}

#[test]
fn arabic_paragraph_lists_runs_right_to_left() {
    let runs = shaped_runs("مرحبا بالعالم", TextDirection::Rtl);
    let texts: Vec<&str> = runs.iter().map(|(_, t)| t.as_str()).collect();
    // The second word is leftmost.
    assert_eq!(texts.first().copied(), Some("ملاعلاب"));
    assert_eq!(texts.last().copied(), Some("ابحرم"));
}

#[test]
fn cjk_break_opportunities_follow_kinsoku() {
    // UAX #14 keeps `。「` together: closing then opening punctuation.
    let text = "语言。「汉字」好";
    let pieces: Vec<&str> = {
        let mut out = Vec::new();
        let mut start = 0;
        for b in ideographic_breaks(text) {
            out.push(&text[start..b]);
            start = b;
        }
        out.push(&text[start..]);
        out
    };
    assert_eq!(pieces, vec!["语", "言。「汉", "字」", "好"]);
}

#[test]
fn emoji_with_skin_tone_and_zwj_stay_in_one_run() {
    for text in ["👍🏽", "👨\u{200D}👩\u{200D}👧", "🏳\u{FE0F}\u{200D}🌈"] {
        let runs = shaped_runs(text, TextDirection::Ltr);
        assert_eq!(runs.len(), 1, "{text:?} split into {runs:?}");
        assert_eq!(runs[0].0, "noto-emoji-400-normal");
    }
}
