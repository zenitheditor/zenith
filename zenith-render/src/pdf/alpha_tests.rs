//! Alpha resource reference coverage for PDF content and dictionaries.

use zenith_core::{AssetKind, BytesAssetProvider, FontProvider, default_provider};
use zenith_scene::{Color, FitMode, Paint, Scene, SceneCommand, SceneGlyph};

use super::{content::PageResources, render_pdf};

fn shape(alpha: u8) -> SceneCommand {
    SceneCommand::FillRect {
        x: 10.0,
        y: 10.0,
        w: 20.0,
        h: 20.0,
        paint: Paint::solid(Color::srgb(10, 20, 30, alpha)),
    }
}

fn glyph(alpha: u8, selectable: bool) -> SceneCommand {
    let fonts = default_provider();
    let faces = fonts.all_faces();
    let font = faces.first().unwrap();
    let data = fonts.by_id(&font.id).unwrap();
    let face = ttf_parser::Face::parse(&data.bytes, data.index).unwrap();
    SceneCommand::DrawGlyphRun {
        x: 10.0,
        y: 40.0,
        font_id: font.id.clone(),
        font_size: 24.0,
        color: Color::srgb(10, 20, 30, alpha),
        stroke_color: None,
        stroke_width: None,
        link: None,
        selectable,
        source_node_id: None,
        glyphs: vec![SceneGlyph {
            glyph_id: face.glyph_index('A').unwrap().0,
            dx: 0.0,
            dy: 0.0,
            text: "A".into(),
        }],
    }
}

fn image(alpha: u8) -> SceneCommand {
    SceneCommand::DrawImage {
        x: 10.0,
        y: 10.0,
        w: 20.0,
        h: 20.0,
        asset_id: "asset.swatch".into(),
        fit: FitMode::Stretch,
        pos_x: 50.0,
        pos_y: 50.0,
        opacity: f64::from(alpha) / 255.0,
        clip_shape: None,
        src_rect: None,
        svg_style: None,
    }
}

fn render(commands: Vec<SceneCommand>) -> Vec<u8> {
    let mut scene = Scene::new(100.0, 80.0);
    scene.commands = commands;
    let mut assets = BytesAssetProvider::new();
    assets.register(
        "asset.swatch",
        AssetKind::Image,
        std::sync::Arc::from(include_bytes!("../../../examples/assets/swatch.png").as_slice()),
    );
    render_pdf(&scene, &default_provider(), &assets)
}

fn check_alpha_references(bytes: &[u8], expected: &[(usize, u8)]) {
    let pdf = String::from_utf8_lossy(bytes);
    let dictionary = pdf
        .split("/ExtGState <<")
        .nth(1)
        .unwrap()
        .split(">>")
        .next()
        .unwrap();
    let applied: Vec<_> = pdf
        .lines()
        .filter_map(|line| line.strip_prefix("/ga")?.strip_suffix(" gs"))
        .map(|index| index.parse::<usize>().unwrap())
        .collect();
    assert_eq!(
        applied,
        expected.iter().map(|(index, _)| *index).collect::<Vec<_>>()
    );
    for &(index, alpha) in expected {
        let binding = format!("/ga{index} ");
        let reference = dictionary
            .split(&binding)
            .nth(1)
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap();
        let header = format!("\n{reference} 0 obj\n");
        let object = pdf
            .split(&header)
            .nth(1)
            .unwrap()
            .split("endobj")
            .next()
            .unwrap();
        for key in ["/ca ", "/CA "] {
            let factor = object
                .split(key)
                .nth(1)
                .unwrap()
                .split_whitespace()
                .next()
                .unwrap()
                .parse::<f32>()
                .unwrap();
            assert_eq!(factor, f32::from(alpha) / 255.0, "ga{index} {key}");
        }
    }
}

#[test]
fn repeated_alpha_retains_first_discovery_index() {
    let mut resources = PageResources::default();
    let indices: Vec<_> = [192, 64, 192, 128, 64]
        .into_iter()
        .map(|alpha| resources.intern_alpha(alpha))
        .collect();
    assert_eq!(indices, [0, 1, 0, 2, 1]);
    assert_eq!(resources.alphas, [192, 64, 128]);
}

#[test]
fn descending_shape_alpha_resolves_each_graphics_state() {
    let bytes = render([192, 64, 192, 128, 64].into_iter().map(shape).collect());
    check_alpha_references(&bytes, &[(0, 192), (1, 64), (0, 192), (2, 128), (1, 64)]);
}

#[test]
fn mixed_draw_alpha_resolves_each_graphics_state() {
    for selectable in [false, true] {
        let commands = vec![
            shape(192),
            glyph(64, selectable),
            image(128),
            shape(64),
            glyph(192, selectable),
        ];
        let bytes = render(commands.clone());
        check_alpha_references(&bytes, &[(0, 192), (1, 64), (2, 128), (1, 64), (0, 192)]);
        assert_eq!(bytes, render(commands));
    }
}

#[test]
fn sorted_discovery_retains_resource_names_and_pdf_bytes() {
    let commands: Vec<_> = [64, 128, 192, 64].into_iter().map(shape).collect();
    let bytes = render(commands.clone());
    check_alpha_references(&bytes, &[(0, 64), (1, 128), (2, 192), (0, 64)]);
    // FNV-1a fingerprint of the complete PDF before discovery-order interning.
    let fingerprint = bytes.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    });
    assert_eq!(bytes.len(), 1182);
    assert_eq!(fingerprint, 0xa3c6c27a46316ea2);
    assert_eq!(bytes, render(commands));
}
