//! Bidi and CJK support for the word pipeline: the node base direction, level
//! pieces of a word, CJK break pieces, and the visual order of a packed line.
//!
//! A word holds one or more pieces. A piece is a maximal stretch of one bidi
//! level, shaped in that level's direction. A line orders its pieces and its
//! inter-word gaps by UAX #9 rule L2. Latin LTR text is one level-0 piece per
//! word, so its order and positions stay as before.

use std::ops::Range;

use zenith_core::TextNode;
use zenith_layout::{
    LayoutError, ShapeRequest, TextDirection, TextLayoutEngine, ZenithGlyphRun, bidi_levels,
    detect_direction, ideographic_breaks, level_direction, level_runs, visual_order,
};

use super::shape::{ResolvedSpan, WordToken};

/// The explicit `direction` attribute of `text`, if any.
pub(in crate::compile) fn explicit_direction(text: &TextNode) -> Option<TextDirection> {
    match text.direction.as_deref() {
        Some("rtl") => Some(TextDirection::Rtl),
        Some("ltr") => Some(TextDirection::Ltr),
        Some(_) | None => None,
    }
}

/// The base direction of `text`.
///
/// An explicit `direction` wins. Otherwise the first strong character of the
/// span text decides (UAX #9 rules P2 and P3). Text with no strong character
/// is LTR.
pub(in crate::compile) fn node_direction(text: &TextNode) -> TextDirection {
    explicit_direction(text).unwrap_or_else(|| {
        text.spans
            .iter()
            .find_map(|span| detect_direction(&span.text))
            .unwrap_or(TextDirection::Ltr)
    })
}

/// The shaped runs of one token, in logical piece order.
pub(in crate::compile) struct LeveledRuns {
    pub(in crate::compile) runs: Vec<ZenithGlyphRun>,
    /// The bidi level of each run, parallel to `runs`.
    pub(in crate::compile) run_levels: Vec<u8>,
    pub(in crate::compile) missing_chars: Vec<char>,
}

/// Shape `req.text` piece by piece, one piece per bidi level.
///
/// `levels` holds one level per byte of `req.text`. A missing level reads as
/// `base`. Each piece shapes with fallback in its level's direction. A piece
/// lists its runs in visual order. The pieces stay in logical order: the line
/// emitter orders them. A token of one level shapes in one call, so it equals
/// a plain fallback shape in that direction.
///
/// # Errors
///
/// Returns the first shaping error of any piece.
pub(in crate::compile) fn shape_leveled(
    engine: &dyn TextLayoutEngine,
    fonts: &dyn zenith_core::FontProvider,
    req: &ShapeRequest<'_>,
    levels: &[u8],
    base: u8,
) -> Result<LeveledRuns, LayoutError> {
    let pieces = level_runs(req.text, levels, base);
    let mut out = LeveledRuns {
        runs: Vec::new(),
        run_levels: Vec::new(),
        missing_chars: Vec::new(),
    };
    let single = pieces.len() <= 1;
    let piece_count = pieces.len();
    let one_piece = [(0, req.text.len(), base)];
    let pieces: &[(usize, usize, u8)] = if pieces.is_empty() {
        &one_piece
    } else {
        &pieces
    };
    for (idx, &(start, end, level)) in pieces.iter().enumerate() {
        let text = if single {
            req.text
        } else {
            req.text.get(start..end).unwrap_or("")
        };
        let piece_req = ShapeRequest {
            text,
            direction: level_direction(level),
            ..req.clone()
        };
        let result = engine.shape_with_fallback(&piece_req, fonts)?;
        let mut runs = result.runs;
        // Letter spacing sits between pieces as it sits between runs.
        let letter_spacing = if req.letter_spacing_px.is_finite() {
            req.letter_spacing_px
        } else {
            0.0
        };
        if letter_spacing != 0.0
            && idx + 1 < piece_count
            && let Some(last) = runs.last_mut()
        {
            last.advance_width += letter_spacing;
        }
        out.run_levels
            .extend(std::iter::repeat_n(level, runs.len()));
        out.runs.extend(runs);
        out.missing_chars.extend(result.missing_chars);
    }
    Ok(out)
}

/// The words of `segment` with their byte offsets, split as
/// [`str::split_whitespace`] splits.
pub(in crate::compile) fn split_whitespace_indexed(
    segment: &str,
) -> impl Iterator<Item = (usize, &str)> {
    let mut rest = segment.char_indices().peekable();
    std::iter::from_fn(move || {
        while rest.next_if(|&(_, ch)| ch.is_whitespace()).is_some() {}
        let &(start, _) = rest.peek()?;
        let mut end = start;
        while let Some((at, ch)) = rest.next_if(|&(_, ch)| !ch.is_whitespace()) {
            end = at + ch.len_utf8();
        }
        segment.get(start..end).map(|word| (start, word))
    })
}

/// The bidi levels of every paragraph of a span list.
///
/// A paragraph is the text between `\n`s, across span boundaries. Each span
/// segment (one `split('\n')` item) maps to a byte range of its paragraph.
pub(in crate::compile) struct ParagraphLevels {
    levels: Vec<Vec<u8>>,
    /// `(paragraph index, byte start)` of each segment, in span order.
    segments: Vec<(usize, usize)>,
}

impl ParagraphLevels {
    /// Resolve the levels of `spans` with `direction` as the paragraph direction.
    pub(in crate::compile) fn new(spans: &[ResolvedSpan], direction: TextDirection) -> Self {
        let mut texts: Vec<String> = vec![String::new()];
        let mut segments: Vec<(usize, usize)> = Vec::new();
        for span in spans {
            for (seg_idx, segment) in span.text.split('\n').enumerate() {
                if seg_idx > 0 {
                    texts.push(String::new());
                }
                let para = texts.len() - 1;
                let start = texts.last().map_or(0, String::len);
                segments.push((para, start));
                if let Some(text) = texts.last_mut() {
                    text.push_str(segment);
                }
            }
        }
        let levels = texts
            .iter()
            .map(|text| bidi_levels(text, direction))
            .collect();
        Self { levels, segments }
    }

    /// The level of the paragraph byte just before byte `offset` of segment
    /// `index`. It crosses span boundaries. `None` at a paragraph start.
    pub(in crate::compile) fn level_before(&self, index: usize, offset: usize) -> Option<u8> {
        let &(para, start) = self.segments.get(index)?;
        let at = (start + offset).checked_sub(1)?;
        self.levels.get(para)?.get(at).copied()
    }

    /// The levels of segment `index`, `len` bytes long. Empty when unknown.
    pub(in crate::compile) fn segment(&self, index: usize, len: usize) -> &[u8] {
        self.segments
            .get(index)
            .and_then(|&(para, start)| self.levels.get(para)?.get(start..start + len))
            .unwrap_or(&[])
    }
}

/// `word` split at its CJK line break opportunities, with byte offsets.
///
/// A word with no ideograph is one piece.
pub(in crate::compile) fn break_pieces(word: &str) -> Vec<(usize, &str)> {
    let mut pieces = Vec::new();
    let mut start = 0;
    for offset in ideographic_breaks(word) {
        if let Some(piece) = word.get(start..offset) {
            pieces.push((start, piece));
        }
        start = offset;
    }
    if let Some(piece) = word.get(start..) {
        pieces.push((start, piece));
    }
    pieces
}

/// One drawn piece of a line: a run range of one word and its left x.
pub(in crate::compile) struct PlacedPiece<'w> {
    pub(in crate::compile) word: &'w WordToken,
    pub(in crate::compile) runs: Range<usize>,
    pub(in crate::compile) x: f64,
    pub(in crate::compile) advance: f64,
}

impl PlacedPiece<'_> {
    /// The piece's runs.
    pub(in crate::compile) fn glyph_runs(&self) -> &[ZenithGlyphRun] {
        self.word.runs.get(self.runs.clone()).unwrap_or(&[])
    }
}

/// One logical item of a line before reordering.
enum LineItem {
    /// A run range of word `word`, with its advance.
    Piece {
        word: usize,
        runs: Range<usize>,
        advance: f64,
    },
    /// An inter-word gap of `width` px.
    Gap { width: f64 },
}

/// Place the pieces of `words` left to right in visual order from `base_x`.
///
/// Each non-glued word after the first opens with its gap plus `extra` (the
/// justify stretch). Pieces and gaps take their visual order from their bidi
/// levels (UAX #9 rule L2). A line of level-0 words keeps logical order, and
/// each word stays one piece at its summed advance, so positions match the
/// plain left-to-right placement exactly.
pub(in crate::compile) fn place_line(
    words: &[WordToken],
    base_x: f64,
    extra: f64,
) -> Vec<PlacedPiece<'_>> {
    let mut items: Vec<LineItem> = Vec::new();
    let mut levels: Vec<u8> = Vec::new();
    for (wi, word) in words.iter().enumerate() {
        if wi > 0 && !word.glued {
            items.push(LineItem::Gap {
                width: word.gap_before_px + extra,
            });
            levels.push(word.gap_level);
        }
        let pieces = word_pieces(word);
        let whole = pieces.len() == 1;
        for (runs, level) in pieces {
            let advance = if whole {
                word.advance
            } else {
                word.runs
                    .get(runs.clone())
                    .unwrap_or(&[])
                    .iter()
                    .map(|run| run.advance_width as f64)
                    .sum()
            };
            items.push(LineItem::Piece {
                word: wi,
                runs,
                advance,
            });
            levels.push(level);
        }
    }

    let mut placed: Vec<PlacedPiece<'_>> = Vec::with_capacity(words.len());
    let mut x = base_x;
    for idx in visual_order(&levels) {
        match items.get(idx) {
            Some(LineItem::Piece {
                word,
                runs,
                advance,
            }) => {
                if let Some(token) = words.get(*word) {
                    placed.push(PlacedPiece {
                        word: token,
                        runs: runs.clone(),
                        x,
                        advance: *advance,
                    });
                }
                x += advance;
            }
            Some(LineItem::Gap { width }) => x += width,
            None => {}
        }
    }
    placed
}

/// The run ranges of `word` that share one level, with that level.
///
/// A word with no runs is one empty piece at its gap level, so it still holds
/// a place in the line.
fn word_pieces(word: &WordToken) -> Vec<(Range<usize>, u8)> {
    let mut pieces: Vec<(Range<usize>, u8)> = Vec::new();
    for idx in 0..word.runs.len() {
        let level = word.run_levels.get(idx).copied().unwrap_or(word.gap_level);
        match pieces.last_mut() {
            Some((range, last)) if *last == level => range.end = idx + 1,
            _ => pieces.push((idx..idx + 1, level)),
        }
    }
    if pieces.is_empty() {
        pieces.push((0..0, word.gap_level));
    }
    pieces
}

#[cfg(test)]
mod tests {
    use zenith_core::FontStyle;

    use super::super::shape::WordSource;
    use super::*;
    use crate::ir::Color;

    fn run(advance: f32, id: &str) -> ZenithGlyphRun {
        ZenithGlyphRun {
            font_id: id.to_owned(),
            font_size: 16.0,
            ascent: 12.0,
            descent: 4.0,
            line_height: 18.0,
            advance_width: advance,
            glyphs: Vec::new(),
        }
    }

    fn word(runs: Vec<(f32, u8)>, gap_level: u8, glued: bool) -> WordToken {
        let advance = runs.iter().map(|&(a, _)| f64::from(a)).sum();
        WordToken {
            run_levels: runs.iter().map(|&(_, l)| l).collect(),
            runs: runs.iter().map(|&(a, _)| run(a, "f")).collect(),
            advance,
            color: Color::srgb(0, 0, 0, 255),
            underline: false,
            strikethrough: false,
            highlight: None,
            code: false,
            link: None,
            baseline_dy: 0.0,
            gap_before_px: 5.0,
            gap_level,
            glued,
            src: WordSource {
                text: String::new(),
                levels: Vec::new(),
                weight: 400,
                style: FontStyle::Normal,
                font_size: 16.0,
                letter_spacing_px: 0.0,
                features: Vec::new(),
                paragraph: 0,
                hyphen_part: None,
            },
        }
    }

    fn xs(placed: &[PlacedPiece<'_>]) -> Vec<f64> {
        placed.iter().map(|p| p.x).collect()
    }

    #[test]
    fn ltr_words_keep_logical_order() {
        let words = vec![
            word(vec![(10.0, 0)], 0, false),
            word(vec![(20.0, 0)], 0, false),
        ];
        assert_eq!(xs(&place_line(&words, 0.0, 0.0)), vec![0.0, 15.0]);
    }

    #[test]
    fn rtl_words_inside_ltr_reverse_as_a_group() {
        // a [R1 R2] b: the two RTL words swap, the gap between them stays.
        let words = vec![
            word(vec![(10.0, 0)], 0, false),
            word(vec![(20.0, 1)], 0, false),
            word(vec![(30.0, 1)], 1, false),
            word(vec![(40.0, 0)], 0, false),
        ];
        let placed = place_line(&words, 0.0, 0.0);
        let order: Vec<f64> = placed.iter().map(|p| p.advance).collect();
        assert_eq!(order, vec![10.0, 30.0, 20.0, 40.0]);
        assert_eq!(xs(&placed), vec![0.0, 15.0, 50.0, 75.0]);
    }

    #[test]
    fn rtl_paragraph_reverses_and_keeps_ltr_pairs() {
        // R1 L1 L2 R2 in an RTL paragraph: L1 L2 stay in order.
        let words = vec![
            word(vec![(10.0, 1)], 1, false),
            word(vec![(20.0, 2)], 1, false),
            word(vec![(30.0, 2)], 2, false),
            word(vec![(40.0, 1)], 1, false),
        ];
        let order: Vec<f64> = place_line(&words, 0.0, 0.0)
            .iter()
            .map(|p| p.advance)
            .collect();
        assert_eq!(order, vec![40.0, 20.0, 30.0, 10.0]);
    }

    #[test]
    fn mixed_word_splits_into_pieces() {
        // One word "(שלום)" style: level 0, 1, 0 runs.
        let words = vec![word(vec![(5.0, 0), (20.0, 1), (5.0, 0)], 0, false)];
        let placed = place_line(&words, 0.0, 0.0);
        assert_eq!(placed.len(), 3);
        assert_eq!(xs(&placed), vec![0.0, 5.0, 25.0]);
    }

    #[test]
    fn glued_pieces_have_no_gap() {
        let words = vec![
            word(vec![(10.0, 0)], 0, false),
            word(vec![(10.0, 0)], 0, true),
        ];
        assert_eq!(xs(&place_line(&words, 2.0, 0.0)), vec![2.0, 12.0]);
    }

    fn span(text: &str) -> ResolvedSpan {
        ResolvedSpan {
            text: text.to_owned(),
            color: Color::srgb(0, 0, 0, 255),
            underline: false,
            strikethrough: false,
            highlight: None,
            code: false,
            link: None,
            weight: 400,
            style: FontStyle::Normal,
            font_size: 16.0,
            baseline_dy: 0.0,
            letter_spacing_px: 0.0,
            features: Vec::new(),
        }
    }

    #[test]
    fn paragraph_levels_span_across_spans() {
        // The comma sits between two Hebrew words in different spans, so it
        // resolves RTL in the joined paragraph.
        let spans = [span("abc שלום"), span(", עולם\nnext")];
        let levels = ParagraphLevels::new(&spans, TextDirection::Ltr);
        assert_eq!(levels.segment(1, ", עולם".len()).first(), Some(&1));
        assert!(levels.segment(2, 4).iter().all(|&l| l == 0));
        assert!(
            levels.segment(9, 4).is_empty(),
            "an unknown segment is empty"
        );
        assert_eq!(levels.level_before(1, 0), Some(1), "the last Hebrew letter");
        assert_eq!(levels.level_before(2, 0), None, "a paragraph start");
    }

    #[test]
    fn gap_between_rtl_words_in_two_spans_is_rtl() {
        // "שלום " then "עולם": the space ends span 0 and resolves RTL.
        let spans = [span("abc שלום "), span("עולם def")];
        let levels = ParagraphLevels::new(&spans, TextDirection::Ltr);
        assert_eq!(levels.level_before(1, 0), Some(1));
    }

    #[test]
    fn break_pieces_split_cjk_only() {
        assert_eq!(break_pieces("hello"), vec![(0, "hello")]);
        assert_eq!(break_pieces("中文"), vec![(0, "中"), (3, "文")]);
    }

    #[test]
    fn node_direction_detects_and_honors_override() {
        let src = |attr: &str, body: &str| {
            format!(
                r#"zenith version=1 {{
  project id="p" name="P"
  tokens format="zenith-token-v1" {{}}
  styles {{}}
  document id="d" title="D" {{
    page id="pg" w=(px)10 h=(px)10 {{
      text id="t" x=(px)0 y=(px)0 {attr} {{ span "{body}" }}
    }}
  }}
}}"#
            )
        };
        let direction = |attr: &str, body: &str| {
            use zenith_core::{KdlAdapter, KdlSource, Node};
            let doc = KdlAdapter
                .parse(src(attr, body).as_bytes())
                .expect("parses");
            let node = doc.body.pages[0].children[0].clone();
            match node {
                Node::Text(text) => node_direction(&text),
                _ => panic!("expected a text node"),
            }
        };
        assert_eq!(direction("", "مرحبا"), TextDirection::Rtl);
        assert_eq!(direction("", "123 abc"), TextDirection::Ltr);
        assert_eq!(direction("", "2025"), TextDirection::Ltr);
        assert_eq!(direction(r#"direction="ltr""#, "שלום"), TextDirection::Ltr);
        assert_eq!(direction(r#"direction="rtl""#, "abc"), TextDirection::Rtl);
    }

    #[test]
    fn split_whitespace_indexed_matches_split_whitespace() {
        let text = "  中文 ab\tc  d ";
        let indexed: Vec<(usize, &str)> = split_whitespace_indexed(text).collect();
        let plain: Vec<&str> = text.split_whitespace().collect();
        assert_eq!(indexed.iter().map(|&(_, w)| w).collect::<Vec<_>>(), plain);
        for (offset, word) in indexed {
            assert_eq!(&text[offset..offset + word.len()], word);
        }
    }
}
