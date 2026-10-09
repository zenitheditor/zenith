//! Compile-time bytes of the bundled Noto fonts.
//!
//! These are the deterministic default faces used by [`super::default_provider`].
//! They live under `zenith-core/assets/fonts/` so the crate is self-contained
//! when published, and are exposed publicly so other workspace crates (e.g.
//! `zenith-layout`) can reuse the exact same bytes without embedding a copy.
//!
//! Noto Sans Regular and Bold are always embedded. The other eight faces
//! (about 5.2 MB) are embedded only with the `bundled-fonts-extended`
//! feature, which is on by default.

/// Noto Sans Regular — `"Noto Sans"`, weight 400, normal.
pub const NOTO_SANS_REGULAR: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
/// Noto Sans Bold — `"Noto Sans"`, weight 700, normal.
pub const NOTO_SANS_BOLD: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Bold.ttf");
/// Noto Sans Italic — `"Noto Sans"`, weight 400, italic.
#[cfg(feature = "bundled-fonts-extended")]
pub const NOTO_SANS_ITALIC: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Italic.ttf");
/// Noto Sans Bold Italic — `"Noto Sans"`, weight 700, italic.
#[cfg(feature = "bundled-fonts-extended")]
pub const NOTO_SANS_BOLD_ITALIC: &[u8] =
    include_bytes!("../../assets/fonts/NotoSans-BoldItalic.ttf");
/// Noto Sans Mono Regular — `"Noto Sans Mono"`, weight 400, normal.
#[cfg(feature = "bundled-fonts-extended")]
pub const NOTO_SANS_MONO_REGULAR: &[u8] =
    include_bytes!("../../assets/fonts/NotoSansMono-Regular.ttf");
/// Noto Sans Mono Bold — `"Noto Sans Mono"`, weight 700, normal.
#[cfg(feature = "bundled-fonts-extended")]
pub const NOTO_SANS_MONO_BOLD: &[u8] = include_bytes!("../../assets/fonts/NotoSansMono-Bold.ttf");
/// Noto Serif Regular — `"Noto Serif"`, weight 400, normal.
#[cfg(feature = "bundled-fonts-extended")]
pub const NOTO_SERIF_REGULAR: &[u8] = include_bytes!("../../assets/fonts/NotoSerif-Regular.ttf");
/// Noto Serif Bold — `"Noto Serif"`, weight 700, normal.
#[cfg(feature = "bundled-fonts-extended")]
pub const NOTO_SERIF_BOLD: &[u8] = include_bytes!("../../assets/fonts/NotoSerif-Bold.ttf");
/// Noto Serif Italic — `"Noto Serif"`, weight 400, italic.
#[cfg(feature = "bundled-fonts-extended")]
pub const NOTO_SERIF_ITALIC: &[u8] = include_bytes!("../../assets/fonts/NotoSerif-Italic.ttf");
/// Noto Serif Bold Italic — `"Noto Serif"`, weight 700, italic.
#[cfg(feature = "bundled-fonts-extended")]
pub const NOTO_SERIF_BOLD_ITALIC: &[u8] =
    include_bytes!("../../assets/fonts/NotoSerif-BoldItalic.ttf");
