mod common;
use common::*;
use zenith_core::default_provider;
use zenith_scene::compile;
use zenith_scene::ir::SceneCommand;

// ── Blend-mode layer bracket ──────────────────────────────────────────

/// A rect with `blend-mode="multiply"` must wrap its FillRect in a
/// `PushLayer { blend_mode: Some(Multiply) } … PopLayer` bracket.
#[test]
fn rect_blend_mode_wraps_fill_in_layer() {
    use zenith_scene::ir::BlendMode;

    let src = r##"zenith version=1 {
  project id="proj.bm" name="BM"
  tokens format="zenith-token-v1" {
    token id="color.fill" type="color" value="#112233"
  }
  styles {}
  document id="doc.bm" title="BM" {
    page id="page.bm" w=(px)100 h=(px)100 {
      rect id="rect.bm" x=(px)10 y=(px)10 w=(px)50 h=(px)50 fill=(token)"color.fill" blend-mode="multiply"
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
    // PushClip, PushLayer, FillRect, PopLayer, PopClip
    assert_eq!(cmds.len(), 5, "expected 5 commands, got: {:?}", cmds);

    assert!(
        matches!(cmds[0], SceneCommand::PushClip { .. }),
        "first command must be PushClip"
    );
    match &cmds[1] {
        SceneCommand::PushLayer {
            blend_mode,
            opacity,
        } => {
            assert_eq!(*blend_mode, Some(BlendMode::Multiply));
            assert_eq!(*opacity, 1.0, "no node opacity → layer opacity 1.0");
        }
        other => panic!("expected PushLayer, got {other:?}"),
    }
    match &cmds[2] {
        SceneCommand::FillRect {
            paint: Paint::Solid { color },
            ..
        } => {
            // Colors emit at full alpha when a blend layer is active.
            assert_eq!(color.a, 255, "blend-layer fill must use full alpha");
        }
        other => panic!("expected FillRect, got {other:?}"),
    }
    assert!(
        matches!(cmds[3], SceneCommand::PopLayer),
        "fourth command must be PopLayer"
    );
    assert!(
        matches!(cmds[4], SceneCommand::PopClip),
        "last command must be PopClip"
    );
}

/// A rect WITHOUT blend-mode must NOT emit any PushLayer/PopLayer — the
/// command stream is byte-identical to before blend-mode existed.
#[test]
fn rect_without_blend_mode_emits_no_layer() {
    let src = r##"zenith version=1 {
  project id="proj.nb" name="NB"
  tokens format="zenith-token-v1" {
    token id="color.fill" type="color" value="#112233"
  }
  styles {}
  document id="doc.nb" title="NB" {
    page id="page.nb" w=(px)100 h=(px)100 {
      rect id="rect.nb" x=(px)10 y=(px)10 w=(px)50 h=(px)50 fill=(token)"color.fill"
    }
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    assert!(result.diagnostics.is_empty());

    let cmds = &result.scene.commands;
    assert!(
        !cmds
            .iter()
            .any(|c| matches!(c, SceneCommand::PushLayer { .. } | SceneCommand::PopLayer)),
        "a node without blend-mode must emit no layer commands: {:?}",
        cmds
    );
}

// ── Element gaussian blur ─────────────────────────────────────────────

/// A rect with `blur=(px)8` must emit a `BeginBlur { radius: 8.0 }` … `EndBlur`
/// bracket around its fill/stroke draws. The radius must match exactly.
#[test]
fn rect_with_blur_emits_begin_end_bracket() {
    let src = r##"zenith version=1 {
  project id="proj.bl1" name="Bl1"
  tokens format="zenith-token-v1" {
    token id="color.fill" type="color" value="#445566"
  }
  styles {}
  document id="doc.bl1" title="Bl1" {
    page id="page.bl1" w=(px)200 h=(px)200 {
      rect id="rect.bl" x=(px)10 y=(px)10 w=(px)80 h=(px)60 fill=(token)"color.fill" blur=(px)8
    }
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    let cmds = &result.scene.commands;

    // Locate the BeginBlur and check the radius.
    let radius = cmds.iter().find_map(|c| match c {
        SceneCommand::BeginBlur { radius } => Some(*radius),
        _ => None,
    });
    assert_eq!(radius, Some(8.0), "BeginBlur radius must be 8.0: {cmds:?}");

    // Exactly one Begin and one End.
    let begins = cmds
        .iter()
        .filter(|c| matches!(c, SceneCommand::BeginBlur { .. }))
        .count();
    let ends = cmds
        .iter()
        .filter(|c| matches!(c, SceneCommand::EndBlur))
        .count();
    assert_eq!(begins, 1, "exactly one BeginBlur: {cmds:?}");
    assert_eq!(ends, 1, "exactly one EndBlur: {cmds:?}");

    // Begin must precede End and a fill must sit between them.
    let begin_idx = cmds
        .iter()
        .position(|c| matches!(c, SceneCommand::BeginBlur { .. }))
        .expect("begin index");
    let end_idx = cmds
        .iter()
        .position(|c| matches!(c, SceneCommand::EndBlur))
        .expect("end index");
    assert!(begin_idx < end_idx, "Begin must precede End");
    let has_fill = cmds
        .get(begin_idx + 1..end_idx)
        .map(|w| w.iter().any(|c| matches!(c, SceneCommand::FillRect { .. })))
        .unwrap_or(false);
    assert!(
        has_fill,
        "a fill must sit inside the blur bracket: {cmds:?}"
    );
}

/// A rect WITHOUT `blur` must emit no `BeginBlur`/`EndBlur` — the command
/// stream is byte-identical to the pre-blur behavior.
#[test]
fn rect_without_blur_emits_no_blur_bracket() {
    let src = r##"zenith version=1 {
  project id="proj.bl2" name="Bl2"
  tokens format="zenith-token-v1" {
    token id="color.fill" type="color" value="#445566"
  }
  styles {}
  document id="doc.bl2" title="Bl2" {
    page id="page.bl2" w=(px)200 h=(px)200 {
      rect id="rect.nb" x=(px)10 y=(px)10 w=(px)80 h=(px)60 fill=(token)"color.fill"
    }
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    let cmds = &result.scene.commands;
    assert!(
        !cmds
            .iter()
            .any(|c| matches!(c, SceneCommand::BeginBlur { .. } | SceneCommand::EndBlur)),
        "no blur attribute → no blur bracket: {cmds:?}"
    );
}

/// When both `blur` and `shadow` are set on the same rect, blur wins:
/// only a `BeginBlur`/`EndBlur` bracket is emitted and NO
/// `BeginShadow`/`EndShadow` appears in the stream.
#[test]
fn blur_wins_over_shadow_when_both_set() {
    let src = r##"zenith version=1 {
  project id="proj.bl3" name="Bl3"
  tokens format="zenith-token-v1" {
    token id="color.fill" type="color" value="#445566"
    token id="color.sh" type="color" value="#000000"
    token id="shadow.s" type="shadow" {
      layer dx=(px)2 dy=(px)2 blur=(px)4 color=(token)"color.sh"
    }
  }
  styles {}
  document id="doc.bl3" title="Bl3" {
    page id="page.bl3" w=(px)200 h=(px)200 {
      rect id="rect.bs" x=(px)10 y=(px)10 w=(px)80 h=(px)60 fill=(token)"color.fill" shadow=(token)"shadow.s" blur=(px)6
    }
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    let cmds = &result.scene.commands;

    // Blur bracket present.
    assert!(
        cmds.iter()
            .any(|c| matches!(c, SceneCommand::BeginBlur { .. })),
        "BeginBlur must be emitted when blur wins: {cmds:?}"
    );
    // Shadow bracket absent.
    assert!(
        !cmds.iter().any(|c| matches!(
            c,
            SceneCommand::BeginShadow { .. } | SceneCommand::EndShadow
        )),
        "BeginShadow must NOT be emitted when blur wins: {cmds:?}"
    );
}

/// A rect carrying a `filter=(token)` must emit a `BeginFilter { filters:[…] }`
/// … `EndFilter` bracket around its draw commands, with the ops resolved from
/// the referenced filter token (and per-kind default amounts substituted).
#[test]
fn filter_emits_begin_end_bracket() {
    let src = r##"zenith version=1 {
  project id="proj.fl" name="Fl"
  tokens format="zenith-token-v1" {
    token id="color.fill" type="color" value="#445566"
    token id="filter.f" type="filter" {
      grayscale
      brightness amount=0.5
    }
  }
  styles {}
  document id="doc.fl" title="Fl" {
    page id="page.fl" w=(px)200 h=(px)200 {
      rect id="rect.fl" x=(px)10 y=(px)10 w=(px)80 h=(px)40 fill=(token)"color.fill" filter=(token)"filter.f"
    }
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    let cmds = &result.scene.commands;

    // Locate the BeginFilter and verify the resolved ops.
    let begin = cmds.iter().find_map(|c| match c {
        SceneCommand::BeginFilter { filters } => Some(filters),
        _ => None,
    });
    let filters = begin.expect("a BeginFilter must be emitted");
    assert_eq!(filters.len(), 2, "two filter ops: {filters:?}");
    assert_eq!(
        filters[0],
        zenith_scene::FilterSpec::Grayscale(1.0),
        "grayscale default amount is 1.0"
    );
    assert_eq!(filters[1], zenith_scene::FilterSpec::Brightness(0.5));

    // Bracket is balanced, Begin precedes a draw which precedes End.
    let begin_idx = cmds
        .iter()
        .position(|c| matches!(c, SceneCommand::BeginFilter { .. }))
        .expect("begin index");
    let end_idx = cmds
        .iter()
        .position(|c| matches!(c, SceneCommand::EndFilter))
        .expect("end index");
    assert!(begin_idx < end_idx, "Begin must precede End");
    let has_draw_between = cmds
        .get(begin_idx + 1..end_idx)
        .map(|window| {
            window
                .iter()
                .any(|c| matches!(c, SceneCommand::FillRect { .. }))
        })
        .unwrap_or(false);
    assert!(
        has_draw_between,
        "a draw must sit inside the bracket: {cmds:?}"
    );
}

/// A bare `noise` op (no props) compiles to `FilterSpec::Noise` with the
/// per-kind defaults substituted: amount 1.0, seed 0, scale 1.0.
#[test]
fn noise_filter_uses_default_params() {
    let src = r##"zenith version=1 {
  project id="proj.nz" name="Nz"
  tokens format="zenith-token-v1" {
    token id="color.fill" type="color" value="#445566"
    token id="filter.f" type="filter" {
      noise
    }
  }
  styles {}
  document id="doc.nz" title="Nz" {
    page id="page.nz" w=(px)200 h=(px)200 {
      rect id="rect.nz" x=(px)10 y=(px)10 w=(px)80 h=(px)40 fill=(token)"color.fill" filter=(token)"filter.f"
    }
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    let cmds = &result.scene.commands;

    let begin = cmds.iter().find_map(|c| match c {
        SceneCommand::BeginFilter { filters } => Some(filters),
        _ => None,
    });
    let filters = begin.expect("a BeginFilter must be emitted");
    assert_eq!(filters.len(), 1, "one filter op: {filters:?}");
    assert_eq!(
        filters[0],
        zenith_scene::FilterSpec::Noise {
            amount: 1.0,
            seed: 0,
            scale: 1.0,
        },
        "bare noise uses default params"
    );
}

/// When a rect sets BOTH `blur` and `filter`, blur wins: only a
/// `BeginBlur`/`EndBlur` bracket is emitted and NO `BeginFilter`/`EndFilter`
/// appears in the stream (precedence: blur > shadow > filter).
#[test]
fn blur_suppresses_filter_when_both_set() {
    let src = r##"zenith version=1 {
  project id="proj.bf" name="Bf"
  tokens format="zenith-token-v1" {
    token id="color.fill" type="color" value="#445566"
    token id="filter.f" type="filter" {
      grayscale
    }
  }
  styles {}
  document id="doc.bf" title="Bf" {
    page id="page.bf" w=(px)200 h=(px)200 {
      rect id="rect.bf" x=(px)10 y=(px)10 w=(px)80 h=(px)60 fill=(token)"color.fill" filter=(token)"filter.f" blur=(px)6
    }
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    let cmds = &result.scene.commands;

    assert!(
        cmds.iter()
            .any(|c| matches!(c, SceneCommand::BeginBlur { .. })),
        "BeginBlur must be emitted when blur wins: {cmds:?}"
    );
    assert!(
        !cmds.iter().any(|c| matches!(
            c,
            SceneCommand::BeginFilter { .. } | SceneCommand::EndFilter
        )),
        "BeginFilter must NOT be emitted when blur wins: {cmds:?}"
    );
}

// ── U7: Per-side borders ───────────────────────────────────────────────

/// A rect with `border-bottom=(token)` produces a `StrokeLine` along the
/// bottom edge (y1 == y2 == y + h) with the resolved color/width.
/// A rect WITHOUT any border-* props produces NO extra `StrokeLine`.
#[test]
fn border_bottom_emits_stroke_line_along_bottom_edge() {
    // ── With border-bottom ──────────────────────────────────────────
    let src_with = r##"zenith version=1 {
  project id="proj.bb1" name="BB1"
  tokens format="zenith-token-v1" {
    token id="color.fill" type="color" value="#ffffff"
    token id="color.border" type="color" value="#ff0000"
    token id="size.bw" type="dimension" value=(px)4
  }
  styles {}
  document id="doc.bb1" title="BB1" {
    page id="page.bb1" w=(px)200 h=(px)200 {
      rect id="rect.bb" x=(px)10 y=(px)20 w=(px)80 h=(px)60 fill=(token)"color.fill" border-bottom=(token)"color.border" border-width=(token)"size.bw"
    }
  }
}
"##;
    let doc_with = parse(src_with);
    let result_with = compile(&doc_with, &default_provider());
    assert!(
        result_with.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        result_with.diagnostics
    );

    let cmds = &result_with.scene.commands;
    // Find the StrokeLine for the bottom border.
    let bottom_line = cmds.iter().find(|c| {
        matches!(c, SceneCommand::StrokeLine { y1, y2, .. } if (*y1 - 80.0).abs() < 1e-9 && (*y2 - 80.0).abs() < 1e-9)
    });
    assert!(
        bottom_line.is_some(),
        "expected a StrokeLine at y=80 (y+h=20+60); commands: {cmds:?}"
    );
    match bottom_line.unwrap() {
        SceneCommand::StrokeLine {
            x1,
            y1,
            x2,
            y2,
            color,
            stroke_width,
            stroke_dash,
            stroke_gap,
            stroke_linecap,
        } => {
            // x span: rect x=10 to x+w=90.
            assert!((*x1 - 10.0).abs() < 1e-9, "x1 must be rect x=10; got {x1}");
            assert!(
                (*x2 - 90.0).abs() < 1e-9,
                "x2 must be rect x+w=90; got {x2}"
            );
            assert!(
                (*y1 - 80.0).abs() < 1e-9 && (*y2 - 80.0).abs() < 1e-9,
                "y1 and y2 must be y+h=80; got y1={y1} y2={y2}"
            );
            // Color: #ff0000 → r=255, g=0, b=0.
            assert_eq!(color.r, 255, "border color must be red");
            assert_eq!(color.g, 0);
            assert_eq!(color.b, 0);
            // Width: token resolves to 4px.
            assert!(
                (*stroke_width - 4.0).abs() < 1e-9,
                "stroke_width must be 4px; got {stroke_width}"
            );
            // Per-side borders never use dashes.
            assert!(stroke_dash.is_none(), "border StrokeLine must have no dash");
            assert!(stroke_gap.is_none());
            assert!(stroke_linecap.is_none());
        }
        _ => panic!("impossible — already matched"),
    }

    // ── Without border-* → no extra StrokeLine ─────────────────────
    let src_without = r##"zenith version=1 {
  project id="proj.bb2" name="BB2"
  tokens format="zenith-token-v1" {
    token id="color.fill" type="color" value="#ffffff"
  }
  styles {}
  document id="doc.bb2" title="BB2" {
    page id="page.bb2" w=(px)200 h=(px)200 {
      rect id="rect.nob" x=(px)10 y=(px)20 w=(px)80 h=(px)60 fill=(token)"color.fill"
    }
  }
}
"##;
    let doc_without = parse(src_without);
    let result_without = compile(&doc_without, &default_provider());
    assert!(
        result_without.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        result_without.diagnostics
    );
    let has_stroke_line = result_without
        .scene
        .commands
        .iter()
        .any(|c| matches!(c, SceneCommand::StrokeLine { .. }));
    assert!(
        !has_stroke_line,
        "a rect without border-* must emit no StrokeLine; commands: {:?}",
        result_without.scene.commands
    );
}

// ── U8: Double border (stroke-outer) ──────────────────────────────────

/// A rect with `stroke-outer=(token)` + `stroke-outer-width=(token)` emits a
/// second `StrokeRect` at outset geometry (x-half, y-half, w+ow, h+ow) with the
/// outer color. A rect WITHOUT `stroke-outer` emits only the primary stroke.
#[test]
fn stroke_outer_emits_second_stroke_rect_at_outset() {
    // ── With stroke-outer ───────────────────────────────────────────
    let src_with = r##"zenith version=1 {
  project id="proj.so1" name="SO1"
  tokens format="zenith-token-v1" {
    token id="color.fill" type="color" value="#ffffff"
    token id="color.inner" type="color" value="#0000ff"
    token id="color.outer" type="color" value="#00ff00"
    token id="size.sw" type="dimension" value=(px)2
    token id="size.ow" type="dimension" value=(px)4
  }
  styles {}
  document id="doc.so1" title="SO1" {
    page id="page.so1" w=(px)300 h=(px)300 {
      rect id="rect.so" x=(px)50 y=(px)60 w=(px)100 h=(px)80 fill=(token)"color.fill" stroke=(token)"color.inner" stroke-width=(token)"size.sw" stroke-outer=(token)"color.outer" stroke-outer-width=(token)"size.ow"
    }
  }
}
"##;
    let doc_with = parse(src_with);
    let result_with = compile(&doc_with, &default_provider());
    assert!(
        result_with.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        result_with.diagnostics
    );

    let cmds = &result_with.scene.commands;
    // Collect all StrokeRect commands.
    let stroke_rects: Vec<_> = cmds
        .iter()
        .filter_map(|c| match c {
            SceneCommand::StrokeRect {
                x,
                y,
                w,
                h,
                color,
                stroke_width,
                ..
            } => Some((*x, *y, *w, *h, *color, *stroke_width)),
            _ => None,
        })
        .collect();

    assert_eq!(
        stroke_rects.len(),
        2,
        "expected 2 StrokeRect commands (primary + outer); got: {cmds:?}"
    );

    // Primary stroke: x=50, y=60, w=100, h=80, color=blue (#0000ff), sw=2.
    let (px0, py0, pw0, ph0, pc0, psw0) = stroke_rects[0];
    assert!(
        (px0 - 50.0).abs() < 1e-9 && (py0 - 60.0).abs() < 1e-9,
        "primary stroke origin must be (50, 60); got ({px0}, {py0})"
    );
    assert!(
        (pw0 - 100.0).abs() < 1e-9 && (ph0 - 80.0).abs() < 1e-9,
        "primary stroke size must be 100×80; got {pw0}×{ph0}"
    );
    assert_eq!(pc0.b, 255, "primary stroke must be blue");
    assert_eq!(pc0.r, 0);
    assert!((psw0 - 2.0).abs() < 1e-9, "primary stroke_width must be 2");

    // Outer stroke: ow=4, half=2 → x=50-2=48, y=60-2=58, w=100+4=104, h=80+4=84.
    let (ox, oy, ow, oh, oc, osw) = stroke_rects[1];
    assert!(
        (ox - 48.0).abs() < 1e-9,
        "outer stroke x must be 48 (50 - 4/2); got {ox}"
    );
    assert!(
        (oy - 58.0).abs() < 1e-9,
        "outer stroke y must be 58 (60 - 4/2); got {oy}"
    );
    assert!(
        (ow - 104.0).abs() < 1e-9,
        "outer stroke w must be 104 (100 + 4); got {ow}"
    );
    assert!(
        (oh - 84.0).abs() < 1e-9,
        "outer stroke h must be 84 (80 + 4); got {oh}"
    );
    assert_eq!(oc.g, 255, "outer stroke must be green");
    assert_eq!(oc.r, 0);
    assert!((osw - 4.0).abs() < 1e-9, "outer stroke_width must be 4");

    // ── Without stroke-outer → only one StrokeRect (the primary) ───
    let src_without = r##"zenith version=1 {
  project id="proj.so2" name="SO2"
  tokens format="zenith-token-v1" {
    token id="color.fill" type="color" value="#ffffff"
    token id="color.inner" type="color" value="#0000ff"
    token id="size.sw" type="dimension" value=(px)2
  }
  styles {}
  document id="doc.so2" title="SO2" {
    page id="page.so2" w=(px)300 h=(px)300 {
      rect id="rect.nos" x=(px)50 y=(px)60 w=(px)100 h=(px)80 fill=(token)"color.fill" stroke=(token)"color.inner" stroke-width=(token)"size.sw"
    }
  }
}
"##;
    let doc_without = parse(src_without);
    let result_without = compile(&doc_without, &default_provider());
    assert!(
        result_without.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        result_without.diagnostics
    );
    let stroke_rect_count = result_without
        .scene
        .commands
        .iter()
        .filter(|c| matches!(c, SceneCommand::StrokeRect { .. }))
        .count();
    assert_eq!(
        stroke_rect_count, 1,
        "a rect without stroke-outer must emit exactly 1 StrokeRect (the primary); \
         commands: {:?}",
        result_without.scene.commands
    );
}
