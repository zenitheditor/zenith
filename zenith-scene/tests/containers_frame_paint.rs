//! Frame box paint: `fill` / `stroke` / `stroke-width` / `radius` paint under
//! the children, and a radius turns the frame clip into a rounded clip.

mod common;
use common::*;

/// A document whose page holds `frame_attrs` on a frame at (40,40,120,100)
/// with one child rect. `styles` is the body of the styles block.
fn frame_doc(frame_attrs: &str, styles: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.fp" name="FP"
  tokens format="zenith-token-v1" {{
token id="color.bg" type="color" value="#102030"
token id="color.line" type="color" value="#405060"
token id="color.child" type="color" value="#708090"
token id="color.top" type="color" value="#112233"
token id="color.bottom" type="color" value="#445566"
token id="grad.bg" type="gradient" angle=(deg)90 {{
  stop offset=0.0 color=(token)"color.top"
  stop offset=1.0 color=(token)"color.bottom"
}}
token id="space.r" type="dimension" value=(px)12
token id="space.zero" type="dimension" value=(px)0
token id="space.line" type="dimension" value=(px)4
token id="shadow.soft" type="shadow" {{
  layer dx=(px)2 dy=(px)3 blur=(px)4 color=(token)"color.line"
}}
  }}
  styles {{
{styles}
  }}
  document id="doc.fp" title="FP" {{
page id="page.fp" w=(px)320 h=(px)200 {{
  frame id="frame.p" x=(px)40 y=(px)40 w=(px)120 h=(px)100 {frame_attrs} {{
    rect id="rect.child" x=(px)10 y=(px)10 w=(px)60 h=(px)60 fill=(token)"color.child"
  }}
}}
  }}
}}
"##
    )
}

fn commands(src: &str) -> Vec<SceneCommand> {
    let result = compile(&parse(src), &default_provider());
    assert!(
        result.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );
    result.scene.commands
}

/// The frame's commands: everything between the page clip push and pop.
fn frame_commands(frame_attrs: &str) -> Vec<SceneCommand> {
    frame_commands_styled(frame_attrs, "")
}

fn frame_commands_styled(frame_attrs: &str, styles: &str) -> Vec<SceneCommand> {
    let cmds = commands(&frame_doc(frame_attrs, styles));
    assert!(matches!(cmds.first(), Some(SceneCommand::PushClip { .. })));
    assert!(matches!(cmds.last(), Some(SceneCommand::PopClip)));
    cmds[1..cmds.len() - 1].to_vec()
}

fn solid(r: u8, g: u8, b: u8, a: u8) -> Paint {
    Paint::solid(Color::srgb(r, g, b, a))
}

fn child_fill() -> SceneCommand {
    SceneCommand::FillRect {
        x: 50.0,
        y: 50.0,
        w: 60.0,
        h: 60.0,
        paint: solid(0x70, 0x80, 0x90, 255),
    }
}

#[test]
fn fill_stroke_radius_paint_under_children_with_rounded_clip() {
    let cmds = frame_commands(
        r#"fill=(token)"color.bg" stroke=(token)"color.line" stroke-width=(token)"space.line" radius=(token)"space.r""#,
    );
    assert_eq!(
        cmds,
        vec![
            SceneCommand::FillRoundedRect {
                x: 40.0,
                y: 40.0,
                w: 120.0,
                h: 100.0,
                radius: 12.0,
                radii: None,
                paint: solid(0x10, 0x20, 0x30, 255),
            },
            SceneCommand::StrokeRoundedRect {
                x: 40.0,
                y: 40.0,
                w: 120.0,
                h: 100.0,
                radius: 12.0,
                radii: None,
                color: Color::srgb(0x40, 0x50, 0x60, 255),
                stroke_width: 4.0,
                stroke_dash: None,
                stroke_gap: None,
                stroke_linecap: None,
            },
            SceneCommand::PushClipRoundedRect {
                x: 40.0,
                y: 40.0,
                w: 120.0,
                h: 100.0,
                radius: 12.0,
            },
            child_fill(),
            SceneCommand::PopClip,
        ]
    );
}

#[test]
fn square_fill_and_stroke_use_rect_commands_and_rect_clip() {
    let cmds = frame_commands(r#"fill=(token)"color.bg" stroke=(token)"color.line""#);
    assert_eq!(
        cmds,
        vec![
            SceneCommand::FillRect {
                x: 40.0,
                y: 40.0,
                w: 120.0,
                h: 100.0,
                paint: solid(0x10, 0x20, 0x30, 255),
            },
            SceneCommand::StrokeRect {
                x: 40.0,
                y: 40.0,
                w: 120.0,
                h: 100.0,
                color: Color::srgb(0x40, 0x50, 0x60, 255),
                stroke_width: 1.0,
                stroke_dash: None,
                stroke_gap: None,
                stroke_linecap: None,
            },
            SceneCommand::PushClip {
                x: 40.0,
                y: 40.0,
                w: 120.0,
                h: 100.0,
            },
            child_fill(),
            SceneCommand::PopClip,
        ]
    );
}

#[test]
fn zero_radius_matches_a_frame_without_paint_attributes() {
    let plain = frame_commands("");
    assert_eq!(
        plain,
        vec![
            SceneCommand::PushClip {
                x: 40.0,
                y: 40.0,
                w: 120.0,
                h: 100.0,
            },
            child_fill(),
            SceneCommand::PopClip,
        ]
    );
    assert_eq!(frame_commands(r#"radius=(token)"space.zero""#), plain);
}

#[test]
fn rounded_frame_without_clip_paints_but_does_not_clip() {
    let cmds = frame_commands(r#"fill=(token)"color.bg" radius=(token)"space.r" clip=#false"#);
    assert_eq!(cmds.len(), 2, "{cmds:?}");
    assert!(matches!(cmds[0], SceneCommand::FillRoundedRect { radius, .. } if radius == 12.0));
    assert_eq!(cmds[1], child_fill());
}

#[test]
fn gradient_fill_paints_the_frame_box() {
    let cmds = frame_commands(r#"fill=(token)"grad.bg""#);
    assert!(
        matches!(
            &cmds[0],
            SceneCommand::FillRect { x, y, w, h, paint: Paint::Gradient(_) }
                if *x == 40.0 && *y == 40.0 && *w == 120.0 && *h == 100.0
        ),
        "{:?}",
        cmds[0]
    );
    assert!(matches!(cmds[1], SceneCommand::PushClip { .. }));
}

#[test]
fn frame_opacity_scales_the_frame_paint_like_the_children() {
    let cmds = frame_commands(r#"fill=(token)"color.bg" opacity=0.5"#);
    let SceneCommand::FillRect { paint, .. } = &cmds[0] else {
        panic!("expected frame FillRect, got {:?}", cmds[0]);
    };
    assert_eq!(*paint, solid(0x10, 0x20, 0x30, 128));
    let SceneCommand::FillRect { paint, .. } = &cmds[2] else {
        panic!("expected child FillRect, got {:?}", cmds[2]);
    };
    assert_eq!(*paint, solid(0x70, 0x80, 0x90, 128));
}

#[test]
fn shadow_wraps_frame_paint_and_children() {
    let cmds = frame_commands(r#"fill=(token)"color.bg" shadow=(token)"shadow.soft""#);
    assert!(
        matches!(cmds[0], SceneCommand::BeginShadow { .. }),
        "{cmds:?}"
    );
    assert!(matches!(cmds[1], SceneCommand::FillRect { .. }), "{cmds:?}");
    assert!(matches!(cmds[2], SceneCommand::PushClip { .. }), "{cmds:?}");
    assert_eq!(cmds[3], child_fill());
    assert!(matches!(cmds[4], SceneCommand::PopClip));
    assert!(matches!(cmds[5], SceneCommand::EndShadow));
    assert_eq!(cmds.len(), 6);
}

#[test]
fn rotated_rounded_frame_clips_under_the_rotation() {
    let cmds = frame_commands(r#"fill=(token)"color.bg" radius=(token)"space.r" rotate=(deg)30"#);
    assert!(
        matches!(cmds[0], SceneCommand::PushTransform { angle_deg, cx, cy }
            if angle_deg == 30.0 && cx == 100.0 && cy == 90.0),
        "{cmds:?}"
    );
    assert!(matches!(cmds[1], SceneCommand::FillRoundedRect { .. }));
    assert!(matches!(
        cmds[2],
        SceneCommand::PushClipRoundedRect { radius, .. } if radius == 12.0
    ));
    assert!(matches!(cmds[4], SceneCommand::PopClip));
    assert!(matches!(cmds[5], SceneCommand::PopTransform));
}

#[test]
fn frame_style_supplies_fill_and_radius() {
    let cmds = frame_commands_styled(
        r#"style="style.card""#,
        r#"    style id="style.card" {
      fill (token)"color.bg"
      radius (token)"space.r"
    }"#,
    );
    assert!(matches!(cmds[0], SceneCommand::FillRoundedRect { radius, .. } if radius == 12.0));
    assert!(matches!(cmds[1], SceneCommand::PushClipRoundedRect { .. }));
}

#[test]
fn frame_in_translated_group_clips_and_paints_at_the_translated_box() {
    let src = r##"zenith version=1 {
  project id="proj.fg" name="FG"
  tokens format="zenith-token-v1" {
token id="color.bg" type="color" value="#102030"
token id="color.child" type="color" value="#708090"
  }
  styles {}
  document id="doc.fg" title="FG" {
page id="page.fg" w=(px)320 h=(px)200 {
  group id="g" x=(px)100 y=(px)20 {
    frame id="frame.g" x=(px)10 y=(px)10 w=(px)50 h=(px)40 fill=(token)"color.bg" {
      rect id="rect.g" x=(px)0 y=(px)0 w=(px)20 h=(px)20 fill=(token)"color.child"
    }
  }
}
  }
}
"##;
    let cmds = commands(src);
    assert!(
        matches!(cmds[1], SceneCommand::FillRect { x, y, w, h, .. }
            if x == 110.0 && y == 30.0 && w == 50.0 && h == 40.0),
        "{cmds:?}"
    );
    assert!(
        matches!(cmds[2], SceneCommand::PushClip { x, y, w, h }
            if x == 110.0 && y == 30.0 && w == 50.0 && h == 40.0),
        "{cmds:?}"
    );
    assert!(
        matches!(cmds[3], SceneCommand::FillRect { x, y, .. } if x == 110.0 && y == 30.0),
        "{cmds:?}"
    );
}

#[test]
fn blend_layer_applies_inherited_opacity_once() {
    // The group fades to 0.5. Inside the multiply layer the frame content draws
    // at full alpha; the layer carries the whole 0.5 cascade.
    let src = r##"zenith version=1 {
  project id="proj.fb" name="FB"
  tokens format="zenith-token-v1" {
token id="color.bg" type="color" value="#102030"
token id="color.child" type="color" value="#708090"
  }
  styles {}
  document id="doc.fb" title="FB" {
page id="page.fb" w=(px)320 h=(px)200 {
  group id="g" opacity=0.5 {
    frame id="frame.b" x=(px)10 y=(px)10 w=(px)50 h=(px)40 fill=(token)"color.bg" blend-mode="multiply" {
      rect id="rect.b" x=(px)0 y=(px)0 w=(px)20 h=(px)20 fill=(token)"color.child"
    }
  }
}
  }
}
"##;
    let cmds = commands(src);
    assert!(
        matches!(cmds[1], SceneCommand::PushLayer { opacity, .. } if opacity == 0.5),
        "{cmds:?}"
    );
    assert!(
        matches!(&cmds[2], SceneCommand::FillRect { paint, .. } if *paint == solid(0x10, 0x20, 0x30, 255)),
        "{cmds:?}"
    );
    assert!(
        matches!(&cmds[4], SceneCommand::FillRect { paint, .. } if *paint == solid(0x70, 0x80, 0x90, 255)),
        "{cmds:?}"
    );
}
