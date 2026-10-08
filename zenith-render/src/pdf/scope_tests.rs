use super::{content::translate, font::build_plan};
use miniz_oxide::inflate::decompress_to_vec_zlib;
use zenith_core::{BytesAssetProvider, default_provider};
use zenith_scene::{Color, Paint, Scene, SceneCommand};

fn shape() -> SceneCommand {
    SceneCommand::FillRect {
        x: -4.0,
        y: 2.0,
        w: 10.0,
        h: 8.0,
        paint: Paint::solid(Color::srgb(200, 30, 60, 255)),
    }
}

pub(super) fn check_raster(scene: &Scene) -> String {
    let fonts = default_provider();
    let assets = BytesAssetProvider::new();
    let plan = build_plan(&Default::default(), &fonts, true);
    let (content, resources) = translate(scene, &fonts, &assets, &plan);
    let raster = crate::render_image(scene, &fonts, &assets).unwrap();
    assert_eq!(resources.images.len(), 1);
    let image = &resources.images[0];
    let rgb = decompress_to_vec_zlib(&image.rgb_flate).unwrap();
    let alpha = image
        .alpha_flate
        .as_ref()
        .map(|data| decompress_to_vec_zlib(data).unwrap())
        .unwrap_or_else(|| vec![255; image.width as usize * image.height as usize]);
    let nonzero: Vec<_> = raster
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .filter(|(_, p)| p[3] > 0)
        .collect();
    let min_x = nonzero
        .iter()
        .map(|(i, _)| i % raster.width as usize)
        .min()
        .unwrap();
    let max_x = nonzero
        .iter()
        .map(|(i, _)| i % raster.width as usize)
        .max()
        .unwrap();
    let min_y = nonzero
        .iter()
        .map(|(i, _)| i / raster.width as usize)
        .min()
        .unwrap();
    let max_y = nonzero
        .iter()
        .map(|(i, _)| i / raster.width as usize)
        .max()
        .unwrap();
    assert_eq!(
        (image.width as usize, image.height as usize),
        (max_x - min_x + 1, max_y - min_y + 1)
    );
    let mut expected_rgb = Vec::new();
    let mut expected_alpha = Vec::new();
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let p = &raster.rgba[(y * raster.width as usize + x) * 4..][..4];
            expected_rgb.extend_from_slice(&p[..3]);
            expected_alpha.push(p[3]);
        }
    }
    assert_eq!(rgb, expected_rgb);
    assert_eq!(alpha, expected_alpha);
    let content = String::from_utf8(content.finish().into_vec()).unwrap();
    assert!(
        content.contains(&format!(
            "{} 0 0 -{} {} {} cm",
            image.width,
            image.height,
            min_x,
            min_y + image.height as usize
        )),
        "{content}"
    );
    content
}

#[test]
fn transformed_effect_captures_offpage_source() {
    let mut scene = Scene::new(24.0, 24.0);
    scene.commands = vec![
        SceneCommand::PushScaleTranslate {
            sx: 1.0,
            sy: 1.0,
            tx: 10.0,
            ty: 3.0,
        },
        SceneCommand::BeginBlur { radius: 1.0 },
        shape(),
        SceneCommand::EndBlur,
        SceneCommand::PopTransform,
    ];
    let content = check_raster(&scene);
    assert!(!content.contains("1 0 0 1 10 3 cm"));
}

#[test]
fn overlapping_layer_applies_opacity_once() {
    let mut scene = Scene::new(24.0, 24.0);
    scene.commands = vec![
        SceneCommand::PushLayer {
            opacity: 0.5,
            blend_mode: None,
        },
        shape(),
        shape(),
        SceneCommand::PopLayer,
    ];
    check_raster(&scene);
}

#[test]
fn unfinished_effect_retains_body() {
    let mut scene = Scene::new(24.0, 24.0);
    scene.commands = vec![SceneCommand::BeginBlur { radius: 1.0 }, shape()];
    let fonts = default_provider();
    let plan = build_plan(&Default::default(), &fonts, true);
    let (content, resources) = translate(&scene, &fonts, &BytesAssetProvider::new(), &plan);
    assert!(resources.images.is_empty());
    let content = String::from_utf8(content.finish().into_vec()).unwrap();
    assert!(content.contains("-4 2 10 8 re"), "{content}");
}

#[test]
fn effect_kinds_capture_transform_clip_and_layer() {
    use zenith_scene::{FilterSpec, MaskShape, MaskSpec, ShadowSpec};
    let effects = [
        (
            SceneCommand::BeginBlur { radius: 1.0 },
            SceneCommand::EndBlur,
        ),
        (
            SceneCommand::BeginShadow {
                shadows: vec![ShadowSpec {
                    dx: 2.0,
                    dy: 1.0,
                    blur: 1.0,
                    color: Color::srgb(0, 0, 0, 160),
                }],
            },
            SceneCommand::EndShadow,
        ),
        (
            SceneCommand::BeginFilter {
                filters: vec![FilterSpec::Invert(0.5)],
            },
            SceneCommand::EndFilter,
        ),
        (
            SceneCommand::BeginMask {
                mask: MaskSpec {
                    shape: MaskShape::Ellipse,
                    radius: 0.0,
                    feather: 1.0,
                    invert: false,
                    x: 3.0,
                    y: 3.0,
                    w: 12.0,
                    h: 12.0,
                },
            },
            SceneCommand::EndMask,
        ),
    ];
    for (begin, end) in effects {
        let mut scene = Scene::new(24.0, 24.0);
        scene.commands = vec![
            SceneCommand::PushScaleTranslate {
                sx: 1.5,
                sy: 1.2,
                tx: 9.0,
                ty: 2.0,
            },
            SceneCommand::PushClip {
                x: 0.0,
                y: 0.0,
                w: 18.0,
                h: 18.0,
            },
            SceneCommand::PushLayer {
                opacity: 0.6,
                blend_mode: None,
            },
            begin,
            shape(),
            end,
            SceneCommand::PopLayer,
            SceneCommand::PopClip,
            SceneCommand::PopTransform,
        ];
        check_raster(&scene);
    }
}

#[test]
fn nested_opacity_layers_and_crossed_scopes_match_raster() {
    let mut scene = Scene::new(24.0, 24.0);
    scene.commands = vec![
        SceneCommand::PushLayer {
            opacity: 0.7,
            blend_mode: None,
        },
        shape(),
        SceneCommand::PushLayer {
            opacity: 0.5,
            blend_mode: None,
        },
        shape(),
        shape(),
        SceneCommand::PopLayer,
        SceneCommand::PopLayer,
    ];
    check_raster(&scene);
    scene.commands = vec![
        SceneCommand::PushScaleTranslate {
            sx: 1.0,
            sy: 1.0,
            tx: 8.0,
            ty: 2.0,
        },
        SceneCommand::PushClip {
            x: 0.0,
            y: 0.0,
            w: 12.0,
            h: 12.0,
        },
        shape(),
        SceneCommand::PopTransform,
        shape(),
        SceneCommand::PopClip,
    ];
    check_raster(&scene);
}

#[test]
fn nonnormal_blends_capture_whole_page_backdrop() {
    use zenith_scene::BlendMode;
    for mode in [BlendMode::Multiply, BlendMode::Screen] {
        let mut scene = Scene::new(24.0, 24.0);
        scene.commands = vec![
            SceneCommand::FillRect {
                x: 0.0,
                y: 0.0,
                w: 24.0,
                h: 24.0,
                paint: Paint::solid(Color::srgb(60, 100, 170, 255)),
            },
            SceneCommand::PushLayer {
                opacity: 0.8,
                blend_mode: Some(mode),
            },
            shape(),
            SceneCommand::PopLayer,
            shape(),
        ];
        let content = check_raster(&scene);
        assert!(!content.contains(" re"));
        let fonts = default_provider();
        let assets = BytesAssetProvider::new();
        assert_eq!(
            super::render_pdf(&scene, &fonts, &assets),
            super::render_pdf(&scene, &fonts, &assets)
        );
    }
}

#[test]
fn fractional_page_uses_integer_raster_dimensions() {
    for size in [20.2, 20.8] {
        let mut scene = Scene::new(size, size);
        scene.commands = vec![
            SceneCommand::PushLayer {
                opacity: 0.5,
                blend_mode: None,
            },
            SceneCommand::FillRect {
                x: 0.0,
                y: 0.0,
                w: 30.0,
                h: 30.0,
                paint: Paint::solid(Color::srgb(200, 30, 60, 255)),
            },
            SceneCommand::PopLayer,
        ];
        check_raster(&scene);
    }
}

#[test]
fn identity_layers_and_empty_filters_keep_native_commands() {
    let mut scene = Scene::new(24.0, 24.0);
    scene.commands = vec![
        SceneCommand::PushLayer {
            opacity: 1.0,
            blend_mode: None,
        },
        SceneCommand::BeginFilter { filters: vec![] },
        shape(),
        SceneCommand::EndFilter,
        SceneCommand::PopLayer,
    ];
    let fonts = default_provider();
    let plan = build_plan(&Default::default(), &fonts, true);
    let (content, resources) = translate(&scene, &fonts, &BytesAssetProvider::new(), &plan);
    assert!(resources.images.is_empty());
    assert!(
        String::from_utf8(content.finish().into_vec())
            .unwrap()
            .contains("-4 2 10 8 re")
    );
}

#[test]
fn zero_blur_and_empty_shadow_keep_raster_capture() {
    for (begin, end) in [
        (
            SceneCommand::BeginBlur { radius: 0.0 },
            SceneCommand::EndBlur,
        ),
        (
            SceneCommand::BeginShadow { shadows: vec![] },
            SceneCommand::EndShadow,
        ),
    ] {
        let mut scene = Scene::new(24.0, 24.0);
        scene.commands = vec![begin, shape(), end];
        check_raster(&scene);
    }
}

#[test]
fn unmatched_close_and_unfinished_structure_retain_draws() {
    for commands in [
        vec![SceneCommand::PopLayer, shape()],
        vec![
            SceneCommand::PushLayer {
                opacity: 0.5,
                blend_mode: None,
            },
            SceneCommand::BeginBlur { radius: 1.0 },
            shape(),
            SceneCommand::EndBlur,
        ],
    ] {
        let mut scene = Scene::new(24.0, 24.0);
        scene.commands = commands;
        let fonts = default_provider();
        let plan = build_plan(&Default::default(), &fonts, true);
        let (content, resources) = translate(&scene, &fonts, &BytesAssetProvider::new(), &plan);
        assert!(resources.images.is_empty());
        assert!(
            String::from_utf8(content.finish().into_vec())
                .unwrap()
                .contains("-4 2 10 8 re")
        );
    }
}

#[test]
fn text_and_links_outside_local_capture_remain_native() {
    use zenith_core::FontProvider;
    use zenith_scene::SceneGlyph;
    let fonts = default_provider();
    let faces = fonts.all_faces();
    let font = faces.first().unwrap();
    let data = fonts.by_id(&font.id).unwrap();
    let face = ttf_parser::Face::parse(&data.bytes, data.index).unwrap();
    let glyph = SceneCommand::DrawGlyphRun {
        x: 10.0,
        y: 20.0,
        font_id: font.id.clone(),
        font_size: 12.0,
        color: Color::srgb(0, 0, 0, 255),
        stroke_color: None,
        stroke_width: None,
        link: Some("https://example.com".into()),
        selectable: true,
        source_node_id: None,
        glyphs: vec![SceneGlyph {
            glyph_id: face.glyph_index('A').unwrap().0,
            dx: 0.0,
            dy: 0.0,
            text: "A".into(),
        }],
    };
    let mut scene = Scene::new(48.0, 48.0);
    scene.commands = vec![
        glyph.clone(),
        SceneCommand::BeginBlur { radius: 1.0 },
        shape(),
        glyph.clone(),
        SceneCommand::EndBlur,
        glyph,
    ];
    let usage = super::font::collect_usage(std::slice::from_ref(&scene));
    let plan = build_plan(&usage, &fonts, true);
    let (content, resources) = translate(&scene, &fonts, &BytesAssetProvider::new(), &plan);
    assert_eq!(resources.images.len(), 1);
    assert_eq!(resources.links.len(), 2);
    let content = String::from_utf8(content.finish().into_vec()).unwrap();
    assert_eq!(content.matches("BT").count(), 2);
    assert_eq!(content.matches("/im0 Do").count(), 1);
}

#[test]
fn raster_error_retains_region_draws() {
    let mut scene = Scene::new(-24.0, 24.0);
    scene.commands = vec![
        SceneCommand::BeginBlur { radius: 1.0 },
        shape(),
        SceneCommand::EndBlur,
    ];
    let fonts = default_provider();
    let plan = build_plan(&Default::default(), &fonts, true);
    let (content, resources) = translate(&scene, &fonts, &BytesAssetProvider::new(), &plan);
    assert!(resources.images.is_empty());
    assert!(
        String::from_utf8(content.finish().into_vec())
            .unwrap()
            .contains("-4 2 10 8 re")
    );
}
