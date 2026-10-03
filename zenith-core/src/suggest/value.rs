//! Value-based token matching for raw visual literals.
//!
//! A raw literal (`fill="#ffffff"`, `font-size=(px)96`) maps to the declared
//! token with the same resolved value, else to the nearest one. Colors compare
//! in OKLab (Euclidean). Dimensions compare in px inside the property's role
//! (`font-size` → `size.*`, `radius` → `radius.*`, …).
//!
//! All math uses `+ - * /` only, with fixed-count Newton roots, so results are
//! the same bytes on any machine.

use std::collections::BTreeMap;

use crate::ast::value::{Dimension, PropertyValue, Unit, dim_to_px};
use crate::color::parse_rgb;
use crate::tokens::{ResolvedToken, ResolvedValue};

/// A raw literal value, read for token matching.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum LiteralValue {
    /// A color as normalized lowercase `#rrggbb` or `#rrggbbaa`.
    Color(String),
    /// A dimension with a px-convertible unit.
    Dimension(Dimension),
    /// A font family name.
    FontFamily(String),
    /// A numeric font weight.
    FontWeight(u32),
}

impl LiteralValue {
    /// A color literal from hex text (`#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`).
    pub(crate) fn color(text: &str) -> Option<Self> {
        normalize_hex(text).map(Self::Color)
    }

    /// A dimension literal. `None` when the unit has no px conversion.
    pub(crate) fn dimension(d: &Dimension) -> Option<Self> {
        dim_to_px(d.value, &d.unit)?;
        Some(Self::Dimension(d.clone()))
    }

    /// A font-weight literal from integer text (`700`, `700.0`).
    pub(crate) fn font_weight(text: &str) -> Option<Self> {
        let n: f64 = text.trim().parse().ok()?;
        if n.fract() != 0.0 || !(1.0..=1000.0).contains(&n) {
            return None;
        }
        Some(Self::FontWeight(n as u32))
    }

    /// A font-family literal. `None` for an empty name.
    pub(crate) fn font_family(text: &str) -> Option<Self> {
        let t = text.trim();
        (!t.is_empty()).then(|| Self::FontFamily(t.to_owned()))
    }
}

/// Source text of a raw property value, as a fix hint carries it:
/// `#ffffff`, `(px)24`, `700`. `None` for references.
pub(crate) fn literal_text(value: &PropertyValue) -> Option<String> {
    match value {
        PropertyValue::Literal(s) => Some(s.clone()),
        PropertyValue::Dimension(d) => Some(d.to_kdl_string()),
        PropertyValue::TokenRef(_) | PropertyValue::DataRef(_) => None,
    }
}

impl LiteralValue {
    /// Read a fix-hint literal (see [`literal_text`]) as the kind
    /// `token_type` names. `None` for a type `zenith fix` cannot mint.
    pub(crate) fn from_hint(token_type: &str, literal: &str) -> Option<Self> {
        match token_type {
            "color" => Self::color(literal),
            "fontWeight" => Self::font_weight(literal),
            "fontFamily" => Self::font_family(literal),
            "dimension" => {
                let (unit, number) = literal.strip_prefix('(')?.split_once(')')?;
                Self::dimension(&Dimension {
                    value: number.parse().ok()?,
                    unit: Unit::from_annotation(unit),
                })
            }
            _ => None,
        }
    }
}

/// A token picked for a literal, with its value for display.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ValueMatch<'a> {
    /// The token id.
    pub id: &'a str,
    /// The token's resolved value as display text (`#f7f9fa`, `64px`, `700`).
    pub value: String,
    /// `true` when the token value equals the literal.
    pub exact: bool,
}

/// The dimension role a property implies: the token-id prefixes that hold
/// its tokens and the prefix `zenith fix` mints under.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DimensionRole {
    /// First dotted segments that belong to the role.
    pub prefixes: &'static [&'static str],
    /// Prefix for a minted token.
    pub mint: &'static str,
}

const FONT_SIZE: DimensionRole = DimensionRole {
    prefixes: &["size", "font-size", "text", "type"],
    mint: "size",
};
const RADIUS: DimensionRole = DimensionRole {
    prefixes: &["radius"],
    mint: "radius",
};
const STROKE: DimensionRole = DimensionRole {
    prefixes: &["border", "stroke"],
    mint: "stroke",
};
const SPACE: DimensionRole = DimensionRole {
    prefixes: &["space", "spacing", "gap"],
    mint: "space",
};

/// The dimension role of property `prop`, when it implies one.
pub(crate) fn dimension_role(prop: &str) -> Option<DimensionRole> {
    match prop {
        "font-size" => Some(FONT_SIZE),
        "stroke-width" | "border-width" => Some(STROKE),
        "padding" | "gap" | "margin" => Some(SPACE),
        p if p.contains("radius") => Some(RADIUS),
        p if p.starts_with("padding-") || p.starts_with("margin-") => Some(SPACE),
        _ => None,
    }
}

/// Normalize hex color text to lowercase `#rrggbb` or `#rrggbbaa`.
///
/// Short forms expand (`#fff` → `#ffffff`). An opaque alpha (`ff`) drops.
pub(crate) fn normalize_hex(text: &str) -> Option<String> {
    let digits = text.trim().strip_prefix('#')?;
    if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let lower = digits.to_ascii_lowercase();
    let expanded: String = match lower.len() {
        3 | 4 => lower.chars().flat_map(|c| [c, c]).collect(),
        6 | 8 => lower,
        _ => return None,
    };
    let trimmed = match expanded.strip_suffix("ff") {
        Some(rgb) if expanded.len() == 8 => rgb.to_owned(),
        _ => expanded,
    };
    Some(format!("#{trimmed}"))
}

/// Display text for a dimension: `64px`, `12pt`, `10.5px`.
pub(crate) fn dimension_text(d: &Dimension) -> String {
    let kdl = d.to_kdl_string();
    // `(px)64` → `64px`.
    match kdl.split_once(')') {
        Some((unit, n)) => format!("{n}{}", unit.trim_start_matches('(')),
        None => kdl,
    }
}

/// The exact-value token for `lit`, as `zenith fix` uses it.
///
/// Colors and font weights search every token of the type. Dimensions search
/// only the role tokens when `prop` implies a role. Families compare without
/// case. Ties go to the lexicographically smallest id.
pub(crate) fn exact_token<'a>(
    lit: &LiteralValue,
    prop: &str,
    resolved: &'a BTreeMap<String, ResolvedToken>,
) -> Option<&'a str> {
    match lit {
        LiteralValue::Dimension(d) => {
            let pool = dimension_pool(resolved, dimension_role(prop), false);
            let px = dim_to_px(d.value, &d.unit)?;
            pool.into_iter()
                .find(|(_, tpx, _)| (tpx - px).abs() < 1e-9)
                .map(|(id, _, _)| id)
        }
        LiteralValue::Color(_) | LiteralValue::FontFamily(_) | LiteralValue::FontWeight(_) => {
            best_token(lit, prop, resolved)
                .filter(|m| m.exact)
                .map(|m| m.id)
        }
    }
}

/// The best token for `lit`: the exact value, else the nearest one.
///
/// Dimensions search the role tokens first and fall back to every dimension
/// token when the role has none. Font families match only exactly.
pub(crate) fn best_token<'a>(
    lit: &LiteralValue,
    prop: &str,
    resolved: &'a BTreeMap<String, ResolvedToken>,
) -> Option<ValueMatch<'a>> {
    match lit {
        LiteralValue::Color(hex) => best_color(hex, resolved),
        LiteralValue::Dimension(d) => best_dimension(d, prop, resolved),
        LiteralValue::FontFamily(name) => resolved.iter().find_map(|(id, t)| {
            let ResolvedValue::FontFamily(f) = &t.value else {
                return None;
            };
            f.eq_ignore_ascii_case(name).then(|| ValueMatch {
                id: id.as_str(),
                value: f.clone(),
                exact: true,
            })
        }),
        LiteralValue::FontWeight(w) => {
            let mut best: Option<(&str, u32)> = None;
            for (id, t) in resolved {
                if let ResolvedValue::FontWeight(tw) = t.value {
                    let closer = best.is_none_or(|(_, bw)| tw.abs_diff(*w) < bw.abs_diff(*w));
                    if closer {
                        best = Some((id.as_str(), tw));
                    }
                }
            }
            best.map(|(id, tw)| ValueMatch {
                id,
                value: tw.to_string(),
                exact: tw == *w,
            })
        }
    }
}

fn best_color<'a>(
    hex: &str,
    resolved: &'a BTreeMap<String, ResolvedToken>,
) -> Option<ValueMatch<'a>> {
    let target = parse_rgb(hex).map(oklab);
    let mut best: Option<(&'a str, &'a str, f64)> = None;
    for (id, t) in resolved {
        let Some(token_hex) = t.value.as_color_hex() else {
            continue;
        };
        let same = normalize_hex(token_hex).is_some_and(|n| n == hex);
        let dist = if same {
            -1.0
        } else {
            match (target, parse_rgb(token_hex).map(oklab)) {
                (Some(a), Some(b)) => lab_distance_sq(a, b),
                _ => continue,
            }
        };
        if best.is_none_or(|(_, _, d)| dist < d) {
            best = Some((id.as_str(), token_hex, dist));
        }
    }
    best.map(|(id, value, dist)| ValueMatch {
        id,
        value: value.to_owned(),
        exact: dist < 0.0,
    })
}

/// `(id, px, dimension)` for every dimension token in the pool, sorted by id.
///
/// With a role, the pool holds the role tokens. When `fallback` is set and the
/// role has no tokens, the pool holds every dimension token.
fn dimension_pool(
    resolved: &BTreeMap<String, ResolvedToken>,
    role: Option<DimensionRole>,
    fallback: bool,
) -> Vec<(&str, f64, &Dimension)> {
    let all: Vec<(&str, f64, &Dimension)> = resolved
        .iter()
        .filter_map(|(id, t)| {
            let ResolvedValue::Dimension(d) = &t.value else {
                return None;
            };
            dim_to_px(d.value, &d.unit).map(|px| (id.as_str(), px, d))
        })
        .collect();
    let Some(role) = role else {
        return all;
    };
    let in_role: Vec<(&str, f64, &Dimension)> = all
        .iter()
        .copied()
        .filter(|(id, _, _)| {
            let first = id.split('.').next().unwrap_or(id);
            role.prefixes.contains(&first)
        })
        .collect();
    if in_role.is_empty() && fallback {
        all
    } else {
        in_role
    }
}

fn best_dimension<'a>(
    d: &Dimension,
    prop: &str,
    resolved: &'a BTreeMap<String, ResolvedToken>,
) -> Option<ValueMatch<'a>> {
    let px = dim_to_px(d.value, &d.unit)?;
    let mut best: Option<(&'a str, &'a Dimension, f64)> = None;
    for (id, tpx, td) in dimension_pool(resolved, dimension_role(prop), true) {
        let dist = (tpx - px).abs();
        if best.is_none_or(|(_, _, b)| dist < b) {
            best = Some((id, td, dist));
        }
    }
    best.map(|(id, td, dist)| ValueMatch {
        id,
        value: dimension_text(td),
        exact: dist < 1e-9,
    })
}

// ── OKLab ─────────────────────────────────────────────────────────────────────

/// The positive real `n`-th root of `x ≥ 0` by Newton's method from above.
///
/// Uses only `+ - * /`, so the result is identical on every IEEE-754 machine.
fn nth_root(x: f64, n: i32) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    let nf = f64::from(n);
    let mut y = x.max(1.0);
    for _ in 0..200 {
        let mut p = 1.0; // y^(n-1)
        for _ in 1..n {
            p *= y;
        }
        let next = y - (p * y - x) / (nf * p);
        if next >= y {
            break;
        }
        y = next;
    }
    y
}

/// sRGB channel (0..=255) to linear light.
fn srgb_to_linear(c: u8) -> f64 {
    let v = f64::from(c) / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        // base^2.4 = base^2 · (base^2)^(1/5).
        let base = (v + 0.055) / 1.055;
        let sq = base * base;
        sq * nth_root(sq, 5)
    }
}

/// OKLab `(L, a, b)` of an sRGB triple.
fn oklab((r, g, b): (u8, u8, u8)) -> (f64, f64, f64) {
    let (r, g, b) = (srgb_to_linear(r), srgb_to_linear(g), srgb_to_linear(b));
    let l = 0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b;
    let m = 0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b;
    let s = 0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b;
    let (l, m, s) = (nth_root(l, 3), nth_root(m, 3), nth_root(s, 3));
    (
        0.210_454_255_3 * l + 0.793_617_785_0 * m - 0.004_072_046_8 * s,
        1.977_998_495_1 * l - 2.428_592_205_0 * m + 0.450_593_709_9 * s,
        0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766_0 * s,
    )
}

/// Squared Euclidean OKLab distance.
fn lab_distance_sq(a: (f64, f64, f64), b: (f64, f64, f64)) -> f64 {
    let (dl, da, db) = (a.0 - b.0, a.1 - b.1, a.2 - b.2);
    dl * dl + da * da + db * db
}

/// The unit annotation used in a minted dimension id (`""` for px).
pub(crate) fn unit_suffix(unit: &Unit) -> &str {
    match unit {
        Unit::Px => "",
        Unit::Pt | Unit::Pct | Unit::Deg | Unit::Unknown(_) => unit.as_annotation(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::token::TokenType;

    fn color(hex: &str) -> ResolvedToken {
        ResolvedToken {
            token_type: TokenType::Color,
            value: ResolvedValue::Color(hex.to_owned()),
        }
    }

    fn px(v: f64) -> ResolvedToken {
        ResolvedToken {
            token_type: TokenType::Dimension,
            value: ResolvedValue::Dimension(Dimension {
                value: v,
                unit: Unit::Px,
            }),
        }
    }

    fn tokens(list: &[(&str, ResolvedToken)]) -> BTreeMap<String, ResolvedToken> {
        list.iter()
            .map(|(id, t)| ((*id).to_owned(), t.clone()))
            .collect()
    }

    fn cobalt() -> BTreeMap<String, ResolvedToken> {
        tokens(&[
            ("color.base.100", color("#f7f9fa")),
            ("color.base.content", color("#0d1529")),
            ("color.primary", color("#605dff")),
            ("border.width", px(1.0)),
            ("radius.box", px(32.0)),
            ("radius.field", px(4.0)),
            ("size.h1", px(64.0)),
            ("size.h2", px(40.0)),
            ("size.body", px(28.0)),
            ("space.unit", px(4.0)),
        ])
    }

    #[test]
    fn nth_root_matches_known_values() {
        assert!((nth_root(27.0, 3) - 3.0).abs() < 1e-12);
        assert!((nth_root(0.008, 3) - 0.2).abs() < 1e-12);
        assert!((nth_root(32.0, 5) - 2.0).abs() < 1e-12);
        assert_eq!(nth_root(0.0, 3), 0.0);
    }

    #[test]
    fn oklab_white_and_black() {
        let (l, a, b) = oklab((255, 255, 255));
        assert!((l - 1.0).abs() < 1e-4, "{l}");
        assert!(a.abs() < 1e-4 && b.abs() < 1e-4, "{a} {b}");
        let (l, _, _) = oklab((0, 0, 0));
        assert!(l.abs() < 1e-12);
    }

    #[test]
    fn normalize_hex_forms() {
        assert_eq!(normalize_hex("#FFF").as_deref(), Some("#ffffff"));
        assert_eq!(normalize_hex("#ffffffff").as_deref(), Some("#ffffff"));
        assert_eq!(normalize_hex("#11223380").as_deref(), Some("#11223380"));
        assert_eq!(normalize_hex("red"), None);
        assert_eq!(normalize_hex("#12345"), None);
    }

    #[test]
    fn color_exact_match_wins() {
        let mut t = cobalt();
        t.insert("color.white".into(), color("#ffffff"));
        let lit = LiteralValue::color("#FFFFFF").expect("hex");
        let m = best_token(&lit, "fill", &t).expect("match");
        assert_eq!((m.id, m.exact), ("color.white", true));
        assert_eq!(exact_token(&lit, "fill", &t), Some("color.white"));
    }

    #[test]
    fn color_nearest_by_oklab() {
        let lit = LiteralValue::color("#ffffff").expect("hex");
        let t = cobalt();
        let m = best_token(&lit, "fill", &t).expect("match");
        assert_eq!(m.id, "color.base.100");
        assert_eq!(m.value, "#f7f9fa");
        assert!(!m.exact);
        assert_eq!(exact_token(&lit, "fill", &cobalt()), None);
    }

    #[test]
    fn dimension_exact_within_role() {
        let lit = LiteralValue::Dimension(Dimension {
            value: 64.0,
            unit: Unit::Px,
        });
        assert_eq!(exact_token(&lit, "font-size", &cobalt()), Some("size.h1"));
    }

    #[test]
    fn dimension_exact_outside_role_is_not_a_fix() {
        // `space.unit` is 4px, but a font size must not reference it.
        let lit = LiteralValue::Dimension(Dimension {
            value: 4.0,
            unit: Unit::Px,
        });
        assert_eq!(exact_token(&lit, "font-size", &cobalt()), None);
        assert_eq!(exact_token(&lit, "radius", &cobalt()), Some("radius.field"));
        assert_eq!(
            exact_token(&lit, "x-offset", &cobalt()),
            Some("radius.field"),
            "no role: every dimension token counts; smallest id wins"
        );
    }

    #[test]
    fn dimension_nearest_within_role() {
        let lit = LiteralValue::Dimension(Dimension {
            value: 96.0,
            unit: Unit::Px,
        });
        let t = cobalt();
        let m = best_token(&lit, "font-size", &t).expect("match");
        assert_eq!(
            (m.id, m.value.as_str(), m.exact),
            ("size.h1", "64px", false)
        );
        let lit = LiteralValue::Dimension(Dimension {
            value: 24.0,
            unit: Unit::Px,
        });
        let t = cobalt();
        let m = best_token(&lit, "radius", &t).expect("match");
        assert_eq!(m.id, "radius.box");
    }

    #[test]
    fn dimension_role_without_tokens_falls_back_to_all() {
        let lit = LiteralValue::Dimension(Dimension {
            value: 2.0,
            unit: Unit::Px,
        });
        let t = tokens(&[("space.unit", px(4.0)), ("border.width", px(1.0))]);
        let m = best_token(&lit, "radius", &t).expect("match");
        assert_eq!(m.id, "border.width");
    }

    #[test]
    fn font_family_matches_without_case() {
        let t = tokens(&[(
            "font.body",
            ResolvedToken {
                token_type: TokenType::FontFamily,
                value: ResolvedValue::FontFamily("Noto Sans".into()),
            },
        )]);
        let lit = LiteralValue::font_family("noto sans").expect("name");
        assert_eq!(exact_token(&lit, "font-family", &t), Some("font.body"));
        let lit = LiteralValue::font_family("Inter").expect("name");
        assert_eq!(best_token(&lit, "font-family", &t), None);
    }

    #[test]
    fn font_weight_exact_numeric() {
        let weight = |w| ResolvedToken {
            token_type: TokenType::FontWeight,
            value: ResolvedValue::FontWeight(w),
        };
        let t = tokens(&[("weight.bold", weight(700)), ("weight.thin", weight(100))]);
        let lit = LiteralValue::font_weight("700").expect("n");
        assert_eq!(exact_token(&lit, "font-weight", &t), Some("weight.bold"));
        let lit = LiteralValue::font_weight("600").expect("n");
        assert_eq!(exact_token(&lit, "font-weight", &t), None);
        let m = best_token(&lit, "font-weight", &t).expect("nearest");
        assert_eq!(m.id, "weight.bold");
    }

    #[test]
    fn hint_literal_round_trips() {
        let d = PropertyValue::Dimension(Dimension {
            value: 10.5,
            unit: Unit::Pt,
        });
        let text = literal_text(&d).expect("text");
        assert_eq!(text, "(pt)10.5");
        assert_eq!(
            LiteralValue::from_hint("dimension", &text),
            Some(LiteralValue::Dimension(Dimension {
                value: 10.5,
                unit: Unit::Pt
            }))
        );
        assert_eq!(
            LiteralValue::from_hint("fontWeight", "700"),
            Some(LiteralValue::FontWeight(700))
        );
        assert_eq!(LiteralValue::from_hint("shadow", "x"), None);
    }

    #[test]
    fn dimension_text_forms() {
        let d = Dimension {
            value: 10.5,
            unit: Unit::Pt,
        };
        assert_eq!(dimension_text(&d), "10.5pt");
    }
}
