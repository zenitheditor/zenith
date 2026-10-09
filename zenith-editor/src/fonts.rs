//! Lazy bundled fonts: turn the `fonts` map (file name → bytes) into faces
//! for the provider, list the faces a document still needs, and turn a
//! missing bundled face into a diagnostic.
//!
//! A size-tuned build (the browser module) embeds only Noto Sans Regular
//! and Bold (see the `bundled-fonts-extended` feature of `zenith-core`).
//! The other bundled faces are plain files the page fetches and passes back
//! keyed by file name (`NotoSerif-Bold.ttf`). Such a face registers exactly
//! as the embedded face does, so the output equals the full build.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::Serialize;
use zenith_core::{
    FaceRequest, FontMissLog, FontSource, FontStyle, bundled_face_by_file, bundled_face_for,
    bundled_faces,
};
use zenith_pipeline::ExtraFont;

use crate::error::EditorError;
use crate::wire::DiagnosticOut;

/// Faces for the provider from `fonts` (bundled file name → bytes).
///
/// # Errors
///
/// `editor.invalid_font` when a name is not a bundled font file name.
/// Project fonts a document declares as assets go in the project files.
pub fn bundled_font_files(fonts: BTreeMap<String, Vec<u8>>) -> Result<Vec<ExtraFont>, EditorError> {
    let mut out = Vec::with_capacity(fonts.len());
    for (name, bytes) in fonts {
        let face = bundled_face_by_file(&name).ok_or_else(|| {
            let known: Vec<&str> = bundled_faces().iter().map(|f| f.file).collect();
            EditorError::new(
                "editor.invalid_font",
                format!(
                    "fonts['{name}'] is not a bundled font file; key each entry by one of {}. \
                     A document's own font assets go in files",
                    known.join(", ")
                ),
            )
        })?;
        out.push(ExtraFont {
            family: face.family.to_owned(),
            weight: face.weight,
            style: face.style,
            bytes: Arc::from(bytes),
            index: 0,
            source: FontSource::Bundled,
        });
    }
    Ok(out)
}

/// A face the document asked for and the engine does not hold.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct NeededFace {
    /// The requested family.
    pub family: String,
    /// The requested weight.
    pub weight: u16,
    /// `normal` or `italic`.
    pub style: &'static str,
    /// The bundled file to fetch and pass in `fonts`. `None` when the
    /// family is not a bundled one: supply it as a project font asset.
    pub file: Option<&'static str>,
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

/// The requests in `log` that name a face the engine lacks: a dropped
/// bundled face, or a family the provider does not hold at all. A request
/// for another weight or style of a held family resolves to the nearest
/// face in every build, so it is not listed.
#[must_use]
pub fn needed_faces(log: &FontMissLog) -> Vec<NeededFace> {
    log.requests()
        .iter()
        .filter_map(|request| {
            let file = dropped_file(request);
            if file.is_none() && request.family_registered {
                return None;
            }
            Some(NeededFace {
                family: request.family.clone(),
                weight: request.weight,
                style: style_name(request.style),
                file,
            })
        })
        .collect()
}

/// One `font.unresolved` advisory per dropped bundled face the document
/// asked for. It names the file to fetch. A build that embeds every face
/// reports none.
#[must_use]
pub fn missing_face_notices(log: &FontMissLog) -> Vec<DiagnosticOut> {
    log.requests()
        .iter()
        .filter_map(|request| {
            let file = dropped_file(request)?;
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
        })
        .collect()
}

/// The bundled font files the build embeds.
#[must_use]
pub fn embedded_files() -> Vec<&'static str> {
    bundled_faces()
        .iter()
        .filter(|f| f.is_embedded())
        .map(|f| f.file)
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
