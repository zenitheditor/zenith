//! `diagnose {source, path?, files?, global_config?, allow?, warn?, deny?}`:
//! the `zenith validate` pipeline over an in-memory project.
//!
//! Config policy and brand (`.zenith.kdl` in `files`, plus the optional
//! global config), composition imports, assets, fonts, and text sources all
//! resolve from `files` exactly as the CLI resolves them from disk. With no
//! error, every page compiles (no raster), so compile-stage diagnostics show
//! in the same round. Repeats are removed.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;
use zenith_pipeline::validate_source;

use super::project::{Project, ProjectFields};
use crate::protocol::{DiagnosticOut, ErrorBody};
use zenith_editor::fonts::missing_face_notices;

/// The `params` of `diagnose`.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DiagnoseParams {
    /// The `.zen` document text.
    pub(crate) source: String,
    /// The document's path in `files`. Defaults to `document.zen`.
    #[serde(default)]
    pub(crate) path: Option<String>,
    /// Project files: path → base64 bytes.
    #[serde(default)]
    pub(crate) files: BTreeMap<String, String>,
    /// Bundled font files the module does not embed: file name (for example
    /// `NotoSerif-Bold.ttf`) → base64 bytes. `fonts` lists the ones a
    /// document needs.
    #[serde(default)]
    pub(crate) fonts: BTreeMap<String, String>,
    /// The path in `files` of the global config.
    #[serde(default)]
    pub(crate) global_config: Option<String>,
    /// Diagnostic codes to suppress.
    #[serde(default)]
    pub(crate) allow: Vec<String>,
    /// Diagnostic codes to force to Warning.
    #[serde(default)]
    pub(crate) warn: Vec<String>,
    /// Diagnostic codes to elevate to Error.
    #[serde(default)]
    pub(crate) deny: Vec<String>,
}

/// The `diagnose` result: `{"valid", "exit_code", "diagnostics"}`.
/// `exit_code` is the `zenith validate` exit code: 0, 1, or 2.
pub(crate) fn run(params: &DiagnoseParams) -> Result<Value, ErrorBody> {
    let project = Project::decode(&ProjectFields {
        path: params.path.as_deref(),
        files: &params.files,
        fonts: &params.fonts,
        global_config: params.global_config.as_deref(),
        allow: &params.allow,
        warn: &params.warn,
        deny: &params.deny,
    })?;
    let config = project.config();
    let src = params.source.as_str();
    let validation = validate_source(
        project.host(&config),
        src,
        Some(&project.dir),
        &project.flags,
    );
    let mut diagnostics = DiagnosticOut::all(&validation.diagnostics, src);
    diagnostics.extend(missing_face_notices(&project.font_log));
    Ok(serde_json::json!({
        "valid": validation.exit_code == 0,
        "exit_code": validation.exit_code,
        "diagnostics": diagnostics,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine as _;
    use base64::engine::general_purpose::STANDARD;

    fn diagnose(params: DiagnoseParams) -> Value {
        run(&params).expect("diagnose runs")
    }

    fn source(source: &str) -> DiagnoseParams {
        DiagnoseParams {
            source: source.to_owned(),
            ..Default::default()
        }
    }

    fn codes(out: &Value) -> Vec<String> {
        out["diagnostics"]
            .as_array()
            .expect("diagnostics array")
            .iter()
            .filter_map(|d| d["code"].as_str().map(str::to_owned))
            .collect()
    }

    const UNUSED_TOKEN: &str = r##"zenith version=1 {
  project id="proj.p" name="Policy"
  tokens format="zenith-token-v1" {
    token id="color.unused" type="color" value="#abcdef"
  }
  styles {}
  document id="doc.p" title="Policy" {
    page id="page.p" w=(px)100 h=(px)100 {
      rect id="r.one" x=(px)0 y=(px)0 w=(px)10 h=(px)10
    }
  }
}
"##;

    #[test]
    fn valid_example_is_valid() {
        let out = diagnose(source(include_str!("../../../examples/flowchart.zen")));
        assert_eq!(out["valid"], true, "{out}");
        assert_eq!(out["exit_code"], 0);
    }

    #[test]
    fn parse_error_is_located() {
        let out = diagnose(source(
            "zenith version=1 {\n  project id=\"p\" name=\"x\"\n  }}}\n",
        ));
        assert_eq!(out["valid"], false);
        assert_eq!(out["exit_code"], 2);
        let first = &out["diagnostics"][0];
        assert_eq!(first["code"], "parse.error");
        assert_eq!(first["severity"], "error");
    }

    #[test]
    fn compile_stage_diagnostic_is_reported() {
        let src = r##"zenith version=1 {
  project id="proj.c" name="C"
  tokens format="zenith-token-v1" {
    token id="color.w" type="color" value="#ffffff"
  }
  styles {}
  document id="doc.c" title="C" {
    page id="page.c" w=(px)200 h=(px)100 background=(token)"color.w" {
      text id="t" x=(px)10 y=(px)10 w=(px)180 h=(px)40 fill=(token)"color.w" { span "Hidden" }
    }
  }
}
"##;
        let out = diagnose(source(src));
        let hit = out["diagnostics"]
            .as_array()
            .expect("diagnostics array")
            .iter()
            .find(|d| d["code"] == "contrast.invisible")
            .cloned()
            .expect("contrast.invisible reported");
        assert_eq!(hit["line"], 9);
        assert_eq!(hit["subject_id"], "t");
    }

    #[test]
    fn local_config_in_files_governs_policy() {
        let mut files = BTreeMap::new();
        files.insert(
            "site/.zenith.kdl".to_owned(),
            STANDARD.encode(b"diagnostics {\n  deny \"token.unused\"\n}\n"),
        );
        let out = diagnose(DiagnoseParams {
            path: Some("site/docs/page.zen".to_owned()),
            files,
            ..source(UNUSED_TOKEN)
        });
        assert_eq!(out["valid"], false, "{out}");
        assert_eq!(out["exit_code"], 1);
        let hit = &out["diagnostics"][0];
        assert_eq!(hit["code"], "token.unused");
        assert_eq!(hit["severity"], "error");
    }

    #[test]
    fn flags_override_policy() {
        let out = diagnose(DiagnoseParams {
            allow: vec!["token.unused".to_owned()],
            ..source(UNUSED_TOKEN)
        });
        assert!(!codes(&out).contains(&"token.unused".to_owned()), "{out}");
    }

    #[test]
    fn missing_import_is_reported_and_present_import_resolves() {
        let host = r#"zenith version=1 {
  project id="proj.host" name="Host"
  imports {
    import id="brand" kind="zen" src="brand.zen"
  }
  document id="doc.host" title="Host" {
    page id="page.host" w=(px)100 h=(px)100
  }
}
"#;
        let out = diagnose(source(host));
        assert!(codes(&out).contains(&"import.missing".to_owned()), "{out}");

        let mut files = BTreeMap::new();
        files.insert(
            "brand.zen".to_owned(),
            STANDARD.encode(
                r#"zenith version=1 {
  project id="proj.brand" name="Brand"
  document id="doc.brand" title="Brand" {
    page id="page.brand" w=(px)10 h=(px)10
  }
}
"#,
            ),
        );
        let out = diagnose(DiagnoseParams {
            files,
            ..source(host)
        });
        assert_eq!(out["valid"], true, "{out}");
    }

    #[test]
    fn code_node_without_the_mono_face_reports_font_unresolved() {
        if zenith_core::bundled_faces().iter().all(|f| f.is_embedded()) {
            return;
        }
        let out = diagnose(source(include_str!("../../../examples/code.zen")));
        let fonts = out["diagnostics"]
            .as_array()
            .expect("diagnostics array")
            .iter()
            .filter(|d| d["code"] == "font.unresolved")
            .count();
        assert!(fonts > 0, "{out}");
        assert!(
            out["diagnostics"]
                .to_string()
                .contains("NotoSansMono-Regular.ttf"),
            "the notice must name the file to fetch: {out}"
        );
    }
}
