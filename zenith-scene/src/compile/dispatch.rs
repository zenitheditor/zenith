//! Per-node compile dispatcher.

use zenith_core::{Diagnostic, Node};

use crate::ir::SceneCommand;

use super::chart::compile_chart;
use super::container::{compile_frame, compile_group, compile_instance};
use super::ctx::NodeCtx;
use super::effect::{compile_light, compile_mesh};
use super::field::resolve_field_to_text;
use super::image::compile_image;
use super::leaf::{
    ConnectorEnv, RectEllipseEnv, ShapeCompileEnv, compile_connector, compile_ellipse,
    compile_line, compile_path, compile_polygon, compile_polyline, compile_rect, compile_shape,
};
use super::line_jumps;
use super::pattern::compile_pattern;
use super::pipeline::RenderCtx;
use super::table::{TableEmitCtx, compile_table};
use super::text::{TextCompileEnv, compile_code, compile_text, empty_md_blocks};
use super::toc::resolve_toc_to_text;

// ── Node dispatch ─────────────────────────────────────────────────────────────

/// Route a single node to the submodule that compiles its kind.
///
/// Each arm forwards the full cascade context to a `compile_*` function; the
/// emitted `SceneCommand` stream is identical to the previous inline match.
///
/// Returns the child's laid-out content height in pixels for the kinds whose
/// intrinsic height is meaningful to flow layout (`text`/`code`); every other
/// kind returns `0.0`. The absolute-positioning callers ignore this value, so
/// command output is unchanged; only the flow-layout path in [`container`]
/// consumes it to advance its vertical cursor.
pub(in crate::compile) fn compile_node(
    node: &Node,
    cx: NodeCtx,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
    connector_strokes: &mut Vec<usize>,
    ctx: RenderCtx,
) -> f64 {
    // Non-printing guide nodes (`role="guide"`) are excluded from render output
    // entirely — including their subtree when the guide is a group/frame.
    if node.role() == Some("guide") {
        return 0.0;
    }

    let NodeCtx {
        resolved,
        style_map,
        components,
        imports,
        fonts,
        engine,
        chains,
        flows,
        anchors,
        field_ctx,
        md_blocks,
        page_block_styles,
        doc_block_styles,
    } = cx;

    match node {
        Node::Rect(rect) => {
            compile_rect(
                rect,
                RectEllipseEnv {
                    resolved,
                    style_map,
                    anchors,
                },
                commands,
                diagnostics,
                ctx,
            );
            0.0
        }
        Node::Ellipse(ellipse) => {
            compile_ellipse(
                ellipse,
                RectEllipseEnv {
                    resolved,
                    style_map,
                    anchors,
                },
                commands,
                diagnostics,
                ctx,
            );
            0.0
        }
        Node::Light(light) => {
            compile_light(light, resolved, commands, diagnostics, ctx);
            0.0
        }
        Node::Mesh(mesh) => {
            compile_mesh(mesh, resolved, commands, diagnostics, ctx);
            0.0
        }
        Node::Text(text) => compile_text(
            text,
            TextCompileEnv {
                resolved,
                style_map,
                fonts,
                engine,
                chains,
                footnote_markers: field_ctx.footnote_markers,
                node_boxes: field_ctx.node_boxes,
                anchors,
                md_blocks,
                page_block_styles,
                doc_block_styles,
            },
            commands,
            diagnostics,
            ctx,
        ),
        Node::Line(line) => {
            compile_line(line, resolved, style_map, commands, diagnostics, ctx);
            0.0
        }
        Node::Frame(frame) => {
            compile_frame(frame, cx, commands, diagnostics, connector_strokes, ctx);
            0.0
        }
        Node::Group(group) => {
            compile_group(group, cx, commands, diagnostics, connector_strokes, ctx);
            0.0
        }
        Node::Instance(instance) => {
            compile_instance(instance, cx, commands, diagnostics, connector_strokes, ctx);
            0.0
        }
        Node::Field(field) => {
            // Resolve the field against this page into a concrete single-line
            // text node and compile it via the normal text path. An unresolved
            // field (absent running-head side, unknown type, unresolved
            // page-ref) yields nothing.
            if let Some(text_node) = resolve_field_to_text(field, field_ctx) {
                compile_text(
                    &text_node,
                    TextCompileEnv {
                        resolved,
                        style_map,
                        fonts,
                        engine,
                        chains,
                        footnote_markers: field_ctx.footnote_markers,
                        node_boxes: field_ctx.node_boxes,
                        anchors,
                        md_blocks: empty_md_blocks(),
                        page_block_styles: &[],
                        doc_block_styles: &[],
                    },
                    commands,
                    diagnostics,
                    ctx,
                );
            }
            0.0
        }
        Node::Toc(toc) => {
            // Resolve the toc against the full document into a multi-line
            // tab-leader text block and compile it via the normal text path.
            // A toc with no matching headings, no selector, or visible=false
            // yields nothing.
            if let Some(text_node) =
                resolve_toc_to_text(toc, field_ctx.pages, field_ctx.page_index_by_node_id)
            {
                compile_text(
                    &text_node,
                    TextCompileEnv {
                        resolved,
                        style_map,
                        fonts,
                        engine,
                        chains,
                        footnote_markers: field_ctx.footnote_markers,
                        node_boxes: field_ctx.node_boxes,
                        anchors,
                        md_blocks: empty_md_blocks(),
                        page_block_styles: &[],
                        doc_block_styles: &[],
                    },
                    commands,
                    diagnostics,
                    ctx,
                );
            }
            0.0
        }
        Node::Image(image) => {
            compile_image(image, resolved, commands, diagnostics, anchors, ctx);
            0.0
        }
        Node::Polygon(poly) => {
            compile_polygon(poly, resolved, style_map, commands, diagnostics, ctx);
            0.0
        }
        Node::Polyline(poly) => {
            compile_polyline(poly, resolved, style_map, commands, diagnostics, ctx);
            0.0
        }
        Node::Path(path) => {
            compile_path(path, resolved, style_map, commands, diagnostics, ctx);
            0.0
        }
        Node::Code(code) => compile_code(
            code,
            TextCompileEnv {
                resolved,
                style_map,
                fonts,
                engine,
                chains,
                footnote_markers: field_ctx.footnote_markers,
                node_boxes: field_ctx.node_boxes,
                anchors,
                md_blocks: empty_md_blocks(),
                page_block_styles: &[],
                doc_block_styles: &[],
            },
            commands,
            diagnostics,
            ctx,
        ),
        Node::Table(table) => {
            compile_table(
                TableEmitCtx {
                    table,
                    resolved,
                    style_map,
                    components,
                    imports,
                    fonts,
                    engine,
                    chains,
                    flows,
                    anchors,
                    field_ctx,
                },
                commands,
                diagnostics,
                ctx,
            );
            0.0
        }
        Node::Shape(shape) => {
            compile_shape(
                shape,
                commands,
                diagnostics,
                ShapeCompileEnv {
                    resolved,
                    style_map,
                    fonts,
                    engine,
                    chains,
                    footnote_markers: field_ctx.footnote_markers,
                    node_boxes: field_ctx.node_boxes,
                    anchors,
                    ctx,
                },
            );
            0.0
        }
        Node::Connector(connector) => {
            // Record the connector's stroke (top-level OR nested) at its dispatch
            // point so the opt-in line-jump post-pass can hop it. The post-pass
            // filters by transform depth, so rotated/bracketed connectors are
            // excluded there, not here.
            let start = commands.len();
            compile_connector(
                connector,
                commands,
                diagnostics,
                ConnectorEnv {
                    resolved,
                    style_map,
                    fonts,
                    engine,
                    chains,
                    footnote_markers: field_ctx.footnote_markers,
                    node_boxes: field_ctx.node_boxes,
                    connector_outline_boxes: field_ctx.connector_outline_boxes,
                    connector_target_kinds: field_ctx.connector_target_kinds,
                    port_map: field_ctx.port_map,
                    anchors,
                    ctx,
                },
            );
            line_jumps::record_connector_stroke(commands, start, connector_strokes);
            0.0
        }
        Node::Pattern(p) => compile_pattern(p, cx, commands, diagnostics, ctx),
        Node::Chart(c) => compile_chart(c, cx, commands, diagnostics, ctx),
        Node::Footnote(_) => {
            // Footnotes are NON-flowing page furniture: they carry no x/y/w/h
            // and are NOT rendered in the normal z-order dispatch. The page-level
            // footnote pass (`footnote::compile_footnote_zone`, run by
            // `compile_page`) collects every page-level footnote in source order,
            // auto-numbers them, and renders the bottom zone + separator. A
            // footnote reached here (e.g. nested in a container) renders nothing.
            0.0
        }
        Node::Unknown(unknown) => {
            diagnostics.push(Diagnostic::advisory(
                "scene.unsupported_node",
                format!(
                    "unknown node kind '{}' cannot be compiled; the node is skipped \
                     (forward-compatibility: this kind may be supported in a later version)",
                    unknown.kind
                ),
                unknown.source_span,
                None,
            ));
            0.0
        }
    }
}
