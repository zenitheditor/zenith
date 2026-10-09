//! Lazy bundled fonts over the wire: decode the base64 `fonts` request map
//! for the engine. Face lists and notices come from `zenith_editor::fonts`.
//!
//! The module embeds only Noto Sans Regular and Bold (see the
//! `bundled-fonts-extended` feature of `zenith-core`). The other eight
//! bundled faces are plain files the page fetches and passes back in
//! `fonts`, keyed by file name (`NotoSerif-Bold.ttf`). Such a face registers
//! exactly as the embedded face does, so the output equals the full build.

use std::collections::BTreeMap;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use zenith_editor::fonts::bundled_font_files;
use zenith_pipeline::ExtraFont;

use crate::protocol::ErrorBody;

/// The raw bytes of `fonts` (file name → base64).
///
/// # Errors
///
/// `request.invalid_params` naming the font whose bytes are not base64.
pub(crate) fn decode_font_bytes(
    fonts: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, Vec<u8>>, ErrorBody> {
    fonts
        .iter()
        .map(|(name, b64)| {
            STANDARD
                .decode(b64)
                .map(|bytes| (name.clone(), bytes))
                .map_err(|e| {
                    ErrorBody::new(
                        "request.invalid_params",
                        format!("fonts['{name}'] is not valid base64: {e}; send standard base64"),
                    )
                })
        })
        .collect()
}

/// Decode `fonts` (file name → base64 bytes) into faces for the provider.
///
/// # Errors
///
/// `request.invalid_params` when a name is not a bundled font file name, or
/// its bytes are not base64. Project fonts a document declares as assets
/// go in `files`, not here.
pub(crate) fn decode_fonts(fonts: &BTreeMap<String, String>) -> Result<Vec<ExtraFont>, ErrorBody> {
    bundled_font_files(decode_font_bytes(fonts)?).map_err(|e| ErrorBody {
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
        fonts.insert("Inter.ttf".to_owned(), STANDARD.encode(b"x"));
        let err = decode_fonts(&fonts).expect_err("unknown name");
        assert_eq!(err.code, "request.invalid_params");
        assert!(
            err.message.contains("NotoSerif-Bold.ttf"),
            "{}",
            err.message
        );
    }

    #[test]
    fn bad_base64_names_the_font() {
        let mut fonts = BTreeMap::new();
        fonts.insert("NotoSerif-Bold.ttf".to_owned(), "***".to_owned());
        let err = decode_fonts(&fonts).expect_err("bad base64");
        assert!(err.message.contains("fonts['NotoSerif-Bold.ttf']"));
    }

    #[test]
    fn bundled_name_registers_as_the_bundled_face() {
        let mut fonts = BTreeMap::new();
        fonts.insert("notoserif-bold".to_owned(), STANDARD.encode(b"abc"));
        let out = decode_fonts(&fonts).expect("decodes");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].family, "Noto Serif");
        assert_eq!(out[0].weight, 700);
        assert_eq!(out[0].style, FontStyle::Normal);
        assert_eq!(out[0].source, FontSource::Bundled);
    }
}
