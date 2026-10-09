mod common;
mod pdf_bitmap_support;
mod svg_font_support;
mod svg_support;

use std::sync::Arc;

use common::pixel;
#[path = "common/no_assets.rs"]
mod no_assets;
use no_assets::no_assets;
#[path = "common/red.rs"]
mod red;
use red::red;
#[path = "common/swatch_png.rs"]
mod swatch_png;
use svg_font_support::{bitmap_font, font_collection};
use svg_support::{assert_pixels_close, data_url_bytes, embedded_png, rasterize};
use swatch_png::SWATCH_PNG;
use zenith_core::{BytesFontProvider, FontStyle};
use zenith_render::render_image;
use zenith_render::{SvgRasterizationReason, render_svg_with};
use zenith_scene::{Paint, Scene, SceneCommand, SceneGlyph};

fn glyph_scene(font_id: String, glyph_id: u16) -> Scene {
    let mut scene = Scene::new(40.0, 40.0);
    scene.commands.push(SceneCommand::DrawGlyphRun {
        x: 8.0,
        y: 32.0,
        font_id,
        font_size: 32.0,
        color: red(),
        stroke_color: None,
        stroke_width: None,
        link: None,
        selectable: true,
        source_node_id: None,
        glyphs: vec![SceneGlyph {
            glyph_id,
            dx: 0.0,
            dy: 0.0,
            text: String::new(),
        }],
    });
    scene
}

#[test]
fn collection_face_index_selects_the_registered_outline() {
    let bytes = font_collection();
    let glyph = ttf_parser::Face::parse(&bytes, 1)
        .expect("collection face")
        .glyph_index('A')
        .expect("A glyph")
        .0;
    let mut fonts = BytesFontProvider::new();
    let font_id = fonts.register(
        "Collection",
        700,
        FontStyle::Normal,
        Arc::from(bytes),
        1,
        zenith_core::FontSource::Project,
    );
    let scene = glyph_scene(font_id, glyph);
    let output = render_svg_with(&scene, &fonts, &no_assets()).expect("collection SVG");
    assert!(output.rasterized_regions.is_empty());
    let svg = std::str::from_utf8(&output.bytes).expect("SVG text");
    assert!(svg.contains("<path"));
    let image = rasterize(&output.bytes);
    assert!(
        image
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| pixel[3] != 0)
    );
    assert_pixels_close(
        &image,
        &render_image(&scene, &fonts, &no_assets()).expect("collection reference"),
        0.7,
    );
    let regular_id = fonts.register(
        "Collection regular",
        400,
        FontStyle::Normal,
        Arc::from(font_collection()),
        0,
        zenith_core::FontSource::Project,
    );
    let regular = render_svg_with(&glyph_scene(regular_id, glyph), &fonts, &no_assets())
        .expect("regular SVG");
    assert_ne!(image.rgba, rasterize(&regular.bytes).rgba);
}

#[test]
fn bitmap_only_glyph_embeds_exact_reference_pixels_and_reports_fallback() {
    check_bitmap_fallback(' ');
}

#[test]
fn glyph_with_outline_and_embedded_png_preserves_bitmap_pixels() {
    check_bitmap_fallback('A');
}

#[test]
fn fractional_page_keeps_raster_fallback_at_pixel_dimensions() {
    let (bytes, glyph) = bitmap_font(SWATCH_PNG, ' ');
    let mut fonts = BytesFontProvider::new();
    let font_id = fonts.register(
        "Bitmap",
        400,
        FontStyle::Normal,
        Arc::from(bytes),
        0,
        zenith_core::FontSource::Project,
    );
    let mut scene = glyph_scene(font_id, glyph);
    scene.width = 10.4;
    scene.height = 10.4;
    if let SceneCommand::DrawGlyphRun { x, y, .. } = &mut scene.commands[0] {
        *x = 4.0;
        *y = 6.0;
    }
    scene.commands.push(SceneCommand::FillRect {
        x: 8.0,
        y: 8.0,
        w: 1.0,
        h: 1.0,
        paint: Paint::solid(red()),
    });
    let output = render_svg_with(&scene, &fonts, &no_assets()).expect("fractional SVG");
    assert_eq!(output.rasterized_regions.len(), 1);
    let svg = std::str::from_utf8(&output.bytes).expect("SVG text");
    let image_start = svg.find("<image ").expect("fallback image");
    let image_end = image_start + svg[image_start..].find("/>").expect("image closing tag");
    let image_tag = &svg[image_start..image_end];
    assert!(image_tag.contains("width=\"10\""));
    assert!(image_tag.contains("height=\"10\""));
    let embedded = embedded_png(&output.bytes);
    assert_eq!((embedded.width, embedded.height), (10, 10));
    let image = rasterize(&output.bytes);
    let reference = render_image(&scene, &fonts, &no_assets()).expect("fractional reference");
    for y in 0..reference.height {
        for x in 0..reference.width {
            assert_eq!(
                pixel(&image.rgba, image.width, x, y),
                pixel(&reference.rgba, reference.width, x, y),
                "pixel ({x}, {y})"
            );
        }
    }
}

fn check_bitmap_fallback(character: char) {
    let (bytes, glyph) = bitmap_font(SWATCH_PNG, character);
    let face = ttf_parser::Face::parse(&bytes, 0).expect("bitmap fixture");
    assert_eq!(
        face.glyph_bounding_box(ttf_parser::GlyphId(glyph))
            .is_some(),
        character == 'A'
    );
    assert!(
        face.glyph_raster_image(ttf_parser::GlyphId(glyph), 32)
            .is_some()
    );
    let mut fonts = BytesFontProvider::new();
    let font_id = fonts.register(
        "Bitmap",
        400,
        FontStyle::Normal,
        Arc::from(bytes),
        0,
        zenith_core::FontSource::Project,
    );
    let scene = glyph_scene(font_id, glyph);
    let output = render_svg_with(&scene, &fonts, &no_assets()).expect("bitmap SVG");
    assert_eq!(output.rasterized_regions.len(), 1);
    let region = &output.rasterized_regions[0];
    assert_eq!((region.command_start, region.command_end), (0, 1));
    assert_eq!(region.reason, SvgRasterizationReason::BitmapGlyph);
    assert!(data_url_bytes(&output.bytes, "image/png").starts_with(b"\x89PNG"));
    let reference = render_image(&scene, &fonts, &no_assets()).expect("bitmap reference");
    assert!(
        reference
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| pixel[3] != 0)
    );
    assert_eq!(embedded_png(&output.bytes).rgba, reference.rgba);
    assert_pixels_close(&rasterize(&output.bytes), &reference, 0.0);
}

#[test]
fn pdf_bitmap_glyphs_capture_reference_pixels() {
    for character in [' ', 'A'] {
        let (bytes, glyph) = bitmap_font(SWATCH_PNG, character);
        let mut fonts = BytesFontProvider::new();
        let id = fonts.register(
            "Bitmap",
            400,
            FontStyle::Normal,
            Arc::from(bytes),
            0,
            zenith_core::FontSource::Project,
        );
        let glyph_scene = glyph_scene(id, glyph);
        for transformed in [false, true] {
            let mut scene = glyph_scene.clone();
            if let SceneCommand::DrawGlyphRun { link, .. } = &mut scene.commands[0] {
                *link = Some("custom:pdf-link".into());
            }
            if transformed {
                scene.commands.insert(
                    0,
                    SceneCommand::PushScaleTranslate {
                        sx: 0.8,
                        sy: 0.8,
                        tx: 3.0,
                        ty: 1.0,
                    },
                );
                scene.commands.insert(
                    1,
                    SceneCommand::PushClip {
                        x: 2.0,
                        y: 0.0,
                        w: 24.0,
                        h: 40.0,
                    },
                );
                scene.commands.push(SceneCommand::PopClip);
                scene.commands.push(SceneCommand::PopTransform);
            }
            let pdf = zenith_render::render_pdf(&scene, &fonts, &no_assets());
            let reference = render_image(&scene, &fonts, &no_assets()).unwrap();
            pdf_bitmap_support::assert_image_planes(&pdf, &reference);
            assert_eq!(pdf, zenith_render::render_pdf(&scene, &fonts, &no_assets()));
        }
    }
}

#[test]
fn pdf_malformed_bitmap_retains_available_outline() {
    let mut png = SWATCH_PNG.to_vec();
    png.truncate(40);
    assert!(tiny_skia::Pixmap::decode_png(&png).is_err());
    let (bytes, glyph) = bitmap_font(&png, 'A');
    let face = ttf_parser::Face::parse(&bytes, 0).unwrap();
    assert!(
        face.glyph_raster_image(ttf_parser::GlyphId(glyph), 32)
            .is_some()
    );
    let mut fonts = BytesFontProvider::new();
    let id = fonts.register(
        "Bitmap",
        400,
        FontStyle::Normal,
        Arc::from(bytes),
        0,
        zenith_core::FontSource::Project,
    );
    let mut scene = glyph_scene(id, glyph);
    if let SceneCommand::DrawGlyphRun { selectable, .. } = &mut scene.commands[0] {
        *selectable = false;
    }
    let pdf = zenith_render::render_pdf(&scene, &fonts, &no_assets());
    let text = String::from_utf8_lossy(&pdf);
    assert!(!text.contains("/Subtype /Image"));
    assert!(text.contains(" m\n"));
    assert!(
        render_image(&scene, &fonts, &no_assets())
            .unwrap()
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| pixel[3] > 0)
    );
}
