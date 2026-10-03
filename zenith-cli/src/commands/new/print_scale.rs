//! Print scale for a themed `zenith new` on a print `--format`.
//!
//! Theme packs tune their type tokens to the 1080×1080 default page. On a
//! print paper format the scaffold rescales the copied theme tokens:
//!
//! - `size.*` (type): `k_type = (16 / size.body) × (short_side / 794)`, so A4
//!   gets a 16 px (12 pt) body. Rounded to whole px. The floor is the
//!   `text.too_small` floor of the page ([`text_size_floor_px`]). Walked in
//!   ascending original order, each step then rises to the smallest whole px
//!   that the `type.near_duplicate_size` test ([`sizes_read_as_one`]) reads as
//!   a distinct size from the step below. Steps keep their order and never
//!   merge. Tokens with equal original values stay equal. A pack with no
//!   literal `size.body` keeps its type.
//! - `radius.*`: `k_page = short_side / 1080`. Rounded to whole px, floor
//!   1 px. A zero radius stays zero: it means square corners.
//! - Every other group stays. `space.unit` is a 4 px base grid unit, the same
//!   in every pack and not tuned to the page. `border.width` is a hairline.
//!
//! Only literal `dimension` tokens in px or pt change, and they are written
//! back in px. An alias (`(token)"…"`) and any other unit stay as they are.
//! Square, no format, and a custom size without a format change nothing.

use std::cmp::Ordering;

use zenith_core::ast::{
    Dimension, Token, TokenBlock, TokenLiteral, TokenType, TokenValue, Unit, dim_to_px,
};
use zenith_scene::{sizes_read_as_one, text_size_floor_px};

use super::page::{DEFAULT_PAGE, PageSpec};

/// Short side of the page the pack type scale is tuned for (the `new`
/// default 1080×1080 square).
const TUNED_SHORT_SIDE_PX: u32 = DEFAULT_PAGE.width;
/// Short side of A4 portrait at 96 dpi, the page that gets [`PRINT_BODY_PX`].
const A4_SHORT_SIDE_PX: f64 = 794.0;
/// Body size on A4: 16 px is 12 pt.
const PRINT_BODY_PX: f64 = 16.0;
/// The token the type factor is computed from.
const BODY_TOKEN_ID: &str = "size.body";
/// Smallest scaled nonzero radius, in px.
const PAGE_FLOOR_PX: f64 = 1.0;

/// The scale factors for one page.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Factors {
    /// Factor for `size.*`, or `None` when the pack has no usable `size.body`.
    k_type: Option<f64>,
    /// Smallest scaled type size, in px: the page's `text.too_small` floor.
    type_floor: f64,
    /// Factor for `radius.*`.
    k_page: f64,
}

/// Which rule a token id falls under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Group {
    /// `size.*`: scaled by `k_type`.
    Type,
    /// `radius.*`: scaled by `k_page`.
    Page,
    /// Every other id: kept.
    Fixed,
}

/// Rescale the dimension tokens of `tokens` for `page`. Does nothing unless
/// `page.format` is a print format.
pub(super) fn apply(tokens: &mut TokenBlock, page: PageSpec) {
    let Some(factors) = factors_for(tokens, page) else {
        return;
    };
    if let Some(k_type) = factors.k_type {
        scale_type(tokens, k_type, factors.type_floor);
    }
    for token in &mut tokens.tokens {
        let scaled = match group_of(&token.id) {
            Group::Page => literal_px(token).map(|px| scale_radius(px, factors.k_page)),
            Group::Type | Group::Fixed => None,
        };
        if let Some(value) = scaled {
            set_px(token, value);
        }
    }
}

/// Scale every literal `size.*` token by `k`, floor at `floor`, then raise
/// each step until it reads as distinct from the step below it.
fn scale_type(tokens: &mut TokenBlock, k: f64, floor: f64) {
    let mut steps: Vec<(f64, usize)> = tokens
        .tokens
        .iter()
        .enumerate()
        .filter(|(_, t)| group_of(&t.id) == Group::Type)
        .filter_map(|(i, t)| literal_px(t).map(|px| (px, i)))
        .collect();
    steps.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));

    // `(original px, scaled px)` of the previous step.
    let mut below: Option<(f64, f64)> = None;
    for (original, index) in steps {
        let rounded = scale(original, k, floor);
        let value = match below {
            Some((below_original, below_scaled))
                if original.total_cmp(&below_original) == Ordering::Equal =>
            {
                below_scaled
            }
            Some((_, below_scaled)) => distinct_above(below_scaled, rounded),
            None => rounded,
        };
        if let Some(token) = tokens.tokens.get_mut(index) {
            set_px(token, value);
        }
        below = Some((original, value));
    }
}

/// The smallest whole px at or above `candidate` that reads as a size
/// distinct from `below`.
fn distinct_above(below: f64, candidate: f64) -> f64 {
    let mut value = candidate.max(below);
    while sizes_read_as_one(below, value) {
        value += 1.0;
    }
    value
}

/// Write `value` px into `token` as a literal dimension.
fn set_px(token: &mut Token, value: f64) {
    token.value = TokenValue::Literal(TokenLiteral::Dimension(Dimension {
        value,
        unit: Unit::Px,
    }));
}

/// The factors for `page`, or `None` when the page is not a print format.
fn factors_for(tokens: &TokenBlock, page: PageSpec) -> Option<Factors> {
    let format = page.format?;
    if !format.is_print() {
        return None;
    }
    let short = f64::from(page.width.min(page.height));
    let k_type = tokens
        .tokens
        .iter()
        .find(|t| t.id == BODY_TOKEN_ID)
        .and_then(literal_px)
        .filter(|body| *body > 0.0)
        .map(|body| type_factor(body, short));
    Some(Factors {
        k_type,
        type_floor: text_size_floor_px(f64::from(page.width), f64::from(page.height)),
        k_page: short / f64::from(TUNED_SHORT_SIDE_PX),
    })
}

/// `k_type = (16 / body) × (short / 794)`.
fn type_factor(body_px: f64, short_side_px: f64) -> f64 {
    (PRINT_BODY_PX / body_px) * (short_side_px / A4_SHORT_SIDE_PX)
}

fn group_of(id: &str) -> Group {
    if id.starts_with("size.") {
        Group::Type
    } else if id.starts_with("radius.") {
        Group::Page
    } else {
        Group::Fixed
    }
}

/// `px × k`, rounded to whole px, at least `floor`.
fn scale(px: f64, k: f64, floor: f64) -> f64 {
    (px * k).round().max(floor)
}

/// A radius scaled by `k`. Zero or negative stays as it is.
fn scale_radius(px: f64, k: f64) -> f64 {
    if px > 0.0 {
        scale(px, k, PAGE_FLOOR_PX)
    } else {
        px
    }
}

/// The px value of a literal `dimension` token in px or pt. `None` for an
/// alias, another type, another unit, or a non-finite value.
fn literal_px(token: &Token) -> Option<f64> {
    if token.token_type != TokenType::Dimension {
        return None;
    }
    if let TokenValue::Literal(TokenLiteral::Dimension(d)) = &token.value {
        dim_to_px(d.value, &d.unit).filter(|v| v.is_finite())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::new::page::PaperFormat;

    fn dim(id: &str, value: f64, unit: Unit) -> Token {
        Token {
            id: id.to_owned(),
            token_type: TokenType::Dimension,
            value: TokenValue::Literal(TokenLiteral::Dimension(Dimension { value, unit })),
            set: None,
            source_span: None,
        }
    }

    fn alias(id: &str, target: &str) -> Token {
        Token {
            id: id.to_owned(),
            token_type: TokenType::Dimension,
            value: TokenValue::Reference {
                token_id: target.to_owned(),
            },
            set: None,
            source_span: None,
        }
    }

    fn block(tokens: Vec<Token>) -> TokenBlock {
        TokenBlock {
            tokens,
            ..TokenBlock::default()
        }
    }

    fn page(format: Option<PaperFormat>) -> PageSpec {
        let (width, height) = format.map_or((1080, 1080), PaperFormat::portrait_px);
        PageSpec {
            width,
            height,
            pages: 1,
            format,
        }
    }

    fn px_of(tokens: &TokenBlock, id: &str) -> Option<f64> {
        tokens
            .tokens
            .iter()
            .find(|t| t.id == id)
            .and_then(literal_px)
    }

    fn slide_pack() -> TokenBlock {
        block(vec![
            dim("size.display", 112.0, Unit::Px),
            dim("size.h1", 64.0, Unit::Px),
            dim("size.h2", 40.0, Unit::Px),
            dim("size.body", 28.0, Unit::Px),
            dim("size.caption", 18.0, Unit::Px),
            dim("radius.box", 32.0, Unit::Px),
            dim("radius.field", 0.0, Unit::Px),
            dim("space.unit", 4.0, Unit::Px),
            dim("border.width", 1.5, Unit::Px),
        ])
    }

    #[test]
    fn a4_factors() {
        let f = factors_for(&slide_pack(), page(Some(PaperFormat::A4))).unwrap();
        assert!((f.k_type.unwrap() - 16.0 / 28.0).abs() < 1e-12);
        assert!((f.k_page - 794.0 / 1080.0).abs() < 1e-12);
    }

    #[test]
    fn letter_factor_scales_with_short_side() {
        let f = factors_for(&slide_pack(), page(Some(PaperFormat::Letter))).unwrap();
        assert!((f.k_type.unwrap() - (16.0 / 28.0) * (816.0 / 794.0)).abs() < 1e-12);
    }

    #[test]
    fn landscape_uses_the_short_side() {
        let landscape = PageSpec {
            width: 1123,
            height: 794,
            pages: 1,
            format: Some(PaperFormat::A4),
        };
        let f = factors_for(&slide_pack(), landscape).unwrap();
        assert!((f.k_type.unwrap() - 16.0 / 28.0).abs() < 1e-12);
    }

    #[test]
    fn non_print_pages_have_no_factors() {
        assert_eq!(factors_for(&slide_pack(), page(None)), None);
        assert_eq!(
            factors_for(&slide_pack(), page(Some(PaperFormat::Square))),
            None
        );
    }

    #[test]
    fn a4_scales_type_and_radius() {
        let mut t = slide_pack();
        apply(&mut t, page(Some(PaperFormat::A4)));
        assert_eq!(px_of(&t, "size.body"), Some(16.0));
        assert_eq!(px_of(&t, "size.display"), Some(64.0));
        // 64 × 16/28 = 36.57 → 37; 40 × 16/28 = 22.86 → 23.
        assert_eq!(px_of(&t, "size.h1"), Some(37.0));
        assert_eq!(px_of(&t, "size.h2"), Some(23.0));
        // 32 × 794/1080 = 23.53 → 24.
        assert_eq!(px_of(&t, "radius.box"), Some(24.0));
    }

    #[test]
    fn rounding_and_floors() {
        assert_eq!(scale(36.57, 1.0, 9.0), 37.0);
        assert_eq!(scale(22.4, 1.0, 9.0), 22.0);
        // 18 × 0.4 = 7.2 → 7 → floor 9.
        assert_eq!(scale(18.0, 0.4, 9.0), 9.0);
        // 1 × 0.3 = 0.3 → 0 → floor 1.
        assert_eq!(scale_radius(1.0, 0.3), 1.0);
        // Zero radius stays zero.
        assert_eq!(scale_radius(0.0, 0.3), 0.0);

        let mut t = slide_pack();
        apply(&mut t, page(Some(PaperFormat::A4)));
        // 18 × 16/28 = 10.29 → 10, above the A4 floor.
        assert_eq!(px_of(&t, "size.caption"), Some(10.0));
        assert_eq!(px_of(&t, "radius.field"), Some(0.0));
    }

    #[test]
    fn type_floor_is_the_small_text_floor() {
        for format in [PaperFormat::A3, PaperFormat::A4, PaperFormat::A5] {
            let p = page(Some(format));
            let f = factors_for(&slide_pack(), p).unwrap();
            let floor = text_size_floor_px(f64::from(p.width), f64::from(p.height));
            assert_eq!(f.type_floor, floor, "{format:?}");
        }
    }

    #[test]
    fn distinct_above_clears_the_near_duplicate_test() {
        // 10 → 11 is 1 px: one size. 12 clears both tests.
        assert_eq!(distinct_above(10.0, 10.0), 12.0);
        // 30 → 32 is under 8 %. 33 is 10 %.
        assert_eq!(distinct_above(30.0, 31.0), 33.0);
        // A candidate that already clears stays.
        assert_eq!(distinct_above(9.0, 16.0), 16.0);
    }

    /// Scaled `size.*` values in ascending original order.
    fn type_steps(t: &TokenBlock) -> Vec<f64> {
        let mut steps: Vec<(f64, f64)> = slide_pack()
            .tokens
            .iter()
            .filter(|tok| tok.id.starts_with("size."))
            .filter_map(|tok| Some((literal_px(tok)?, px_of(t, &tok.id)?)))
            .collect();
        steps.sort_by(|a, b| a.0.total_cmp(&b.0));
        steps.into_iter().map(|(_, scaled)| scaled).collect()
    }

    #[test]
    fn a5_steps_are_distinct_and_increasing() {
        let mut t = slide_pack();
        apply(&mut t, page(Some(PaperFormat::A5)));
        let steps = type_steps(&t);
        assert_eq!(steps, vec![9.0, 11.0, 16.0, 26.0, 45.0]);
        for pair in steps.windows(2) {
            if let [lo, hi] = pair {
                assert!(hi > lo, "{steps:?}");
                assert!(!sizes_read_as_one(*lo, *hi), "{steps:?}");
            }
        }
    }

    #[test]
    fn crowded_steps_are_pushed_apart_in_order() {
        // 14 and 15 × 0.5 round to 7 and 8, and both floor to 9.
        let mut t = block(vec![
            dim("size.body", 32.0, Unit::Px),
            dim("size.a", 14.0, Unit::Px),
            dim("size.b", 15.0, Unit::Px),
            dim("size.c", 15.0, Unit::Px),
        ]);
        let pg = PageSpec {
            width: 794,
            height: 1123,
            pages: 1,
            format: Some(PaperFormat::A4),
        };
        apply(&mut t, pg);
        // a → 9 (floor). b → 11, the first px distinct from 9. c equals b.
        assert_eq!(px_of(&t, "size.a"), Some(9.0));
        assert_eq!(px_of(&t, "size.b"), Some(11.0));
        assert_eq!(px_of(&t, "size.c"), Some(11.0));
        assert_eq!(px_of(&t, "size.body"), Some(16.0));
    }

    #[test]
    fn space_and_border_stay() {
        let mut t = slide_pack();
        apply(&mut t, page(Some(PaperFormat::A5)));
        assert_eq!(px_of(&t, "space.unit"), Some(4.0));
        assert_eq!(px_of(&t, "border.width"), Some(1.5));
    }

    #[test]
    fn missing_body_leaves_type_and_scales_radius() {
        let mut t = block(vec![
            dim("size.h1", 64.0, Unit::Px),
            dim("radius.box", 32.0, Unit::Px),
        ]);
        apply(&mut t, page(Some(PaperFormat::A4)));
        assert_eq!(px_of(&t, "size.h1"), Some(64.0));
        assert_eq!(px_of(&t, "radius.box"), Some(24.0));
    }

    #[test]
    fn alias_tokens_are_untouched() {
        let mut t = slide_pack();
        t.tokens.push(alias("size.lead", "size.h2"));
        t.tokens.push(alias("radius.card", "radius.box"));
        apply(&mut t, page(Some(PaperFormat::A4)));
        let tail = [
            alias("size.lead", "size.h2"),
            alias("radius.card", "radius.box"),
        ];
        assert!(t.tokens.ends_with(&tail));
    }

    #[test]
    fn pt_body_is_read_in_px_and_written_in_px() {
        // 21 pt = 28 px, so the factor matches a 28 px body.
        let mut t = block(vec![
            dim("size.body", 21.0, Unit::Pt),
            dim("size.h1", 48.0, Unit::Pt),
        ]);
        apply(&mut t, page(Some(PaperFormat::A4)));
        assert_eq!(
            t.tokens[0].value,
            TokenValue::Literal(TokenLiteral::Dimension(Dimension {
                value: 16.0,
                unit: Unit::Px
            }))
        );
        assert_eq!(px_of(&t, "size.h1"), Some(37.0));
    }

    #[test]
    fn non_px_units_stay() {
        let mut t = slide_pack();
        t.tokens.push(dim("size.fluid", 50.0, Unit::Pct));
        apply(&mut t, page(Some(PaperFormat::A4)));
        assert_eq!(t.tokens.last(), Some(&dim("size.fluid", 50.0, Unit::Pct)));
    }

    #[test]
    fn non_print_pages_are_unchanged() {
        for p in [page(None), page(Some(PaperFormat::Square))] {
            let mut t = slide_pack();
            apply(&mut t, p);
            assert_eq!(t, slide_pack());
        }
    }
}
