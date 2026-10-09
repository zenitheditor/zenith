mod common;

use common::parse;
use zenith_core::{Document, default_provider};
use zenith_scene::ir::{Paint, SceneCommand};
use zenith_scene::{ImportGraph, compile_page, compile_page_with_imports};

fn imported_doc(page_w: f64, page_h: f64) -> Document {
    parse(&format!(
        r##"zenith version=1 {{
  project id="proj.imported" name="Imported"
  tokens format="zenith-token-v1" {{
    token id="color.page" type="color" value="#0000ff"
  }}
  styles {{}}
  document id="doc.imported" title="Imported" {{
    page id="page.imported" w=(px){page_w} h=(px){page_h} background=(token)"color.page" {{
      rect id="mark" x=(px)10 y=(px)20 w=(px)30 h=(px)40 fill="#00ff00"
    }}
  }}
}}
"##
    ))
}

fn imported_doc_with_image() -> Document {
    parse(
        r##"zenith version=1 {
  project id="proj.imported" name="Imported"
  assets {
    asset id="logo" kind="image" src="logo.png"
  }
  document id="doc.imported" title="Imported" {
    page id="page.imported" w=(px)100 h=(px)100 {
      image id="logo.node" asset="logo" x=(px)0 y=(px)0 w=(px)20 h=(px)20
    }
  }
}
"##,
    )
}

fn host_doc(source: &str, fit: Option<&str>, page_w: f64, page_h: f64) -> Document {
    let fit_attr = fit.map_or(String::new(), |value| format!(r#" fit="{value}""#));
    parse(&format!(
        r##"zenith version=1 {{
  project id="proj.host" name="Host"
  document id="doc.host" title="Host" {{
    page id="page.host" source="{source}"{fit_attr} w=(px){page_w} h=(px){page_h} background="#ff0000" {{
      rect id="overlay" x=(px)1 y=(px)2 w=(px)3 h=(px)4 fill="#ffff00"
    }}
  }}
}}
"##
    ))
}

fn diagnostic_codes(result: &zenith_scene::CompileResult) -> Vec<&str> {
    result
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect()
}

fn solid_fill_colors(result: &zenith_scene::CompileResult) -> Vec<(u8, u8, u8)> {
    result
        .scene
        .commands
        .iter()
        .filter_map(|command| match command {
            SceneCommand::FillRect {
                paint: Paint::Solid { color },
                ..
            } => Some((color.r, color.g, color.b)),
            SceneCommand::FillRect {
                paint: Paint::Gradient(_),
                ..
            }
            | SceneCommand::StrokeRect { .. }
            | SceneCommand::FillRoundedRect { .. }
            | SceneCommand::StrokeRoundedRect { .. }
            | SceneCommand::FillEllipse { .. }
            | SceneCommand::StrokeEllipse { .. }
            | SceneCommand::StrokeLine { .. }
            | SceneCommand::FillPolygon { .. }
            | SceneCommand::StrokePolyline { .. }
            | SceneCommand::FillPath { .. }
            | SceneCommand::StrokePath { .. }
            | SceneCommand::DrawImage { .. }
            | SceneCommand::DrawSvgAsset { .. }
            | SceneCommand::DrawGlyphRun { .. }
            | SceneCommand::PushClip { .. }
            | SceneCommand::PushClipRoundedRect { .. }
            | SceneCommand::PopClip
            | SceneCommand::PushLayer { .. }
            | SceneCommand::PopLayer
            | SceneCommand::PushTransform { .. }
            | SceneCommand::PushScaleTranslate { .. }
            | SceneCommand::PushTransformMatrix { .. }
            | SceneCommand::PopTransform
            | SceneCommand::BeginShadow { .. }
            | SceneCommand::EndShadow
            | SceneCommand::BeginBlur { .. }
            | SceneCommand::EndBlur
            | SceneCommand::BeginFilter { .. }
            | SceneCommand::EndFilter
            | SceneCommand::BeginMask { .. }
            | SceneCommand::EndMask => None,
        })
        .collect()
}

fn scale_translate_commands(result: &zenith_scene::CompileResult) -> Vec<(f64, f64, f64, f64)> {
    result
        .scene
        .commands
        .iter()
        .filter_map(|command| match command {
            SceneCommand::PushScaleTranslate { sx, sy, tx, ty } => Some((*sx, *sy, *tx, *ty)),
            SceneCommand::FillRect { .. }
            | SceneCommand::StrokeRect { .. }
            | SceneCommand::FillRoundedRect { .. }
            | SceneCommand::StrokeRoundedRect { .. }
            | SceneCommand::FillEllipse { .. }
            | SceneCommand::StrokeEllipse { .. }
            | SceneCommand::StrokeLine { .. }
            | SceneCommand::FillPolygon { .. }
            | SceneCommand::StrokePolyline { .. }
            | SceneCommand::FillPath { .. }
            | SceneCommand::StrokePath { .. }
            | SceneCommand::DrawImage { .. }
            | SceneCommand::DrawSvgAsset { .. }
            | SceneCommand::DrawGlyphRun { .. }
            | SceneCommand::PushClip { .. }
            | SceneCommand::PushClipRoundedRect { .. }
            | SceneCommand::PopClip
            | SceneCommand::PushLayer { .. }
            | SceneCommand::PopLayer
            | SceneCommand::PushTransform { .. }
            | SceneCommand::PushTransformMatrix { .. }
            | SceneCommand::PopTransform
            | SceneCommand::BeginShadow { .. }
            | SceneCommand::EndShadow
            | SceneCommand::BeginBlur { .. }
            | SceneCommand::EndBlur
            | SceneCommand::BeginFilter { .. }
            | SceneCommand::EndFilter
            | SceneCommand::BeginMask { .. }
            | SceneCommand::EndMask => None,
        })
        .collect()
}

fn image_asset_ids(result: &zenith_scene::CompileResult) -> Vec<&str> {
    result
        .scene
        .commands
        .iter()
        .filter_map(|command| match command {
            SceneCommand::DrawImage { asset_id, .. } => Some(asset_id.as_str()),
            SceneCommand::FillRect { .. }
            | SceneCommand::StrokeRect { .. }
            | SceneCommand::FillRoundedRect { .. }
            | SceneCommand::StrokeRoundedRect { .. }
            | SceneCommand::FillEllipse { .. }
            | SceneCommand::StrokeEllipse { .. }
            | SceneCommand::StrokeLine { .. }
            | SceneCommand::FillPolygon { .. }
            | SceneCommand::StrokePolyline { .. }
            | SceneCommand::FillPath { .. }
            | SceneCommand::StrokePath { .. }
            | SceneCommand::DrawSvgAsset { .. }
            | SceneCommand::DrawGlyphRun { .. }
            | SceneCommand::PushClip { .. }
            | SceneCommand::PushClipRoundedRect { .. }
            | SceneCommand::PopClip
            | SceneCommand::PushLayer { .. }
            | SceneCommand::PopLayer
            | SceneCommand::PushTransform { .. }
            | SceneCommand::PushScaleTranslate { .. }
            | SceneCommand::PushTransformMatrix { .. }
            | SceneCommand::PopTransform
            | SceneCommand::BeginShadow { .. }
            | SceneCommand::EndShadow
            | SceneCommand::BeginBlur { .. }
            | SceneCommand::EndBlur
            | SceneCommand::BeginFilter { .. }
            | SceneCommand::EndFilter
            | SceneCommand::BeginMask { .. }
            | SceneCommand::EndMask => None,
        })
        .collect()
}

#[test]
fn page_source_imports_same_size_page_as_native_scene_content() {
    let host = host_doc("library#page.page.imported", None, 100.0, 80.0);
    let imported = imported_doc(100.0, 80.0);
    let imports = ImportGraph::new().with_document("library", &imported);

    let result = compile_page_with_imports(&host, &default_provider(), 0, None, &imports);

    assert_eq!(diagnostic_codes(&result), Vec::<&str>::new());
    assert_eq!(
        solid_fill_colors(&result),
        vec![(255, 0, 0), (0, 0, 255), (0, 255, 0), (255, 255, 0)]
    );
    assert!(scale_translate_commands(&result).is_empty());
}

#[test]
fn page_source_contain_centers_imported_page() {
    let host = host_doc("library#page.page.imported", Some("contain"), 200.0, 100.0);
    let imported = imported_doc(100.0, 100.0);
    let imports = ImportGraph::new().with_document("library", &imported);

    let result = compile_page_with_imports(&host, &default_provider(), 0, None, &imports);

    assert_eq!(diagnostic_codes(&result), Vec::<&str>::new());
    assert_eq!(
        scale_translate_commands(&result),
        vec![(1.0, 1.0, 50.0, 0.0)]
    );
}

#[test]
fn page_source_fill_scales_imported_page_to_host_page() {
    let host = host_doc("library#page.page.imported", Some("fill"), 200.0, 100.0);
    let imported = imported_doc(100.0, 100.0);
    let imports = ImportGraph::new().with_document("library", &imported);

    let result = compile_page_with_imports(&host, &default_provider(), 0, None, &imports);

    assert_eq!(diagnostic_codes(&result), Vec::<&str>::new());
    assert_eq!(
        scale_translate_commands(&result),
        vec![(2.0, 1.0, 0.0, 0.0)]
    );
}

#[test]
fn page_source_namespaces_imported_asset_ids() {
    let host = host_doc("library#page.page.imported", None, 100.0, 100.0);
    let imported = imported_doc_with_image();
    let imports = ImportGraph::new().with_document("library", &imported);

    let result = compile_page_with_imports(&host, &default_provider(), 0, None, &imports);

    assert_eq!(diagnostic_codes(&result), Vec::<&str>::new());
    assert_eq!(image_asset_ids(&result), vec!["library/logo"]);
}

#[test]
fn page_source_without_graph_emits_unsupported_source_advisory() {
    let host = host_doc("library#page.page.imported", None, 100.0, 100.0);

    let result = compile_page(&host, &default_provider(), 0, None);

    assert_eq!(
        diagnostic_codes(&result),
        vec!["scene.unsupported_import_source"]
    );
}

#[test]
fn page_source_unknown_page_emits_scene_diagnostic() {
    let host = host_doc("library#page.missing", None, 100.0, 100.0);
    let imported = imported_doc(100.0, 100.0);
    let imports = ImportGraph::new().with_document("library", &imported);

    let result = compile_page_with_imports(&host, &default_provider(), 0, None, &imports);

    assert_eq!(diagnostic_codes(&result), vec!["scene.unknown_import_page"]);
}
