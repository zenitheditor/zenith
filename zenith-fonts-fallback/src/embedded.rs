//! Compile-time bytes of the fallback faces, with the `embed` feature.

/// Noto Sans Arabic Regular, instanced at weight 400, width 100.
pub(crate) const NOTO_SANS_ARABIC: &[u8] = include_bytes!("../fonts/NotoSansArabic-Regular.ttf");
/// Noto Sans Hebrew Regular, instanced at weight 400, width 100.
pub(crate) const NOTO_SANS_HEBREW: &[u8] = include_bytes!("../fonts/NotoSansHebrew-Regular.ttf");
/// Noto Sans CJK SC Regular, subset to `fonts/cjk-sc-codepoints.txt`.
pub(crate) const NOTO_SANS_CJK_SC: &[u8] = include_bytes!("../fonts/NotoSansCJKsc-Fallback.otf");
/// Noto Emoji (monochrome), instanced at weight 400.
pub(crate) const NOTO_EMOJI: &[u8] = include_bytes!("../fonts/NotoEmoji-Regular.ttf");
