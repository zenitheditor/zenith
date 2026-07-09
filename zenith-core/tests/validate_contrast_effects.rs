//! Integration tests: contrast validation — unmodeled / translated backdrops.
//!
//! Split out of `validate_contrast.rs` (bug-fix coverage for backdrop kinds the
//! sampler previously mishandled: translated/rotated groups, rounded/rotated
//! rects, path fills, mask/filter/blur/blend effects, boxless anchored text).
//! Test bodies moved verbatim; only the file location changed.

mod common;

use common::contrast::*;
use common::*;
use zenith_core::{Dimension, Unit};

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
    let report = validate(&doc);
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
    let report = validate(&doc);
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
    let report = validate(&doc);
    assert!(
        has_code(&report, "contrast.invisible"),
        "nested group offsets must accumulate onto the backdrop; codes: {:?}",
        codes(&report)
    );
}

#[test]
fn translated_text_within_frame_clip_over_backdrop_flags() {
    // A frame clip (400,100,400,300) holds a navy rect filling it and a group
    // translated by (300,200); the text lands at absolute (450,250), inside both
    // the clip and the rect. The clip test compares the ABSOLUTE text box.
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
                    rect_backdrop_at("backdrop", "color.backdrop", 400.0, 100.0, 400.0, 300.0),
                    group_at(
                        "badge",
                        300.0,
                        200.0,
                        vec![text_at("mono", "color.text", 150.0, 50.0, 80.0, 30.0)],
                    ),
                ],
            )],
        )],
    );
    let report = validate(&doc);
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
    let report = validate(&doc);
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
    let report = validate(&doc);
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
    let report = validate(&doc);
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
    let report = validate(&doc);
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
    let report = validate(&doc);
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
    let report = validate(&doc);
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

// ── Item 2d: path fills → indeterminate ────────────────────────────────

#[test]
fn path_fill_backdrop_is_indeterminate() {
    let doc = backdrop_over_text_doc(path_box_backdrop(
        "backdrop",
        "color.backdrop",
        100.0,
        100.0,
        240.0,
        120.0,
    ));
    let report = validate(&doc);
    assert!(
        has_code(&report, "contrast.indeterminate_backdrop"),
        "a filled path backdrop must be indeterminate rather than a clean pass; codes: {:?}",
        codes(&report)
    );
    assert!(
        !has_code(&report, "contrast.invisible"),
        "a path backdrop must not assert invisible; codes: {:?}",
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
    let report = validate(&doc);
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
    let report = validate(&doc);
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
    let report = validate(&doc);
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
    let report = validate(&doc);
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
    let report = validate(&doc);
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
    let report = validate(&doc);
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
    let report = validate(&doc);
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
    let report = validate(&doc);
    assert!(
        !has_code(&report, "contrast.indeterminate_backdrop"),
        "a contrast-bg hint wins over the unknown-extent advisory; codes: {:?}",
        codes(&report)
    );
}
