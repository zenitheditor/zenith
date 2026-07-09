//! The connector's optional owned label: a synthesized [`TextNode`] rendered
//! centered at the geometric midpoint of the routed polyline.

use std::collections::BTreeMap;

use zenith_core::{ConnectorNode, Diagnostic, TextNode};

use crate::ir::SceneCommand;

use super::super::super::RenderCtx;
use super::super::super::text::{
    MeasureEnv, TextCompileEnv, compile_text, empty_md_blocks, measure_text_wrapped_height,
    resolve_text_families,
};
use super::super::super::util::px_prop;
use super::compile::ConnectorEnv;
use super::route::polyline_midpoint;

/// Synthesize a [`TextNode`] for the connector's owned label and render it
/// centered at the geometric midpoint of the routed polyline.
///
/// The label box is `LABEL_W × LABEL_H` px, centered on the midpoint so the
/// text is visually at the connector's middle. The synthesis mirrors
/// `emit_shape_label` but without padding or vertical alignment (the box is
/// auto-sized at a fixed small height; if the text wraps the center is still
/// approximately correct). The label inherits `ctx.opacity`.
///
/// When `connector.spans` is empty this function returns immediately (no-op),
/// preserving byte-identical output for span-less connectors.
pub(super) fn emit_connector_label(
    connector: &ConnectorNode,
    flat_points: &[f64],
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
    env: ConnectorEnv,
) {
    // Early-out: no spans → no label → byte-identical to the pre-label behaviour.
    if connector.spans.is_empty() {
        return;
    }

    let Some((mx, my)) = polyline_midpoint(flat_points) else {
        return;
    };

    let ConnectorEnv {
        resolved,
        style_map,
        fonts,
        engine,
        chains,
        footnote_markers,
        node_boxes,
        connector_outline_boxes: _,
        connector_target_kinds: _,
        port_map: _,
        anchors,
        ctx,
    } = env;

    // Fixed label box dimensions: wide enough for a short branch label, short
    // enough to not overlap arrowheads on typical connectors. The box is
    // centered on the midpoint.
    const LABEL_W: f64 = 120.0;
    const LABEL_H: f64 = 40.0;

    let lx = mx - LABEL_W / 2.0;
    let ly = my - LABEL_H / 2.0;

    let mut synth = TextNode {
        id: format!("{}/label", connector.id),
        name: None,
        role: None,
        x: Some(px_prop(lx)),
        y: Some(px_prop(ly)),
        w: Some(px_prop(LABEL_W)),
        h: Some(px_prop(LABEL_H)),
        align: Some("center".to_owned()),
        v_align: None,
        direction: None,
        overflow: None,
        overflow_wrap: None,
        style: connector.text_style.clone(),
        fill: None,
        stroke: None,
        stroke_width: None,
        contrast_bg: None,
        font_family: None,
        font_size: None,
        font_size_min: None,
        font_weight: None,
        font_features: None,
        font_alternates: None,
        letter_spacing: None,
        kerning_pairs: Vec::new(),
        shadow: None,
        filter: None,
        mask: None,
        blend_mode: None,
        blur: None,
        opacity: None,
        visible: None,
        locked: None,
        selectable: None,
        rotate: None,
        chain: None,
        drop_cap_lines: None,
        hyphenate: None,
        widow_orphan: None,
        tab_leader: None,
        text_exclusion: None,
        padding_left: None,
        text_indent: None,
        content_format: None,
        src: None,
        bullet: None,
        bullet_gap: None,
        anchor: None,
        anchor_zone: None,
        anchor_sibling: None,
        anchor_edge: None,
        anchor_gap: None,
        anchor_parent: None,
        spans: connector.spans.clone(),
        block_styles: Vec::new(),
        source_span: connector.source_span,
        unknown_props: BTreeMap::new(),
    };

    // VERTICAL CENTERING: pre-offset `y` by the measured wrapped height so the
    // label text is visually centered in the box (mirrors emit_shape_label).
    let families = resolve_text_families(&synth, resolved, style_map, fonts, diagnostics);
    let wrapped_h = measure_text_wrapped_height(
        &synth,
        LABEL_W,
        &families,
        MeasureEnv {
            resolved,
            style_map,
            fonts,
            engine,
        },
        diagnostics,
    )
    .unwrap_or(0.0);
    let v_offset = ((LABEL_H - wrapped_h) / 2.0).max(0.0);
    synth.y = Some(px_prop(ly + v_offset));

    // The midpoint (mx, my) is already in page-absolute coordinates (the flat
    // points were built from page-absolute anchor points). Zero the ctx
    // translation so compile_text does not double-translate — same guard as
    // emit_shape_label.
    let label_ctx = RenderCtx {
        dx: 0.0,
        dy: 0.0,
        ..ctx
    };
    let _ = compile_text(
        &synth,
        TextCompileEnv {
            resolved,
            style_map,
            fonts,
            engine,
            chains,
            footnote_markers,
            node_boxes,
            anchors,
            md_blocks: empty_md_blocks(),
            page_block_styles: &[],
            doc_block_styles: &[],
        },
        commands,
        diagnostics,
        label_ctx,
    );
}
