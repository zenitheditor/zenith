mod common;
use common::parse;
use zenith_core::default_provider;
use zenith_scene::compile;
use zenith_scene::ir::SceneCommand;

// ── shape node: kind → background primitive mapping (U1, background-only) ──
//
// U1 of the `shape` node emits ONLY the background primitive (no text/glyph
// yet). These tests assert the kind → primitive mapping in `compile_shape`
// and that NO DrawGlyphRun is emitted. They reuse the same harness as the rect
// tests above: `parse(src)` (common) → `compile(&doc, &default_provider())` →
// inspect `result.scene.commands`.

/// `kind="process"` WITH a radius token → rounded-rect fill + stroke.
#[test]
fn shape_process_with_radius_emits_rounded_rect() {
    let src = r##"zenith version=1 {
  project id="proj.shp" name="SHP"
  tokens format="zenith-token-v1" {
token id="color.fill" type="color" value="#dbeafe"
token id="color.line" type="color" value="#1e3a8a"
token id="size.stroke" type="dimension" value=(px)2
token id="size.radius" type="dimension" value=(px)8
  }
  styles {}
  document id="doc.shp" title="SHP" {
page id="page.shp" w=(px)640 h=(px)360 {
  shape id="s1" x=(px)40 y=(px)40 w=(px)200 h=(px)120 kind="process" fill=(token)"color.fill" stroke=(token)"color.line" stroke-width=(token)"size.stroke" radius=(token)"size.radius"
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    let cmds = &result.scene.commands;

    assert!(
        cmds.iter()
            .any(|c| matches!(c, SceneCommand::FillRoundedRect { .. })),
        "process shape with radius must emit FillRoundedRect; got: {cmds:?}"
    );
    assert!(
        cmds.iter()
            .any(|c| matches!(c, SceneCommand::StrokeRoundedRect { .. })),
        "process shape with radius must emit StrokeRoundedRect; got: {cmds:?}"
    );
    assert!(
        !cmds
            .iter()
            .any(|c| matches!(c, SceneCommand::DrawGlyphRun { .. })),
        "U1 shape is background-only — no DrawGlyphRun; got: {cmds:?}"
    );
}

/// `kind="process"` WITHOUT a radius → plain rect fill + stroke.
#[test]
fn shape_process_without_radius_emits_plain_rect() {
    let src = r##"zenith version=1 {
  project id="proj.shp" name="SHP"
  tokens format="zenith-token-v1" {
token id="color.fill" type="color" value="#dbeafe"
token id="color.line" type="color" value="#1e3a8a"
token id="size.stroke" type="dimension" value=(px)2
  }
  styles {}
  document id="doc.shp" title="SHP" {
page id="page.shp" w=(px)640 h=(px)360 {
  shape id="s1" x=(px)40 y=(px)40 w=(px)200 h=(px)120 kind="process" fill=(token)"color.fill" stroke=(token)"color.line" stroke-width=(token)"size.stroke"
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    let cmds = &result.scene.commands;

    assert!(
        cmds.iter()
            .any(|c| matches!(c, SceneCommand::FillRect { .. })),
        "process shape without radius must emit FillRect; got: {cmds:?}"
    );
    assert!(
        cmds.iter()
            .any(|c| matches!(c, SceneCommand::StrokeRect { .. })),
        "process shape without radius must emit StrokeRect; got: {cmds:?}"
    );
    assert!(
        !cmds
            .iter()
            .any(|c| matches!(c, SceneCommand::FillRoundedRect { .. })),
        "process shape without radius must NOT emit a rounded rect; got: {cmds:?}"
    );
    assert!(
        !cmds
            .iter()
            .any(|c| matches!(c, SceneCommand::DrawGlyphRun { .. })),
        "U1 shape is background-only — no DrawGlyphRun; got: {cmds:?}"
    );
}

/// `kind="ellipse"` → ellipse fill + stroke.
#[test]
fn shape_ellipse_emits_ellipse() {
    let src = r##"zenith version=1 {
  project id="proj.shp" name="SHP"
  tokens format="zenith-token-v1" {
token id="color.fill" type="color" value="#dbeafe"
token id="color.line" type="color" value="#1e3a8a"
token id="size.stroke" type="dimension" value=(px)2
  }
  styles {}
  document id="doc.shp" title="SHP" {
page id="page.shp" w=(px)640 h=(px)360 {
  shape id="s1" x=(px)40 y=(px)40 w=(px)200 h=(px)120 kind="ellipse" fill=(token)"color.fill" stroke=(token)"color.line" stroke-width=(token)"size.stroke"
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    let cmds = &result.scene.commands;

    assert!(
        cmds.iter()
            .any(|c| matches!(c, SceneCommand::FillEllipse { .. })),
        "ellipse shape must emit FillEllipse; got: {cmds:?}"
    );
    assert!(
        cmds.iter()
            .any(|c| matches!(c, SceneCommand::StrokeEllipse { .. })),
        "ellipse shape must emit StrokeEllipse; got: {cmds:?}"
    );
    assert!(
        !cmds
            .iter()
            .any(|c| matches!(c, SceneCommand::DrawGlyphRun { .. })),
        "U1 shape is background-only — no DrawGlyphRun; got: {cmds:?}"
    );
}

/// `kind="decision"` → diamond polygon fill + closed polyline stroke. The
/// polygon has 4 vertices at the bbox edge midpoints (top, right, bottom, left).
#[test]
fn shape_decision_emits_diamond_polygon() {
    let src = r##"zenith version=1 {
  project id="proj.shp" name="SHP"
  tokens format="zenith-token-v1" {
token id="color.fill" type="color" value="#dbeafe"
token id="color.line" type="color" value="#1e3a8a"
token id="size.stroke" type="dimension" value=(px)2
  }
  styles {}
  document id="doc.shp" title="SHP" {
page id="page.shp" w=(px)640 h=(px)360 {
  shape id="s1" x=(px)40 y=(px)40 w=(px)200 h=(px)120 kind="decision" fill=(token)"color.fill" stroke=(token)"color.line" stroke-width=(token)"size.stroke"
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    let cmds = &result.scene.commands;

    // Expected diamond vertices for bbox x=40 y=40 w=200 h=120:
    // top-mid (140, 40), right-mid (240, 100), bottom-mid (140, 160), left-mid (40, 100).
    let expected = vec![140.0, 40.0, 240.0, 100.0, 140.0, 160.0, 40.0, 100.0];

    let fill = cmds
        .iter()
        .find_map(|c| match c {
            SceneCommand::FillPolygon { points, .. } => Some(points),
            _ => None,
        })
        .unwrap_or_else(|| panic!("decision shape must emit FillPolygon; got: {cmds:?}"));
    assert_eq!(
        fill.len(),
        8,
        "diamond polygon must have 4 points (8 flat coords); got: {fill:?}"
    );
    assert_eq!(*fill, expected, "fill polygon must be the bbox diamond");

    let stroke_closed = cmds.iter().any(|c| {
        matches!(
            c,
            SceneCommand::StrokePolyline { points, closed: true, .. } if *points == expected
        )
    });
    assert!(
        stroke_closed,
        "decision shape must emit a closed StrokePolyline diamond; got: {cmds:?}"
    );
    assert!(
        !cmds
            .iter()
            .any(|c| matches!(c, SceneCommand::DrawGlyphRun { .. })),
        "U1 shape is background-only — no DrawGlyphRun; got: {cmds:?}"
    );
}

// ── shape node: owned centered text label (U2) ────────────────────────────
//
// U2 renders the shape's owned label spans as a synthesized text node laid into
// the shape's padded content box, REUSING the production text path. A labeled
// shape now emits BOTH a background AND a DrawGlyphRun; an unlabeled shape still
// emits background only (the U1 regression guards above stay valid because those
// shapes carry no spans).

/// A `process` shape WITH a `span` label emits its background AND a glyph run.
/// The glyph run sits inside the padded content box (its origin x ≥ content_x)
/// and its baseline is pushed down toward the vertical middle (baseline y >
/// content_y) for the default `middle` v-align. The background paints BEFORE
/// the label.
#[test]
fn shape_with_label_emits_background_and_glyph_run() {
    let src = r##"zenith version=1 {
  project id="proj.shp" name="SHP"
  tokens format="zenith-token-v1" {
token id="color.fill" type="color" value="#dbeafe"
token id="color.line" type="color" value="#1e3a8a"
token id="size.stroke" type="dimension" value=(px)2
token id="size.pad" type="dimension" value=(px)8
  }
  styles {}
  document id="doc.shp" title="SHP" {
page id="page.shp" w=(px)640 h=(px)360 {
  shape id="s1" x=(px)40 y=(px)40 w=(px)200 h=(px)120 kind="process" fill=(token)"color.fill" stroke=(token)"color.line" stroke-width=(token)"size.stroke" padding=(token)"size.pad" {
    span "Hi"
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    let cmds = &result.scene.commands;

    // Background still present.
    assert!(
        cmds.iter()
            .any(|c| matches!(c, SceneCommand::FillRect { .. })),
        "labeled shape must still emit its background fill; got: {cmds:?}"
    );

    // Label present.
    let glyph = cmds.iter().find_map(|c| match c {
        SceneCommand::DrawGlyphRun { x, y, .. } => Some((*x, *y)),
        _ => None,
    });
    let (gx, gy) =
        glyph.unwrap_or_else(|| panic!("labeled shape must emit a DrawGlyphRun; got: {cmds:?}"));

    // Content box: bbox (40,40,200,120) inset by padding 8 → x=48, y=48.
    let content_x = 48.0;
    let content_y = 48.0;
    assert!(
        gx >= content_x,
        "glyph-run origin x ({gx}) must be inside the padded content box (≥ {content_x})"
    );
    assert!(
        gy > content_y,
        "default middle v-align must push the baseline below content_y ({content_y}); got {gy}"
    );

    // Z-order: the background fill must come before the glyph run.
    let fill_idx = cmds
        .iter()
        .position(|c| matches!(c, SceneCommand::FillRect { .. }));
    let glyph_idx = cmds
        .iter()
        .position(|c| matches!(c, SceneCommand::DrawGlyphRun { .. }));
    assert!(
        fill_idx < glyph_idx,
        "background must paint BEFORE the label; fill at {fill_idx:?}, glyph at {glyph_idx:?}"
    );
}

/// Regression: a shape's owned label inside a translating container (group /
/// instance) must NOT be double-translated by the container offset. The same
/// shape placed at an absolute position must produce the SAME glyph origin as
/// the shape placed at the equivalent group-local position inside a translated
/// group. (Before the fix, the grouped label was offset by the container amount
/// because `compile_text` re-applied `ctx.dx/dy` to already-absolute coords.)
#[test]
fn shape_label_in_group_is_not_double_translated() {
    let glyph_pos = |src: &str| -> (f64, f64) {
        let doc = parse(src);
        let result = compile(&doc, &default_provider());
        result
            .scene
            .commands
            .iter()
            .find_map(|c| match c {
                SceneCommand::DrawGlyphRun { x, y, .. } => Some((*x, *y)),
                _ => None,
            })
            .unwrap_or_else(|| panic!("expected a DrawGlyphRun"))
    };

    // Shape at absolute (140, 90).
    let flat = r##"zenith version=1 {
  project id="proj.shp" name="SHP"
  tokens format="zenith-token-v1" {
token id="color.fill" type="color" value="#dbeafe"
token id="color.line" type="color" value="#1e3a8a"
token id="size.stroke" type="dimension" value=(px)2
token id="size.pad" type="dimension" value=(px)8
  }
  styles {}
  document id="doc.shp" title="SHP" {
page id="page.shp" w=(px)640 h=(px)360 {
  shape id="s1" x=(px)140 y=(px)90 w=(px)200 h=(px)120 kind="process" fill=(token)"color.fill" stroke=(token)"color.line" stroke-width=(token)"size.stroke" padding=(token)"size.pad" {
    span "Hi"
  }
}
  }
}
"##;

    // Same shape at group-local (40, 40) inside a group translated by (100, 50)
    // → identical absolute position (140, 90).
    let grouped = r##"zenith version=1 {
  project id="proj.shp" name="SHP"
  tokens format="zenith-token-v1" {
token id="color.fill" type="color" value="#dbeafe"
token id="color.line" type="color" value="#1e3a8a"
token id="size.stroke" type="dimension" value=(px)2
token id="size.pad" type="dimension" value=(px)8
  }
  styles {}
  document id="doc.shp" title="SHP" {
page id="page.shp" w=(px)640 h=(px)360 {
  group id="g1" x=(px)100 y=(px)50 {
    shape id="s1" x=(px)40 y=(px)40 w=(px)200 h=(px)120 kind="process" fill=(token)"color.fill" stroke=(token)"color.line" stroke-width=(token)"size.stroke" padding=(token)"size.pad" {
      span "Hi"
    }
  }
}
  }
}
"##;

    let (fx, fy) = glyph_pos(flat);
    let (gx, gy) = glyph_pos(grouped);
    assert!(
        (fx - gx).abs() < 0.01 && (fy - gy).abs() < 0.01,
        "grouped label must land at the same absolute position as the flat one \
         (flat=({fx},{fy}), grouped=({gx},{gy})); a mismatch of the group offset \
         (100,50) indicates double-translation"
    );
}

/// A shape with EMPTY spans (no `span` child) still emits its background and
/// NO glyph run — the U1 background-only behavior is preserved for unlabeled
/// shapes.
#[test]
fn shape_without_label_emits_no_glyph_run() {
    let src = r##"zenith version=1 {
  project id="proj.shp" name="SHP"
  tokens format="zenith-token-v1" {
token id="color.fill" type="color" value="#dbeafe"
token id="color.line" type="color" value="#1e3a8a"
token id="size.stroke" type="dimension" value=(px)2
  }
  styles {}
  document id="doc.shp" title="SHP" {
page id="page.shp" w=(px)640 h=(px)360 {
  shape id="s1" x=(px)40 y=(px)40 w=(px)200 h=(px)120 kind="process" fill=(token)"color.fill" stroke=(token)"color.line" stroke-width=(token)"size.stroke"
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    let cmds = &result.scene.commands;

    assert!(
        cmds.iter()
            .any(|c| matches!(c, SceneCommand::FillRect { .. })),
        "unlabeled shape must still emit its background; got: {cmds:?}"
    );
    assert!(
        !cmds
            .iter()
            .any(|c| matches!(c, SceneCommand::DrawGlyphRun { .. })),
        "unlabeled shape must NOT emit a DrawGlyphRun; got: {cmds:?}"
    );
}

/// `v-align="top"` places the label's baseline HIGHER than the default
/// `middle`. Same shape + label, different v-align → different baseline y.
#[test]
fn shape_label_top_valign_is_higher_than_middle() {
    let mk = |valign: &str| -> f64 {
        let src = format!(
            r##"zenith version=1 {{
  project id="proj.shp" name="SHP"
  tokens format="zenith-token-v1" {{
token id="color.fill" type="color" value="#dbeafe"
  }}
  styles {{}}
  document id="doc.shp" title="SHP" {{
page id="page.shp" w=(px)640 h=(px)360 {{
  shape id="s1" x=(px)40 y=(px)40 w=(px)200 h=(px)120 kind="process" fill=(token)"color.fill" v-align="{valign}" {{
    span "Hi"
  }}
}}
  }}
}}
"##
        );
        let doc = parse(&src);
        let result = compile(&doc, &default_provider());
        result
            .scene
            .commands
            .iter()
            .find_map(|c| match c {
                SceneCommand::DrawGlyphRun { y, .. } => Some(*y),
                _ => None,
            })
            .unwrap_or_else(|| panic!("expected a DrawGlyphRun for v-align={valign}"))
    };

    let top_y = mk("top");
    let middle_y = mk("middle");
    assert!(
        top_y < middle_y,
        "top-aligned baseline ({top_y}) must be higher (smaller y) than middle ({middle_y})"
    );
}
