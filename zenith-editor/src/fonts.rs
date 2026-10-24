//! Lazy bundled and fallback fonts: turn the `fonts` map (file name →
//! bytes) into faces for the provider, list the faces a document still
//! needs, and turn a missing face into a diagnostic.
//!
//! A size-tuned build (the browser module) embeds only Noto Sans Regular
//! and Bold (see the `bundled-fonts-extended` feature of `zenith-core`),
//! and no fallback face (see the `embed` feature of
//! `zenith-fonts-fallback`). The other faces are plain files the page
//! fetches and passes back keyed by file name (`NotoSerif-Bold.ttf`,
//! `NotoSansArabic-Regular.ttf`). Such a face registers exactly as the
//! embedded face does, so the output equals the full build.
//!
//! A bundled face is needed when a document names its family. A fallback
//! face is needed when the shaper gives a char to no face, or to a later
//! face of the chain than [`fallback_face_for`] names. The shaper notes each
//! such char in the [`FontMissLog`].

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde::Serialize;
use zenith_core::{
    FaceRequest, FontMissLog, FontSource, FontStyle, bundled_face_by_file, bundled_face_for,
    bundled_faces,
};
use zenith_fonts_fallback::{
    FallbackFace, fallback_face_by_file, fallback_face_for, fallback_faces,
};
use zenith_pipeline::ExtraFont;

use crate::error::EditorError;
use crate::wire::DiagnosticOut;

/// Faces for the provider from `fonts` (bundled or fallback file name →
/// bytes).
///
/// The fallback faces come last, in chain order, whatever the map order.
/// The provider registers them in that order, so a char shapes with the
/// same fallback face as in a build that embeds the chain.
///
/// # Errors
///
/// `editor.invalid_font` when a name is not a bundled or fallback font file
/// name. Project fonts a document declares as assets go in the project
/// files.
pub fn bundled_font_files(fonts: BTreeMap<String, Vec<u8>>) -> Result<Vec<ExtraFont>, EditorError> {
    let mut out = Vec::with_capacity(fonts.len());
    let mut fallback: BTreeMap<usize, ExtraFont> = BTreeMap::new();
    for (name, bytes) in fonts {
        if let Some(face) = bundled_face_by_file(&name) {
            out.push(ExtraFont {
                family: face.family.to_owned(),
                weight: face.weight,
                style: face.style,
                bytes: Arc::from(bytes),
                index: 0,
                source: FontSource::Bundled,
            });
            continue;
        }
        let Some((position, face)) = fallback_face_by_file(&name).and_then(|face| {
            fallback_faces()
                .iter()
                .position(|f| f.file == face.file)
                .map(|position| (position, face))
        }) else {
            return Err(unknown_font_file(&name));
        };
        fallback.insert(
            position,
            ExtraFont {
                family: face.family.to_owned(),
                weight: face.weight,
                style: FontStyle::Normal,
                bytes: Arc::from(bytes),
                index: 0,
                source: FontSource::Fallback,
            },
        );
    }
    out.extend(fallback.into_values());
    Ok(out)
}

/// The `editor.invalid_font` error for font file `name`.
fn unknown_font_file(name: &str) -> EditorError {
    let known: Vec<&str> = bundled_faces()
        .iter()
        .map(|f| f.file)
        .chain(fallback_faces().iter().map(|f| f.file))
        .collect();
    EditorError::new(
        "editor.invalid_font",
        format!(
            "fonts['{name}'] is not a bundled or fallback font file; key each entry by one of {}. \
             A document's own font assets go in files",
            known.join(", ")
        ),
    )
}

/// A face the document needs and the engine does not hold.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct NeededFace {
    /// The requested family, or the family of the fallback face.
    pub family: String,
    /// The requested weight.
    pub weight: u16,
    /// `normal` or `italic`.
    pub style: &'static str,
    /// The bundled or fallback file to fetch and pass in `fonts`. `None`
    /// when the family is not a bundled one: supply it as a project font
    /// asset.
    pub file: Option<&'static str>,
    /// `true` for a fallback face: text has chars only this face covers.
    pub fallback: bool,
}

/// The wire name of `style`.
#[must_use]
pub fn style_name(style: FontStyle) -> &'static str {
    match style {
        FontStyle::Normal => "normal",
        FontStyle::Italic => "italic",
    }
}

/// The bundled-but-not-embedded file that is exactly this request.
fn dropped_file(request: &FaceRequest) -> Option<&'static str> {
    bundled_face_for(&request.family, request.weight, request.style)
        .filter(|face| !face.is_embedded())
        .map(|face| face.file)
}

/// The fallback faces the build does not embed and the text needs, in
/// chain order.
///
/// A face is needed when the log holds a char whose face
/// ([`fallback_face_for`]) is not the face that shaped it. A face the host
/// passed in `fonts` shapes its chars, so it leaves the list.
#[must_use]
pub fn needed_fallback_faces(log: &FontMissLog) -> Vec<&'static FallbackFace> {
    let mut needed: BTreeSet<&'static str> = BTreeSet::new();
    for glyph in log.fallback_glyphs() {
        let Some(face) = fallback_face_for(glyph.ch) else {
            continue;
        };
        let shaped_by_it = glyph
            .face_family
            .as_deref()
            .is_some_and(|family| family.eq_ignore_ascii_case(face.family));
        if !face.is_embedded() && !shaped_by_it {
            needed.insert(face.file);
        }
    }
    fallback_faces()
        .iter()
        .filter(|f| needed.contains(f.file))
        .collect()
}

/// The faces in `log` the engine lacks: a dropped bundled face, a family
/// the provider does not hold at all, or a fallback face the text needs. A
/// request for another weight or style of a held family resolves to the
/// nearest face in every build, so it is not listed.
#[must_use]
pub fn needed_faces(log: &FontMissLog) -> Vec<NeededFace> {
    let requested = log.requests().into_iter().filter_map(|request| {
        let file = dropped_file(&request);
        if file.is_none() && request.family_registered {
            return None;
        }
        Some(NeededFace {
            family: request.family,
            weight: request.weight,
            style: style_name(request.style),
            file,
            fallback: false,
        })
    });
    let fallback = needed_fallback_faces(log)
        .into_iter()
        .map(|face| NeededFace {
            family: face.family.to_owned(),
            weight: face.weight,
            style: style_name(FontStyle::Normal),
            file: Some(face.file),
            fallback: true,
        });
    requested.chain(fallback).collect()
}

/// One `font.unresolved` advisory per dropped bundled face the document
/// asked for, and per fallback face the text needs. It names the file to
/// fetch. A build that embeds every face reports none.
#[must_use]
pub fn missing_face_notices(log: &FontMissLog) -> Vec<DiagnosticOut> {
    let bundled = log.requests().into_iter().filter_map(|request| {
        let file = dropped_file(&request)?;
        Some(DiagnosticOut::advisory(
            "font.unresolved",
            format!(
                "font '{}' {} {} is a bundled face this module does not embed; \
                 fetch {file} and pass it in fonts",
                request.family,
                request.weight,
                style_name(request.style),
            ),
        ))
    });
    let fallback = needed_fallback_faces(log).into_iter().map(|face| {
        DiagnosticOut::advisory(
            "font.unresolved",
            format!(
                "text has characters only the fallback face '{}' covers, and this module \
                 does not embed it; fetch {} and pass it in fonts",
                face.family, face.file,
            ),
        )
    });
    bundled.chain(fallback).collect()
}

/// The bundled and fallback font files the build embeds.
#[must_use]
pub fn embedded_files() -> Vec<&'static str> {
    bundled_faces()
        .iter()
        .filter(|f| f.is_embedded())
        .map(|f| f.file)
        .chain(
            fallback_faces()
                .iter()
                .filter(|f| f.is_embedded())
                .map(|f| f.file),
        )
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_name_is_rejected_with_the_known_names() {
        let mut fonts = BTreeMap::new();
        fonts.insert("Inter.ttf".to_owned(), b"x".to_vec());
        let err = bundled_font_files(fonts).expect_err("unknown name");
        assert_eq!(err.code, "editor.invalid_font");
        assert!(
            err.message.contains("NotoSerif-Bold.ttf"),
            "{}",
            err.message
        );
    }

    #[test]
    fn unknown_name_lists_the_fallback_files_too() {
        let mut fonts = BTreeMap::new();
        fonts.insert("Inter.ttf".to_owned(), b"x".to_vec());
        let err = bundled_font_files(fonts).expect_err("unknown name");
        assert!(
            err.message.contains("NotoSansArabic-Regular.ttf"),
            "{}",
            err.message
        );
    }

    #[test]
    fn fallback_files_register_in_chain_order_whatever_the_map_order() {
        let mut fonts = BTreeMap::new();
        // Map order: Emoji, Arabic, CJK, Hebrew, then a bundled face.
        for name in [
            "NotoEmoji-Regular.ttf",
            "NotoSansArabic-Regular.ttf",
            "NotoSansCJKsc-Fallback.otf",
            "NotoSansHebrew-Regular.ttf",
            "NotoSerif-Bold.ttf",
        ] {
            fonts.insert(name.to_owned(), b"x".to_vec());
        }
        let out = bundled_font_files(fonts).expect("faces");
        let got: Vec<(&str, FontSource)> =
            out.iter().map(|f| (f.family.as_str(), f.source)).collect();
        assert_eq!(
            got,
            vec![
                ("Noto Serif", FontSource::Bundled),
                ("Noto Sans Arabic", FontSource::Fallback),
                ("Noto Sans Hebrew", FontSource::Fallback),
                ("Noto Sans CJK SC", FontSource::Fallback),
                ("Noto Emoji", FontSource::Fallback),
            ]
        );
    }

    /// A log holding `glyphs` (char, family of the face that shaped it).
    fn log_with(glyphs: &[(char, Option<&str>)]) -> FontMissLog {
        let log = FontMissLog::new();
        let mut provider = zenith_core::BytesFontProvider::new().with_miss_log(log.clone());
        let mut noted = Vec::new();
        for &(ch, family) in glyphs {
            let id = family.map(|family| {
                provider.register_fallback(family, 400, FontStyle::Normal, Arc::from(&b"x"[..]), 0)
            });
            noted.push((ch, id));
        }
        let noted: Vec<(char, Option<&str>)> =
            noted.iter().map(|(ch, id)| (*ch, id.as_deref())).collect();
        zenith_core::FontProvider::note_fallback_glyphs(&provider, &noted);
        log
    }

    /// `true` when this build embeds the fallback chain, as when the
    /// workspace builds the CLI beside this crate.
    fn chain_embedded() -> bool {
        fallback_faces().iter().all(|f| f.is_embedded())
    }

    #[test]
    fn unshaped_chars_name_their_fallback_files_in_chain_order() {
        let log = log_with(&[('\u{1F600}', None), ('\u{05E9}', None), ('\u{0639}', None)]);
        let faces = needed_faces(&log);
        let files: Vec<Option<&str>> = faces.iter().map(|f| f.file).collect();
        if chain_embedded() {
            assert!(files.is_empty(), "{files:?}");
            assert!(missing_face_notices(&log).is_empty());
            return;
        }
        assert_eq!(
            files,
            vec![
                Some("NotoSansArabic-Regular.ttf"),
                Some("NotoSansHebrew-Regular.ttf"),
                Some("NotoEmoji-Regular.ttf"),
            ]
        );
        assert!(faces.iter().all(|f| f.fallback && f.style == "normal"));
        let notices = missing_face_notices(&log);
        assert_eq!(notices.len(), 3);
        assert!(notices.iter().all(|d| d.code == "font.unresolved"));
        assert!(notices[0].message.contains("NotoSansArabic-Regular.ttf"));
    }

    #[test]
    fn a_char_shaped_by_its_own_face_needs_nothing() {
        let log = log_with(&[('\u{0639}', Some("Noto Sans Arabic"))]);
        assert!(needed_faces(&log).is_empty());
        assert!(missing_face_notices(&log).is_empty());
    }

    #[test]
    fn a_char_shaped_by_a_later_face_needs_its_own_face() {
        // U+3030 WAVY DASH: CJK and Emoji both cover it, and CJK comes first.
        let ch = '\u{3030}';
        let cjk = "NotoSansCJKsc-Fallback.otf";
        assert_eq!(fallback_face_for(ch).map(|f| f.file), Some(cjk));
        let log = log_with(&[(ch, Some("Noto Emoji"))]);
        let files: Vec<Option<&str>> = needed_faces(&log).iter().map(|f| f.file).collect();
        if chain_embedded() {
            assert!(files.is_empty(), "{files:?}");
        } else {
            assert_eq!(files, vec![Some(cjk)]);
        }
    }

    #[test]
    fn a_char_no_face_covers_needs_nothing() {
        let log = log_with(&[('\u{E000}', None)]);
        assert!(needed_faces(&log).is_empty());
    }

    #[test]
    fn bundled_name_registers_as_the_bundled_face() {
        let mut fonts = BTreeMap::new();
        fonts.insert("notoserif-bold".to_owned(), b"abc".to_vec());
        let out = bundled_font_files(fonts).expect("faces");
        let face = out.first().expect("one face");
        assert_eq!(face.family, "Noto Serif");
        assert_eq!(face.weight, 700);
        assert_eq!(face.source, FontSource::Bundled);
        assert!(embedded_files().contains(&"NotoSans-Regular.ttf"));
    }
}
