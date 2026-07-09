//! Imported page-source composition.

use zenith_core::{DataContext, Diagnostic, FontProvider, Page, dim_to_px};

use crate::ir::SceneCommand;

use super::font_ns::NamespacedFontProvider;
use super::imports::{ImportGraph, ImportScopes, ImportSource, parse_import_source};
use super::{RenderCtx, compile_page_inner};

#[derive(Clone, Copy)]
pub(in crate::compile) struct PageSourceEnv<'a> {
    pub(in crate::compile) page: &'a Page,
    pub(in crate::compile) page_w: f64,
    pub(in crate::compile) page_h: f64,
    pub(in crate::compile) root_ctx: RenderCtx,
    pub(in crate::compile) fonts: &'a dyn FontProvider,
    pub(in crate::compile) data: Option<&'a DataContext>,
    pub(in crate::compile) graph: Option<&'a ImportGraph<'a>>,
    pub(in crate::compile) scopes: &'a ImportScopes<'a>,
}

pub(in crate::compile) fn compile_page_source(
    env: PageSourceEnv<'_>,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(source) = env.page.source.as_deref() else {
        return;
    };

    let Some(graph) = env.graph else {
        diagnostics.push(Diagnostic::advisory(
            "scene.unsupported_import_source",
            format!(
                "page '{}' references imported source '{}' without a scene import graph; the source is skipped",
                env.page.id, source
            ),
            env.page.source_span,
            Some(env.page.id.clone()),
        ));
        return;
    };

    let (import_id, page_id) = match parse_import_source(source) {
        ImportSource::Page { import_id, page_id } => (import_id, page_id),
        ImportSource::Component {
            import_id,
            component_id,
        } => {
            diagnostics.push(Diagnostic::advisory(
                "scene.unsupported_import_target",
                format!(
                    "page '{}' references unsupported import target '{}#component.{}'; the source is skipped",
                    env.page.id, import_id, component_id
                ),
                env.page.source_span,
                Some(env.page.id.clone()),
            ));
            return;
        }
        ImportSource::UnsupportedTarget { import_id, target } => {
            diagnostics.push(Diagnostic::advisory(
                "scene.unsupported_import_target",
                format!(
                    "page '{}' references unsupported import target '{}#{}'; the source is skipped",
                    env.page.id, import_id, target
                ),
                env.page.source_span,
                Some(env.page.id.clone()),
            ));
            return;
        }
        ImportSource::Invalid => {
            diagnostics.push(Diagnostic::advisory(
                "scene.invalid_import_source",
                format!(
                    "page '{}' source '{}' is not a valid import source; the source is skipped",
                    env.page.id, source
                ),
                env.page.source_span,
                Some(env.page.id.clone()),
            ));
            return;
        }
    };

    let Some(scope) = env.scopes.get(import_id) else {
        diagnostics.push(Diagnostic::advisory(
            "scene.unknown_import",
            format!(
                "page '{}' references import '{}' which is not in the scene import graph; the source is skipped",
                env.page.id, import_id
            ),
            env.page.source_span,
            Some(env.page.id.clone()),
        ));
        return;
    };

    let Some(imported_page) = scope.pages.get(page_id).copied() else {
        diagnostics.push(Diagnostic::advisory(
            "scene.unknown_import_page",
            format!(
                "page '{}' references unknown page '{}' in import '{}'; the source is skipped",
                env.page.id, page_id, import_id
            ),
            env.page.source_span,
            Some(env.page.id.clone()),
        ));
        return;
    };

    if imported_page_bleed_px(imported_page).is_some_and(|bleed| bleed > 0.0) {
        diagnostics.push(Diagnostic::advisory(
            "scene.unsupported_import_target",
            format!(
                "page '{}' source '{}' references an imported page with bleed; imported bleed composition is not supported",
                env.page.id, source
            ),
            env.page.source_span,
            Some(env.page.id.clone()),
        ));
        return;
    }

    let Some(imported_index) = scope
        .document
        .body
        .pages
        .iter()
        .position(|candidate| candidate.id == imported_page.id)
    else {
        return;
    };

    let Some((sx, sy, tx, ty)) = fit_transform(env, imported_page, source, diagnostics) else {
        return;
    };

    // The imported page's text requests plain family names; route them through a
    // namespaced wrapper so the import's own faces (registered under
    // `"{import_id}/{family}"`) win, then fall back to bundled/host families.
    let imported_fonts = NamespacedFontProvider::new(env.fonts, import_id);
    let mut imported_result = compile_page_inner(
        scope.document,
        &imported_fonts,
        imported_index,
        env.data,
        Some(graph),
    );
    diagnostics.append(&mut imported_result.diagnostics);
    prefix_imported_command_refs(
        &mut imported_result.scene.commands,
        ImportedCommandPrefixes {
            source_node: &format!("{}/{}#page.{}/", env.page.id, import_id, page_id),
            asset: &format!("{import_id}/"),
        },
    );

    if is_identity_transform(sx, sy, tx, ty) {
        commands.extend(imported_result.scene.commands);
        return;
    }

    commands.push(SceneCommand::PushScaleTranslate { sx, sy, tx, ty });
    commands.extend(imported_result.scene.commands);
    commands.push(SceneCommand::PopTransform);
}

fn fit_transform(
    env: PageSourceEnv<'_>,
    imported_page: &Page,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(f64, f64, f64, f64)> {
    let source_w = dim_to_px(imported_page.width.value, &imported_page.width.unit)?;
    let source_h = dim_to_px(imported_page.height.value, &imported_page.height.unit)?;
    if source_w <= 0.0 || source_h <= 0.0 {
        return None;
    }

    let (sx, sy, offset_x, offset_y) = match env.page.fit.as_deref() {
        None => {
            if !same_px(env.page_w, source_w) || !same_px(env.page_h, source_h) {
                diagnostics.push(Diagnostic::advisory(
                    "scene.unsupported_import_target",
                    format!(
                        "page '{}' source '{}' has different dimensions and no explicit fit; the source is skipped",
                        env.page.id, source
                    ),
                    env.page.source_span,
                    Some(env.page.id.clone()),
                ));
                return None;
            }
            (1.0, 1.0, 0.0, 0.0)
        }
        Some("none") => (1.0, 1.0, 0.0, 0.0),
        Some("contain") => {
            let scale = (env.page_w / source_w).min(env.page_h / source_h);
            (
                scale,
                scale,
                (env.page_w - source_w * scale) / 2.0,
                (env.page_h - source_h * scale) / 2.0,
            )
        }
        Some("fill") => (env.page_w / source_w, env.page_h / source_h, 0.0, 0.0),
        Some(fit) => {
            diagnostics.push(Diagnostic::advisory(
                "scene.unsupported_import_target",
                format!(
                    "page '{}' source '{}' uses unsupported fit '{}'; the source is skipped",
                    env.page.id, source, fit
                ),
                env.page.source_span,
                Some(env.page.id.clone()),
            ));
            return None;
        }
    };

    Some((
        sx,
        sy,
        env.root_ctx.dx + offset_x,
        env.root_ctx.dy + offset_y,
    ))
}

fn imported_page_bleed_px(page: &Page) -> Option<f64> {
    page.bleed
        .as_ref()
        .and_then(|bleed| dim_to_px(bleed.value, &bleed.unit))
}

fn same_px(left: f64, right: f64) -> bool {
    (left - right).abs() <= f64::EPSILON
}

fn is_identity_transform(sx: f64, sy: f64, tx: f64, ty: f64) -> bool {
    same_px(sx, 1.0) && same_px(sy, 1.0) && same_px(tx, 0.0) && same_px(ty, 0.0)
}

struct ImportedCommandPrefixes<'a> {
    source_node: &'a str,
    asset: &'a str,
}

fn prefix_imported_command_refs(commands: &mut [SceneCommand], prefixes: ImportedCommandPrefixes) {
    for command in commands {
        match command {
            SceneCommand::DrawGlyphRun {
                source_node_id: Some(id),
                ..
            } => {
                *id = format!("{}{}", prefixes.source_node, id);
            }
            SceneCommand::DrawImage { asset_id, .. } => {
                *asset_id = format!("{}{}", prefixes.asset, asset_id);
            }
            SceneCommand::DrawSvgAsset { asset, .. } => {
                *asset = format!("{}{}", prefixes.asset, asset);
            }
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
            | SceneCommand::EndMask => {}
        }
    }
}
