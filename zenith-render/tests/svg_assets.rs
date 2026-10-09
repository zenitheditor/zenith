mod common;
mod svg_support;

use std::sync::Arc;

use common::pixel;
#[path = "common/no_assets.rs"]
mod no_assets;
use no_assets::no_assets;
#[path = "common/red.rs"]
mod red;
use red::red;
#[path = "common/three_column_rgb_png.rs"]
mod three_column_rgb_png;
use svg_support::{assert_pixels_close, data_url_bytes, embedded_png, rasterize};
use three_column_rgb_png::three_column_rgb_png;
use zenith_core::{AssetKind, BytesAssetProvider, BytesFontProvider, FontStyle, default_provider};
use zenith_layout::{
    FontFaceStore, RustybuzzEngine, ShapeRequest, TextDirection, TextLayoutEngine,
};
use zenith_render::render_image;
use zenith_render::{render_svg, render_svg_with};
use zenith_scene::ir::SvgStyle;
use zenith_scene::{Color, FitMode, ImageClip, Scene, SceneCommand, SceneGlyph, SrcRect};

fn text_scene(text: &str, stroke: bool) -> Scene {
    let fonts = default_provider();
    let families = vec!["Noto Sans".to_string()];
    let request = ShapeRequest {
        text,
        families: &families,
        weight: 400,
        style: FontStyle::Normal,
        font_size: 32.0,
        direction: TextDirection::Ltr,
        features: &[],
        kerning_pairs: &[],
        letter_spacing_px: 0.0,
    };
    let run = RustybuzzEngine::new(&FontFaceStore::new(&fonts))
        .shape(&request, &fonts)
        .expect("shape bundled font");
    let mut scene = Scene::new(160.0, 60.0);
    scene.commands.push(SceneCommand::DrawGlyphRun {
        x: 12.0,
        y: 42.0,
        font_id: run.font_id,
        font_size: 32.0,
        color: Color::srgb(0, 0, 200, 255),
        stroke_color: stroke.then_some(red()),
        stroke_width: stroke.then_some(2.0),
        link: None,
        selectable: true,
        source_node_id: None,
        glyphs: run
            .glyphs
            .iter()
            .map(|glyph| SceneGlyph {
                glyph_id: glyph.glyph_id,
                dx: glyph.x + 3.0,
                dy: glyph.y + 2.0,
                text: String::new(),
            })
            .collect(),
    });
    scene
}

#[test]
fn positioned_glyph_outlines_and_strokes_match_reference() {
    for stroke in [false, true] {
        let scene = text_scene("A Vg", stroke);
        let fonts = default_provider();
        let output = render_svg_with(&scene, &fonts, &no_assets()).expect("glyph SVG");
        assert!(output.rasterized_regions.is_empty());
        let text = std::str::from_utf8(&output.bytes).expect("SVG text");
        assert!(text.contains("<path"));
        assert!(!text.contains("<text"));
        assert!(!text.contains("<image"));
        assert_pixels_close(
            &rasterize(&output.bytes),
            &render_image(&scene, &fonts, &no_assets()).expect("text reference"),
            0.7,
        );
        assert_eq!(
            output.bytes,
            render_svg(&scene, &fonts, &no_assets()).expect("repeat text SVG")
        );
    }
}

#[test]
fn overlapping_glyphs_keep_per_glyph_fill_and_stroke_order() {
    let mut scene = text_scene("AA", true);
    if let SceneCommand::DrawGlyphRun {
        stroke_width,
        glyphs,
        ..
    } = &mut scene.commands[0]
    {
        *stroke_width = Some(8.0);
        glyphs[1].dx = glyphs[0].dx + 8.0;
    }
    let fonts = default_provider();
    let svg = render_svg(&scene, &fonts, &no_assets()).expect("overlapping glyph SVG");
    assert_pixels_close(
        &rasterize(&svg),
        &render_image(&scene, &fonts, &no_assets()).expect("overlapping glyph reference"),
        0.7,
    );
}

#[test]
fn whitespace_glyphs_draw_no_ink_and_missing_fonts_return_errors() {
    let fonts = default_provider();
    let scene = text_scene("   ", false);
    let svg = render_svg(&scene, &fonts, &no_assets()).expect("space glyph SVG");
    assert!(
        rasterize(&svg)
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| pixel[3] == 0)
    );
    let mut missing = text_scene("A", false);
    if let SceneCommand::DrawGlyphRun { font_id, .. } = &mut missing.commands[0] {
        *font_id = "missing-font".to_string();
    }
    assert!(render_svg(&missing, &fonts, &no_assets()).is_err());
}

#[test]
fn invalid_glyph_ids_fonts_and_positions_return_errors() {
    let fonts = default_provider();
    for invalid_position in [false, true] {
        let mut scene = text_scene("A", false);
        if let SceneCommand::DrawGlyphRun { glyphs, .. } = &mut scene.commands[0] {
            if invalid_position {
                glyphs[0].dx = f32::NAN;
            } else {
                glyphs[0].glyph_id = u16::MAX;
            }
        }
        assert!(render_svg(&scene, &fonts, &no_assets()).is_err());
    }
    let mut invalid_fonts = BytesFontProvider::new();
    let font_id = invalid_fonts.register(
        "Invalid",
        400,
        FontStyle::Normal,
        Arc::from(&b"invalid font bytes"[..]),
        0,
        zenith_core::FontSource::Bundled,
    );
    let mut scene = text_scene("A", false);
    if let SceneCommand::DrawGlyphRun { font_id: id, .. } = &mut scene.commands[0] {
        *id = font_id;
    }
    assert!(render_svg(&scene, &invalid_fonts, &no_assets()).is_err());
}

fn image_scene(fit: FitMode, crop: Option<SrcRect>, clip: Option<ImageClip>) -> Scene {
    let mut scene = Scene::new(48.0, 40.0);
    scene.commands = vec![
        SceneCommand::PushClip {
            x: 8.0,
            y: 8.0,
            w: 32.0,
            h: 24.0,
        },
        SceneCommand::DrawImage {
            x: 8.0,
            y: 8.0,
            w: 32.0,
            h: 24.0,
            asset_id: "asset.image".to_string(),
            fit,
            pos_x: 25.0,
            pos_y: 75.0,
            opacity: 0.8,
            clip_shape: clip,
            src_rect: crop,
            svg_style: None,
        },
        SceneCommand::PopClip,
    ];
    scene
}

#[test]
fn raster_image_fits_crops_and_shape_clips_are_self_contained() {
    let mut assets = BytesAssetProvider::new();
    assets.register("asset.image", AssetKind::Image, three_column_rgb_png());
    let fonts = default_provider();
    for fit in [
        FitMode::Contain,
        FitMode::Cover,
        FitMode::Stretch,
        FitMode::None,
    ] {
        for clip in [
            None,
            Some(ImageClip::Ellipse),
            Some(ImageClip::RoundedRect { radius: 6.0 }),
        ] {
            let scene = image_scene(fit, None, clip);
            let output = render_svg_with(&scene, &fonts, &assets).expect("image SVG");
            assert!(output.rasterized_regions.is_empty());
            let text = std::str::from_utf8(&output.bytes).expect("SVG text");
            assert!(text.contains("data:image/png;base64,"));
            assert_pixels_close(
                &rasterize(&output.bytes),
                &render_image(&scene, &fonts, &assets).expect("image reference"),
                3.0,
            );
        }
    }
    let scene = image_scene(
        FitMode::Stretch,
        Some(SrcRect {
            x: 2.0,
            y: 0.0,
            w: 1.0,
            h: 3.0,
        }),
        None,
    );
    let svg = render_svg(&scene, &fonts, &assets).expect("cropped SVG");
    let image = embedded_png(&svg);
    assert_eq!((image.width, image.height), (1, 3));
    assert!(
        image
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| *pixel == [0, 0, 255, 255])
    );
    assert_pixels_close(
        &rasterize(&svg),
        &render_image(&scene, &fonts, &assets).expect("crop reference"),
        1.0,
    );
}

#[test]
fn styled_svg_asset_embeds_outlined_text_and_removes_active_content() {
    const ASSET: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="40">
      <script>alert('active')</script><foreignObject width="80" height="40">active</foreignObject>
      <rect width="10" height="10" fill="currentColor"/>
      <text x="12" y="30" font-size="24" fill="currentColor" stroke="#0000ff" stroke-width="1">Hi</text>
    </svg>"##;
    let mut assets = BytesAssetProvider::new();
    assets.register("asset.image", AssetKind::Svg, Arc::from(ASSET));
    let mut scene = image_scene(FitMode::Stretch, None, None);
    if let SceneCommand::DrawImage { svg_style, .. } = &mut scene.commands[1] {
        *svg_style = Some(SvgStyle {
            fill: Some(red()),
            stroke: None,
            stroke_width: None,
        });
    }
    let fonts = default_provider();
    let output = render_svg_with(&scene, &fonts, &assets).expect("styled SVG asset");
    assert!(output.rasterized_regions.is_empty());
    let embedded = data_url_bytes(&output.bytes, "image/svg+xml");
    let text = std::str::from_utf8(&embedded).expect("SVG asset text");
    assert!(!text.contains("<script"));
    assert!(!text.contains("foreignObject"));
    assert!(!text.contains("<text"));
    assert_pixels_close(
        &rasterize(&output.bytes),
        &render_image(&scene, &fonts, &assets).expect("SVG asset reference"),
        2.0,
    );
}

#[test]
fn svg_text_with_resolved_and_unresolved_spans_returns_error() {
    const ASSET: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="160" height="40">
      <text x="0" y="30" font-size="24"><tspan font-family="Noto Serif">Known</tspan><tspan font-family="Missing">Absent</tspan></text>
    </svg>"#;
    let mut fonts = BytesFontProvider::new();
    fonts.register(
        "Noto Serif",
        400,
        FontStyle::Normal,
        Arc::from(&include_bytes!("../../zenith-core/assets/fonts/NotoSerif-Regular.ttf")[..]),
        0,
        zenith_core::FontSource::Project,
    );
    let mut assets = BytesAssetProvider::new();
    let scene = image_scene(FitMode::Stretch, None, None);
    let resolved = std::str::from_utf8(ASSET)
        .expect("SVG fixture")
        .replace("Missing", "Noto Serif");
    assets.register(
        "asset.image",
        AssetKind::Svg,
        Arc::from(resolved.as_bytes()),
    );
    assert!(render_svg(&scene, &fonts, &assets).is_ok());
    assets.register("asset.image", AssetKind::Svg, Arc::from(ASSET));
    assert!(render_svg(&scene, &fonts, &assets).is_err());
}

#[test]
fn pre_resolved_svg_asset_stays_self_contained() {
    let mut assets = BytesAssetProvider::new();
    assets.register("asset.vector", AssetKind::Svg,
        Arc::from(&br##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><rect width="8" height="8" fill="#ff0000"/></svg>"##[..]));
    let mut scene = Scene::new(24.0, 24.0);
    scene.commands.push(SceneCommand::DrawSvgAsset {
        x: 4.0,
        y: 4.0,
        w: 16.0,
        h: 16.0,
        asset: "asset.vector".to_string(),
    });
    let output =
        render_svg_with(&scene, &default_provider(), &assets).expect("pre-resolved SVG asset");
    assert!(output.rasterized_regions.is_empty());
    let nested = data_url_bytes(&output.bytes, "image/svg+xml");
    assert!(
        std::str::from_utf8(&nested)
            .expect("normalized SVG")
            .contains("<path")
    );
    let image = rasterize(&output.bytes);
    assert_eq!(pixel(&image.rgba, image.width, 12, 12), (255, 0, 0, 255));
    assert_eq!(pixel(&image.rgba, image.width, 0, 0), (0, 0, 0, 0));
}

#[test]
fn svg_asset_external_references_return_errors() {
    let scene = image_scene(FitMode::Stretch, None, None);
    for href in [
        "../../examples/assets/swatch.png",
        "file:///etc/passwd",
        "https://example.invalid/tracking.png",
    ] {
        let asset = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="20" height="20"><image width="20" height="20" xlink:href="{href}"/></svg>"#
        );
        let mut assets = BytesAssetProvider::new();
        assets.register("asset.image", AssetKind::Svg, Arc::from(asset.as_bytes()));
        assert!(
            render_svg(&scene, &default_provider(), &assets).is_err(),
            "external reference {href}"
        );
    }
}

#[test]
fn missing_and_malformed_image_assets_return_errors() {
    let scene = image_scene(FitMode::Stretch, None, None);
    let fonts = default_provider();
    assert!(render_svg(&scene, &fonts, &no_assets()).is_err());
    for kind in [AssetKind::Image, AssetKind::Svg] {
        let mut assets = BytesAssetProvider::new();
        assets.register("asset.image", kind, Arc::from(&b"malformed image"[..]));
        assert!(render_svg(&scene, &fonts, &assets).is_err());
    }
}

#[test]
fn svg_asset_malformed_embedded_images_return_errors() {
    let fonts = default_provider();
    let scene = image_scene(FitMode::Stretch, None, None);
    for mime in ["image/png", "image/jpeg", "image/gif"] {
        let asset = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="20" height="20"><image width="20" height="20" xlink:href="data:{mime};base64,YmFk"/></svg>"#
        );
        let mut assets = BytesAssetProvider::new();
        assets.register("asset.image", AssetKind::Svg, Arc::from(asset.as_bytes()));
        assert!(
            render_svg(&scene, &fonts, &assets).is_err(),
            "malformed embedded {mime}"
        );
    }
}

#[test]
fn glyph_links_escape_attributes_and_reject_active_schemes() {
    let fonts = default_provider();
    let mut scene = text_scene("A", false);
    let links = [
        "https://example.com/?a=1&b=\"two\"",
        "javascript:alert(1)",
        "data:text/html,active",
    ];
    for link_value in links {
        if let SceneCommand::DrawGlyphRun { link, .. } = &mut scene.commands[0] {
            *link = Some(link_value.to_string());
        }
        if link_value.starts_with("https:") {
            let svg = render_svg(&scene, &fonts, &no_assets()).expect("linked text SVG");
            let text = std::str::from_utf8(&svg).expect("SVG text");
            rasterize(&svg);
            assert!(text.contains("&amp;"));
            assert!(text.contains("&quot;"));
        } else {
            assert!(render_svg(&scene, &fonts, &no_assets()).is_err());
        }
    }
}
