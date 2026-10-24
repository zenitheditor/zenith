//! Lazy bundled and fallback fonts over the wire: turn the font files that
//! follow the envelope into faces for the engine. Face lists and notices
//! come from `zenith_editor::fonts`.
//!
//! The module embeds only Noto Sans Regular and Bold (see the
//! `bundled-fonts-extended` feature of `zenith-core`) and no fallback face.
//! The other eight bundled faces and the four fallback faces are plain
//! files the page fetches and sends after the envelope, keyed by file name
//! (`NotoSerif-Bold.ttf`, `NotoEmoji-Regular.ttf`). Such a face registers
//! exactly as the embedded face does, so the output equals the full build.

use std::collections::BTreeMap;

use zenith_editor::fonts::bundled_font_files;
use zenith_pipeline::ExtraFont;

use crate::protocol::ErrorBody;

/// Faces for the provider from `fonts` (file name → raw bytes).
///
/// # Errors
///
/// `request.invalid_params` when a name is not a bundled or fallback font
/// file name. Project fonts a document declares as assets go in `files`,
/// not here.
pub(crate) fn decode_fonts(fonts: BTreeMap<String, Vec<u8>>) -> Result<Vec<ExtraFont>, ErrorBody> {
    bundled_font_files(fonts).map_err(|e| ErrorBody {
        code: "request.invalid_params".to_owned(),
        ..ErrorBody::from(e)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::{FontSource, FontStyle};

    #[test]
    fn unknown_name_is_rejected_with_the_known_names() {
        let mut fonts = BTreeMap::new();
        fonts.insert("Inter.ttf".to_owned(), b"x".to_vec());
        let err = decode_fonts(fonts).expect_err("unknown name");
        assert_eq!(err.code, "request.invalid_params");
        assert!(
            err.message.contains("NotoSerif-Bold.ttf"),
            "{}",
            err.message
        );
    }

    #[test]
    fn bundled_name_registers_as_the_bundled_face() {
        let mut fonts = BTreeMap::new();
        fonts.insert("notoserif-bold".to_owned(), b"abc".to_vec());
        let out = decode_fonts(fonts).expect("decodes");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].family, "Noto Serif");
        assert_eq!(out[0].weight, 700);
        assert_eq!(out[0].style, FontStyle::Normal);
        assert_eq!(out[0].source, FontSource::Bundled);
        assert_eq!(&out[0].bytes[..], b"abc");
    }
}
