//! `fonts {source, path?, files?, fonts?, global_config?}`: list the font
//! faces the document needs and the module does not hold.
//!
//! The module embeds only Noto Sans Regular and Bold. This method compiles
//! every page of the document (as `diagnose` does) while recording each face
//! the shaper asks for, then returns the ones with no face. An entry whose
//! `file` is set names a bundled font file. The page fetches that file and
//! passes it as `fonts: {"<file>": "<base64>"}` to `diagnose` and `render`.
//!
//! A document can ask for a further face only after the first is supplied,
//! because an unresolved family falls back to Noto Sans. The page calls
//! `fonts` again with the faces it holds in `fonts` until `faces` is empty.
//! An entry with `file: null` is a family that is neither bundled nor a
//! font asset of the project. Supply it as a font asset in `files`.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;
use zenith_pipeline::validate_source;

use super::project::{Project, ProjectFields};
use crate::protocol::ErrorBody;
use zenith_editor::fonts::{embedded_files, needed_faces};

/// The `params` of `fonts`.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FontsParams {
    /// The `.zen` document text.
    pub(crate) source: String,
    /// The document's path in `files`. Defaults to `document.zen`.
    #[serde(default)]
    pub(crate) path: Option<String>,
    /// Project files: path → base64 bytes.
    #[serde(default)]
    pub(crate) files: BTreeMap<String, String>,
    /// Bundled font files the page already holds: file name → base64.
    #[serde(default)]
    pub(crate) fonts: BTreeMap<String, String>,
    /// The path in `files` of the global config.
    #[serde(default)]
    pub(crate) global_config: Option<String>,
}

/// The `fonts` result: `{"faces": [{"family", "weight", "style", "file"}],
/// "embedded": ["<file>"], "complete"}`.
///
/// `embedded` lists the bundled files this module embeds. `complete` is
/// `false` when the document has an error that stops compilation, so the
/// list can be short until the error is fixed.
pub(crate) fn run(params: &FontsParams) -> Result<Value, ErrorBody> {
    let project = Project::decode(&ProjectFields {
        path: params.path.as_deref(),
        files: &params.files,
        fonts: &params.fonts,
        global_config: params.global_config.as_deref(),
        allow: &[],
        warn: &[],
        deny: &[],
    })?;
    let config = project.config();
    let validation = validate_source(
        project.host(&config),
        params.source.as_str(),
        Some(&project.dir),
        &project.flags,
    );
    Ok(serde_json::json!({
        "faces": needed_faces(&project.font_log),
        "embedded": embedded_files(),
        "complete": validation.exit_code == 0,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine as _;
    use base64::engine::general_purpose::STANDARD;

    const SERIF: &str = r##"zenith version=1 {
  project id="proj.s" name="S"
  tokens format="zenith-token-v1" {
    token id="font.serif" type="fontFamily" value="Noto Serif"
    token id="color.ink" type="color" value="#101010"
  }
  styles {}
  document id="doc.s" title="S" {
    page id="page.s" w=(px)240 h=(px)80 {
      text id="t" x=(px)10 y=(px)10 w=(px)220 h=(px)40 font-family=(token)"font.serif" fill=(token)"color.ink" { span "Serif" }
    }
  }
}
"##;

    const SANS: &str = r##"zenith version=1 {
  project id="proj.n" name="N"
  tokens format="zenith-token-v1" {
    token id="color.ink" type="color" value="#101010"
  }
  styles {}
  document id="doc.n" title="N" {
    page id="page.n" w=(px)240 h=(px)80 {
      text id="t" x=(px)10 y=(px)10 w=(px)220 h=(px)40 fill=(token)"color.ink" { span "Sans" }
    }
  }
}
"##;

    fn params(source: &str) -> FontsParams {
        FontsParams {
            source: source.to_owned(),
            ..Default::default()
        }
    }

    #[test]
    fn sans_document_needs_nothing() {
        let out = run(&params(SANS)).expect("fonts runs");
        assert_eq!(out["faces"], serde_json::json!([]), "{out}");
        assert_eq!(out["complete"], true);
        let embedded = out["embedded"].as_array().expect("embedded");
        assert!(embedded.iter().any(|f| f == "NotoSans-Regular.ttf"));
    }

    /// `true` when `zenith-core` embeds every bundled face, as when the
    /// workspace builds the CLI beside this crate.
    fn full_build() -> bool {
        zenith_core::bundled_faces().iter().all(|f| f.is_embedded())
    }

    #[test]
    fn serif_document_names_the_file_to_fetch() {
        if full_build() {
            return;
        }
        let out = run(&params(SERIF)).expect("fonts runs");
        let faces = out["faces"].as_array().expect("faces");
        assert!(
            faces.iter().any(|f| f["family"] == "Noto Serif"
                && f["weight"] == 400
                && f["style"] == "normal"
                && f["file"] == "NotoSerif-Regular.ttf"),
            "{out}"
        );
    }

    #[test]
    fn supplied_face_leaves_the_list() {
        let regular = include_bytes!("../../../zenith-core/assets/fonts/NotoSerif-Regular.ttf");
        let mut fonts = BTreeMap::new();
        fonts.insert("NotoSerif-Regular.ttf".to_owned(), STANDARD.encode(regular));
        let out = run(&FontsParams {
            fonts,
            ..params(SERIF)
        })
        .expect("fonts runs");
        assert_eq!(out["faces"], serde_json::json!([]), "{out}");
    }

    #[test]
    fn full_build_needs_no_bundled_file() {
        if !full_build() {
            return;
        }
        let out = run(&params(SERIF)).expect("fonts runs");
        assert_eq!(out["faces"], serde_json::json!([]), "{out}");
    }

    #[test]
    fn unbundled_family_has_a_null_file() {
        let src = SERIF.replace("Noto Serif", "Some Unbundled Face");
        let out = run(&params(&src)).expect("fonts runs");
        let faces = out["faces"].as_array().expect("faces");
        assert!(
            faces
                .iter()
                .any(|f| f["family"] == "Some Unbundled Face" && f["file"].is_null()),
            "{out}"
        );
    }

    #[test]
    fn parse_error_is_incomplete() {
        let out = run(&params("not kdl !!!{{{")).expect("fonts runs");
        assert_eq!(out["complete"], false);
    }
}
