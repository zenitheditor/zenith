//! `render {source, page?, scale?, path?, files?, global_config?, data?,
//! construction_overlay?, locked?, allow?, warn?, deny?}`: rasterize one page
//! to a base64 PNG, exactly as `zenith render --png` does.
//!
//! The project (`.zenith.kdl` config, imports, assets, project fonts, text
//! sources, the data file) resolves from `files`. Bundled fonts apply; no
//! machine-local font does. As on the CLI, any Error diagnostic blocks the
//! output.

use std::collections::BTreeMap;
use std::path::Path;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;
use serde_json::Value;
use zenith_core::{DataContext, Diagnostic};
use zenith_pipeline::assets::load_data_context;
use zenith_pipeline::render::{MAX_RENDER_SCALE, check_render_scale, render_png};
use zenith_pipeline::{PipelineError, RenderOptions};

use super::project::{Project, ProjectFields};
use crate::protocol::{DiagnosticOut, ErrorBody};
use zenith_editor::fonts::missing_face_notices;

/// The `params` of `render`.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RenderParams {
    /// The `.zen` document text.
    pub(crate) source: String,
    /// 1-based page number. Defaults to 1.
    #[serde(default)]
    pub(crate) page: Option<usize>,
    /// Raster scale, `1.0` = page pixels. Defaults to `1.0`.
    #[serde(default)]
    pub(crate) scale: Option<f64>,
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
    /// `--data`: the path in `files` of a JSON or CSV data file.
    #[serde(default)]
    pub(crate) data: Option<String>,
    /// `--construction-overlay`: draw page construction guides.
    #[serde(default)]
    pub(crate) construction_overlay: bool,
    /// `--locked`: verify asset `sha256` values.
    #[serde(default)]
    pub(crate) locked: bool,
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

/// The `render` result: `{"png_base64", "width", "height", "page",
/// "page_count", "scale", "diagnostics"}`.
pub(crate) fn run(params: &RenderParams) -> Result<Value, ErrorBody> {
    let src = params.source.as_str();
    let scale = checked_scale(params.scale)?;
    let page = params.page.unwrap_or(1);
    let project = Project::decode(&ProjectFields {
        path: params.path.as_deref(),
        files: &params.files,
        fonts: &params.fonts,
        global_config: params.global_config.as_deref(),
        allow: &params.allow,
        warn: &params.warn,
        deny: &params.deny,
    })?;
    let data = load_data(&project, params.data.as_deref())?;
    let config = project.config();
    let opts = RenderOptions::new(&project.flags)
        .with_locked(params.locked)
        .with_data(data.as_ref())
        .with_construction_overlay(params.construction_overlay)
        .with_scale(scale);
    let artifact = render_png(project.host(&config), src, Some(&project.dir), page, opts)
        .map_err(|e| pipeline_error(&e, src))?;
    if Diagnostic::has_errors(&artifact.diagnostics) {
        return Err(blocked(
            "the render has error diagnostics",
            &artifact.diagnostics,
            src,
        ));
    }
    let mut diagnostics = DiagnosticOut::all(&artifact.diagnostics, src);
    diagnostics.extend(missing_face_notices(&project.font_log));
    Ok(serde_json::json!({
        "png_base64": STANDARD.encode(&artifact.png),
        "width": artifact.width,
        "height": artifact.height,
        "page": page,
        "page_count": artifact.page_count,
        "scale": scale,
        "diagnostics": diagnostics,
    }))
}

/// Check `scale`: finite, `> 0`, and `<= 4`. `None` is `1.0`.
fn checked_scale(scale: Option<f64>) -> Result<f64, ErrorBody> {
    let value = scale.unwrap_or(1.0);
    check_render_scale(value, &value.to_string()).map_err(|message| {
        ErrorBody::new(
            "render.invalid_scale",
            format!("{message}; the largest scale is {MAX_RENDER_SCALE}"),
        )
    })
}

/// The data context of the `data` file, when one is named.
fn load_data(project: &Project, data: Option<&str>) -> Result<Option<DataContext>, ErrorBody> {
    let Some(rel) = data else {
        return Ok(None);
    };
    load_data_context(&project.fs, Path::new(rel))
        .map(Some)
        .map_err(|e| {
            ErrorBody::new(
                "data.load_failed",
                format!("{e}; check files['{rel}'] is valid JSON or CSV"),
            )
        })
}

/// The protocol error of a stopped pipeline call. Parse and validation
/// errors are `render.blocked`; any other stop keeps the code of its first
/// error diagnostic (for example `render.page_out_of_range` or
/// `config.error`).
fn pipeline_error(e: &PipelineError, src: &str) -> ErrorBody {
    let first = e.diagnostics.iter().find(|d| d.is_error());
    let code = first.map_or("render.failed", |d| d.code.as_str());
    if e.exit_code == 1 || code == "parse.error" {
        return blocked("the document has error diagnostics", &e.diagnostics, src);
    }
    ErrorBody {
        code: code.to_owned(),
        message: e.message.clone(),
        diagnostics: DiagnosticOut::all(&e.diagnostics, src),
        offers: Vec::new(),
    }
}

/// A `render.blocked` error that carries `diagnostics`.
fn blocked(reason: &str, diagnostics: &[Diagnostic], src: &str) -> ErrorBody {
    ErrorBody {
        code: "render.blocked".to_owned(),
        message: format!("{reason}; fix the listed error diagnostics and render again"),
        diagnostics: DiagnosticOut::all(diagnostics, src),
        offers: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLOWCHART: &str = include_str!("../../../examples/flowchart.zen");
    const MULTIPAGE: &str = include_str!("../../../examples/multipage.zen");
    const IMAGE: &str = include_str!("../../../examples/image.zen");
    const SWATCH: &[u8] = include_bytes!("../../../examples/assets/swatch.png");

    fn params(source: &str) -> RenderParams {
        RenderParams {
            source: source.to_owned(),
            ..Default::default()
        }
    }

    fn png_bytes(out: &Value) -> Vec<u8> {
        let b64 = out["png_base64"].as_str().expect("png_base64");
        STANDARD.decode(b64).expect("valid base64")
    }

    #[test]
    fn renders_example_page_one() {
        let out = run(&params(FLOWCHART)).expect("render ok");
        assert_eq!(out["width"], 360);
        assert_eq!(out["height"], 420);
        assert_eq!(out["page"], 1);
        assert_eq!(out["page_count"], 1);
        assert!(png_bytes(&out).starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[test]
    fn render_is_deterministic() {
        let a = run(&params(FLOWCHART)).expect("render ok");
        let b = run(&params(FLOWCHART)).expect("render ok");
        assert_eq!(png_bytes(&a), png_bytes(&b));
    }

    #[test]
    fn scale_changes_size() {
        let out = run(&RenderParams {
            scale: Some(0.5),
            ..params(FLOWCHART)
        })
        .expect("render ok");
        assert_eq!(out["width"], 180);
        assert_eq!(out["height"], 210);
    }

    #[test]
    fn out_of_range_scale_is_rejected() {
        for bad in [0.0, -1.0, 4.5, f64::NAN] {
            let err = run(&RenderParams {
                scale: Some(bad),
                ..params(FLOWCHART)
            })
            .expect_err("bad scale");
            assert_eq!(err.code, "render.invalid_scale");
        }
    }

    #[test]
    fn other_pages_render_and_report_the_count() {
        let first = run(&params(MULTIPAGE)).expect("page 1");
        let count = first["page_count"].as_u64().expect("count");
        assert!(count >= 2, "{first}");
        let second = run(&RenderParams {
            page: Some(2),
            ..params(MULTIPAGE)
        })
        .expect("page 2");
        assert_eq!(second["page"], 2);
        assert_ne!(png_bytes(&first), png_bytes(&second));

        let err = run(&RenderParams {
            page: Some(99),
            ..params(MULTIPAGE)
        })
        .expect_err("out of range");
        assert_eq!(err.code, "render.page_out_of_range");
    }

    #[test]
    fn asset_from_files_renders_and_missing_asset_blocks() {
        let err = run(&params(IMAGE)).expect_err("asset missing");
        assert_eq!(err.code, "render.blocked");
        assert!(err.diagnostics.iter().any(|d| d.code == "asset.missing"));

        let mut files = BTreeMap::new();
        files.insert("assets/swatch.png".to_owned(), STANDARD.encode(SWATCH));
        let out = run(&RenderParams {
            files,
            ..params(IMAGE)
        })
        .expect("render ok");
        assert_eq!(out["width"], 320);
    }

    #[test]
    fn parse_error_blocks_render() {
        let err = run(&params("not kdl !!!{{{")).expect_err("blocked");
        assert_eq!(err.code, "render.blocked");
        assert_eq!(err.diagnostics[0].code, "parse.error");
    }

    #[test]
    fn validation_error_blocks_render() {
        let src = r##"zenith version=1 {
  project id="p" name="P"
  tokens format="zenith-token-v1" {
  }
  styles {}
  document id="d" title="D" {
    page id="pg" w=(px)100 h=(px)100 {
      rect id="r" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill=(token)"color.missing"
    }
  }
}
"##;
        let err = run(&params(src)).expect_err("blocked");
        assert_eq!(err.code, "render.blocked");
        assert!(err.diagnostics.iter().any(|d| d.severity == "error"));
    }

    #[test]
    fn missing_data_file_is_a_data_error() {
        let err = run(&RenderParams {
            data: Some("rows.csv".to_owned()),
            ..params(FLOWCHART)
        })
        .expect_err("no data file");
        assert_eq!(err.code, "data.load_failed");
        assert!(err.message.contains("rows.csv"), "{}", err.message);
    }

    const SERIF_DOC: &str = r##"zenith version=1 {
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

    fn full_build() -> bool {
        zenith_core::bundled_faces().iter().all(|f| f.is_embedded())
    }

    fn font_map(files: &[&str]) -> BTreeMap<String, String> {
        files
            .iter()
            .map(|name| {
                let path = format!(
                    "{}/../zenith-core/assets/fonts/{name}",
                    env!("CARGO_MANIFEST_DIR")
                );
                let bytes = std::fs::read(&path).expect("bundled font file");
                ((*name).to_owned(), STANDARD.encode(bytes))
            })
            .collect()
    }

    #[test]
    fn dropped_face_reports_font_unresolved_naming_the_file() {
        if full_build() {
            return;
        }
        let out = run(&params(SERIF_DOC)).expect("render ok");
        let notice = out["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .find(|d| {
                d["code"] == "font.unresolved"
                    && d["message"]
                        .as_str()
                        .is_some_and(|m| m.contains("NotoSerif-Regular.ttf"))
            })
            .cloned();
        assert!(notice.is_some(), "{out}");
    }

    #[test]
    fn supplied_fonts_render_like_the_full_build() {
        let plain = SERIF_DOC;
        let supplied = run(&RenderParams {
            fonts: font_map(&["NotoSerif-Regular.ttf"]),
            ..params(plain)
        })
        .expect("render ok");
        let has_font_notice = supplied["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .any(|d| d["code"] == "font.unresolved");
        assert!(!has_font_notice, "{supplied}");
        if full_build() {
            let embedded = run(&params(plain)).expect("render ok");
            assert_eq!(png_bytes(&supplied), png_bytes(&embedded));
        } else {
            let without = run(&params(plain)).expect("render ok");
            assert_ne!(
                png_bytes(&supplied),
                png_bytes(&without),
                "the serif face must change the page"
            );
        }
    }

    #[test]
    fn unknown_font_key_is_invalid_params() {
        let mut fonts = BTreeMap::new();
        fonts.insert("Inter.ttf".to_owned(), STANDARD.encode(b"x"));
        let err = run(&RenderParams {
            fonts,
            ..params(FLOWCHART)
        })
        .expect_err("unknown key");
        assert_eq!(err.code, "request.invalid_params");
    }
}
