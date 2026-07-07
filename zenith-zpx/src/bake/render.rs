use zenith_raster::{
    Adjustment as RasterAdjustment, LinearRgba, Surface, blend_pixel, encode_linear_to_srgb_u8,
};

use crate::error::ZpxError;
use crate::manifest::serialize_manifest;
use crate::model::{Adjustment, Canvas, ContentHash, Layer, LayerSource, ZpxDoc};
use crate::paint::render_program;

/// Result of baking a ZPX document to deterministic PNG bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BakeOutput {
    pub png: Vec<u8>,
    pub png_sha256: ContentHash,
    pub provenance: BakeProvenance,
}

/// Minimal provenance for an in-memory ZPX bake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BakeProvenance {
    pub source_sha256: ContentHash,
}

/// Bake supported ZPX layer trees into deterministic PNG bytes.
pub fn bake(doc: &ZpxDoc) -> Result<BakeOutput, ZpxError> {
    let manifest = serialize_manifest(doc);
    let source_sha256 = ContentHash::from_bytes(manifest.as_bytes());
    let surface = render_document_surface(doc)?;
    let rgba = surface_to_straight_srgb_rgba8(&surface)?;
    let png = encode_png(surface.width(), surface.height(), &rgba)?;
    let png_sha256 = ContentHash::from_bytes(&png);

    Ok(BakeOutput {
        png,
        png_sha256,
        provenance: BakeProvenance { source_sha256 },
    })
}

fn render_document_surface(doc: &ZpxDoc) -> Result<Surface, ZpxError> {
    let base = Surface::new(doc.canvas.width_px, doc.canvas.height_px).map_err(raster_error)?;
    render_layers_on(base, &doc.canvas, &doc.layers)
}

fn render_layers_on(
    mut target: Surface,
    canvas: &Canvas,
    layers: &[Layer],
) -> Result<Surface, ZpxError> {
    let mut previous_boundary = None;

    for layer in layers {
        if !layer.visible {
            continue;
        }

        let opacity = validate_opacity(layer.opacity)?;
        if opacity == 0.0 {
            continue;
        }

        reject_blob_mask(layer)?;
        let source = render_layer_source(&target, canvas, &layer.source)?;
        let boundary =
            layer_boundary(&source, opacity, layer.clipping, previous_boundary.as_ref())?;
        composite_boundary(&mut target, &boundary, layer)?;
        previous_boundary = Some(boundary);
    }

    Ok(target)
}

fn render_layer_source(
    target: &Surface,
    canvas: &Canvas,
    source: &LayerSource,
) -> Result<Surface, ZpxError> {
    match source {
        LayerSource::Program(program) => render_program(program, canvas),
        LayerSource::Adjustment(Adjustment::GradientMap { stops }) => {
            validate_surface_dimensions(target, canvas)?;
            RasterAdjustment::gradient_map(stops)
                .apply_to(target)
                .map_err(raster_error)
        }
        LayerSource::Group(layers) => {
            let base = Surface::new(canvas.width_px, canvas.height_px).map_err(raster_error)?;
            render_layers_on(base, canvas, layers)
        }
        LayerSource::Buffer(blob) => Err(ZpxError::new(format!(
            "cannot bake buffer layer without a blob provider: {}",
            blob.hash.as_str()
        ))),
    }
}

fn layer_boundary(
    source: &Surface,
    opacity: f32,
    clipping: bool,
    previous_boundary: Option<&Surface>,
) -> Result<Surface, ZpxError> {
    if let Some(previous_boundary) = previous_boundary {
        validate_surface_dimensions_for_source(source, previous_boundary)?;
    }

    let mut boundary = source.clone();
    for y in 0..boundary.height() {
        for x in 0..boundary.width() {
            let mut coverage = opacity;
            if clipping {
                let clip_coverage = match previous_boundary {
                    Some(surface) => surface
                        .get(x, y)
                        .ok_or_else(|| ZpxError::new("clipping boundary pixel out of bounds"))?
                        .a(),
                    None => 0.0,
                };
                coverage *= clip_coverage;
            }
            let pixel = boundary
                .get(x, y)
                .ok_or_else(|| ZpxError::new("layer boundary pixel out of bounds"))?;
            boundary
                .set(x, y, scale_pixel(pixel, coverage)?)
                .map_err(raster_error)?;
        }
    }
    Ok(boundary)
}

fn composite_boundary(
    target: &mut Surface,
    boundary: &Surface,
    layer: &Layer,
) -> Result<(), ZpxError> {
    validate_surface_dimensions_for_source(target, boundary)?;
    for y in 0..target.height() {
        for x in 0..target.width() {
            let backdrop = target
                .get(x, y)
                .ok_or_else(|| ZpxError::new("bake target pixel out of bounds"))?;
            let source = boundary
                .get(x, y)
                .ok_or_else(|| ZpxError::new("layer boundary pixel out of bounds"))?;
            let blended = blend_pixel(layer.blend_mode, backdrop, source).map_err(raster_error)?;
            target.set(x, y, blended).map_err(raster_error)?;
        }
    }
    Ok(())
}

fn reject_blob_mask(layer: &Layer) -> Result<(), ZpxError> {
    if let Some(mask) = &layer.mask {
        return Err(ZpxError::new(format!(
            "cannot bake blob mask without a blob provider: {}",
            mask.blob.hash.as_str()
        )));
    }
    Ok(())
}

fn validate_opacity(opacity: f64) -> Result<f32, ZpxError> {
    if opacity.is_finite() && (0.0..=1.0).contains(&opacity) {
        Ok(opacity as f32)
    } else {
        Err(ZpxError::new("layer opacity must be finite and in 0..=1"))
    }
}

fn validate_surface_dimensions(surface: &Surface, canvas: &Canvas) -> Result<(), ZpxError> {
    if surface.width() == canvas.width_px && surface.height() == canvas.height_px {
        Ok(())
    } else {
        Err(ZpxError::new("layer surface dimensions must match canvas"))
    }
}

fn validate_surface_dimensions_for_source(
    target: &Surface,
    source: &Surface,
) -> Result<(), ZpxError> {
    if target.width() == source.width() && target.height() == source.height() {
        Ok(())
    } else {
        Err(ZpxError::new("layer surface dimensions must match canvas"))
    }
}

fn scale_pixel(pixel: LinearRgba, coverage: f32) -> Result<LinearRgba, ZpxError> {
    LinearRgba::premultiplied(
        pixel.r() * coverage,
        pixel.g() * coverage,
        pixel.b() * coverage,
        pixel.a() * coverage,
    )
    .map_err(raster_error)
}

fn surface_to_straight_srgb_rgba8(surface: &Surface) -> Result<Vec<u8>, ZpxError> {
    let byte_len = surface
        .pixels()
        .len()
        .checked_mul(4)
        .ok_or_else(|| ZpxError::new("PNG pixel buffer is too large"))?;
    let mut rgba = Vec::with_capacity(byte_len);
    for pixel in surface.pixels() {
        push_straight_srgb_rgba8(&mut rgba, *pixel);
    }
    Ok(rgba)
}

fn push_straight_srgb_rgba8(rgba: &mut Vec<u8>, pixel: LinearRgba) {
    let alpha = pixel.a();
    let (r, g, b) = if alpha <= 0.0 {
        (0.0, 0.0, 0.0)
    } else {
        (
            clamp_unit(pixel.r() / alpha),
            clamp_unit(pixel.g() / alpha),
            clamp_unit(pixel.b() / alpha),
        )
    };
    rgba.push(encode_linear_to_srgb_u8(r));
    rgba.push(encode_linear_to_srgb_u8(g));
    rgba.push(encode_linear_to_srgb_u8(b));
    rgba.push(quantize_unit_to_u8(alpha));
}

fn quantize_unit_to_u8(channel: f32) -> u8 {
    let scaled = clamp_unit(channel) * 255.0;
    let lower = scaled.floor();
    let fraction = scaled - lower;
    let lower_int = lower as u16;

    let rounded = if fraction < 0.5 {
        lower_int
    } else if fraction > 0.5 {
        lower_int + 1
    } else if lower_int % 2 == 0 {
        lower_int
    } else {
        lower_int + 1
    };

    if rounded >= 255 { 255 } else { rounded as u8 }
}

fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, ZpxError> {
    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|error| ZpxError::new(format!("PNG encoding failed: {error}")))?;
        writer
            .write_image_data(rgba)
            .map_err(|error| ZpxError::new(format!("PNG encoding failed: {error}")))?;
    }
    Ok(png)
}

fn clamp_unit(channel: f32) -> f32 {
    if !channel.is_finite() || channel <= 0.0 {
        0.0
    } else if channel >= 1.0 {
        1.0
    } else {
        channel
    }
}

fn raster_error(error: zenith_raster::RasterError) -> ZpxError {
    ZpxError::new(format!("raster bake error: {error:?}"))
}

#[cfg(test)]
mod tests {
    use zenith_core::{BlendMode, Color, GradientStop};

    use super::*;
    use crate::model::{BlobRef, Brush, DabSample, Mask, MaskSource, Stroke, StrokeProgram};

    const PNG_MAGIC: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

    fn hash(byte: u8) -> ContentHash {
        ContentHash::from_bytes(&[byte])
    }

    fn test_doc() -> ZpxDoc {
        ZpxDoc {
            canvas: Canvas::new(8, 6),
            layers: vec![program_layer("paint", Color::srgb(255, 0, 0, 255))],
        }
    }

    fn program_layer(id: &str, color: Color) -> Layer {
        Layer {
            id: id.to_owned(),
            blend_mode: BlendMode::Normal,
            opacity: 1.0,
            visible: true,
            clipping: false,
            mask: None,
            source: LayerSource::Program(StrokeProgram {
                strokes: vec![Stroke {
                    brush: Brush::Round {
                        radius_px: 2.0,
                        hardness: 0.5,
                        spacing: 0.5,
                    },
                    path: vec![DabSample {
                        x: 3.0,
                        y: 3.0,
                        pressure: 1.0,
                    }],
                    color,
                    opacity: 1.0,
                    blend_mode: BlendMode::Normal,
                    seed: 7,
                }],
            }),
        }
    }

    #[test]
    fn bake_same_doc_same_png_bytes() {
        let doc = test_doc();

        let first = bake(&doc).expect("bake succeeds");
        let second = bake(&doc).expect("bake succeeds");

        assert_eq!(first.png, second.png);
        assert_eq!(first.png_sha256, second.png_sha256);
    }

    #[test]
    fn bake_hash_matches_png_bytes() {
        let output = bake(&test_doc()).expect("bake succeeds");

        assert_eq!(output.png_sha256, ContentHash::from_bytes(&output.png));
        assert_eq!(
            output.png_sha256.as_str(),
            zenith_session::object_hash(&output.png)
        );
    }

    #[test]
    fn bake_source_hash_matches_manifest_bytes() {
        let doc = test_doc();
        let output = bake(&doc).expect("bake succeeds");
        let manifest = serialize_manifest(&doc);

        assert_eq!(
            output.provenance.source_sha256,
            ContentHash::from_bytes(manifest.as_bytes())
        );
    }

    #[test]
    fn bake_png_has_magic_bytes_and_canvas_dimensions() {
        let output = bake(&test_doc()).expect("bake succeeds");

        assert_eq!(&output.png[0..8], PNG_MAGIC);
        assert_eq!(
            u32::from_be_bytes(output.png[16..20].try_into().unwrap()),
            8
        );
        assert_eq!(
            u32::from_be_bytes(output.png[20..24].try_into().unwrap()),
            6
        );
    }

    #[test]
    fn surface_conversion_keeps_alpha_linear() {
        let pixel = LinearRgba::straight(1.0, 0.0, 0.0, 0.5).expect("valid pixel");
        let surface = Surface::filled(1, 1, pixel).expect("valid surface");

        let rgba = surface_to_straight_srgb_rgba8(&surface).expect("convert surface");

        assert_eq!(rgba, vec![255, 0, 0, 128]);
    }

    #[test]
    fn bake_group_or_adjustment_doc() {
        let doc = ZpxDoc {
            canvas: Canvas::new(8, 6),
            layers: vec![Layer {
                id: "group".to_owned(),
                blend_mode: BlendMode::Normal,
                opacity: 1.0,
                visible: true,
                clipping: false,
                mask: None,
                source: LayerSource::Group(vec![
                    program_layer("paint", Color::srgb(128, 128, 128, 255)),
                    Layer {
                        id: "map".to_owned(),
                        blend_mode: BlendMode::Normal,
                        opacity: 1.0,
                        visible: true,
                        clipping: false,
                        mask: None,
                        source: LayerSource::Adjustment(Adjustment::GradientMap {
                            stops: vec![
                                GradientStop {
                                    offset: 0.0,
                                    color: Color::srgb(0, 0, 0, 255),
                                },
                                GradientStop {
                                    offset: 1.0,
                                    color: Color::srgb(255, 255, 255, 255),
                                },
                            ],
                        }),
                    },
                ]),
            }],
        };

        let output = bake(&doc).expect("bake succeeds");

        assert_eq!(&output.png[0..8], PNG_MAGIC);
    }

    #[test]
    fn bake_buffer_returns_unsupported_error() {
        let mut doc = test_doc();
        doc.layers[0].source = LayerSource::Buffer(BlobRef::new(hash(1)));

        let error = bake(&doc).expect_err("buffer bake is unsupported");

        assert!(error.message().contains("without a blob provider"));
    }

    #[test]
    fn bake_blob_mask_returns_unsupported_error() {
        let mut doc = test_doc();
        doc.layers[0].mask = Some(Mask {
            source: MaskSource::Alpha,
            blob: BlobRef::new(hash(2)),
            invert: false,
        });

        let error = bake(&doc).expect_err("blob mask bake is unsupported");

        assert!(error.message().contains("without a blob provider"));
    }
}
