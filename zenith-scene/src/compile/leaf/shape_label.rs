//! The synthetic `text` node a `shape` label renders and measures through.

use std::collections::BTreeMap;

use zenith_core::{ShapeNode, TextNode};

use super::super::util::px_prop;

/// Build the label text node of `shape`, laid into the content box
/// `(x, y, w, h)` (the shape box inset by its padding).
///
/// `h_align` maps to the text `align` (default `center`). The id derives from
/// the shape id (`<shape>/label`), so it never collides with an authored id.
pub(in crate::compile) fn label_text_node(
    shape: &ShapeNode,
    (content_x, content_y, content_w, content_h): (f64, f64, f64, f64),
) -> TextNode {
    // Map the shape's `h_align` to the text node's `align` (default center).
    let align = match shape.h_align.as_deref() {
        Some("end") => Some("end".to_owned()),
        Some("start") => Some("start".to_owned()),
        // "center", any unrecognized value, and absent all center the label.
        _ => Some("center".to_owned()),
    };

    // Synthesize the label as a fresh TextNode laid into the content box. A
    // synthetic id derived from the shape id keeps it unique (never collides).
    TextNode {
        id: format!("{}/label", shape.id),
        name: None,
        role: None,
        x: Some(px_prop(content_x)),
        y: Some(px_prop(content_y)),
        w: Some(px_prop(content_w)),
        h: Some(px_prop(content_h)),
        layout_item: Default::default(),
        align,
        v_align: None,
        direction: None,
        overflow: None,
        overflow_wrap: None,
        style: shape.text_style.clone(),
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
        spans: shape.spans.clone(),
        block_styles: Vec::new(),
        source_span: shape.source_span,
        unknown_props: BTreeMap::new(),
    }
}
