mod common;

use common::{Paint, SceneCommand, default_provider, parse};
use zenith_scene::{ImportGraph, compile_page, compile_page_with_imports};

type FillRectSummary = (f64, f64, f64, f64, (u8, u8, u8));

fn imported_doc() -> common::Document {
    parse(
        r##"zenith version=1 {
  project id="proj.imported" name="Imported"
  tokens format="zenith-token-v1" {
    token id="color.brand" type="color" value="#0000ff"
    token id="color.alt" type="color" value="#00ff00"
    token id="size.core.w" type="dimension" value=(px)100
    token id="size.core.h" type="dimension" value=(px)80
  }
  styles {}
  assets {
    asset id="logo" kind="image" src="logo.png"
  }
  components {
    component id="component.card" {
      rect id="bg" x=(px)0 y=(px)0 w=(px)40 h=(px)20 fill=(token)"color.brand"
    }
    component id="component.alt" {
      rect id="bg" x=(px)0 y=(px)0 w=(px)30 h=(px)10 fill=(token)"color.alt"
    }
    component id="component.image" {
      image id="img" asset="logo" x=(px)0 y=(px)0 w=(px)10 h=(px)10
    }
    component id="component.agent.node" {
      ports {
        port node="core" id="out" anchor="1/4"
      }
      rect id="core" x=(px)0 y=(px)0 w=(token)"size.core.w" h=(token)"size.core.h" fill=(token)"color.brand"
    }
  }
  document id="doc.imported" title="Imported" {
    page id="page.imported" w=(px)10 h=(px)10 {}
  }
}
"##,
    )
}

fn host_doc(source: &str) -> common::Document {
    host_doc_with_instance_body(&format!(
        r#"instance id="inst.imported" source="{source}" x=(px)5 y=(px)7"#
    ))
}

fn host_doc_with_instance_body(instance_body: &str) -> common::Document {
    let src = format!(
        r##"zenith version=1 {{
  project id="proj.host" name="Host"
  tokens format="zenith-token-v1" {{
    token id="color.brand" type="color" value="#ff0000"
    token id="color.override" type="color" value="#ffff00"
  }}
  styles {{}}
  components {{}}
  document id="doc.host" title="Host" {{
    page id="page.host" w=(px)100 h=(px)80 {{
      {instance_body}
    }}
  }}
}}
"##
    );
    parse(&src)
}

fn fill_rects(result: &zenith_scene::CompileResult) -> Vec<FillRectSummary> {
    result
        .scene
        .commands
        .iter()
        .filter_map(|command| match command {
            SceneCommand::FillRect {
                x,
                y,
                w,
                h,
                paint: Paint::Solid { color },
            } => Some((*x, *y, *w, *h, (color.r, color.g, color.b))),
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

fn scale_translates(result: &zenith_scene::CompileResult) -> Vec<(f64, f64, f64, f64)> {
    result
        .scene
        .commands
        .iter()
        .filter_map(|command| match command {
            SceneCommand::PushScaleTranslate { sx, sy, tx, ty } => Some((*sx, *sy, *tx, *ty)),
            _ => None,
        })
        .collect()
}

/// A host document declaring a `token-map from="color.brand" to="{map_to}"` on
/// the `library` import, plus a single imported-card instance.
fn host_doc_with_token_map(map_to: &str) -> common::Document {
    parse(&format!(
        r##"zenith version=1 {{
  project id="proj.host" name="Host"
  tokens format="zenith-token-v1" {{
    token id="color.host" type="color" value="#ff00ff"
  }}
  imports {{
    import id="library" kind="zen" src="lib.zen" {{
      token-map from="color.brand" to="{map_to}"
    }}
  }}
  styles {{}}
  components {{}}
  document id="doc.host" title="Host" {{
    page id="page.host" w=(px)100 h=(px)80 {{
      instance id="inst.imported" source="library#component.component.card" x=(px)5 y=(px)7
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

fn first_stroke_polyline_points(result: &zenith_scene::CompileResult) -> Vec<f64> {
    result
        .scene
        .commands
        .iter()
        .find_map(|command| match command {
            SceneCommand::StrokePolyline { points, .. } => Some(points.clone()),
            _ => None,
        })
        .expect("expected imported connector to emit a StrokePolyline")
}

#[test]
fn imported_instance_expands_component_from_in_memory_graph() {
    let host = host_doc("library#component.component.card");
    let imported = imported_doc();
    let imports = ImportGraph::new().with_document("library", &imported);

    let result = compile_page_with_imports(&host, &default_provider(), 0, None, &imports);

    assert_eq!(diagnostic_codes(&result), Vec::<&str>::new());
    assert_eq!(
        fill_rects(&result),
        vec![(5.0, 7.0, 40.0, 20.0, (0, 0, 255))]
    );
}

#[test]
fn compile_page_without_graph_keeps_unsupported_source_advisory() {
    let host = host_doc("library#component.component.card");

    let result = compile_page(&host, &default_provider(), 0, None);

    assert_eq!(
        diagnostic_codes(&result),
        vec!["scene.unsupported_import_source"]
    );
    assert!(fill_rects(&result).is_empty());
}

#[test]
fn imported_component_uses_imported_tokens_not_host_tokens() {
    let host = host_doc("library#component.component.card");
    let imported = imported_doc();
    let imports = ImportGraph::new().with_document("library", &imported);

    let result = compile_page_with_imports(&host, &default_provider(), 0, None, &imports);

    let fills = fill_rects(&result);
    assert_eq!(fills, vec![(5.0, 7.0, 40.0, 20.0, (0, 0, 255))]);
}

#[test]
fn imported_component_namespaces_asset_ids() {
    let host = host_doc("library#component.component.image");
    let imported = imported_doc();
    let imports = ImportGraph::new().with_document("library", &imported);

    let result = compile_page_with_imports(&host, &default_provider(), 0, None, &imports);

    assert_eq!(diagnostic_codes(&result), Vec::<&str>::new());
    assert_eq!(image_asset_ids(&result), vec!["library/logo"]);
}

#[test]
fn imported_component_override_fill_uses_host_token_scope() {
    let host = host_doc_with_instance_body(
        r#"instance id="inst.imported" source="library#component.component.card" x=(px)5 y=(px)7 {
        override ref="bg" fill=(token)"color.override"
      }"#,
    );
    let imported = imported_doc();
    let imports = ImportGraph::new().with_document("library", &imported);

    let result = compile_page_with_imports(&host, &default_provider(), 0, None, &imports);

    assert_eq!(diagnostic_codes(&result), Vec::<&str>::new());
    assert_eq!(
        fill_rects(&result),
        vec![(5.0, 7.0, 40.0, 20.0, (255, 255, 0))]
    );
}

#[test]
fn connector_port_projects_through_imported_instance() {
    let host = host_doc_with_instance_body(
        r##"instance id="agent" source="library#component.component.agent.node" x=(px)40 y=(px)40
      rect id="store" x=(px)300 y=(px)60 w=(px)100 h=(px)80 fill=(token)"color.brand"
      connector id="c1" from="agent#out" to="store" to-anchor="left" stroke=(token)"color.brand""##,
    );
    let imported = imported_doc();
    let imports = ImportGraph::new().with_document("library", &imported);

    let result = compile_page_with_imports(&host, &default_provider(), 0, None, &imports);

    assert_eq!(diagnostic_codes(&result), Vec::<&str>::new());
    assert_eq!(
        first_stroke_polyline_points(&result),
        vec![140.0, 80.0, 300.0, 100.0]
    );
}

#[test]
fn missing_import_emits_unknown_import_and_skips() {
    let host = host_doc("missing#component.component.card");
    let imports = ImportGraph::new();

    let result = compile_page_with_imports(&host, &default_provider(), 0, None, &imports);

    assert_eq!(diagnostic_codes(&result), vec!["scene.unknown_import"]);
    assert!(fill_rects(&result).is_empty());
}

#[test]
fn missing_imported_component_emits_unknown_import_component_and_skips() {
    let host = host_doc("library#component.component.missing");
    let imported = imported_doc();
    let imports = ImportGraph::new().with_document("library", &imported);

    let result = compile_page_with_imports(&host, &default_provider(), 0, None, &imports);

    assert_eq!(
        diagnostic_codes(&result),
        vec!["scene.unknown_import_component"]
    );
    assert!(fill_rects(&result).is_empty());
}

#[test]
fn unsupported_page_target_emits_unsupported_import_target_and_skips() {
    let host = host_doc("library#page.page.imported");
    let imported = imported_doc();
    let imports = ImportGraph::new().with_document("library", &imported);

    let result = compile_page_with_imports(&host, &default_provider(), 0, None, &imports);

    assert_eq!(
        diagnostic_codes(&result),
        vec!["scene.unsupported_import_target"]
    );
    assert!(fill_rects(&result).is_empty());
}

#[test]
fn imported_instance_contain_scales_and_centers() {
    // Source bounds are (0,0,40,20). w=100,h=40 → contain scale=min(2.5,2.0)=2.0;
    // centered horizontally: ox=(100-80)/2=10, oy=0; origin (5,7).
    let host = host_doc_with_instance_body(
        r#"instance id="inst.imported" source="library#component.component.card" x=(px)5 y=(px)7 w=(px)100 h=(px)40 fit="contain""#,
    );
    let imported = imported_doc();
    let imports = ImportGraph::new().with_document("library", &imported);

    let result = compile_page_with_imports(&host, &default_provider(), 0, None, &imports);

    assert_eq!(diagnostic_codes(&result), Vec::<&str>::new());
    assert_eq!(scale_translates(&result), vec![(2.0, 2.0, 15.0, 7.0)]);
}

#[test]
fn imported_instance_fill_distorts_axes_independently() {
    // w=80,h=60 → sx=80/40=2, sy=60/20=3; no centering; origin (5,7).
    let host = host_doc_with_instance_body(
        r#"instance id="inst.imported" source="library#component.component.card" x=(px)5 y=(px)7 w=(px)80 h=(px)60 fit="fill""#,
    );
    let imported = imported_doc();
    let imports = ImportGraph::new().with_document("library", &imported);

    let result = compile_page_with_imports(&host, &default_provider(), 0, None, &imports);

    assert_eq!(diagnostic_codes(&result), Vec::<&str>::new());
    assert_eq!(scale_translates(&result), vec![(2.0, 3.0, 5.0, 7.0)]);
}

#[test]
fn imported_instance_fit_none_ignores_declared_size() {
    // fit="none" keeps scale 1; content is placed at the origin unscaled.
    let host = host_doc_with_instance_body(
        r#"instance id="inst.imported" source="library#component.component.card" x=(px)5 y=(px)7 w=(px)80 h=(px)60 fit="none""#,
    );
    let imported = imported_doc();
    let imports = ImportGraph::new().with_document("library", &imported);

    let result = compile_page_with_imports(&host, &default_provider(), 0, None, &imports);

    assert_eq!(diagnostic_codes(&result), Vec::<&str>::new());
    assert_eq!(scale_translates(&result), vec![(1.0, 1.0, 5.0, 7.0)]);
    // The rect is compiled at the LOCAL origin; the PushScaleTranslate places it
    // (rendered at 1*0+5 = 5, 1*0+7 = 7) without scaling.
    assert_eq!(
        fill_rects(&result),
        vec![(0.0, 0.0, 40.0, 20.0, (0, 0, 255))]
    );
}

#[test]
fn imported_instance_without_wh_emits_no_scale_transform() {
    // No w/h → the pre-fit translate path is preserved (byte-identical): the
    // rect lands at the instance origin with no PushScaleTranslate.
    let host = host_doc("library#component.component.card");
    let imported = imported_doc();
    let imports = ImportGraph::new().with_document("library", &imported);

    let result = compile_page_with_imports(&host, &default_provider(), 0, None, &imports);

    assert_eq!(diagnostic_codes(&result), Vec::<&str>::new());
    assert!(scale_translates(&result).is_empty());
    assert_eq!(
        fill_rects(&result),
        vec![(5.0, 7.0, 40.0, 20.0, (0, 0, 255))]
    );
}

#[test]
fn imported_instance_unknown_fit_emits_advisory_and_skips() {
    let host = host_doc_with_instance_body(
        r#"instance id="inst.imported" source="library#component.component.card" x=(px)5 y=(px)7 w=(px)80 h=(px)60 fit="cover""#,
    );
    let imported = imported_doc();
    let imports = ImportGraph::new().with_document("library", &imported);

    let result = compile_page_with_imports(&host, &default_provider(), 0, None, &imports);

    assert_eq!(
        diagnostic_codes(&result),
        vec!["scene.unsupported_import_target"]
    );
    assert!(fill_rects(&result).is_empty());
    assert!(scale_translates(&result).is_empty());
}

#[test]
fn token_map_bridges_host_token_into_imported_subtree() {
    // token-map from="color.brand" to="color.host" (#ff00ff) → the imported
    // rect that fills with color.brand paints with the host color.
    let host = host_doc_with_token_map("color.host");
    let imported = imported_doc();
    let imports = ImportGraph::new().with_document("library", &imported);

    let result = compile_page_with_imports(&host, &default_provider(), 0, None, &imports);

    assert_eq!(diagnostic_codes(&result), Vec::<&str>::new());
    assert_eq!(
        fill_rects(&result),
        vec![(5.0, 7.0, 40.0, 20.0, (255, 0, 255))]
    );
}

#[test]
fn token_map_missing_host_target_emits_conflict_and_keeps_imported_value() {
    let host = host_doc_with_token_map("color.nonexistent");
    let imported = imported_doc();
    let imports = ImportGraph::new().with_document("library", &imported);

    let result = compile_page_with_imports(&host, &default_provider(), 0, None, &imports);

    assert_eq!(diagnostic_codes(&result), vec!["import.token_conflict"]);
    // Isolation preserved: the imported value (#0000ff) still paints.
    assert_eq!(
        fill_rects(&result),
        vec![(5.0, 7.0, 40.0, 20.0, (0, 0, 255))]
    );
}

#[test]
fn malformed_source_emits_invalid_import_source_and_skips() {
    let host = host_doc("library/component.component.card");
    let imported = imported_doc();
    let imports = ImportGraph::new().with_document("library", &imported);

    let result = compile_page_with_imports(&host, &default_provider(), 0, None, &imports);

    assert_eq!(
        diagnostic_codes(&result),
        vec!["scene.invalid_import_source"]
    );
    assert!(fill_rects(&result).is_empty());
}
