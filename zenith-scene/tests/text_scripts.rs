//! Mixed-direction text, CJK line breaking, and emoji through the full text
//! compile, with the fallback faces registered.
//!
//! Each case reads the drawn glyph runs: their x order, baselines, fonts, and
//! source text. An RTL run lists its glyph text in visual order, so the
//! Hebrew word `שלום` reads `םולש` there.

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;

use common::parse;
use zenith_core::{BytesFontProvider, FontStyle, default_provider};
use zenith_scene::compile;
use zenith_scene::ir::SceneCommand;

/// The bundled faces plus the embedded fallback chain, as the CLI builds it.
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

/// One drawn run: left x, baseline y, font id, and glyph text in draw order.
#[derive(Debug, Clone)]
struct Run {
    x: f64,
    y: f64,
    font: String,
    text: String,
}

/// A one-node document: `attrs` go on the text node, `body` is its span text.
fn doc(attrs: &str, body: &str) -> String {
    doc_spans(attrs, &format!(r#"span "{body}""#))
}

/// A one-node document with the span list `spans` written as KDL.
fn doc_spans(attrs: &str, spans: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.scripts" name="Scripts"
  tokens format="zenith-token-v1" {{
    token id="size.body" type="dimension" value=(px)20
  }}
  styles {{}}
  document id="doc.scripts" title="Scripts" {{
    page id="page.scripts" w=(px)700 h=(px)400 {{
      text id="t" x=(px)10 y=(px)10 font-size=(token)"size.body" {attrs} {{ {spans} }}
    }}
  }}
}}"##
    )
}

/// The runs of node `t`, sorted by line then by x.
fn runs(attrs: &str, body: &str) -> Vec<Run> {
    runs_of(&doc(attrs, body))
}

/// The runs of node `t` in the document `src`, sorted by line then by x.
fn runs_of(src: &str) -> Vec<Run> {
    let result = compile(&parse(src), &provider());
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.code != "font.glyph_missing" && d.code != "scene.text_unshaped"),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );
    let mut out: Vec<Run> = result
        .scene
        .commands
        .iter()
        .filter_map(|cmd| match cmd {
            SceneCommand::DrawGlyphRun {
                x,
                y,
                font_id,
                glyphs,
                ..
            } => Some(Run {
                x: *x,
                y: *y,
                font: font_id.clone(),
                text: glyphs.iter().map(|g| g.text.as_str()).collect(),
            }),
            _ => None,
        })
        .collect();
    out.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
    out
}

/// The non-blank run texts of each line, left to right.
fn lines(runs: &[Run]) -> Vec<Vec<String>> {
    let mut by_line: BTreeMap<i64, Vec<String>> = BTreeMap::new();
    for run in runs {
        let text = run.text.trim();
        if !text.is_empty() {
            by_line
                .entry(run.y.round() as i64)
                .or_default()
                .push(text.to_owned());
        }
    }
    by_line.into_values().collect()
}

/// The x of the run whose text is `text`.
fn x_of(runs: &[Run], text: &str) -> f64 {
    runs.iter()
        .find(|r| r.text.trim() == text)
        .map(|r| r.x)
        .unwrap_or_else(|| panic!("no run {text:?} in {runs:?}"))
}

#[test]
fn arabic_phrase_inside_english_reads_right_to_left() {
    let runs = runs(r#"w=(px)600 h=(px)40"#, "Say مرحبا بالعالم to greet.");
    // The first Arabic word sits right of the second. Each word is joined
    // Arabic in visual order.
    assert!(x_of(&runs, "ابحرم") > x_of(&runs, "ملاعلاب"), "{runs:?}");
    assert!(x_of(&runs, "ملاعلاب") > x_of(&runs, "Say"));
    assert!(x_of(&runs, "to greet.") > x_of(&runs, "ابحرم"));
    // One baseline for every run on the line.
    assert!(runs.iter().all(|r| r.y == runs[0].y), "{runs:?}");
    assert!(runs.iter().any(|r| r.font == "noto-sans-arabic-400-normal"));
}

#[test]
fn hebrew_phrase_inside_english_reads_right_to_left() {
    let runs = runs(r#"w=(px)600 h=(px)40"#, "He said שלום עולם and left.");
    assert!(x_of(&runs, "םולש") > x_of(&runs, "םלוע"), "{runs:?}");
    assert!(x_of(&runs, "and left.") > x_of(&runs, "םולש"));
}

#[test]
fn arabic_paragraph_wraps_right_aligned_in_rtl_order() {
    let runs = runs(
        r#"w=(px)300 h=(px)200"#,
        "اللغة العربية لغة جميلة يتحدث بها ملايين الناس في العالم وتكتب من اليمين إلى اليسار",
    );
    let lines = lines(&runs);
    assert_eq!(lines.len(), 3, "{lines:?}");
    // The first logical word is the rightmost run of line 1.
    assert_eq!(lines[0].last().map(String::as_str), Some("ةغللا"));
    // The last logical word is the leftmost run of the last line.
    assert_eq!(lines[2].first().map(String::as_str), Some("راسيلا"));
    // Start alignment under RTL: every line ends at the box right edge.
    let first_line_y = runs[0].y;
    let right = runs
        .iter()
        .filter(|r| r.y == first_line_y)
        .map(|r| r.x)
        .fold(f64::MIN, f64::max);
    assert!(right > 200.0, "the first line hugs the right edge: {right}");
}

#[test]
fn hebrew_paragraph_wraps_in_rtl_order() {
    let runs = runs(
        r#"w=(px)300 h=(px)200"#,
        "השפה העברית נכתבת מימין לשמאל ומדוברת על ידי מיליוני אנשים בישראל ובעולם",
    );
    let lines = lines(&runs);
    assert!(lines.len() >= 2, "{lines:?}");
    assert_eq!(lines[0].last().map(String::as_str), Some("הפשה"));
    assert_eq!(
        lines.last().and_then(|l| l.first()).map(String::as_str),
        Some("םלועבו")
    );
}

#[test]
fn numbers_and_punctuation_next_to_arabic() {
    let runs = runs(r#"w=(px)600 h=(px)40"#, "السعر 25.50 دولار، شكرا!");
    // Digits keep their LTR order and sit left of the first word.
    assert!(runs.iter().any(|r| r.text == "25.50"), "{runs:?}");
    assert!(x_of(&runs, "رعسلا") > x_of(&runs, "25.50"));
    // The Arabic comma shapes with its word and sits on its left.
    assert!(x_of(&runs, "25.50") > x_of(&runs, "،رالود"));
    // The final `!` ends the RTL line on the left.
    let leftmost = runs
        .iter()
        .min_by(|a, b| a.x.total_cmp(&b.x))
        .map(|r| r.text.clone());
    assert_eq!(leftmost.as_deref(), Some("!"));
}

#[test]
fn chinese_wraps_between_ideographs_with_kinsoku() {
    let runs = runs(
        r#"w=(px)300 h=(px)300"#,
        "中文是世界上使用人数最多的语言之一。汉字没有空格，所以换行可以发生在任何两个字之间，「但是」标点不能出现在行首。",
    );
    let lines = lines(&runs);
    assert!(lines.len() >= 4, "the paragraph wraps: {lines:?}");
    for line in &lines {
        let first = line.first().and_then(|t| t.chars().next());
        assert!(
            !matches!(first, Some('。' | '，' | '」' | '、')),
            "a line starts with closing punctuation: {line:?}"
        );
        let last = line.last().and_then(|t| t.chars().next_back());
        assert!(
            !matches!(last, Some('「')),
            "a line ends with `「`: {line:?}"
        );
    }
    // Every line fits the 300px box.
    assert!(runs.iter().all(|r| r.x < 310.0 + 10.0));
}

#[test]
fn japanese_small_kana_never_starts_a_line() {
    let runs = runs(
        r#"w=(px)200 h=(px)300"#,
        "日本語はひらがなとカタカナと漢字を使います。ちょっとした文章です。",
    );
    let lines = lines(&runs);
    assert!(lines.len() >= 3, "{lines:?}");
    for line in &lines {
        let first = line.first().and_then(|t| t.chars().next());
        assert!(!matches!(first, Some('ょ' | 'っ' | '。')), "{line:?}");
    }
}

#[test]
fn korean_breaks_only_at_spaces() {
    let runs = runs(
        r#"w=(px)300 h=(px)300"#,
        "한국어는 띄어쓰기를 사용합니다 그래서 줄바꿈은 공백에서 일어납니다",
    );
    let words: Vec<String> = runs.iter().map(|r| r.text.clone()).collect();
    for word in [
        "한국어는",
        "띄어쓰기를",
        "사용합니다",
        "그래서",
        "줄바꿈은",
        "공백에서",
        "일어납니다",
    ] {
        assert!(words.iter().any(|w| w == word), "{word} split: {words:?}");
    }
}

#[test]
fn latin_and_cjk_share_one_line() {
    let runs = runs(
        r#"w=(px)600 h=(px)40"#,
        "Zenith 支持中文和English混排 in one line.",
    );
    assert_eq!(lines(&runs).len(), 1);
    assert!(runs.iter().any(|r| r.font == "noto-sans-cjk-sc-400-normal"));
}

#[test]
fn emoji_zwj_sequence_draws_as_one_run() {
    let runs = runs(r#"w=(px)600 h=(px)40"#, "Family 👨‍👩‍👧 and 👍🏽");
    let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
    let run = runs
        .iter()
        .find(|r| r.text == family)
        .unwrap_or_else(|| panic!("the ZWJ sequence is split: {runs:?}"));
    assert_eq!(run.font, "noto-emoji-400-normal");
    assert!(runs.iter().any(|r| r.text == "👍🏽"), "{runs:?}");
}

#[test]
fn explicit_rtl_keeps_latin_and_digits_in_order() {
    let runs = runs(
        r#"w=(px)600 h=(px)40 direction="rtl""#,
        "שלום Zenith 2025 עולם",
    );
    assert!(runs.iter().any(|r| r.text == "Zenith 2025"), "{runs:?}");
    assert!(x_of(&runs, "םולש") > x_of(&runs, "Zenith 2025"));
    assert!(x_of(&runs, "Zenith 2025") > x_of(&runs, "םלוע"));
}

#[test]
fn explicit_ltr_overrides_detection() {
    let runs = runs(r#"w=(px)600 h=(px)40 direction="ltr""#, "שלום abc");
    // LTR base: the Hebrew word starts at the box left.
    assert!(x_of(&runs, "םולש") < x_of(&runs, "abc"), "{runs:?}");
    assert_eq!(x_of(&runs, "םולש"), 10.0);
}

#[test]
fn latin_text_is_unchanged_by_an_explicit_ltr() {
    let auto = compile(
        &parse(&doc(r#"w=(px)120 h=(px)200"#, "The quick brown fox jumps.")),
        &provider(),
    );
    let ltr = compile(
        &parse(&doc(
            r#"w=(px)120 h=(px)200 direction="ltr""#,
            "The quick brown fox jumps.",
        )),
        &provider(),
    );
    assert_eq!(auto.scene, ltr.scene);
}

#[test]
fn rtl_words_split_across_spans_keep_rtl_order() {
    let src = doc_spans(
        r#"w=(px)600 h=(px)40"#,
        r#"span "abc שלום "; span "עולם def" font-weight=700"#,
    );
    let runs = runs_of(&src);
    // The space ends the first span, but it sits between two Hebrew words,
    // so the two words still swap.
    assert!(x_of(&runs, "םולש") > x_of(&runs, "םלוע"), "{runs:?}");
    assert!(x_of(&runs, "def") > x_of(&runs, "םולש"));
}

#[test]
fn rtl_words_split_across_spans_keep_rtl_order_when_wrapped() {
    let src = doc_spans(
        r#"w=(px)220 h=(px)200"#,
        r#"span "abc שלום "; span "עולם def ghi jkl mno pqr stu vwx" font-weight=700"#,
    );
    let runs = runs_of(&src);
    let first_y = runs[0].y;
    let first: Vec<Run> = runs.into_iter().filter(|r| r.y == first_y).collect();
    assert!(x_of(&first, "םולש") > x_of(&first, "םלוע"), "{first:?}");
}
