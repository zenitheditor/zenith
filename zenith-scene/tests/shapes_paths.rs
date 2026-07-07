mod common;
use common::*;
use zenith_core::default_provider;
use zenith_scene::compile;
use zenith_scene::ir::{LineJoin, Paint, PathSegment, SceneCommand, StrokeAlign};

#[test]
fn path_emits_cubic_fill_and_stroke_metadata() {
    let src = r##"zenith version=1 {
  project id="proj.path" name="Path"
  tokens format="zenith-token-v1" {
token id="color.fill" type="color" value="#00ff00"
token id="color.stroke" type="color" value="#0000ff"
  }
  styles {}
  document id="doc.path" title="Path" {
page id="page.path" w=(px)320 h=(px)220 {
  path id="path.curve" closed=#true fill=(token)"color.fill" stroke=(token)"color.stroke" stroke-width=(px)3 stroke-alignment="inside" stroke-linejoin="bevel" stroke-miter-limit=6 fill-rule="evenodd" {
    anchor x=(px)50 y=(px)150 out-x=(px)80 out-y=(px)30
    anchor x=(px)160 y=(px)50 in-x=(px)120 in-y=(px)20 out-x=(px)210 out-y=(px)80
    anchor x=(px)260 y=(px)150 in-x=(px)230 in-y=(px)30
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
        4,
        "expected clip, fill, stroke, clip; got {cmds:?}"
    );

    match &cmds[1] {
        SceneCommand::FillPath {
            segments,
            paint: Paint::Solid { color },
            even_odd,
        } => {
            assert_eq!(color.g, 255);
            assert!(*even_odd);
            assert!(
                segments
                    .iter()
                    .any(|s| matches!(s, PathSegment::CubicTo { .. })),
                "path fill must preserve cubic segments: {segments:?}"
            );
            assert!(
                matches!(segments.last(), Some(PathSegment::Close)),
                "closed path fill must end with Close"
            );
        }
        other => panic!("cmd[1] must be FillPath, got {other:?}"),
    }

    match &cmds[2] {
        SceneCommand::StrokePath {
            segments,
            color,
            stroke_width,
            closed,
            align,
            fill_even_odd,
            stroke_linejoin,
            stroke_miter_limit,
        } => {
            assert_eq!(color.b, 255);
            assert!((*stroke_width - 3.0).abs() < 1e-9);
            assert!(*closed);
            assert_eq!(*align, StrokeAlign::Inside);
            assert!(*fill_even_odd);
            assert_eq!(*stroke_linejoin, Some(LineJoin::Bevel));
            assert_eq!(*stroke_miter_limit, Some(6.0));
            assert!(
                segments
                    .iter()
                    .any(|s| matches!(s, PathSegment::CubicTo { .. })),
                "path stroke must preserve cubic segments: {segments:?}"
            );
        }
        other => panic!("cmd[2] must be StrokePath, got {other:?}"),
    }
}

#[test]
fn compound_path_emits_multiple_contours() {
    let src = r##"zenith version=1 {
  project id="proj.path" name="Path"
  tokens format="zenith-token-v1" {
token id="color.fill" type="color" value="#00ff00"
token id="color.stroke" type="color" value="#0000ff"
  }
  styles {}
  document id="doc.path" title="Path" {
page id="page.path" w=(px)320 h=(px)220 {
  path id="path.compound" fill=(token)"color.fill" stroke=(token)"color.stroke" stroke-width=(px)2 fill-rule="evenodd" {
    subpath closed=#true {
      anchor x=(px)10 y=(px)10
      anchor x=(px)110 y=(px)10
      anchor x=(px)110 y=(px)110
    }
    subpath closed=#true {
      anchor x=(px)40 y=(px)40
      anchor x=(px)80 y=(px)40
      anchor x=(px)80 y=(px)80
    }
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

    let fill_segments = result
        .scene
        .commands
        .iter()
        .find_map(|command| match command {
            SceneCommand::FillPath { segments, .. } => Some(segments),
            _ => None,
        })
        .expect("compound path should emit a fill path");

    assert_eq!(
        fill_segments
            .iter()
            .filter(|segment| matches!(segment, PathSegment::MoveTo { .. }))
            .count(),
        2
    );
    assert_eq!(
        fill_segments
            .iter()
            .filter(|segment| matches!(segment, PathSegment::Close))
            .count(),
        2
    );
}

#[test]
fn path_missing_anchor_coordinate_reports_scene_diagnostic() {
    let src = r##"zenith version=1 {
  project id="proj.path" name="Path"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.path" title="Path" {
page id="page.path" w=(px)320 h=(px)220 {
  path id="path.bad" stroke=(token)"color.stroke" {
    anchor x=(px)50
    anchor x=(px)160 y=(px)50
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "scene.missing_geometry"
                && diagnostic.message.contains("anchor[0]")),
        "expected missing anchor coordinate diagnostic, got {:?}",
        result.diagnostics
    );
    assert!(
        result
            .scene
            .commands
            .iter()
            .all(|command| !matches!(command, SceneCommand::StrokePath { .. })),
        "invalid path should not emit a stroke command: {:?}",
        result.scene.commands
    );
}

#[test]
fn path_unsupported_handle_unit_reports_scene_diagnostic() {
    let src = r##"zenith version=1 {
  project id="proj.path" name="Path"
  tokens format="zenith-token-v1" {
token id="color.stroke" type="color" value="#0000ff"
  }
  styles {}
  document id="doc.path" title="Path" {
page id="page.path" w=(px)320 h=(px)220 {
  path id="path.bad" stroke=(token)"color.stroke" {
    anchor x=(px)50 y=(px)150 out-x=(pct)80 out-y=(px)30
    anchor x=(px)160 y=(px)50
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "scene.unsupported_unit"
                && diagnostic.message.contains("out handle")),
        "expected unsupported handle unit diagnostic, got {:?}",
        result.diagnostics
    );
    assert!(
        result
            .scene
            .commands
            .iter()
            .all(|command| !matches!(command, SceneCommand::StrokePath { .. })),
        "invalid path should not emit a stroke command: {:?}",
        result.scene.commands
    );
}
