mod common;
use common::*;
use zenith_core::default_provider;
use zenith_scene::compile;
use zenith_scene::ir::{FitMode, ImageClip, SceneCommand, SrcRect};

#[test]
fn image_emits_pushclip_drawimage_popclip() {
    let src = r##"zenith version=1 {
  project id="proj.i1" name="I1"
  assets {
asset id="asset.swatch" kind="image" src="assets/swatch.png"
  }
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.i1" title="I1" {
page id="page.i1" w=(px)320 h=(px)200 {
  image id="img.i1" asset="asset.swatch" x=(px)40 y=(px)40 w=(px)160 h=(px)120 fit="stretch"
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
    // PushClip(page), PushClip(box), DrawImage, PopClip(box), PopClip(page)
    assert_eq!(cmds.len(), 5, "expected 5 commands, got: {:?}", cmds);
    assert!(
        matches!(cmds[1], SceneCommand::PushClip { x, y, w, h } if x == 40.0 && y == 40.0 && w == 160.0 && h == 120.0),
        "cmd[1] must be the image box PushClip"
    );
    match &cmds[2] {
        SceneCommand::DrawImage {
            x,
            y,
            w,
            h,
            asset_id,
            fit,
            pos_x,
            pos_y,
            opacity,
            clip_shape,
            src_rect,
        } => {
            assert_eq!(*x, 40.0);
            assert_eq!(*y, 40.0);
            assert_eq!(*w, 160.0);
            assert_eq!(*h, 120.0);
            assert_eq!(asset_id, "asset.swatch");
            assert_eq!(*fit, FitMode::Stretch);
            assert_eq!(*pos_x, 50.0, "default object-position-x must be 50");
            assert_eq!(*pos_y, 50.0, "default object-position-y must be 50");
            assert_eq!(*opacity, 1.0);
            assert_eq!(*clip_shape, None, "default image has no clip shape");
            assert_eq!(*src_rect, None, "default image has no src_rect");
        }
        other => panic!("expected DrawImage, got {other:?}"),
    }
    assert!(matches!(cmds[3], SceneCommand::PopClip));
}

#[test]
fn image_fit_and_object_position_mapped() {
    let src = r##"zenith version=1 {
  project id="proj.i2" name="I2"
  assets {
asset id="asset.swatch" kind="image" src="assets/swatch.png"
  }
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.i2" title="I2" {
page id="page.i2" w=(px)320 h=(px)200 {
  image id="img.i2" asset="asset.swatch" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fit="cover" object-position-x=(pct)25 object-position-y="start"
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    let draw = result
        .scene
        .commands
        .iter()
        .find_map(|c| match c {
            SceneCommand::DrawImage {
                fit, pos_x, pos_y, ..
            } => Some((*fit, *pos_x, *pos_y)),
            _ => None,
        })
        .expect("must emit a DrawImage");
    assert_eq!(draw.0, FitMode::Cover);
    assert_eq!(draw.1, 25.0, "object-position-x (pct)25 → 25.0");
    assert_eq!(draw.2, 0.0, "object-position-y start → 0.0");
}

#[test]
fn invisible_image_not_emitted() {
    let src = r##"zenith version=1 {
  project id="proj.i3" name="I3"
  assets {
asset id="asset.swatch" kind="image" src="assets/swatch.png"
  }
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.i3" title="I3" {
page id="page.i3" w=(px)320 h=(px)200 {
  image id="img.i3" asset="asset.swatch" x=(px)40 y=(px)40 w=(px)160 h=(px)120 visible=#false
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    let cmds = &result.scene.commands;
    // Only the page PushClip + PopClip; no image commands.
    assert_eq!(
        cmds.len(),
        2,
        "expected PushClip + PopClip only; got: {cmds:?}"
    );
    assert!(
        !cmds
            .iter()
            .any(|c| matches!(c, SceneCommand::DrawImage { .. })),
        "no DrawImage expected for invisible image"
    );
}

#[test]
fn image_clip_ellipse_sets_clip_shape() {
    let src = r##"zenith version=1 {
  project id="proj.ic1" name="IC1"
  assets {
asset id="asset.pfp" kind="image" src="assets/pfp.png"
  }
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.ic1" title="IC1" {
page id="page.ic1" w=(px)320 h=(px)200 {
  image id="img.ic1" asset="asset.pfp" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fit="cover" clip="ellipse"
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    let clip = result
        .scene
        .commands
        .iter()
        .find_map(|c| match c {
            SceneCommand::DrawImage { clip_shape, .. } => Some(clip_shape.clone()),
            _ => None,
        })
        .expect("must emit a DrawImage");
    assert_eq!(
        clip,
        Some(ImageClip::Ellipse),
        "clip=\"ellipse\" must set clip_shape to Ellipse"
    );
}

#[test]
fn image_clip_rounded_resolves_radius() {
    let src = r##"zenith version=1 {
  project id="proj.ic2" name="IC2"
  assets {
asset id="asset.av" kind="image" src="assets/av.png"
  }
  tokens format="zenith-token-v1" {
token id="size.radius.avatar" type="dimension" value=(px)24
  }
  styles {}
  document id="doc.ic2" title="IC2" {
page id="page.ic2" w=(px)320 h=(px)200 {
  image id="img.ic2" asset="asset.av" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fit="cover" clip="rounded" clip-radius=(token)"size.radius.avatar"
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    let clip = result
        .scene
        .commands
        .iter()
        .find_map(|c| match c {
            SceneCommand::DrawImage { clip_shape, .. } => Some(clip_shape.clone()),
            _ => None,
        })
        .expect("must emit a DrawImage");
    assert_eq!(
        clip,
        Some(ImageClip::RoundedRect { radius: 24.0 }),
        "clip=\"rounded\" must resolve clip-radius token to px"
    );
}

#[test]
fn image_no_clip_has_none_clip_shape() {
    let src = r##"zenith version=1 {
  project id="proj.ic3" name="IC3"
  assets {
asset id="asset.bg" kind="image" src="assets/bg.png"
  }
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.ic3" title="IC3" {
page id="page.ic3" w=(px)320 h=(px)200 {
  image id="img.ic3" asset="asset.bg" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fit="cover"
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    let clip = result
        .scene
        .commands
        .iter()
        .find_map(|c| match c {
            SceneCommand::DrawImage { clip_shape, .. } => Some(clip_shape.clone()),
            _ => None,
        })
        .expect("must emit a DrawImage");
    assert_eq!(clip, None, "image without clip must have clip_shape None");
}

#[test]
fn image_opacity_cascades() {
    // Group opacity 0.5 × image opacity 0.5 = 0.25.
    let src = r##"zenith version=1 {
  project id="proj.i4" name="I4"
  assets {
asset id="asset.swatch" kind="image" src="assets/swatch.png"
  }
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.i4" title="I4" {
page id="page.i4" w=(px)320 h=(px)200 {
  group id="group.i4" opacity=0.5 {
    image id="img.i4" asset="asset.swatch" x=(px)40 y=(px)40 w=(px)160 h=(px)120 opacity=0.5
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    let opacity = result
        .scene
        .commands
        .iter()
        .find_map(|c| match c {
            SceneCommand::DrawImage { opacity, .. } => Some(*opacity),
            _ => None,
        })
        .expect("must emit a DrawImage");
    assert!(
        (opacity - 0.25).abs() < 1e-9,
        "cascaded opacity must be 0.25; got {opacity}"
    );
}

#[test]
fn bleed_expands_canvas_and_shifts_content() {
    let doc = parse(&bleed_doc_src(" bleed=(px)35"));
    let result = compile(&doc, &default_provider());
    assert!(
        !result.diagnostics.iter().any(|d| d.code != "token.unused"),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );

    // Media box = (400 + 70) × (600 + 70).
    assert_eq!(result.scene.width, 470.0);
    assert_eq!(result.scene.height, 670.0);

    let cmds = &result.scene.commands;

    // Background fills the ENTIRE media box (bleeds off the trim edge).
    match cmds
        .iter()
        .find(|c| matches!(c, SceneCommand::FillRect { paint: Paint::Solid { color }, .. } if color.r == 0x10))
    {
        Some(SceneCommand::FillRect { x, y, w, h, .. }) => {
            assert_eq!((*x, *y, *w, *h), (0.0, 0.0, 470.0, 670.0));
        }
        other => panic!("expected full-media background FillRect, got {other:?}"),
    }

    // Hero rect shifted by (b, b) = (35, 35).
    match cmds
        .iter()
        .find(|c| matches!(c, SceneCommand::FillRect { paint: Paint::Solid { color }, .. } if color.r == 0xff))
    {
        Some(SceneCommand::FillRect { x, y, w, h, .. }) => {
            assert_eq!((*x, *y, *w, *h), (35.0, 35.0, 100.0, 100.0));
        }
        other => panic!("expected shifted hero FillRect, got {other:?}"),
    }
}

#[test]
fn bleed_emits_eight_crop_mark_segments_all_in_margin() {
    let b = 35.0;
    let doc = parse(&bleed_doc_src(" bleed=(px)35"));
    let result = compile(&doc, &default_provider());

    let lines: Vec<&SceneCommand> = result
        .scene
        .commands
        .iter()
        .filter(|c| matches!(c, SceneCommand::StrokeLine { .. }))
        .collect();
    assert_eq!(lines.len(), 8, "expected 8 crop-mark segments");

    // Trim box: [35, 35] .. [435, 635]. Every segment endpoint must lie OUTSIDE
    // the trim box (in the bleed margin) — i.e. NOT strictly interior to it.
    let trim_left = b;
    let trim_top = b;
    let trim_right = b + 400.0;
    let trim_bottom = b + 600.0;
    let interior =
        |x: f64, y: f64| x > trim_left && x < trim_right && y > trim_top && y < trim_bottom;
    for cmd in &lines {
        if let SceneCommand::StrokeLine { x1, y1, x2, y2, .. } = cmd {
            assert!(
                !interior(*x1, *y1) && !interior(*x2, *y2),
                "crop-mark segment must stay in the bleed margin: {cmd:?}"
            );
        }
    }
}

#[test]
fn bleed_absent_is_byte_identical_to_no_bleed() {
    // The exact same document MINUS the bleed attribute must yield the same
    // scene as a document that never mentioned bleed.
    let with_none = parse(&bleed_doc_src(""));
    let result = compile(&with_none, &default_provider());

    // Canvas is the plain page size; no crop marks emitted.
    assert_eq!(result.scene.width, 400.0);
    assert_eq!(result.scene.height, 600.0);
    assert!(
        !result
            .scene
            .commands
            .iter()
            .any(|c| matches!(c, SceneCommand::StrokeLine { .. })),
        "no bleed → no crop marks"
    );
    // PushClip covers the plain page rectangle (origin unshifted).
    assert!(
        matches!(
            result.scene.commands.first(),
            Some(SceneCommand::PushClip { x, y, w, h }) if *x == 0.0 && *y == 0.0 && *w == 400.0 && *h == 600.0
        ),
        "first command must be a page-sized PushClip"
    );
    // Hero rect is NOT shifted.
    match result
        .scene
        .commands
        .iter()
        .find(|c| matches!(c, SceneCommand::FillRect { paint: Paint::Solid { color }, .. } if color.r == 0xff))
    {
        Some(SceneCommand::FillRect { x, y, .. }) => assert_eq!((*x, *y), (0.0, 0.0)),
        other => panic!("expected unshifted hero FillRect, got {other:?}"),
    }
}

#[test]
fn bleed_render_is_two_run_byte_identical() {
    let doc = parse(&bleed_doc_src(" bleed=(px)35"));
    let a = compile(&doc, &default_provider());
    let b = compile(&doc, &default_provider());
    assert_eq!(
        a.scene.to_json().expect("serialize a"),
        b.scene.to_json().expect("serialize b"),
        "two compile runs must be byte-identical"
    );
}

// ── src-rect compile tests ─────────────────────────────────────────────────────

#[test]
fn image_with_src_rect_compiles_to_some_src_rect() {
    let src = r##"zenith version=1 {
  project id="proj.sr1" name="SR1"
  assets {
asset id="asset.photo" kind="image" src="assets/photo.png"
  }
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.sr1" title="SR1" {
page id="page.sr1" w=(px)320 h=(px)200 {
  image id="img.sr1" asset="asset.photo" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fit="stretch" src-x=(px)10 src-y=(px)20 src-w=(px)50 src-h=(px)60
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

    let sr = result
        .scene
        .commands
        .iter()
        .find_map(|c| match c {
            SceneCommand::DrawImage { src_rect, .. } => Some(src_rect.clone()),
            _ => None,
        })
        .expect("must emit a DrawImage");
    assert_eq!(
        sr,
        Some(SrcRect {
            x: 10.0,
            y: 20.0,
            w: 50.0,
            h: 60.0
        }),
        "src-rect must compile to SrcRect with correct px values"
    );
}

#[test]
fn image_without_src_rect_has_none_src_rect() {
    let src = r##"zenith version=1 {
  project id="proj.sr2" name="SR2"
  assets {
asset id="asset.photo" kind="image" src="assets/photo.png"
  }
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.sr2" title="SR2" {
page id="page.sr2" w=(px)320 h=(px)200 {
  image id="img.sr2" asset="asset.photo" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fit="stretch"
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    let sr = result
        .scene
        .commands
        .iter()
        .find_map(|c| match c {
            SceneCommand::DrawImage { src_rect, .. } => Some(src_rect.clone()),
            _ => None,
        })
        .expect("must emit a DrawImage");
    assert_eq!(
        sr, None,
        "image without src-rect must have src_rect == None (byte-compat)"
    );
}
