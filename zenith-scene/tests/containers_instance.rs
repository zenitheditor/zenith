mod common;
use common::*;
use zenith_core::{DataContext, default_provider};
use zenith_scene::ir::SceneCommand;
use zenith_scene::{compile, compile_page};

#[test]
fn instance_expands_component_translated_three_times() {
    let doc = parse(COMPONENT_SRC);
    let result = compile(&doc, &default_provider());
    assert!(
        !result
            .diagnostics
            .iter()
            .any(|d| d.code == "scene.unknown_component"),
        "no unknown-component advisory expected: {:?}",
        result.diagnostics
    );

    // The component's bg rect should appear 3× as a FillRect at x = 0, 200, 400.
    let rect_xs: Vec<f64> = result
        .scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::FillRect { x, w, h, .. } if *w == 100.0 && *h == 60.0 => Some(*x),
            _ => None,
        })
        .collect();
    assert_eq!(
        rect_xs,
        vec![0.0, 200.0, 400.0],
        "the master bg rect must appear 3× at the 3 instance origins"
    );

    // Three glyph runs (one label per instance).
    let glyph_runs = result
        .scene
        .commands
        .iter()
        .filter(|c| matches!(c, SceneCommand::DrawGlyphRun { .. }))
        .count();
    assert_eq!(glyph_runs, 3, "each expanded instance draws its label");
}

#[test]
fn instance_override_fill_recolors_target_label() {
    let doc = parse(COMPONENT_SRC);
    let result = compile(&doc, &default_provider());

    // inst.2 overrides the label fill to color.alt (#ff0000); the other two
    // labels keep color.fg (#fafafa). Collect glyph-run colors in z-order.
    let colors: Vec<(u8, u8, u8)> = result
        .scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::DrawGlyphRun { color, .. } => Some((color.r, color.g, color.b)),
            _ => None,
        })
        .collect();
    assert_eq!(colors.len(), 3);
    assert_eq!(
        colors[0],
        (0xfa, 0xfa, 0xfa),
        "inst.1 label keeps default fg"
    );
    assert_eq!(
        colors[1],
        (0xff, 0x00, 0x00),
        "inst.2 label overridden to color.alt (red)"
    );
    assert_eq!(
        colors[2],
        (0xfa, 0xfa, 0xfa),
        "inst.3 label keeps default fg"
    );
}

#[test]
fn instance_override_stroke_restyles_native_path() {
    let src = r##"zenith version=1 {
  project id="proj.path-override" name="Path Override"
  tokens format="zenith-token-v1" {
    token id="color.default" type="color" value="#111827"
    token id="color.override" type="color" value="#2563eb"
    token id="size.default" type="dimension" value=(px)2
    token id="size.override" type="dimension" value=(px)5
  }
  styles {}
  components {
    component id="icon.native" {
      path id="icon.0" stroke=(token)"color.default" stroke-width=(token)"size.default" {
        subpath closed=#false {
          anchor x=(px)0 y=(px)0
          anchor x=(px)24 y=(px)24
        }
      }
    }
  }
  document id="doc.path-override" title="Path Override" {
    page id="page.path-override" w=(px)80 h=(px)80 {
      instance id="icon" component="icon.native" x=(px)10 y=(px)10 {
        override ref="icon.0" stroke=(token)"color.override" stroke-width=(token)"size.override"
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
    let stroke = result
        .scene
        .commands
        .iter()
        .find_map(|cmd| match cmd {
            SceneCommand::StrokePath {
                color,
                stroke_width,
                ..
            } => Some((*color, *stroke_width)),
            _ => None,
        })
        .expect("path stroke command emitted");
    assert_eq!((stroke.0.r, stroke.0.g, stroke.0.b), (0x25, 0x63, 0xeb));
    assert_eq!(stroke.1, 5.0);
}

#[test]
fn instance_override_stroke_data_refs_resolve_on_native_paths() {
    let src = r##"zenith version=1 {
  project id="proj.path-override-data" name="Path Override Data"
  assets {
  }
  tokens format="zenith-token-v1" {
    token id="color.default" type="color" value="#111827"
    token id="size.default" type="dimension" value=(px)2
  }
  styles {}
  components {
    component id="icon.native" {
      path id="icon.0" stroke=(token)"color.default" stroke-width=(token)"size.default" {
        subpath closed=#false {
          anchor x=(px)0 y=(px)0
          anchor x=(px)24 y=(px)24
        }
      }
    }
  }
  document id="doc.path-override-data" title="Path Override Data" {
    page id="page.path-override-data" w=(px)80 h=(px)80 {
      instance id="icon" component="icon.native" x=(px)10 y=(px)10 {
        override ref="icon.0" stroke=(data)"path.stroke" stroke-width=(data)"path.stroke_width"
      }
    }
  }
}
"##;
    let doc = parse(src);
    let mut ctx = DataContext::default();
    ctx.fields
        .insert("path.stroke".to_owned(), "#2563eb".to_owned());
    ctx.fields
        .insert("path.stroke_width".to_owned(), "5".to_owned());

    let result = compile_page(&doc, &default_provider(), 0, Some(&ctx));
    assert!(
        !result
            .diagnostics
            .iter()
            .any(|d| d.code.starts_with("data.")),
        "unexpected data diagnostics: {:?}",
        result.diagnostics
    );

    let stroke = result
        .scene
        .commands
        .iter()
        .find_map(|cmd| match cmd {
            SceneCommand::StrokePath {
                color,
                stroke_width,
                ..
            } => Some((*color, *stroke_width)),
            _ => None,
        })
        .expect("path stroke command emitted");
    assert_eq!((stroke.0.r, stroke.0.g, stroke.0.b), (0x25, 0x63, 0xeb));
    assert_eq!(stroke.1, 5.0);
}

#[test]
fn unknown_component_emits_advisory_and_skips() {
    let src = r##"zenith version=1 {
  project id="proj.uc" name="UC"
  tokens format="zenith-token-v1" {}
  styles {}
  components {}
  document id="doc.uc" title="UC" {
page id="page.uc" w=(px)200 h=(px)200 {
  instance id="inst.bad" component="nonexistent.panel" x=(px)0 y=(px)0
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    let unknown: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.code == "scene.unknown_component")
        .collect();
    assert_eq!(
        unknown.len(),
        1,
        "expected 1 scene.unknown_component advisory; got: {:?}",
        result.diagnostics
    );

    // The instance emits NO commands (just the page PushClip/PopClip).
    let cmds = &result.scene.commands;
    assert_eq!(
        cmds.len(),
        2,
        "expected PushClip + PopClip only (instance skipped); got: {:?}",
        cmds
    );
}
// ── Unknown node: subtree skipped, no commands, advisory emitted ───────

#[test]
fn unknown_node_with_children_emits_no_commands() {
    // An unrecognized `sparkle` node carries a real rect child. Because the
    // unknown parent's layout semantics are unknown, the WHOLE subtree is
    // skipped at compile time: NO scene commands are emitted for it or its
    // children, and the existing `scene.unsupported_node` advisory fires.
    let src = r##"zenith version=1 {
  project id="proj.uk" name="UK"
  tokens format="zenith-token-v1" {
token id="color.r" type="color" value="#ff0000"
  }
  styles {}
  document id="doc.uk" title="UK" {
page id="page.uk" w=(px)320 h=(px)200 {
  sparkle id="fx" {
    rect id="inner" x=(px)10 y=(px)10 w=(px)50 h=(px)50 fill=(token)"color.r"
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    // The unknown subtree is skipped: no fill commands for the node or its
    // rect child.
    let fills = fill_rects(&result);
    assert!(
        fills.is_empty(),
        "unknown node subtree must emit no FillRect commands; got: {fills:?}"
    );
    assert!(
        !result.scene.commands.iter().any(|c| matches!(
            c,
            SceneCommand::FillRect { .. } | SceneCommand::FillEllipse { .. }
        )),
        "unknown node subtree must emit no fill commands; got: {:?}",
        result.scene.commands
    );

    // The existing unsupported-node advisory must still fire.
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "scene.unsupported_node"),
        "unknown node must emit the scene.unsupported_node advisory; got: {:?}",
        result.diagnostics
    );
}
