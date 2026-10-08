//! Imported SVG capability selection and raster resource fidelity.
use super::{content::translate, font::build_plan, scope_tests::check_raster_with_assets};
use std::sync::Arc;
use zenith_core::{AssetKind, BytesAssetProvider, default_provider};
use zenith_scene::{Color, FitMode, ImageClip, Paint, Scene, SceneCommand, SvgStyle};

fn document(body: &str) -> String {
    format!(
        "<svg xmlns='http://www.w3.org/2000/svg' xmlns:xlink='http://www.w3.org/1999/xlink' width='20' height='20'>{body}</svg>"
    )
}

fn svg_assets(svg: &str) -> BytesAssetProvider {
    let mut assets = BytesAssetProvider::new();
    assets.register("asset.svg", AssetKind::Svg, Arc::from(svg.as_bytes()));
    assets
}

fn image() -> SceneCommand {
    SceneCommand::DrawImage {
        x: 3.0,
        y: 4.0,
        w: 20.0,
        h: 20.0,
        asset_id: "asset.svg".into(),
        fit: FitMode::Contain,
        pos_x: 50.0,
        pos_y: 50.0,
        opacity: 1.0,
        clip_shape: None,
        src_rect: None,
        svg_style: None,
    }
}

fn scene(command: SceneCommand) -> Scene {
    let mut scene = Scene::new(40.0, 40.0);
    scene.commands.push(command);
    scene
}

fn content(scene: &Scene, assets: &BytesAssetProvider) -> (String, usize) {
    let fonts = default_provider();
    let plan = build_plan(&Default::default(), &fonts, true);
    let (content, resources) = translate(scene, &fonts, assets, &plan);
    (
        String::from_utf8(content.finish().into_vec()).unwrap(),
        resources.images.len(),
    )
}

#[test]
fn svg_complex_paints_and_compositing_match_raster_planes() {
    let bodies = [
        "<defs><radialGradient id='g'><stop stop-color='red'/><stop offset='1' stop-color='blue'/></radialGradient></defs><rect width='20' height='20' fill='url(#g)'/>",
        "<defs><linearGradient id='g'><stop stop-color='red' stop-opacity='.2'/><stop offset='1' stop-color='blue'/></linearGradient></defs><rect width='20' height='20' fill='url(#g)'/>",
        "<defs><linearGradient id='g'><stop stop-color='red'/><stop offset='1' stop-color='blue'/></linearGradient></defs><path d='M2 5 L18 15' fill='none' stroke='url(#g)' stroke-width='4'/>",
        "<defs><clipPath id='c'><circle cx='10' cy='10' r='7'/></clipPath></defs><rect width='20' height='20' fill='red' clip-path='url(#c)'/>",
        "<defs><mask id='m'><rect width='10' height='20' fill='white'/></mask></defs><rect width='20' height='20' fill='red' mask='url(#m)'/>",
        "<defs><pattern id='p' width='4' height='4' patternUnits='userSpaceOnUse'><rect width='2' height='4' fill='blue'/></pattern></defs><rect width='20' height='20' fill='url(#p)'/>",
        "<g opacity='.5'><rect width='14' height='14' fill='red'/><rect x='6' y='6' width='14' height='14' fill='blue'/></g>",
        "<defs><filter id='f'><feGaussianBlur stdDeviation='1'/></filter></defs><rect x='3' y='3' width='14' height='14' fill='red' filter='url(#f)'/>",
        "<g style='isolation:isolate'><rect width='20' height='20' fill='red'/></g>",
        "<rect width='20' height='20' fill='red'/><rect x='4' y='4' width='12' height='12' fill='blue' style='mix-blend-mode:multiply'/>",
        "<path d='M2 3 L10 17 L18 3 Z' fill='red' stroke='blue' stroke-width='4' paint-order='stroke fill'/>",
        "<path d='M2 17 L10 3 L18 17' fill='none' stroke='red' stroke-width='3' stroke-linejoin='miter-clip'/>",
        "<path transform='scale(1 .7)' d='M2 3 L10 17 L18 3' fill='none' stroke='red' stroke-width='3'/>",
    ];
    for body in bodies {
        let svg = document(body);
        check_raster_with_assets(&scene(image()), &svg_assets(&svg));
    }
}

#[test]
fn svg_linear_gradient_affine_spread_and_stop_layout_match_raster_planes() {
    for attributes in [
        "gradientTransform='rotate(25)'",
        "spreadMethod='reflect' x2='.3'",
        "spreadMethod='repeat' x2='.3'",
    ] {
        let svg = document(&format!(
            "<defs><linearGradient id='g' {attributes}><stop stop-color='red'/><stop offset='1' stop-color='blue'/></linearGradient></defs><rect width='20' height='20' fill='url(#g)'/>"
        ));
        assert_eq!(
            content(&scene(image()), &svg_assets(&svg)).1,
            1,
            "{attributes}"
        );
        check_raster_with_assets(&scene(image()), &svg_assets(&svg));
    }
    let stops = "<stop offset='.2' stop-color='red'/><stop offset='.8' stop-color='blue'/>";
    let svg = document(&format!(
        "<defs><linearGradient id='g'>{stops}</linearGradient></defs><rect width='20' height='20' fill='url(#g)'/>"
    ));
    assert_eq!(content(&scene(image()), &svg_assets(&svg)).1, 1, "{stops}");
    check_raster_with_assets(&scene(image()), &svg_assets(&svg));
    let svg = document(
        "<defs><linearGradient id='g' x2='1' y2='1'><stop stop-color='red'/><stop offset='1' stop-color='blue'/></linearGradient></defs><rect width='20' height='10' fill='url(#g)'/>",
    );
    check_raster_with_assets(&scene(image()), &svg_assets(&svg));
}

#[test]
fn svg_placement_opacity_fit_and_stroke_scaling_depend_on_placement() {
    let svg = document(
        "<rect width='14' height='14' fill='red'/><rect x='6' y='6' width='14' height='14' fill='blue'/>",
    );
    let assets = svg_assets(&svg);
    let mut translucent = image();
    if let SceneCommand::DrawImage { opacity, .. } = &mut translucent {
        *opacity = 0.5;
    }
    let mut fit_none = image();
    if let SceneCommand::DrawImage { fit, w, h, .. } = &mut fit_none {
        *fit = FitMode::None;
        *w = 15.0;
        *h = 17.0;
    }
    check_raster_with_assets(&scene(translucent.clone()), &assets);
    check_raster_with_assets(&scene(fit_none), &assets);
    let combined = Scene {
        commands: vec![image(), translucent],
        ..Scene::new(40.0, 40.0)
    };
    let (text, count) = content(&combined, &assets);
    assert_eq!(count, 1);
    assert!(text.contains(" f\n") || text.contains("\nf\n"));
    let stroke = svg_assets(&document(
        "<path d='M2 3 L18 17' stroke='red' stroke-width='3'/>",
    ));
    let mut stretch = image();
    if let SceneCommand::DrawImage { fit, w, .. } = &mut stretch {
        *fit = FitMode::Stretch;
        *w = 30.0;
    }
    check_raster_with_assets(&scene(stretch), &stroke);
}

#[test]
fn svg_fallback_captures_complete_transform_clip_and_preserves_unrelated_vectors() {
    let svg = document(
        "<defs><radialGradient id='g'><stop stop-color='red'/><stop offset='1' stop-color='blue'/></radialGradient></defs><rect width='20' height='20' fill='url(#g)'/>",
    );
    let assets = svg_assets(&svg);
    let mut image = image();
    if let SceneCommand::DrawImage { clip_shape, .. } = &mut image {
        *clip_shape = Some(ImageClip::Ellipse);
    }
    let mut scene = Scene::new(40.0, 40.0);
    scene.commands = vec![
        SceneCommand::PushTransform {
            angle_deg: 20.0,
            cx: 10.0,
            cy: 10.0,
        },
        SceneCommand::PushClip {
            x: 5.0,
            y: 5.0,
            w: 20.0,
            h: 18.0,
        },
        image,
        SceneCommand::PopClip,
        SceneCommand::PopTransform,
    ];
    check_raster_with_assets(&scene, &assets);
    let fonts = default_provider();
    assert_eq!(
        super::scopes::plan(&scene, &fonts, &assets).unwrap(),
        vec![0..5]
    );
    scene.commands.push(SceneCommand::FillRect {
        x: 30.0,
        y: 30.0,
        w: 5.0,
        h: 5.0,
        paint: Paint::solid(Color::srgb(0, 255, 0, 255)),
    });
    let (text, count) = content(&scene, &assets);
    assert_eq!(count, 1);
    assert!(text.contains("30 30 5 5 re"));
    assert_eq!(
        super::render_pdf(&scene, &fonts, &assets),
        super::render_pdf(&scene, &fonts, &assets)
    );
}

#[test]
fn svg_native_strokes_apply_caps_joins_miters_and_scaled_dashes() {
    for (cap, cap_operator, join, join_operator) in [
        ("butt", "0 J", "miter", "0 j"),
        ("round", "1 J", "round", "1 j"),
        ("square", "2 J", "bevel", "2 j"),
    ] {
        let svg = document(&format!(
            "<path d='M2 3 L10 17 L18 3' fill='none' stroke='red' stroke-width='2' stroke-linecap='{cap}' stroke-linejoin='{join}' stroke-miterlimit='7' stroke-dasharray='3 2' stroke-dashoffset='1'/>"
        ));
        let mut image = image();
        if let SceneCommand::DrawImage { w, h, .. } = &mut image {
            *w = 30.0;
            *h = 30.0;
        }
        let (text, count) = content(&scene(image), &svg_assets(&svg));
        assert_eq!(count, 0);
        for operator in [cap_operator, join_operator, "7 M", "3 w", "[4.5 3] 1.5 d"] {
            assert!(text.contains(operator), "{operator}: {text}");
        }
    }
}

#[test]
fn svg_style_override_keeps_native_stroke() {
    let svg = document(
        "<path d='M2 3 L10 17 L18 3' fill='none' stroke='currentColor' stroke-width='1'/>",
    );
    let assets = svg_assets(&svg);
    let mut styled = image();
    if let SceneCommand::DrawImage { svg_style, .. } = &mut styled {
        *svg_style = Some(SvgStyle {
            stroke: Some(Color::srgb(10, 40, 220, 255)),
            fill: None,
            stroke_width: Some(2.0),
        });
    }
    let (text, count) = content(&scene(styled), &assets);
    assert_eq!(count, 0);
    assert!(text.contains("2 w"));
}

#[test]
fn svg_missing_and_malformed_assets_keep_empty_output() {
    for assets in [BytesAssetProvider::new(), svg_assets("<broken")] {
        let (text, count) = content(&scene(image()), &assets);
        assert_eq!(count, 0);
        assert!(!text.contains(" Do"));
        assert!(!text.contains(" m\n"));
    }
}

#[test]
fn svg_single_stop_gradient_normalizes_to_native_solid_fill() {
    let svg = document(
        "<defs><linearGradient id='g'><stop stop-color='red'/></linearGradient></defs><rect width='20' height='20' fill='url(#g)'/>",
    );
    let (_, count) = content(&scene(image()), &svg_assets(&svg));
    assert_eq!(count, 0);
}

#[test]
fn svg_negative_dash_offsets_use_nonnegative_full_period() {
    for (dashes, phase) in [("3 2", "[3 2] 4 d"), ("3", "[3 3] 5 d")] {
        let svg = document(&format!(
            "<path d='M2 3 L18 17' fill='none' stroke='red' stroke-dasharray='{dashes}' stroke-dashoffset='-1'/>"
        ));
        let (text, count) = content(&scene(image()), &svg_assets(&svg));
        assert_eq!(count, 0);
        assert!(text.contains(phase), "{text}");
    }
}

#[test]
fn svg_text_uses_registered_font_outlines_and_complex_text_paints_match_raster() {
    let svg =
        document("<text x='1' y='15' font-family='Noto Sans' font-size='14' fill='red'>Hi</text>");
    let (text, count) = content(&scene(image()), &svg_assets(&svg));
    assert_eq!(count, 0);
    assert!(text.contains(" c\n"));
    let svg = document(
        "<defs><radialGradient id='g'><stop stop-color='red'/><stop offset='1' stop-color='blue'/></radialGradient></defs><text x='1' y='15' font-family='Noto Sans' font-size='14' fill='url(#g)'>Hi</text>",
    );
    check_raster_with_assets(&scene(image()), &svg_assets(&svg));
}

#[test]
fn svg_nested_raster_and_svg_images_match_raster_planes() {
    for uri in [
        "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DQAAAEgQGALFXOsAAAAABJRU5ErkJggg==",
        "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIyMCIgaGVpZ2h0PSIyMCI+PGNpcmNsZSBjeD0iMTAiIGN5PSIxMCIgcj0iOCIgZmlsbD0iYmx1ZSIvPjwvc3ZnPg==",
    ] {
        let svg = document(&format!(
            "<image width='20' height='20' xlink:href='{uri}'/>"
        ));
        check_raster_with_assets(&scene(image()), &svg_assets(&svg));
    }
}

#[test]
fn svg_viewbox_origin_scaling_and_aspect_placement_match_raster_planes() {
    for header in [
        "viewBox='5 7 20 20'",
        "viewBox='0 0 10 10'",
        "viewBox='0 0 10 20' preserveAspectRatio='xMidYMid meet'",
        "viewBox='0 0 10 20' preserveAspectRatio='xMaxYMax slice'",
    ] {
        let svg = format!(
            "<svg xmlns='http://www.w3.org/2000/svg' width='20' height='20' {header}><rect x='5' y='7' width='10' height='10' fill='red'/></svg>"
        );
        check_raster_with_assets(&scene(image()), &svg_assets(&svg));
    }
}

#[test]
fn svg_object_bbox_text_gradients_match_raster_planes() {
    let svg = document(
        "<defs><linearGradient id='g'><stop stop-color='red'/><stop offset='1' stop-color='blue'/></linearGradient></defs><text x='1' y='15' font-family='Noto Sans' font-size='14' fill='url(#g)'>Hi</text>",
    );
    check_raster_with_assets(&scene(image()), &svg_assets(&svg));
}

#[test]
fn svg_tiny_scale_keeps_anisotropic_strokes_out_of_native_capabilities() {
    let svg = document("<path d='M2 3 L18 17' fill='none' stroke='red' stroke-width='3'/>");
    let mut image = image();
    if let SceneCommand::DrawImage { fit, w, h, .. } = &mut image {
        *fit = FitMode::Stretch;
        *w = 0.00002;
        *h = 0.0002;
    }
    let fonts = default_provider();
    assert_eq!(
        super::scopes::plan(&scene(image), &fonts, &svg_assets(&svg)).unwrap(),
        vec![0..1]
    );
}

#[test]
fn svg_normalized_duplicate_stops_keep_native_shading() {
    let svg = document(
        "<defs><linearGradient id='g'><stop stop-color='red'/><stop offset='.5' stop-color='red'/><stop offset='.5' stop-color='blue'/><stop offset='1' stop-color='blue'/></linearGradient></defs><rect width='20' height='20' fill='url(#g)'/>",
    );
    let (text, count) = content(&scene(image()), &svg_assets(&svg));
    assert_eq!(count, 0);
    assert!(text.contains("/sh0 sh"));
}
