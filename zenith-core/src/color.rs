//! Colour math — hex/CMYK parsing, WCAG 2.2 luminance/contrast, and the
//! WCAG 3 candidate APCA `Lc`.
//!
//! All functions are pure and panic-free.  Alpha is ignored for luminance
//! (WCAG 2.2 §1.4.3 measures opaque foreground on opaque background).

// ── Public API ────────────────────────────────────────────────────────────────

/// Parse a `#rrggbb` or `#rrggbbaa` hex string into `(r, g, b)` as raw u8 bytes.
///
/// The leading `#` is required.  Only 6-digit and 8-digit forms are accepted;
/// 3-digit short-form is NOT (Zenith tokens always store canonical long form).
/// Alpha is silently discarded — only the RGB channels are returned.
///
/// Returns `None` on any parse error or unexpected length.
pub fn parse_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let hex = hex.strip_prefix('#')?;
    // Accept exactly 6 (rrggbb) or 8 (rrggbbaa) hex digits.
    if hex.len() != 6 && hex.len() != 8 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some((r, g, b))
}

/// A parsed CMYK color: four channels each a percentage in `0.0..=100.0`.
///
/// Stored as `f32` to match the scene-IR `cmyk` tag. Carries no alpha (CMYK is
/// always opaque in v0; the converted sRGB alpha is `255`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cmyk {
    pub c: f32,
    pub m: f32,
    pub y: f32,
    pub k: f32,
}

/// Parse a `cmyk(c,m,y,k)` color string.
///
/// Each of `c`, `m`, `y`, `k` is a percentage in `0..=100` (integer or decimal,
/// comma-separated, optional surrounding spaces). Examples:
/// `cmyk(59,85,0,7)`, `cmyk(0, 0, 0, 100)`, `cmyk(12.5, 0, 0, 0)`.
///
/// Returns `None` on any malformed input or out-of-range channel; never panics.
pub fn parse_cmyk(s: &str) -> Option<Cmyk> {
    let inner = s.strip_prefix("cmyk(")?.strip_suffix(')')?;
    let mut parts = inner.split(',');
    let c = parse_pct(parts.next()?)?;
    let m = parse_pct(parts.next()?)?;
    let y = parse_pct(parts.next()?)?;
    let k = parse_pct(parts.next()?)?;
    // Reject any trailing component (e.g. `cmyk(1,2,3,4,5)`).
    if parts.next().is_some() {
        return None;
    }
    Some(Cmyk { c, m, y, k })
}

/// Parse one CMYK channel: a trimmed decimal in `0.0..=100.0`. Returns `None`
/// when the token is empty, non-numeric, non-finite, or out of range.
fn parse_pct(tok: &str) -> Option<f32> {
    let v: f32 = tok.trim().parse().ok()?;
    if v.is_finite() && (0.0..=100.0).contains(&v) {
        Some(v)
    } else {
        None
    }
}

/// Convert CMYK percentages to an sRGB `(r, g, b)` triple using the standard
/// naive device conversion, rounded deterministically to the nearest `u8`.
///
/// `R = 255*(1-c/100)*(1-k/100)`, and likewise for G (from m) and B (from y).
/// The result is always opaque (caller supplies alpha `255`).
pub fn cmyk_to_srgb(cmyk: Cmyk) -> (u8, u8, u8) {
    let chan = |v: f32, kk: f32| -> u8 {
        let f = 255.0_f32 * (1.0 - v / 100.0) * (1.0 - kk / 100.0);
        // `round()` is half-away-from-zero and deterministic; clamp guards
        // against any float drift outside 0..=255 before the cast.
        f.round().clamp(0.0, 255.0) as u8
    };
    (
        chan(cmyk.c, cmyk.k),
        chan(cmyk.m, cmyk.k),
        chan(cmyk.y, cmyk.k),
    )
}

/// Format a CMYK color as the canonical lowercase `#rrggbb` sRGB hex string of
/// its naive device conversion (alpha is always opaque, so the 6-digit form is
/// used). Deterministic; used both by token resolution and the formatter.
pub fn cmyk_to_hex(cmyk: Cmyk) -> String {
    let (r, g, b) = cmyk_to_srgb(cmyk);
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// Format an sRGB triple as a lowercase `#rrggbb` hex string.
pub fn rgb_to_hex(rgb: (u8, u8, u8)) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb.0, rgb.1, rgb.2)
}

/// WCAG 2.2 relative luminance of an sRGB color, in the range 0.0..=1.0.
///
/// Formula: <https://www.w3.org/TR/WCAG22/#dfn-relative-luminance>
pub fn relative_luminance(rgb: (u8, u8, u8)) -> f64 {
    let linearize = |channel: u8| -> f64 {
        let c = channel as f64 / 255.0;
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let r = linearize(rgb.0);
    let g = linearize(rgb.1);
    let b = linearize(rgb.2);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// WCAG 2.2 contrast ratio between two colors, in the range 1.0..=21.0.
///
/// The order of the arguments does not matter — the brighter color is always
/// placed in the numerator.
///
/// Formula: `(L1 + 0.05) / (L2 + 0.05)` where `L1 >= L2`.
pub fn contrast_ratio(a: (u8, u8, u8), b: (u8, u8, u8)) -> f64 {
    let la = relative_luminance(a);
    let lb = relative_luminance(b);
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// APCA (WCAG 3 candidate, "APCA-W3") lightness contrast `Lc`.
///
/// Returns a signed value roughly in `-108.0..=106.0`: **positive** for dark
/// text on a light background, **negative** for light text on a dark background;
/// a magnitude near `0.0` means no usable contrast. Unlike the WCAG 2.2
/// [`contrast_ratio`], APCA is polarity-aware, so argument order matters —
/// `text` is the foreground, `bg` the background.
///
/// Rough readability guide (use the absolute value): `>= 90` ≈ any text;
/// `>= 75` fluent body text; `>= 60` body/large; `>= 45` large/bold only.
///
/// Reference: <https://github.com/Myndex/apca-w3>
pub fn apca_lc(text: (u8, u8, u8), bg: (u8, u8, u8)) -> f64 {
    const TRC: f64 = 2.4;
    const RCO: f64 = 0.212_672_9;
    const GCO: f64 = 0.715_152_2;
    const BCO: f64 = 0.072_175_0;
    const BLK_THRS: f64 = 0.022;
    const BLK_CLMP: f64 = 1.414;
    const DELTA_Y_MIN: f64 = 0.0005;
    const LO_CLIP: f64 = 0.1;
    const NORM_BG: f64 = 0.56;
    const NORM_TXT: f64 = 0.57;
    const REV_TXT: f64 = 0.62;
    const REV_BG: f64 = 0.65;
    const SCALE: f64 = 1.14;
    const LO_OFFSET: f64 = 0.027;

    let screen_y = |rgb: (u8, u8, u8)| -> f64 {
        let ch = |c: u8| (c as f64 / 255.0).powf(TRC);
        let mut y = RCO * ch(rgb.0) + GCO * ch(rgb.1) + BCO * ch(rgb.2);
        if y < BLK_THRS {
            y += (BLK_THRS - y).powf(BLK_CLMP);
        }
        y
    };

    let ytxt = screen_y(text);
    let ybg = screen_y(bg);
    if (ybg - ytxt).abs() < DELTA_Y_MIN {
        return 0.0;
    }
    let output = if ybg > ytxt {
        let sapc = (ybg.powf(NORM_BG) - ytxt.powf(NORM_TXT)) * SCALE;
        if sapc < LO_CLIP {
            0.0
        } else {
            sapc - LO_OFFSET
        }
    } else {
        let sapc = (ybg.powf(REV_BG) - ytxt.powf(REV_TXT)) * SCALE;
        if sapc > -LO_CLIP {
            0.0
        } else {
            sapc + LO_OFFSET
        }
    };
    output * 100.0
}

/// The better of two candidate foregrounds for text on `bg`, by APCA contrast.
///
/// Returns whichever of `a` / `b` yields the larger `|Lc|` against `bg` — the
/// WCAG-3 analogue of picking black-vs-white text for maximum readability.
pub fn best_text_color(bg: (u8, u8, u8), a: (u8, u8, u8), b: (u8, u8, u8)) -> (u8, u8, u8) {
    if apca_lc(a, bg).abs() >= apca_lc(b, bg).abs() {
        a
    } else {
        b
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // apca_lc ───────────────────────────────────────────────────────────────
    #[test]
    fn apca_black_on_white_approx_106() {
        let lc = apca_lc((0, 0, 0), (255, 255, 255));
        assert!(
            (lc - 106.04).abs() < 0.5,
            "black-on-white Lc should be ~106, got {lc}"
        );
    }

    #[test]
    fn apca_white_on_black_is_negative_approx_108() {
        let lc = apca_lc((255, 255, 255), (0, 0, 0));
        assert!(
            (lc + 107.88).abs() < 0.5,
            "white-on-black Lc should be ~-108, got {lc}"
        );
    }

    #[test]
    fn apca_same_color_is_zero() {
        assert_eq!(apca_lc((120, 120, 120), (120, 120, 120)), 0.0);
    }

    #[test]
    fn apca_polarity_sign() {
        // dark text on light bg → positive; light text on dark bg → negative.
        assert!(apca_lc((20, 20, 20), (240, 240, 240)) > 0.0);
        assert!(apca_lc((240, 240, 240), (20, 20, 20)) < 0.0);
    }

    #[test]
    fn best_text_color_picks_higher_contrast() {
        let dark = (24, 26, 30);
        let light = (247, 248, 250);
        // On a light surface, dark text wins; on a dark surface, light text wins.
        assert_eq!(best_text_color((250, 250, 250), dark, light), dark);
        assert_eq!(best_text_color((10, 12, 16), dark, light), light);
        // On a mid green, the higher-|Lc| option is chosen (light here).
        let g = (34, 197, 94);
        let chosen = best_text_color(g, dark, light);
        assert!(
            apca_lc(chosen, g).abs() >= apca_lc(dark, g).abs().max(apca_lc(light, g).abs()) - 1e-9
        );
    }

    // parse_rgb ─────────────────────────────────────────────────────────────

    #[test]
    fn parse_rgb_6_digits() {
        assert_eq!(parse_rgb("#ffffff"), Some((255, 255, 255)));
        assert_eq!(parse_rgb("#000000"), Some((0, 0, 0)));
        assert_eq!(parse_rgb("#aabbcc"), Some((0xaa, 0xbb, 0xcc)));
    }

    #[test]
    fn parse_rgb_8_digits_ignores_alpha() {
        // Alpha byte (ff) is dropped; RGB is preserved.
        assert_eq!(parse_rgb("#aabbccff"), Some((0xaa, 0xbb, 0xcc)));
        assert_eq!(parse_rgb("#ffffff80"), Some((255, 255, 255)));
    }

    #[test]
    fn parse_rgb_rejects_bad_input() {
        assert_eq!(parse_rgb("ffffff"), None); // missing #
        assert_eq!(parse_rgb("#fff"), None); // 3-digit short form
        assert_eq!(parse_rgb("#fffffg"), None); // invalid hex digit
        assert_eq!(parse_rgb(""), None);
        assert_eq!(parse_rgb("#"), None);
    }

    // parse_cmyk / cmyk_to_srgb ─────────────────────────────────────────────

    #[test]
    fn cmyk_zero_is_white() {
        let c = parse_cmyk("cmyk(0,0,0,0)").expect("must parse");
        assert_eq!(cmyk_to_srgb(c), (255, 255, 255));
        assert_eq!(cmyk_to_hex(c), "#ffffff");
    }

    #[test]
    fn cmyk_full_k_is_black() {
        let c = parse_cmyk("cmyk(0,0,0,100)").expect("must parse");
        assert_eq!(cmyk_to_srgb(c), (0, 0, 0));
        assert_eq!(cmyk_to_hex(c), "#000000");
    }

    #[test]
    fn cmyk_violet_converts_to_expected_purple() {
        let c = parse_cmyk("cmyk(59,85,0,7)").expect("must parse");
        // R = 255*(1-0.59)*(1-0.07) = 255*0.41*0.93 = 97.23 -> 97 (0x61)
        // G = 255*(1-0.85)*(1-0.07) = 255*0.15*0.93 = 35.57 -> 36 (0x24)
        // B = 255*(1-0.00)*(1-0.07) = 255*1.00*0.93 = 237.15 -> 237 (0xed)
        assert_eq!(cmyk_to_srgb(c), (97, 36, 237));
        assert_eq!(cmyk_to_hex(c), "#6124ed");
    }

    #[test]
    fn cmyk_accepts_spaces_and_decimals() {
        let c = parse_cmyk("cmyk( 12.5 , 0 , 0 , 0 )").expect("must parse");
        assert_eq!(c.c, 12.5);
        assert_eq!(c.m, 0.0);
    }

    #[test]
    fn cmyk_rejects_malformed_and_out_of_range() {
        assert!(parse_cmyk("cmyk(0,0,0)").is_none()); // too few
        assert!(parse_cmyk("cmyk(0,0,0,0,0)").is_none()); // too many
        assert!(parse_cmyk("cmyk(0,0,0,101)").is_none()); // out of range
        assert!(parse_cmyk("cmyk(-1,0,0,0)").is_none()); // negative
        assert!(parse_cmyk("cmyk(a,0,0,0)").is_none()); // non-numeric
        assert!(parse_cmyk("rgb(0,0,0,0)").is_none()); // wrong prefix
        assert!(parse_cmyk("cmyk(0,0,0,0").is_none()); // missing paren
        assert!(parse_cmyk("#ffffff").is_none()); // hex, not cmyk
    }

    // relative_luminance ────────────────────────────────────────────────────

    #[test]
    fn luminance_black_is_zero() {
        let l = relative_luminance((0, 0, 0));
        assert!(l.abs() < 1e-10, "black luminance should be 0, got {l}");
    }

    #[test]
    fn luminance_white_is_one() {
        let l = relative_luminance((255, 255, 255));
        assert!(
            (l - 1.0).abs() < 1e-6,
            "white luminance should be ~1, got {l}"
        );
    }

    // contrast_ratio ────────────────────────────────────────────────────────

    #[test]
    fn contrast_white_vs_black_approx_21() {
        let ratio = contrast_ratio((255, 255, 255), (0, 0, 0));
        assert!(
            (ratio - 21.0).abs() < 0.1,
            "white/black ratio should be ~21, got {ratio}"
        );
    }

    #[test]
    fn contrast_white_vs_white_is_1() {
        let ratio = contrast_ratio((255, 255, 255), (255, 255, 255));
        assert!(
            (ratio - 1.0).abs() < 1e-6,
            "same color ratio should be 1, got {ratio}"
        );
    }

    #[test]
    fn contrast_gray_777_on_white_approx_4_48() {
        // #777777 on #ffffff — well-known WCAG reference pair ≈ 4.48:1.
        let ratio = contrast_ratio((0x77, 0x77, 0x77), (255, 255, 255));
        assert!(
            (ratio - 4.48).abs() < 0.1,
            "#777777 on white should be ~4.48, got {ratio}"
        );
    }

    #[test]
    fn contrast_ratio_is_symmetric() {
        let ab = contrast_ratio((0x77, 0x77, 0x77), (255, 255, 255));
        let ba = contrast_ratio((255, 255, 255), (0x77, 0x77, 0x77));
        assert!(
            (ab - ba).abs() < 1e-10,
            "contrast_ratio must be symmetric: {ab} vs {ba}"
        );
    }
}
