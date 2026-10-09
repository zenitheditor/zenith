//! Integration tests: contrast validation — unmodeled / translated backdrops.
//!
//! Split out of `validate_contrast.rs` (bug-fix coverage for backdrop kinds the
//! sampler previously mishandled: translated/rotated groups, rounded/rotated
//! rects, path fills, mask/filter/blur/blend effects, boxless anchored text).
//! Test bodies moved verbatim; only the file location changed.

#[path = "common/codes.rs"]
mod codes;
#[path = "common/color_token_hex.rs"]
mod color_token_hex;
#[path = "common/contrast_report.rs"]
mod contrast_report;
#[path = "common/contrast_shared.rs"]
mod contrast_shared;
#[path = "common/has_code.rs"]
mod has_code;
#[path = "common/minimal_rect.rs"]
mod minimal_rect;
#[path = "common/minimal_text.rs"]
mod minimal_text;
#[path = "common/px.rs"]
mod px;
#[path = "common/pxv_doc_with.rs"]
mod pxv_doc_with;

use codes::codes;
use color_token_hex::color_token_hex;
use contrast_report::contrast_report;
use contrast_shared::{ellipse_backdrop, group_at, page_with_bg, rect_backdrop_at, text_at};
use has_code::has_code;
use minimal_text::minimal_text;
use px::px;
use pxv_doc_with::{doc_with, pxv};
use std::collections::BTreeMap;
use zenith_core::{
    Dimension, Document, FrameNode, Node, PathAnchor, PathNode, PathSubpath, PropertyValue,
    RectNode, Token, Unit,
};

// ── Local builders (used only by this binary) ──────────────────────────

/// The three colour tokens used across the unmodeled-backdrop tests: a white
/// page, a navy backdrop, and black text (black on navy is APCA-invisible).
fn base_contrast_tokens() -> Vec<Token> {
    vec![
        color_token_hex("color.page", "#ffffff"),
        color_token_hex("color.backdrop", "#003087"),
        color_token_hex("color.text", "#000000"),
    ]
}

/// A page (white bg) holding `backdrop` then black text at (130,130,80,30).
fn backdrop_over_text_doc(backdrop: Node) -> Document {
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
fn rect_backdrop_with(
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
fn rotated_group(id: &str, x: f64, y: f64, deg: f64, children: Vec<Node>) -> Node {
    let Node::Group(mut group) = group_at(id, x, y, children) else {
        unreachable!("group_at returns Node::Group");
    };
    group.rotate = Some(Dimension {
        value: deg,
        unit: Unit::Deg,
    });
    Node::Group(group)
}

/// Corner-style path anchor at absolute page pixels (no Bezier handles).
fn path_corner(px_x: f64, px_y: f64) -> PathAnchor {
    PathAnchor {
        x: Some(px(px_x)),
        y: Some(px(px_y)),
        kind: None,
        in_x: None,
        in_y: None,
        out_x: None,
        out_y: None,
    }
}

/// Four corner anchors for an axis-aligned rectangle in page pixels.
fn path_rect_anchors(x: f64, y: f64, w: f64, h: f64) -> Vec<PathAnchor> {
    vec![
        path_corner(x, y),
        path_corner(x + w, y),
        path_corner(x + w, y + h),
        path_corner(x, y + h),
    ]
}

/// A filled `path` whose anchors trace the rectangle (x,y,w,h).
fn path_box_backdrop(id: &str, fill_token: &str, x: f64, y: f64, w: f64, h: f64) -> Node {
    path_closed_backdrop(id, fill_token, None, path_rect_anchors(x, y, w, h))
}

/// A filled closed `path` with an explicit fill-rule and anchor list (legacy
/// single-contour form).
fn path_closed_backdrop(
    id: &str,
    fill_token: &str,
    fill_rule: Option<&str>,
    anchors: Vec<PathAnchor>,
) -> Node {
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
        fill_rule: fill_rule.map(|s| s.to_owned()),
        opacity: None,
        visible: None,
        locked: None,
        rotate: None,
        style: None,
        anchors,
        subpaths: Vec::new(),
        source_span: None,
        unknown_props: BTreeMap::new(),
    })
}

/// Evenodd compound path: outer closed contour with an inner hole contour.
fn path_evenodd_hole_backdrop(
    id: &str,
    fill_token: &str,
    outer: (f64, f64, f64, f64),
    hole: (f64, f64, f64, f64),
) -> Node {
    let (ox, oy, ow, oh) = outer;
    let (hx, hy, hw, hh) = hole;
    Node::Path(PathNode {
        id: id.to_owned(),
        name: None,
        role: None,
        closed: None,
        fill: Some(PropertyValue::TokenRef(fill_token.to_owned())),
        stroke: None,
        stroke_width: None,
        stroke_alignment: None,
        stroke_linejoin: None,
        stroke_linecap: None,
        stroke_miter_limit: None,
        fill_rule: Some("evenodd".to_owned()),
        opacity: None,
        visible: None,
        locked: None,
        rotate: None,
        style: None,
        anchors: Vec::new(),
        subpaths: vec![
            PathSubpath {
                closed: Some(true),
                anchors: path_rect_anchors(ox, oy, ow, oh),
            },
            PathSubpath {
                closed: Some(true),
                anchors: path_rect_anchors(hx, hy, hw, hh),
            },
        ],
        source_span: None,
        unknown_props: BTreeMap::new(),
    })
}

/// A frame (clip box) holding the given children.
fn frame_clip(id: &str, x: f64, y: f64, w: f64, h: f64, children: Vec<Node>) -> Node {
    Node::Frame(FrameNode {
        id: id.to_owned(),
        name: None,
        role: None,
        x: Some(pxv(x)),
        y: Some(pxv(y)),
        w: Some(pxv(w)),
        h: Some(pxv(h)),
        layout_item: Default::default(),
        layout: None,
        container: Default::default(),
        clip: None,
        fill: None,
        stroke: None,
        stroke_width: None,
        radius: None,
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
fn anchored_boxless_text(id: &str, fill_token: &str, contrast_bg: Option<&str>) -> Node {
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

// ── Item 1: text sample box translated into page space ─────────────────

#[test]
fn translated_group_text_over_ellipse_flags_invisible() {
    // Regression: black text inside a group translated by (300,300) lands on the
    // page-level navy ellipse (center 640,360). Before the fix the text box was
    // sampled at its un-translated local coordinates and silently passed clean.
    let doc = doc_with(
        base_contrast_tokens(),
        vec![page_with_bg(
            "page.one",
            "color.page",
            vec![
                ellipse_backdrop("backdrop", "color.backdrop"),
                group_at(
                    "badge",
                    300.0,
                    300.0,
                    vec![text_at("mono", "color.text", 300.0, 40.0, 80.0, 30.0)],
                ),
            ],
        )],
    );
    let report = contrast_report(&doc);
    assert!(
        has_code(&report, "contrast.invisible"),
        "translated-group text over the ellipse must flag invisible; codes: {:?}",
        codes(&report)
    );
}

#[test]
fn zero_translation_group_text_still_flags() {
    // Control: same geometry at group (0,0) — the text is authored directly over
    // the ellipse, so both before and after the fix it must flag invisible.
    let doc = doc_with(
        base_contrast_tokens(),
        vec![page_with_bg(
            "page.one",
            "color.page",
            vec![
                ellipse_backdrop("backdrop", "color.backdrop"),
                group_at(
                    "badge",
                    0.0,
                    0.0,
                    vec![text_at("mono", "color.text", 600.0, 340.0, 80.0, 30.0)],
                ),
            ],
        )],
    );
    let report = contrast_report(&doc);
    assert!(
        has_code(&report, "contrast.invisible"),
        "zero-offset group text over the ellipse must still flag invisible; codes: {:?}",
        codes(&report)
    );
}

#[test]
fn nested_translated_groups_accumulate_offset() {
    // Outer (200,200) + inner (100,100) = (300,300) total offset onto the ellipse.
    let doc = doc_with(
        base_contrast_tokens(),
        vec![page_with_bg(
            "page.one",
            "color.page",
            vec![
                ellipse_backdrop("backdrop", "color.backdrop"),
                group_at(
                    "outer",
                    200.0,
                    200.0,
                    vec![group_at(
                        "inner",
                        100.0,
                        100.0,
                        vec![text_at("mono", "color.text", 300.0, 40.0, 80.0, 30.0)],
                    )],
                ),
            ],
        )],
    );
    let report = contrast_report(&doc);
    assert!(
        has_code(&report, "contrast.invisible"),
        "nested group offsets must accumulate onto the backdrop; codes: {:?}",
        codes(&report)
    );
}

#[test]
fn translated_text_within_frame_clip_over_backdrop_flags() {
    // A frame clip (400,100,400,300) holds a navy rect filling it and a group
    // at frame-local (-100,100); the text lands at absolute (450,250), inside
    // both the clip and the rect. The clip test compares the ABSOLUTE text box.
    let doc = doc_with(
        base_contrast_tokens(),
        vec![page_with_bg(
            "page.one",
            "color.page",
            vec![frame_clip(
                "frame",
                400.0,
                100.0,
                400.0,
                300.0,
                vec![
                    rect_backdrop_at("backdrop", "color.backdrop", 0.0, 0.0, 400.0, 300.0),
                    group_at(
                        "badge",
                        -100.0,
                        100.0,
                        vec![text_at("mono", "color.text", 150.0, 50.0, 80.0, 30.0)],
                    ),
                ],
            )],
        )],
    );
    let report = contrast_report(&doc);
    assert!(
        has_code(&report, "contrast.invisible"),
        "translated text inside the frame clip over the rect must flag invisible; codes: {:?}",
        codes(&report)
    );
}

// ── Item 2a: rounded rectangles ────────────────────────────────────────

#[test]
fn rounded_rect_corner_text_not_flagged() {
    // Text sits in the clipped-away top-left corner of a heavily rounded rect, so
    // its true backdrop is the white page, not the navy fill.
    let doc = doc_with(
        base_contrast_tokens(),
        vec![page_with_bg(
            "page.one",
            "color.page",
            vec![
                rect_backdrop_with(
                    "backdrop",
                    "color.backdrop",
                    100.0,
                    100.0,
                    240.0,
                    120.0,
                    |r| {
                        r.radius = Some(pxv(60.0));
                    },
                ),
                text_at("mono", "color.text", 102.0, 102.0, 10.0, 10.0),
            ],
        )],
    );
    let report = contrast_report(&doc);
    assert!(
        !has_code(&report, "contrast.invisible"),
        "text in the rounded-away corner must NOT be flagged; codes: {:?}",
        codes(&report)
    );
}

#[test]
fn rounded_rect_body_text_flagged() {
    let doc = doc_with(
        base_contrast_tokens(),
        vec![page_with_bg(
            "page.one",
            "color.page",
            vec![
                rect_backdrop_with(
                    "backdrop",
                    "color.backdrop",
                    100.0,
                    100.0,
                    240.0,
                    120.0,
                    |r| {
                        r.radius = Some(pxv(60.0));
                    },
                ),
                text_at("mono", "color.text", 180.0, 150.0, 60.0, 20.0),
            ],
        )],
    );
    let report = contrast_report(&doc);
    assert!(
        has_code(&report, "contrast.invisible"),
        "text in the rounded rect body must be flagged; codes: {:?}",
        codes(&report)
    );
}

// ── Item 2b: rotated leaf backdrops (exact) ────────────────────────────

#[test]
fn rotated_rect_covers_text_after_rotation_flags() {
    // A 200×200 navy square rotated 45° about (200,200); text near the rotated
    // top vertex (196,60) is covered only because of the rotation.
    let doc = doc_with(
        base_contrast_tokens(),
        vec![page_with_bg(
            "page.one",
            "color.page",
            vec![
                rect_backdrop_with(
                    "backdrop",
                    "color.backdrop",
                    100.0,
                    100.0,
                    200.0,
                    200.0,
                    |r| {
                        r.rotate = Some(Dimension {
                            value: 45.0,
                            unit: Unit::Deg,
                        });
                    },
                ),
                text_at("mono", "color.text", 196.0, 60.0, 10.0, 10.0),
            ],
        )],
    );
    let report = contrast_report(&doc);
    assert!(
        has_code(&report, "contrast.invisible"),
        "text over the rotated square must be flagged; codes: {:?}",
        codes(&report)
    );
}

#[test]
fn rotated_rect_misses_axis_aligned_corner() {
    // Text at the un-rotated square's bottom-left corner (105,285) is NOT covered
    // once the square is rotated 45°, so it must fall back to the white page.
    let doc = doc_with(
        base_contrast_tokens(),
        vec![page_with_bg(
            "page.one",
            "color.page",
            vec![
                rect_backdrop_with(
                    "backdrop",
                    "color.backdrop",
                    100.0,
                    100.0,
                    200.0,
                    200.0,
                    |r| {
                        r.rotate = Some(Dimension {
                            value: 45.0,
                            unit: Unit::Deg,
                        });
                    },
                ),
                text_at("mono", "color.text", 105.0, 285.0, 10.0, 10.0),
            ],
        )],
    );
    let report = contrast_report(&doc);
    assert!(
        !has_code(&report, "contrast.invisible"),
        "text off the rotated square must NOT be flagged; codes: {:?}",
        codes(&report)
    );
}

// ── Item 2c: rotation on an ancestor group → indeterminate ─────────────

#[test]
fn rotated_group_backdrop_is_indeterminate() {
    let doc = backdrop_over_text_doc(rotated_group(
        "spin",
        0.0,
        0.0,
        30.0,
        vec![rect_backdrop_at(
            "backdrop",
            "color.backdrop",
            0.0,
            0.0,
            300.0,
            200.0,
        )],
    ));
    let report = contrast_report(&doc);
    assert!(
        has_code(&report, "contrast.indeterminate_backdrop"),
        "a backdrop inside a rotated group must be indeterminate; codes: {:?}",
        codes(&report)
    );
    assert!(
        !has_code(&report, "contrast.invisible"),
        "an indeterminate rotated-group backdrop must not assert invisible; codes: {:?}",
        codes(&report)
    );
}

#[test]
fn unrotated_group_backdrop_still_samples() {
    // Control for the rotated-group case: without rotation the navy fill is
    // sampled normally and the black text flags invisible.
    let doc = backdrop_over_text_doc(group_at(
        "still",
        0.0,
        0.0,
        vec![rect_backdrop_at(
            "backdrop",
            "color.backdrop",
            0.0,
            0.0,
            300.0,
            200.0,
        )],
    ));
    let report = contrast_report(&doc);
    assert!(
        has_code(&report, "contrast.invisible"),
        "an unrotated group backdrop must still be sampled; codes: {:?}",
        codes(&report)
    );
    assert!(
        !has_code(&report, "contrast.indeterminate_backdrop"),
        "an unrotated group backdrop is not indeterminate; codes: {:?}",
        codes(&report)
    );
}

// ── Item 2d: path fills → exact solid coverage (doc 24 Unit 2) ─────────

#[test]
fn path_fill_backdrop_samples_solid_paint() {
    // Solid closed path fill covering the text: black-on-navy must resolve
    // real paint (invisible), not indeterminate bbox coverage.
    let doc = backdrop_over_text_doc(path_box_backdrop(
        "backdrop",
        "color.backdrop",
        100.0,
        100.0,
        240.0,
        120.0,
    ));
    let report = contrast_report(&doc);
    assert!(
        has_code(&report, "contrast.invisible"),
        "a solid path fill covering the text must sample real paint; codes: {:?}",
        codes(&report)
    );
    assert!(
        !has_code(&report, "contrast.indeterminate_backdrop"),
        "a solid path fill is not indeterminate; codes: {:?}",
        codes(&report)
    );
}

#[test]
fn monogram_on_path_fill_flags_invisible_not_indeterminate() {
    // Logo monogram plate: navy path under black monogram text.
    let doc = backdrop_over_text_doc(path_box_backdrop(
        "logo.mark",
        "color.backdrop",
        100.0,
        100.0,
        240.0,
        120.0,
    ));
    let report = contrast_report(&doc);
    assert!(
        has_code(&report, "contrast.invisible") || has_code(&report, "contrast.low"),
        "monogram on path fill must see the path paint; codes: {:?}",
        codes(&report)
    );
    assert!(
        !has_code(&report, "contrast.indeterminate_backdrop"),
        "monogram on solid path fill must not be indeterminate; codes: {:?}",
        codes(&report)
    );
}

#[test]
fn text_in_path_aabb_outside_fill_uses_page_bg() {
    // Right triangle covering the lower-left of the AABB; text sits in the
    // upper-right corner (inside the anchor hull, outside the filled region).
    // A bbox-only path candidate would falsely cover the text with navy.
    let triangle = path_closed_backdrop(
        "backdrop",
        "color.backdrop",
        None,
        vec![
            path_corner(100.0, 100.0),
            path_corner(100.0, 220.0),
            path_corner(340.0, 220.0),
        ],
    );
    let doc = doc_with(
        base_contrast_tokens(),
        vec![page_with_bg(
            "page.one",
            "color.page",
            vec![
                triangle,
                // Upper-right of the AABB (100..340, 100..220); outside the triangle.
                text_at("headline", "color.text", 260.0, 110.0, 60.0, 24.0),
            ],
        )],
    );
    let report = contrast_report(&doc);
    assert!(
        !has_code(&report, "contrast.invisible"),
        "text outside the path fill must not inherit path paint; codes: {:?}",
        codes(&report)
    );
    assert!(
        !has_code(&report, "contrast.indeterminate_backdrop"),
        "outside-fill sample is page bg, not indeterminate; codes: {:?}",
        codes(&report)
    );
}

#[test]
fn evenodd_hole_text_not_covered_by_path_fill() {
    // Outer navy plate with evenodd hole over the text sample box — samples
    // land in the hole and must see the white page, not the path fill.
    let doc = backdrop_over_text_doc(path_evenodd_hole_backdrop(
        "backdrop",
        "color.backdrop",
        (80.0, 80.0, 280.0, 160.0),
        (120.0, 120.0, 100.0, 50.0),
    ));
    let report = contrast_report(&doc);
    assert!(
        !has_code(&report, "contrast.invisible"),
        "text in an evenodd hole must not see the path fill; codes: {:?}",
        codes(&report)
    );
    assert!(
        !has_code(&report, "contrast.indeterminate_backdrop"),
        "evenodd hole sample is page bg, not indeterminate; codes: {:?}",
        codes(&report)
    );
}

// ── Item 2e: mask/filter/blur/blend on a candidate → indeterminate ─────

#[test]
fn masked_rect_backdrop_is_indeterminate() {
    let doc = backdrop_over_text_doc(rect_backdrop_with(
        "backdrop",
        "color.backdrop",
        100.0,
        100.0,
        220.0,
        100.0,
        |r| r.mask = Some(PropertyValue::TokenRef("mask.reveal".to_owned())),
    ));
    let report = contrast_report(&doc);
    assert!(
        has_code(&report, "contrast.indeterminate_backdrop"),
        "a masked fill must be indeterminate; codes: {:?}",
        codes(&report)
    );
    assert!(!has_code(&report, "contrast.invisible"));
}

#[test]
fn filtered_rect_backdrop_is_indeterminate() {
    let doc = backdrop_over_text_doc(rect_backdrop_with(
        "backdrop",
        "color.backdrop",
        100.0,
        100.0,
        220.0,
        100.0,
        |r| r.filter = Some(PropertyValue::TokenRef("filter.duo".to_owned())),
    ));
    let report = contrast_report(&doc);
    assert!(
        has_code(&report, "contrast.indeterminate_backdrop"),
        "a filtered fill must be indeterminate; codes: {:?}",
        codes(&report)
    );
    assert!(!has_code(&report, "contrast.invisible"));
}

#[test]
fn blurred_rect_backdrop_is_indeterminate() {
    let doc = backdrop_over_text_doc(rect_backdrop_with(
        "backdrop",
        "color.backdrop",
        100.0,
        100.0,
        220.0,
        100.0,
        |r| {
            r.blur = Some(Dimension {
                value: 8.0,
                unit: Unit::Px,
            })
        },
    ));
    let report = contrast_report(&doc);
    assert!(
        has_code(&report, "contrast.indeterminate_backdrop"),
        "a blurred fill must be indeterminate; codes: {:?}",
        codes(&report)
    );
    assert!(!has_code(&report, "contrast.invisible"));
}

#[test]
fn blended_rect_backdrop_is_indeterminate() {
    let doc = backdrop_over_text_doc(rect_backdrop_with(
        "backdrop",
        "color.backdrop",
        100.0,
        100.0,
        220.0,
        100.0,
        |r| r.blend_mode = Some("multiply".to_owned()),
    ));
    let report = contrast_report(&doc);
    assert!(
        has_code(&report, "contrast.indeterminate_backdrop"),
        "a non-normal blend fill must be indeterminate; codes: {:?}",
        codes(&report)
    );
    assert!(!has_code(&report, "contrast.invisible"));
}

#[test]
fn plain_rect_backdrop_flags_invisible_control() {
    // Control proving the effect tests above discriminate: the same navy rect
    // with NO effect is sampled and flags invisible.
    let doc = backdrop_over_text_doc(rect_backdrop_at(
        "backdrop",
        "color.backdrop",
        100.0,
        100.0,
        220.0,
        100.0,
    ));
    let report = contrast_report(&doc);
    assert!(
        has_code(&report, "contrast.invisible"),
        "a plain navy rect must flag invisible; codes: {:?}",
        codes(&report)
    );
    assert!(!has_code(&report, "contrast.indeterminate_backdrop"));
}

#[test]
fn zero_opacity_rect_backdrop_yields_no_backdrop() {
    // opacity=0 still yields NO backdrop (not indeterminate): the text is judged
    // against the white page and passes.
    let doc = backdrop_over_text_doc(rect_backdrop_with(
        "backdrop",
        "color.backdrop",
        100.0,
        100.0,
        220.0,
        100.0,
        |r| r.opacity = Some(0.0),
    ));
    let report = contrast_report(&doc);
    assert!(
        !has_code(&report, "contrast.indeterminate_backdrop"),
        "a fully transparent fill is no backdrop, not indeterminate; codes: {:?}",
        codes(&report)
    );
    assert!(!has_code(&report, "contrast.invisible"));
    assert!(!has_code(&report, "contrast.low"));
}

// ── Item 4: anchored text with no authored w/h ─────────────────────────

#[test]
fn anchored_boxless_text_is_indeterminate() {
    // Light-gray text that WOULD read as contrast.low if judged against the white
    // page; without a computable box we must flag indeterminate instead.
    let doc = doc_with(
        vec![
            color_token_hex("color.page", "#ffffff"),
            color_token_hex("color.text", "#aaaaaa"),
        ],
        vec![page_with_bg(
            "page.one",
            "color.page",
            vec![anchored_boxless_text("floating", "color.text", None)],
        )],
    );
    let report = contrast_report(&doc);
    assert!(
        has_code(&report, "contrast.indeterminate_backdrop"),
        "boxless anchored text must be indeterminate; codes: {:?}",
        codes(&report)
    );
    assert!(
        !has_code(&report, "contrast.low"),
        "boxless anchored text must NOT be silently judged against the page bg; codes: {:?}",
        codes(&report)
    );
}

#[test]
fn anchored_boxless_text_hint_suppresses_indeterminate() {
    let doc = doc_with(
        vec![
            color_token_hex("color.page", "#ffffff"),
            color_token_hex("color.text", "#aaaaaa"),
            color_token_hex("color.hint", "#ffffff"),
        ],
        vec![page_with_bg(
            "page.one",
            "color.page",
            vec![anchored_boxless_text(
                "floating",
                "color.text",
                Some("color.hint"),
            )],
        )],
    );
    let report = contrast_report(&doc);
    assert!(
        !has_code(&report, "contrast.indeterminate_backdrop"),
        "a contrast-bg hint wins over the unknown-extent advisory; codes: {:?}",
        codes(&report)
    );
}

// ── Frame fill as a backdrop ────────────────────────────────────────────

/// A frame at (100,100,300,200) with `fill` set, holding `children`.
fn filled_frame(fill_token: &str, children: Vec<Node>) -> Node {
    let Node::Frame(mut frame) = frame_clip("frame", 100.0, 100.0, 300.0, 200.0, children) else {
        unreachable!("frame_clip returns Node::Frame");
    };
    frame.fill = Some(PropertyValue::TokenRef(fill_token.to_owned()));
    Node::Frame(frame)
}

#[test]
fn frame_fill_is_a_backdrop_for_its_text() {
    let doc = doc_with(
        base_contrast_tokens(),
        vec![page_with_bg(
            "page.one",
            "color.page",
            vec![filled_frame(
                "color.backdrop",
                vec![text_at("mono", "color.text", 50.0, 50.0, 80.0, 30.0)],
            )],
        )],
    );
    let report = contrast_report(&doc);
    assert!(
        has_code(&report, "contrast.invisible"),
        "black text on the navy frame fill must flag invisible; codes: {:?}",
        codes(&report)
    );
}

#[test]
fn frame_child_paints_over_frame_fill() {
    // The white child rect paints after the navy frame fill, so the black text
    // sits on white and passes.
    let mut tokens = base_contrast_tokens();
    tokens.push(color_token_hex("color.card", "#ffffff"));
    let doc = doc_with(
        tokens,
        vec![page_with_bg(
            "page.one",
            "color.page",
            vec![filled_frame(
                "color.backdrop",
                vec![
                    rect_backdrop_at("card", "color.card", 20.0, 20.0, 200.0, 100.0),
                    text_at("mono", "color.text", 50.0, 50.0, 80.0, 30.0),
                ],
            )],
        )],
    );
    let report = contrast_report(&doc);
    assert!(
        !has_code(&report, "contrast.invisible") && !has_code(&report, "contrast.low"),
        "text on the white child card must pass; codes: {:?}",
        codes(&report)
    );
}

#[test]
fn unclipped_frame_does_not_clip_text_sampling() {
    // clip=#false: text outside the frame box still renders over the navy
    // page-level rect, so it must be sampled and flagged.
    let Node::Frame(mut frame) = frame_clip(
        "frame",
        100.0,
        100.0,
        100.0,
        100.0,
        vec![text_at("mono", "color.text", 350.0, 350.0, 80.0, 30.0)],
    ) else {
        unreachable!("frame_clip returns Node::Frame");
    };
    frame.clip = Some(false);
    let doc = doc_with(
        base_contrast_tokens(),
        vec![page_with_bg(
            "page.one",
            "color.page",
            vec![
                rect_backdrop_at("backdrop", "color.backdrop", 400.0, 400.0, 200.0, 200.0),
                Node::Frame(frame),
            ],
        )],
    );
    let report = contrast_report(&doc);
    assert!(
        has_code(&report, "contrast.invisible"),
        "text outside an unclipped frame must be sampled; codes: {:?}",
        codes(&report)
    );
}
