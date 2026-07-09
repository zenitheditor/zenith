//! Shared contrast-test builder helpers.
//!
//! Moved verbatim from `validate_contrast.rs` (bodies unchanged) so they can be
//! shared across the split `validate_contrast*.rs` test binaries.

use super::*;
use std::collections::BTreeMap;
use zenith_core::{GradientKind, GradientLiteral, GradientStopRef};

/// Build a dimension token in pt.
pub fn dim_token_pt(id: &str, value: f64) -> Token {
    Token {
        id: id.to_owned(),
        token_type: TokenType::Dimension,
        value: TokenValue::Literal(TokenLiteral::Dimension(Dimension {
            value,
            unit: Unit::Pt,
        })),
        set: None,
        source_span: None,
    }
}

/// Build a font-weight token.
pub fn fw_token(id: &str, weight: f64) -> Token {
    Token {
        id: id.to_owned(),
        token_type: TokenType::FontWeight,
        value: TokenValue::Literal(TokenLiteral::Number(weight)),
        set: None,
        source_span: None,
    }
}

pub fn linear_gradient_token(id: &str, stops: Vec<(f64, &str)>) -> Token {
    Token {
        id: id.to_owned(),
        token_type: TokenType::Gradient,
        value: TokenValue::Literal(TokenLiteral::Gradient(GradientLiteral {
            kind: GradientKind::Linear,
            angle_deg: 0.0,
            center_x: None,
            center_y: None,
            radius: None,
            stops: stops
                .into_iter()
                .map(|(offset, color)| GradientStopRef {
                    offset,
                    color_token: color.to_owned(),
                })
                .collect(),
        })),
        set: None,
        source_span: None,
    }
}

/// Helper: build a page with a background color token reference.
pub fn page_with_bg(id: &str, bg_token_id: &str, children: Vec<Node>) -> Page {
    Page {
        id: id.to_owned(),
        name: None,
        source: None,
        fit: None,
        width: px(1280.0),
        height: px(720.0),
        background: Some(PropertyValue::TokenRef(bg_token_id.to_owned())),
        bleed: None,
        margin_inner: None,
        margin_outer: None,
        margin_top: None,
        margin_bottom: None,
        baseline_grid: None,
        line_jumps: None,
        parity: None,
        master: None,
        safe_zones: Vec::new(),
        folds: Vec::new(),
        construction: zenith_core::ConstructionBlock::default(),
        ports: Vec::new(),
        block_styles: Vec::new(),
        children,
        source_span: None,
    }
}

pub fn backdrop_image_asset(id: &str) -> AssetDecl {
    AssetDecl {
        id: id.to_owned(),
        kind: AssetKind::Image,
        src: "assets/backdrop.png".to_owned(),
        sha256: None,
        producer_kind: None,
        producer_source: None,
        ai_prompt: None,
        ai_model: None,
        ai_provider: None,
        ai_seed: None,
        ai_generation_date: None,
        ai_license: None,
        ai_source_rights: None,
        ai_safety_status: None,
        ai_reuse_policy: None,
        source_span: None,
        unknown_props: BTreeMap::new(),
    }
}

pub fn doc_with_backdrop_image(tokens: Vec<Token>, children: Vec<Node>) -> Document {
    let mut doc = doc_with(
        tokens,
        vec![page_with_bg("page.one", "color.page", children)],
    );
    doc.assets = AssetBlock {
        assets: vec![backdrop_image_asset("asset.backdrop")],
        source_span: None,
    };
    doc
}

/// Build a text node with explicit fill and optional font-size / font-weight.
pub fn text_with_fill_and_size(
    id: &str,
    fill_token: Option<&str>,
    font_size_token: Option<&str>,
    font_weight_token: Option<&str>,
) -> Node {
    Node::Text(Box::new(zenith_core::TextNode {
        shadow: None,
        filter: None,
        mask: None,
        id: id.to_owned(),
        name: None,
        role: None,
        x: Some(pxv(0.0)),
        y: Some(pxv(0.0)),
        w: Some(pxv(200.0)),
        h: Some(pxv(40.0)),
        align: None,
        v_align: None,
        direction: None,
        overflow: None,
        overflow_wrap: None,
        style: None,
        fill: fill_token.map(|t| PropertyValue::TokenRef(t.to_owned())),
        stroke: None,
        stroke_width: None,
        contrast_bg: None,
        font_family: None,
        font_size: font_size_token.map(|t| PropertyValue::TokenRef(t.to_owned())),
        font_size_min: None,
        font_weight: font_weight_token.map(|t| PropertyValue::TokenRef(t.to_owned())),
        font_features: None,
        font_alternates: None,
        letter_spacing: None,
        kerning_pairs: Vec::new(),
        opacity: None,
        visible: None,
        locked: None,
        selectable: None,
        rotate: None,
        blend_mode: None,
        blur: None,
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
        spans: vec![],
        block_styles: Vec::new(),
        source_span: None,
        unknown_props: BTreeMap::new(),
    }))
}

/// Build a filled ellipse backdrop large enough for a centered text box.
pub fn ellipse_backdrop(id: &str, fill_token: &str) -> Node {
    Node::Ellipse(EllipseNode {
        shadow: None,
        filter: None,
        mask: None,
        id: id.to_owned(),
        name: None,
        role: None,
        x: Some(pxv(520.0)),
        y: Some(pxv(300.0)),
        w: Some(pxv(240.0)),
        h: Some(pxv(120.0)),
        rx: None,
        ry: None,
        style: None,
        fill: Some(PropertyValue::TokenRef(fill_token.to_owned())),
        stroke: None,
        stroke_width: None,
        stroke_dash: None,
        stroke_gap: None,
        stroke_linecap: None,
        opacity: None,
        visible: None,
        locked: None,
        rotate: None,
        blend_mode: None,
        blur: None,
        anchor: None,
        anchor_zone: None,
        anchor_sibling: None,
        anchor_edge: None,
        anchor_gap: None,
        anchor_parent: None,
        source_span: None,
        unknown_props: BTreeMap::new(),
    })
}

pub fn rect_backdrop_at(id: &str, fill_token: &str, x: f64, y: f64, w: f64, h: f64) -> Node {
    rect_backdrop_at_with_opacity(id, fill_token, x, y, w, h, None)
}

pub fn rect_backdrop_at_with_opacity(
    id: &str,
    fill_token: &str,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    opacity: Option<f64>,
) -> Node {
    let Node::Rect(mut rect) =
        minimal_rect(id, Some(PropertyValue::TokenRef(fill_token.to_owned())))
    else {
        unreachable!("minimal_rect returns Node::Rect");
    };
    rect.x = Some(pxv(x));
    rect.y = Some(pxv(y));
    rect.w = Some(pxv(w));
    rect.h = Some(pxv(h));
    rect.opacity = opacity;
    Node::Rect(rect)
}

pub fn group_at(id: &str, x: f64, y: f64, children: Vec<Node>) -> Node {
    group_at_with_opacity(id, x, y, None, children)
}

pub fn group_at_with_opacity(
    id: &str,
    x: f64,
    y: f64,
    opacity: Option<f64>,
    children: Vec<Node>,
) -> Node {
    Node::Group(GroupNode {
        id: id.to_owned(),
        name: None,
        role: None,
        x: Some(pxv(x)),
        y: Some(pxv(y)),
        w: None,
        h: None,
        opacity,
        visible: None,
        locked: None,
        rotate: None,
        blend_mode: None,
        shadow: None,
        filter: None,
        mask: None,
        blur: None,
        style: None,
        semantic_role: None,
        intensity: None,
        layer_priority: None,
        symmetry_count: None,
        symmetry_cx: None,
        symmetry_cy: None,
        symmetry_start_angle: None,
        symmetry_mode: None,
        anchor: None,
        anchor_zone: None,
        anchor_sibling: None,
        anchor_edge: None,
        anchor_gap: None,
        anchor_parent: None,
        children,
        protected_regions: Vec::new(),
        editable_param_ids: Vec::new(),
        source_span: None,
        unknown_props: BTreeMap::new(),
    })
}

pub fn text_at(id: &str, fill_token: &str, x: f64, y: f64, w: f64, h: f64) -> Node {
    let Node::Text(mut text) =
        minimal_text(id, Some(PropertyValue::TokenRef(fill_token.to_owned())))
    else {
        unreachable!("minimal_text returns Node::Text");
    };
    text.x = Some(pxv(x));
    text.y = Some(pxv(y));
    text.w = Some(pxv(w));
    text.h = Some(pxv(h));
    Node::Text(text)
}

pub fn shape_backdrop_at(
    id: &str,
    kind: &str,
    fill_token: &str,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) -> Node {
    Node::Shape(Box::new(ShapeNode {
        id: id.to_owned(),
        name: None,
        role: None,
        x: Some(pxv(x)),
        y: Some(pxv(y)),
        w: Some(pxv(w)),
        h: Some(pxv(h)),
        kind: Some(kind.to_owned()),
        fill: Some(PropertyValue::TokenRef(fill_token.to_owned())),
        stroke: None,
        stroke_width: None,
        radius: None,
        stroke_alignment: None,
        padding: None,
        h_align: None,
        v_align: None,
        text_style: None,
        spans: Vec::new(),
        style: None,
        opacity: None,
        visible: None,
        locked: None,
        rotate: None,
        anchor: None,
        anchor_zone: None,
        anchor_sibling: None,
        anchor_edge: None,
        anchor_gap: None,
        anchor_parent: None,
        source_span: None,
        unknown_props: BTreeMap::new(),
    }))
}

pub fn image_backdrop_at(id: &str, x: f64, y: f64, w: f64, h: f64) -> Node {
    image_backdrop_at_with_opacity(id, x, y, w, h, None)
}

pub fn image_backdrop_at_with_opacity(
    id: &str,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    opacity: Option<f64>,
) -> Node {
    Node::Image(ImageNode {
        shadow: None,
        filter: None,
        mask: None,
        id: id.to_owned(),
        name: None,
        role: None,
        asset: "asset.backdrop".to_owned(),
        x: Some(pxv(x)),
        y: Some(pxv(y)),
        w: Some(pxv(w)),
        h: Some(pxv(h)),
        src_x: None,
        src_y: None,
        src_w: None,
        src_h: None,
        fit: None,
        svg_stroke: None,
        svg_fill: None,
        svg_stroke_width: None,
        clip: None,
        clip_radius: None,
        object_position_x: None,
        object_position_y: None,
        opacity,
        visible: None,
        locked: None,
        rotate: None,
        blend_mode: None,
        blur: None,
        style: None,
        anchor: None,
        anchor_zone: None,
        anchor_sibling: None,
        anchor_edge: None,
        anchor_gap: None,
        anchor_parent: None,
        source_span: None,
        unknown_props: BTreeMap::new(),
    })
}

pub fn polygon_backdrop(id: &str, fill_token: &str, points: Vec<(f64, f64)>) -> Node {
    Node::Polygon(PolygonNode {
        id: id.to_owned(),
        name: None,
        role: None,
        fill: Some(PropertyValue::TokenRef(fill_token.to_owned())),
        stroke: None,
        stroke_width: None,
        stroke_alignment: None,
        fill_rule: None,
        opacity: None,
        visible: None,
        locked: None,
        rotate: None,
        style: None,
        points: points
            .into_iter()
            .map(|(x, y)| Point {
                x: Some(px(x)),
                y: Some(px(y)),
            })
            .collect(),
        source_span: None,
        unknown_props: BTreeMap::new(),
    })
}

pub fn polyline_backdrop(id: &str, fill_token: &str, points: Vec<(f64, f64)>) -> Node {
    Node::Polyline(PolylineNode {
        id: id.to_owned(),
        name: None,
        role: None,
        fill: Some(PropertyValue::TokenRef(fill_token.to_owned())),
        stroke: None,
        stroke_width: None,
        fill_rule: None,
        opacity: None,
        visible: None,
        locked: None,
        rotate: None,
        style: None,
        points: points
            .into_iter()
            .map(|(x, y)| Point {
                x: Some(px(x)),
                y: Some(px(y)),
            })
            .collect(),
        source_span: None,
        unknown_props: BTreeMap::new(),
    })
}

/// Build a text node with explicit dimensions and page-relative anchor.
pub fn anchored_text_with_fill_and_size(
    id: &str,
    fill_token: &str,
    font_size_token: &str,
    anchor: &str,
) -> Node {
    let Node::Text(mut text) =
        minimal_text(id, Some(PropertyValue::TokenRef(fill_token.to_owned())))
    else {
        unreachable!("minimal_text returns Node::Text");
    };
    text.font_size = Some(PropertyValue::TokenRef(font_size_token.to_owned()));
    text.x = None;
    text.y = None;
    text.w = Some(pxv(120.0));
    text.h = Some(pxv(40.0));
    text.anchor = Some(anchor.to_owned());
    Node::Text(text)
}

/// Build a text node with an explicit fill token AND a `contrast-bg` hint token.
pub fn text_with_fill_and_contrast_bg(id: &str, fill_token: &str, contrast_bg_token: &str) -> Node {
    Node::Text(Box::new(zenith_core::TextNode {
        shadow: None,
        filter: None,
        mask: None,
        id: id.to_owned(),
        name: None,
        role: None,
        x: Some(pxv(0.0)),
        y: Some(pxv(0.0)),
        w: Some(pxv(200.0)),
        h: Some(pxv(40.0)),
        align: None,
        v_align: None,
        direction: None,
        overflow: None,
        overflow_wrap: None,
        style: None,
        fill: Some(PropertyValue::TokenRef(fill_token.to_owned())),
        stroke: None,
        stroke_width: None,
        contrast_bg: Some(PropertyValue::TokenRef(contrast_bg_token.to_owned())),
        font_family: None,
        font_size: None,
        font_size_min: None,
        font_weight: None,
        font_features: None,
        font_alternates: None,
        letter_spacing: None,
        kerning_pairs: Vec::new(),
        opacity: None,
        visible: None,
        locked: None,
        selectable: None,
        rotate: None,
        blend_mode: None,
        blur: None,
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
        spans: vec![],
        block_styles: Vec::new(),
        source_span: None,
        unknown_props: BTreeMap::new(),
    }))
}

/// Build a minimal `TableNode` with one body row containing one cell, where the
/// cell holds a single text child.
pub fn table_with_cell_text(
    cell_fill: Option<PropertyValue>,
    table_fill: Option<PropertyValue>,
    header_fill: Option<PropertyValue>,
    header_rows: Option<u32>,
    text_fill_token: &str,
) -> Node {
    let text = minimal_text(
        "cell.text",
        Some(PropertyValue::TokenRef(text_fill_token.to_owned())),
    );
    let cell = TableCell {
        colspan: 1,
        rowspan: 1,
        children: vec![text],
        fill: cell_fill,
        border: None,
        border_width: None,
        h_align: None,
        v_align: None,
        source_span: None,
        unknown_props: BTreeMap::new(),
    };
    let row = TableRow {
        cells: vec![cell],
        source_span: None,
        unknown_props: BTreeMap::new(),
    };
    Node::Table(Box::new(TableNode {
        id: "table.one".to_owned(),
        name: None,
        role: None,
        x: Some(pxv(0.0)),
        y: Some(pxv(0.0)),
        w: Some(pxv(400.0)),
        h: Some(pxv(200.0)),
        columns: vec![],
        rows: vec![row],
        header_rows,
        flows: None,
        gap: None,
        cell_padding: None,
        border_collapse: None,
        fill: table_fill,
        border: None,
        border_width: None,
        header_fill,
        header_style: None,
        h_align: None,
        v_align: None,
        style: None,
        opacity: None,
        visible: None,
        locked: None,
        rotate: None,
        anchor: None,
        anchor_zone: None,
        anchor_sibling: None,
        anchor_edge: None,
        anchor_gap: None,
        anchor_parent: None,
        source_span: None,
        unknown_props: BTreeMap::new(),
    }))
}

/// The three colour tokens used across the unmodeled-backdrop tests: a white
/// page, a navy backdrop, and black text (black on navy is APCA-invisible).
pub fn base_contrast_tokens() -> Vec<Token> {
    vec![
        color_token_hex("color.page", "#ffffff"),
        color_token_hex("color.backdrop", "#003087"),
        color_token_hex("color.text", "#000000"),
    ]
}

/// A page (white bg) holding `backdrop` then black text at (130,130,80,30).
pub fn backdrop_over_text_doc(backdrop: Node) -> Document {
    doc_with(
        base_contrast_tokens(),
        vec![page_with_bg(
            "page.one",
            "color.page",
            vec![
                backdrop,
                text_at("headline", "color.text", 130.0, 130.0, 80.0, 30.0),
            ],
        )],
    )
}

/// Build a rect backdrop then mutate it (radius/rotate/mask/blur/blend/…).
pub fn rect_backdrop_with(
    id: &str,
    fill_token: &str,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    mutate: impl FnOnce(&mut RectNode),
) -> Node {
    let Node::Rect(mut rect) = rect_backdrop_at(id, fill_token, x, y, w, h) else {
        unreachable!("rect_backdrop_at returns Node::Rect");
    };
    mutate(&mut rect);
    Node::Rect(rect)
}

/// A `group` at (x,y) rotated `deg` degrees around its subtree.
pub fn rotated_group(id: &str, x: f64, y: f64, deg: f64, children: Vec<Node>) -> Node {
    let Node::Group(mut group) = group_at(id, x, y, children) else {
        unreachable!("group_at returns Node::Group");
    };
    group.rotate = Some(Dimension {
        value: deg,
        unit: Unit::Deg,
    });
    Node::Group(group)
}

/// A filled `path` whose anchors trace the rectangle (x,y,w,h).
pub fn path_box_backdrop(id: &str, fill_token: &str, x: f64, y: f64, w: f64, h: f64) -> Node {
    let corner = |px_x: f64, px_y: f64| PathAnchor {
        x: Some(px(px_x)),
        y: Some(px(px_y)),
        kind: None,
        in_x: None,
        in_y: None,
        out_x: None,
        out_y: None,
    };
    Node::Path(PathNode {
        id: id.to_owned(),
        name: None,
        role: None,
        closed: Some(true),
        fill: Some(PropertyValue::TokenRef(fill_token.to_owned())),
        stroke: None,
        stroke_width: None,
        stroke_alignment: None,
        stroke_linejoin: None,
        stroke_linecap: None,
        stroke_miter_limit: None,
        fill_rule: None,
        opacity: None,
        visible: None,
        locked: None,
        rotate: None,
        style: None,
        anchors: vec![
            corner(x, y),
            corner(x + w, y),
            corner(x + w, y + h),
            corner(x, y + h),
        ],
        subpaths: Vec::new(),
        source_span: None,
        unknown_props: BTreeMap::new(),
    })
}

/// A frame (clip box) holding the given children.
pub fn frame_clip(id: &str, x: f64, y: f64, w: f64, h: f64, children: Vec<Node>) -> Node {
    Node::Frame(FrameNode {
        id: id.to_owned(),
        name: None,
        role: None,
        x: Some(pxv(x)),
        y: Some(pxv(y)),
        w: Some(pxv(w)),
        h: Some(pxv(h)),
        layout: None,
        columns: None,
        rows: None,
        opacity: None,
        visible: None,
        locked: None,
        rotate: None,
        blend_mode: None,
        shadow: None,
        filter: None,
        mask: None,
        blur: None,
        style: None,
        anchor: None,
        anchor_zone: None,
        anchor_sibling: None,
        anchor_edge: None,
        anchor_gap: None,
        anchor_parent: None,
        children,
        source_span: None,
        unknown_props: BTreeMap::new(),
    })
}

/// Anchored text (page anchor, no w/h) with a resolvable fill, no `contrast-bg`.
pub fn anchored_boxless_text(id: &str, fill_token: &str, contrast_bg: Option<&str>) -> Node {
    let Node::Text(mut text) =
        minimal_text(id, Some(PropertyValue::TokenRef(fill_token.to_owned())))
    else {
        unreachable!("minimal_text returns Node::Text");
    };
    text.x = None;
    text.y = None;
    text.w = None;
    text.h = None;
    text.anchor = Some("center".to_owned());
    text.contrast_bg = contrast_bg.map(|t| PropertyValue::TokenRef(t.to_owned()));
    Node::Text(text)
}
