mod common;
use common::parse;
#[path = "common/fill_rects.rs"]
mod fill_rects;
use fill_rects::fill_rects;
use zenith_core::{DataContext, default_provider};
use zenith_scene::ir::SceneCommand;
use zenith_scene::{compile, compile_page};

/// Source with a `panel.master` component (bg rect + text label) instanced
/// three times at three x positions, each overriding the label text.
const COMPONENT_SRC: &str = r##"zenith version=1 {
  project id="proj.c" name="C"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#101010"
    token id="color.fg" type="color" value="#fafafa"
    token id="color.alt" type="color" value="#ff0000"
    token id="size.body" type="dimension" value=(pt)18
    token id="font.fam" type="fontFamily" value="Noto Sans"
  }
  styles {}
  components {
    component id="panel.master" {
      rect id="bg" x=(px)0 y=(px)0 w=(px)100 h=(px)60 fill=(token)"color.bg"
      text id="label" x=(px)5 y=(px)5 w=(px)90 h=(px)30 fill=(token)"color.fg" font-family=(token)"font.fam" font-size=(token)"size.body" {
        span "Default"
      }
    }
  }
  document id="doc.c" title="C" {
    page id="page.c" w=(px)640 h=(px)360 {
      instance id="inst.1" component="panel.master" x=(px)0 y=(px)0 {
        override ref="label" { span "Back" }
      }
      instance id="inst.2" component="panel.master" x=(px)200 y=(px)0 {
        override ref="label" fill=(token)"color.alt" { span "Center" }
      }
      instance id="inst.3" component="panel.master" x=(px)400 y=(px)0 {
        override ref="label" { span "Cover" }
      }
    }
  }
}
"##;

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

// ── `w`/`h`/`fit` on a LOCAL component instance ──────────────────────────────
//
// An icon component is made only of `path` nodes. Before these paths contributed
// to `group_children_bounds`, an icon instance had no resolvable extent, so
// `w`/`h`/`fit` parsed, validated clean, and silently did nothing.

/// A 0,0..24,24 icon component — the shape every converted SVG icon takes.
const ICON_SRC: &str = r##"zenith version=1 {
  project id="proj.icon-fit" name="Icon Fit"
  tokens format="zenith-token-v1" {
    token id="color.ink" type="color" value="#111827"
    token id="size.stroke" type="dimension" value=(px)2
  }
  styles {}
  components {
    component id="icon" {
      path id="icon.0" stroke=(token)"color.ink" stroke-width=(token)"size.stroke" {
        subpath closed=#false {
          anchor x=(px)0 y=(px)0
          anchor x=(px)24 y=(px)0
          anchor x=(px)24 y=(px)24
          anchor x=(px)0 y=(px)24
        }
      }
    }
  }
  document id="doc.icon-fit" title="Icon Fit" {
    page id="page.icon-fit" w=(px)400 h=(px)200 {
      BODY
    }
  }
}
"##;

fn compile_icon_body(body: &str) -> zenith_scene::CompileResult {
    let doc = parse(&ICON_SRC.replace("BODY", body));
    compile(&doc, &default_provider())
}

/// The single `PushScaleTranslate` emitted, if any.
fn scale_transform(result: &zenith_scene::CompileResult) -> Option<(f64, f64, f64, f64)> {
    result.scene.commands.iter().find_map(|c| match c {
        SceneCommand::PushScaleTranslate { sx, sy, tx, ty } => Some((*sx, *sy, *tx, *ty)),
        _ => None,
    })
}

/// The feature is absent ⇒ no transform, byte-identical to the translate-only path.
#[test]
fn local_instance_without_w_h_emits_no_transform() {
    let result = compile_icon_body(r#"instance id="i" component="icon" x=(px)10 y=(px)20"#);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(scale_transform(&result), None);
}

/// `contain` scales the 24×24 icon uniformly into the box and centers it.
#[test]
fn local_instance_w_h_scales_an_all_path_component() {
    let result = compile_icon_body(
        r#"instance id="i" component="icon" x=(px)10 y=(px)20 w=(px)96 h=(px)96 fit="contain""#,
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let (sx, sy, tx, ty) = scale_transform(&result).expect("a scale transform must be emitted");
    assert_eq!((sx, sy), (4.0, 4.0), "24px icon into a 96px box");
    // Square icon in a square box: no centering slack, so the transform is the origin.
    assert_eq!((tx, ty), (10.0, 20.0));
}

/// `fit` defaults to `contain` whenever a positive `w`/`h` box is present.
#[test]
fn local_instance_w_h_defaults_to_contain() {
    let with = compile_icon_body(
        r#"instance id="i" component="icon" x=(px)0 y=(px)0 w=(px)48 h=(px)48 fit="contain""#,
    );
    let without =
        compile_icon_body(r#"instance id="i" component="icon" x=(px)0 y=(px)0 w=(px)48 h=(px)48"#);
    assert_eq!(scale_transform(&with), scale_transform(&without));
}

/// `contain` preserves aspect and centers on the slack axis; `fill` stretches.
#[test]
fn local_instance_contain_centers_and_fill_stretches() {
    let contain = compile_icon_body(
        r#"instance id="i" component="icon" x=(px)0 y=(px)0 w=(px)96 h=(px)48 fit="contain""#,
    );
    let (sx, sy, tx, ty) = scale_transform(&contain).expect("transform");
    assert_eq!((sx, sy), (2.0, 2.0), "uniform scale = min(96/24, 48/24)");
    assert_eq!((tx, ty), (24.0, 0.0), "centered on the slack (x) axis");

    let fill = compile_icon_body(
        r#"instance id="i" component="icon" x=(px)0 y=(px)0 w=(px)96 h=(px)48 fit="fill""#,
    );
    let (sx, sy, _, _) = scale_transform(&fill).expect("transform");
    assert_eq!((sx, sy), (4.0, 2.0), "non-uniform stretch");
}

/// `none` keeps the natural size even inside a box.
#[test]
fn local_instance_fit_none_does_not_scale() {
    let result = compile_icon_body(
        r#"instance id="i" component="icon" x=(px)5 y=(px)5 w=(px)96 h=(px)96 fit="none""#,
    );
    let (sx, sy, _, _) = scale_transform(&result).expect("transform");
    assert_eq!((sx, sy), (1.0, 1.0));
}

/// An unknown `fit` is diagnosed against its own code, not the import code.
#[test]
fn local_instance_unknown_fit_emits_advisory_and_skips() {
    let result = compile_icon_body(
        r#"instance id="i" component="icon" x=(px)0 y=(px)0 w=(px)96 h=(px)96 fit="cover""#,
    );
    let codes: Vec<&str> = result.diagnostics.iter().map(|d| d.code.as_str()).collect();
    assert_eq!(codes, vec!["scene.unsupported_fit"]);
    assert!(
        !result
            .scene
            .commands
            .iter()
            .any(|c| matches!(c, SceneCommand::StrokePath { .. })),
        "the instance is skipped entirely"
    );
}
