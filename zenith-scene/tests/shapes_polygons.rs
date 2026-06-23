mod common;
use common::*;
use zenith_core::default_provider;
use zenith_scene::compile;
use zenith_scene::ir::{Paint, SceneCommand, StrokeAlign};

// ── polygon: fill + stroke emits FillPolygon then StrokePolyline(closed) ─

#[test]
fn polygon_emits_fill_and_stroke() {
    let src = r##"zenith version=1 {
  project id="proj.p1" name="P1"
  tokens format="zenith-token-v1" {
token id="color.fill" type="color" value="#ff0000"
token id="color.stroke" type="color" value="#000000"
token id="size.stroke" type="dimension" value=(px)2
  }
  styles {}
  document id="doc.p1" title="P1" {
page id="page.p1" w=(px)320 h=(px)200 {
  polygon id="poly.tri" fill=(token)"color.fill" stroke=(token)"color.stroke" stroke-width=(token)"size.stroke" {
    point x=(px)160 y=(px)40
    point x=(px)260 y=(px)170
    point x=(px)60 y=(px)170
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    assert!(
        result.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );

    // PushClip, FillPolygon, StrokePolyline, PopClip
    let cmds = &result.scene.commands;
    assert_eq!(cmds.len(), 4, "expected 4 commands, got: {:?}", cmds);

    match &cmds[1] {
        SceneCommand::FillPolygon {
            points,
            paint: Paint::Solid { color },
            even_odd,
        } => {
            // 3 points × 2 = 6 coordinates
            assert_eq!(points.len(), 6, "must have 6 flat coords");
            assert_eq!(points[0], 160.0, "x0 must be 160");
            assert_eq!(points[1], 40.0, "y0 must be 40");
            assert_eq!(color.r, 255, "fill color must be red");
            assert!(!even_odd, "even_odd must be false by default");
        }
        other => panic!("cmd[1] must be FillPolygon, got {other:?}"),
    }

    match &cmds[2] {
        SceneCommand::StrokePolyline {
            points,
            closed,
            color,
            stroke_width,
            align,
            fill_even_odd,
        } => {
            assert_eq!(points.len(), 6);
            assert!(closed, "polygon stroke must be closed");
            assert_eq!(color.r, 0, "stroke color must be black");
            assert!((stroke_width - 2.0).abs() < 1e-9);
            assert_eq!(
                *align,
                StrokeAlign::Center,
                "absent stroke-alignment must default to Center"
            );
            assert!(!fill_even_odd, "default fill rule is nonzero");
        }
        other => panic!("cmd[2] must be StrokePolyline, got {other:?}"),
    }
}

// ── polygon: stroke-alignment="inside" → StrokePolyline.align == Inside ─

#[test]
fn polygon_stroke_alignment_inside() {
    let src = r##"zenith version=1 {
  project id="proj.sa1" name="SA1"
  tokens format="zenith-token-v1" {
token id="color.stroke" type="color" value="#000000"
  }
  styles {}
  document id="doc.sa1" title="SA1" {
page id="page.sa1" w=(px)200 h=(px)200 {
  polygon id="poly.in" stroke=(token)"color.stroke" stroke-width=(px)6 stroke-alignment="inside" {
    point x=(px)100 y=(px)20
    point x=(px)180 y=(px)180
    point x=(px)20 y=(px)180
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    let sp = result.scene.commands.iter().find_map(|c| match c {
        SceneCommand::StrokePolyline {
            align,
            fill_even_odd,
            closed,
            ..
        } => Some((*align, *fill_even_odd, *closed)),
        _ => None,
    });
    assert_eq!(
        sp,
        Some((StrokeAlign::Inside, false, true)),
        "stroke-alignment=inside must set align=Inside, closed=true, fill_even_odd matches nonzero default"
    );
}

// ── polygon: stroke-alignment="outside" + evenodd → align=Outside, fill_even_odd=true ─

#[test]
fn polygon_stroke_alignment_outside_evenodd() {
    let src = r##"zenith version=1 {
  project id="proj.sa2" name="SA2"
  tokens format="zenith-token-v1" {
token id="color.stroke" type="color" value="#000000"
  }
  styles {}
  document id="doc.sa2" title="SA2" {
page id="page.sa2" w=(px)200 h=(px)200 {
  polygon id="poly.out" stroke=(token)"color.stroke" stroke-width=(px)6 stroke-alignment="outside" fill-rule="evenodd" {
    point x=(px)100 y=(px)10
    point x=(px)40 y=(px)180
    point x=(px)190 y=(px)60
    point x=(px)10 y=(px)60
    point x=(px)160 y=(px)180
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    let sp = result.scene.commands.iter().find_map(|c| match c {
        SceneCommand::StrokePolyline {
            align,
            fill_even_odd,
            ..
        } => Some((*align, *fill_even_odd)),
        _ => None,
    });
    assert_eq!(
        sp,
        Some((StrokeAlign::Outside, true)),
        "stroke-alignment=outside + fill-rule=evenodd must set align=Outside, fill_even_odd=true"
    );
}

// ── polygon: fill-rule="evenodd" → FillPolygon.even_odd == true ───────

#[test]
fn polygon_evenodd_fill_rule() {
    let src = r##"zenith version=1 {
  project id="proj.p2" name="P2"
  tokens format="zenith-token-v1" {
token id="color.fill" type="color" value="#0000ff"
  }
  styles {}
  document id="doc.p2" title="P2" {
page id="page.p2" w=(px)200 h=(px)200 {
  polygon id="poly.star" fill=(token)"color.fill" fill-rule="evenodd" {
    point x=(px)100 y=(px)10
    point x=(px)40 y=(px)180
    point x=(px)190 y=(px)60
    point x=(px)10 y=(px)60
    point x=(px)160 y=(px)180
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    let fp = result.scene.commands.iter().find_map(|c| match c {
        SceneCommand::FillPolygon { even_odd, .. } => Some(*even_odd),
        _ => None,
    });
    assert_eq!(fp, Some(true), "fill-rule=evenodd must set even_odd=true");
}

// ── polyline: stroke-only → one StrokePolyline(closed:false), no FillPolygon ─

#[test]
fn polyline_emits_open_stroke() {
    let src = r##"zenith version=1 {
  project id="proj.pl1" name="PL1"
  tokens format="zenith-token-v1" {
token id="color.stroke" type="color" value="#334155"
token id="size.stroke" type="dimension" value=(px)3
  }
  styles {}
  document id="doc.pl1" title="PL1" {
page id="page.pl1" w=(px)320 h=(px)200 {
  polyline id="line.conn" stroke=(token)"color.stroke" stroke-width=(token)"size.stroke" {
    point x=(px)40 y=(px)100
    point x=(px)120 y=(px)60
    point x=(px)200 y=(px)140
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    assert!(
        result.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );

    // PushClip, StrokePolyline, PopClip — no FillPolygon
    let cmds = &result.scene.commands;
    assert_eq!(cmds.len(), 3, "expected 3 commands, got: {:?}", cmds);

    assert!(
        !cmds
            .iter()
            .any(|c| matches!(c, SceneCommand::FillPolygon { .. })),
        "stroke-only polyline must not emit FillPolygon"
    );

    match &cmds[1] {
        SceneCommand::StrokePolyline {
            points,
            closed,
            align,
            ..
        } => {
            assert_eq!(points.len(), 6, "3 points × 2 = 6 flat coords");
            assert!(!closed, "polyline stroke must NOT be closed");
            assert_eq!(
                *align,
                StrokeAlign::Center,
                "polyline (open path) must always be Center-aligned"
            );
        }
        other => panic!("cmd[1] must be StrokePolyline, got {other:?}"),
    }
}

// ── polygon: visible=false → not emitted ──────────────────────────────

#[test]
fn invisible_polygon_not_emitted() {
    let src = r##"zenith version=1 {
  project id="proj.p3" name="P3"
  tokens format="zenith-token-v1" {
token id="color.fill" type="color" value="#ff0000"
  }
  styles {}
  document id="doc.p3" title="P3" {
page id="page.p3" w=(px)100 h=(px)100 {
  polygon id="poly.hidden" fill=(token)"color.fill" visible=#false {
    point x=(px)10 y=(px)10
    point x=(px)90 y=(px)10
    point x=(px)50 y=(px)90
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    assert!(
        result.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );
    let cmds = &result.scene.commands;
    assert_eq!(
        cmds.len(),
        2,
        "expected PushClip + PopClip only; got: {:?}",
        cmds
    );
    assert!(matches!(cmds[0], SceneCommand::PushClip { .. }));
    assert!(matches!(cmds[1], SceneCommand::PopClip));
}

// ── polygon: group opacity 0.5 cascades into fill color.a ─────────────

#[test]
fn polygon_opacity_cascades() {
    let src = r##"zenith version=1 {
  project id="proj.p4" name="P4"
  tokens format="zenith-token-v1" {
token id="color.fill" type="color" value="#ffffff"
  }
  styles {}
  document id="doc.p4" title="P4" {
page id="page.p4" w=(px)200 h=(px)200 {
  group id="grp.p4" opacity=0.5 {
    polygon id="poly.p4" fill=(token)"color.fill" {
      point x=(px)10 y=(px)10
      point x=(px)100 y=(px)10
      point x=(px)55 y=(px)100
    }
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    let fill_a = result.scene.commands.iter().find_map(|c| match c {
        SceneCommand::FillPolygon {
            paint: Paint::Solid { color },
            ..
        } => Some(color.a),
        _ => None,
    });
    // #ffffff α=255, node opacity=1.0, ctx opacity=0.5 → 255*0.5 ≈ 128
    assert!(
        fill_a.map(|a| (a as i32 - 128).abs() <= 1).unwrap_or(false),
        "cascaded opacity 0.5 must halve fill alpha to ≈128; got {fill_a:?}"
    );
}

// ── Style cascade tests ───────────────────────────────────────────────

/// A rect with no local fill but a style that provides fill → FillRect emitted.
#[test]
fn rect_inherits_fill_from_style() {
    let src = r##"zenith version=1 {
  project id="proj.sc1" name="SC1"
  tokens format="zenith-token-v1" {
token id="color.panel" type="color" value="#3b82f6"
  }
  styles {
style id="style.panel" {
  fill (token)"color.panel"
}
  }
  document id="doc.sc1" title="SC1" {
page id="page.sc1" w=(px)320 h=(px)200 {
  rect id="rect.sc1" x=(px)0 y=(px)0 w=(px)100 h=(px)100 style="style.panel"
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    assert!(
        result.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );

    // PushClip, FillRect (from style fill), PopClip
    let cmds = &result.scene.commands;
    assert_eq!(cmds.len(), 3, "expected 3 commands; got: {:?}", cmds);

    match &cmds[1] {
        SceneCommand::FillRect {
            paint: Paint::Solid { color },
            ..
        } => {
            // #3b82f6 → r=0x3b=59, g=0x82=130, b=0xf6=246
            assert_eq!(color.r, 0x3b, "r must be 0x3b from style fill");
            assert_eq!(color.g, 0x82, "g must be 0x82 from style fill");
            assert_eq!(color.b, 0xf6, "b must be 0xf6 from style fill");
        }
        other => panic!("expected FillRect from style cascade, got {other:?}"),
    }
}

/// A rect with BOTH local fill AND a style fill → local fill wins.
#[test]
fn node_local_fill_overrides_style() {
    let src = r##"zenith version=1 {
  project id="proj.sc2" name="SC2"
  tokens format="zenith-token-v1" {
token id="color.style" type="color" value="#ff0000"
token id="color.local" type="color" value="#00ff00"
  }
  styles {
style id="style.red" {
  fill (token)"color.style"
}
  }
  document id="doc.sc2" title="SC2" {
page id="page.sc2" w=(px)320 h=(px)200 {
  rect id="rect.sc2" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"color.local" style="style.red"
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    assert!(
        result.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );

    let cmds = &result.scene.commands;
    assert_eq!(cmds.len(), 3, "expected 3 commands; got: {:?}", cmds);

    match &cmds[1] {
        SceneCommand::FillRect {
            paint: Paint::Solid { color },
            ..
        } => {
            // Must be local (green #00ff00), NOT the style (red #ff0000).
            assert_eq!(color.r, 0x00, "local fill r=0 must override style r=255");
            assert_eq!(color.g, 0xff, "local fill g=255 must override style g=0");
            assert_eq!(color.b, 0x00, "local fill b=0 must override style b=0");
        }
        other => panic!("expected FillRect with local color, got {other:?}"),
    }
}

/// A polygon with no local fill/stroke but a style providing both → both emitted.
#[test]
fn polygon_inherits_stroke_from_style() {
    let src = r##"zenith version=1 {
  project id="proj.sc4" name="SC4"
  tokens format="zenith-token-v1" {
token id="color.stroke" type="color" value="#ef4444"
token id="size.sw" type="dimension" value=(px)2
  }
  styles {
style id="style.outlined" {
  stroke (token)"color.stroke"
  stroke-width (token)"size.sw"
}
  }
  document id="doc.sc4" title="SC4" {
page id="page.sc4" w=(px)320 h=(px)200 {
  polygon id="poly.sc4" style="style.outlined" {
    point x=(px)50 y=(px)10
    point x=(px)90 y=(px)90
    point x=(px)10 y=(px)90
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    assert!(
        result.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );

    // PushClip, StrokePolyline (no fill), PopClip
    let cmds = &result.scene.commands;
    assert_eq!(cmds.len(), 3, "expected 3 commands; got: {:?}", cmds);

    match &cmds[1] {
        SceneCommand::StrokePolyline {
            color,
            stroke_width,
            closed,
            ..
        } => {
            // #ef4444 → r=0xef=239
            assert_eq!(color.r, 0xef, "stroke r must be 0xef from style");
            assert!(
                (*stroke_width - 2.0).abs() < 0.01,
                "stroke-width must be 2px from style"
            );
            assert!(closed, "polygon stroke must be closed");
        }
        other => panic!("expected StrokePolyline from style cascade, got {other:?}"),
    }
}
