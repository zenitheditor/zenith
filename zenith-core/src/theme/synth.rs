//! Palette synthesis from brand seed colors.
//!
//! Given a primary (plus optional role overrides) and a light/dark scheme, this
//! derives the full theme colour contract — surfaces, every role with a readable
//! `.content` foreground, and status colours — picking foregrounds by APCA
//! (WCAG 3) contrast so text on any role is legible by construction.

use crate::color::best_text_color;

/// An sRGB colour triple.
pub type Rgb = (u8, u8, u8);

/// Light or dark base scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    /// Near-white surfaces, dark text.
    Light,
    /// Near-black surfaces, light text.
    Dark,
}

/// Brand input: a primary plus optional overrides. Unset roles are derived.
#[derive(Debug, Clone)]
pub struct PaletteSpec {
    /// Light or dark base.
    pub scheme: Scheme,
    /// The brand's primary colour (required).
    pub primary: Rgb,
    /// Secondary colour; defaults to the primary when absent.
    pub secondary: Option<Rgb>,
    /// Accent colour; defaults to the secondary (or primary) when absent.
    pub accent: Option<Rgb>,
    /// Neutral colour; derived from a tinted grey when absent.
    pub neutral: Option<Rgb>,
    /// Status colours; sensible universal hues are used when absent.
    pub info: Option<Rgb>,
    /// See [`PaletteSpec::info`].
    pub success: Option<Rgb>,
    /// See [`PaletteSpec::info`].
    pub warning: Option<Rgb>,
    /// See [`PaletteSpec::info`].
    pub error: Option<Rgb>,
}

/// The two foreground candidates every `.content` colour is chosen between.
const DARK_FG: Rgb = (24, 26, 30);
const LIGHT_FG: Rgb = (247, 248, 250);

// Universal, accessible status hues used unless the brand overrides them.
const INFO: Rgb = (14, 165, 233);
const SUCCESS: Rgb = (22, 163, 74);
const WARNING: Rgb = (245, 158, 11);
const ERROR: Rgb = (239, 68, 68);

/// The ordered token-id suffixes every synthesized palette emits. Each entry is
/// the part after `color.` — e.g. `"base.100"`, `"primary.content"`.
pub const PALETTE_ORDER: [&str; 20] = [
    "base.100",
    "base.200",
    "base.300",
    "base.content",
    "primary",
    "primary.content",
    "secondary",
    "secondary.content",
    "accent",
    "accent.content",
    "neutral",
    "neutral.content",
    "info",
    "info.content",
    "success",
    "success.content",
    "warning",
    "warning.content",
    "error",
    "error.content",
];

/// Linearly mix two colours; `t = 0.0` is `a`, `t = 1.0` is `b`.
fn mix(a: Rgb, b: Rgb, t: f64) -> Rgb {
    let m = |x: u8, y: u8| -> u8 {
        let v = x as f64 * (1.0 - t) + y as f64 * t;
        v.round().clamp(0.0, 255.0) as u8
    };
    (m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
}

/// A readable foreground for text on `bg` (near-black or near-white), by APCA.
fn content(bg: Rgb) -> Rgb {
    best_text_color(bg, DARK_FG, LIGHT_FG)
}

/// Derive the full ordered palette as `(id-suffix, rgb)` pairs, in
/// [`PALETTE_ORDER`]. The id suffix is the part after `color.`.
pub fn synth_palette(spec: &PaletteSpec) -> Vec<(&'static str, Rgb)> {
    let primary = spec.primary;
    let secondary = spec.secondary.unwrap_or(primary);
    let accent = spec.accent.unwrap_or(secondary);

    // Surfaces: near-white (light) / near-black (dark), faintly tinted toward
    // the brand hue for cohesion. Base text is the APCA-best foreground.
    let (b100, b200, b300, neutral_default) = match spec.scheme {
        Scheme::Light => (
            mix((248, 248, 249), primary, 0.02),
            mix((241, 242, 244), primary, 0.03),
            mix((227, 229, 233), primary, 0.05),
            mix((72, 78, 90), primary, 0.10),
        ),
        Scheme::Dark => (
            mix((12, 14, 18), primary, 0.05),
            mix((22, 24, 29), primary, 0.05),
            mix((35, 39, 48), primary, 0.05),
            mix((68, 74, 86), primary, 0.10),
        ),
    };
    let neutral = spec.neutral.unwrap_or(neutral_default);
    let info = spec.info.unwrap_or(INFO);
    let success = spec.success.unwrap_or(SUCCESS);
    let warning = spec.warning.unwrap_or(WARNING);
    let error = spec.error.unwrap_or(ERROR);

    vec![
        ("base.100", b100),
        ("base.200", b200),
        ("base.300", b300),
        ("base.content", content(b100)),
        ("primary", primary),
        ("primary.content", content(primary)),
        ("secondary", secondary),
        ("secondary.content", content(secondary)),
        ("accent", accent),
        ("accent.content", content(accent)),
        ("neutral", neutral),
        ("neutral.content", content(neutral)),
        ("info", info),
        ("info.content", content(info)),
        ("success", success),
        ("success.content", content(success)),
        ("warning", warning),
        ("warning.content", content(warning)),
        ("error", error),
        ("error.content", content(error)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::apca_lc;

    fn spec(scheme: Scheme, primary: Rgb) -> PaletteSpec {
        PaletteSpec {
            scheme,
            primary,
            secondary: None,
            accent: None,
            neutral: None,
            info: None,
            success: None,
            warning: None,
            error: None,
        }
    }

    fn get(p: &[(&'static str, Rgb)], id: &str) -> Rgb {
        p.iter().find(|(k, _)| *k == id).expect("role present").1
    }

    /// Every `<role>` / `<role>.content` pair must be legible by APCA in BOTH
    /// schemes: body text on the base surface ≥ 75 Lc, role labels ≥ 45 Lc.
    #[test]
    fn content_pairs_meet_apca_in_light_and_dark() {
        for scheme in [Scheme::Light, Scheme::Dark] {
            for primary in [(124, 58, 237), (34, 197, 94), (252, 183, 0), (248, 40, 52)] {
                let p = synth_palette(&spec(scheme, primary));
                let base_lc = apca_lc(get(&p, "base.content"), get(&p, "base.100")).abs();
                assert!(
                    base_lc >= 75.0,
                    "{scheme:?} primary {primary:?}: base text Lc {base_lc:.1} < 75"
                );
                for role in [
                    "primary",
                    "secondary",
                    "accent",
                    "neutral",
                    "info",
                    "success",
                    "warning",
                    "error",
                ] {
                    let lc = apca_lc(get(&p, &format!("{role}.content")), get(&p, role)).abs();
                    assert!(
                        lc >= 45.0,
                        "{scheme:?} {primary:?}: {role} label Lc {lc:.1} < 45"
                    );
                }
            }
        }
    }

    #[test]
    fn synthesis_is_deterministic() {
        let a = synth_palette(&spec(Scheme::Light, (97, 93, 255)));
        let b = synth_palette(&spec(Scheme::Light, (97, 93, 255)));
        assert_eq!(a, b);
    }

    #[test]
    fn overrides_are_respected() {
        let mut s = spec(Scheme::Dark, (10, 20, 30));
        s.accent = Some((255, 0, 128));
        let p = synth_palette(&s);
        assert_eq!(get(&p, "accent"), (255, 0, 128));
    }

    #[test]
    fn order_is_the_contract() {
        let p = synth_palette(&spec(Scheme::Light, (100, 100, 100)));
        let ids: Vec<&str> = p.iter().map(|(k, _)| *k).collect();
        assert_eq!(ids, PALETTE_ORDER.to_vec());
    }
}
