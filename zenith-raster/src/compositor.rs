//! Deterministic borrowed layer-tree compositing over raster surfaces.

use zenith_core::BlendMode;

use crate::adjustment::Adjustment;
use crate::blend_pixel;
use crate::mask::Mask;
use crate::surface::{LinearRgba, RasterError, Surface};

/// A borrowed raster layer.
#[derive(Debug, Clone, Copy)]
pub struct Layer<'a> {
    pub visible: bool,
    pub clipping: bool,
    pub opacity: f32,
    pub blend_mode: BlendMode,
    pub mask: Option<Mask<'a>>,
    pub source: LayerSource<'a>,
}

impl<'a> Layer<'a> {
    /// Create a visible normal-blend layer from a surface.
    pub const fn surface(surface: &'a Surface) -> Self {
        Self {
            visible: true,
            clipping: false,
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
            mask: None,
            source: LayerSource::Surface(surface),
        }
    }

    /// Create a visible normal-blend group layer from child layers.
    pub const fn group(layers: &'a [Layer<'a>]) -> Self {
        Self {
            visible: true,
            clipping: false,
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
            mask: None,
            source: LayerSource::Group(layers),
        }
    }

    /// Create a visible normal-blend adjustment layer.
    pub const fn adjustment(adjustment: Adjustment<'a>) -> Self {
        Self {
            visible: true,
            clipping: false,
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
            mask: None,
            source: LayerSource::Adjustment(adjustment),
        }
    }

    /// Return this layer with a new boundary opacity.
    pub const fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }

    /// Return this layer with a new boundary blend mode.
    pub const fn with_blend_mode(mut self, blend_mode: BlendMode) -> Self {
        self.blend_mode = blend_mode;
        self
    }

    /// Return this layer with a boundary mask.
    pub const fn with_mask(mut self, mask: Mask<'a>) -> Self {
        self.mask = Some(mask);
        self
    }

    /// Return this layer clipped to the previous boundary alpha.
    pub const fn clipped(mut self) -> Self {
        self.clipping = true;
        self
    }

    /// Return this layer hidden.
    pub const fn hidden(mut self) -> Self {
        self.visible = false;
        self
    }
}

/// Borrowed layer content.
#[derive(Debug, Clone, Copy)]
pub enum LayerSource<'a> {
    Surface(&'a Surface),
    Group(&'a [Layer<'a>]),
    Adjustment(Adjustment<'a>),
}

/// Compose layers over a transparent target surface.
pub fn compose(width: u32, height: u32, layers: &[Layer<'_>]) -> Result<Surface, RasterError> {
    let base = Surface::new(width, height)?;
    compose_onto(&base, layers)
}

/// Compose layers over a copy of `base`.
pub fn compose_onto(base: &Surface, layers: &[Layer<'_>]) -> Result<Surface, RasterError> {
    let mut target = base.clone();
    compose_layers_into(&mut target, layers)?;
    Ok(target)
}

fn compose_layers_into(target: &mut Surface, layers: &[Layer<'_>]) -> Result<(), RasterError> {
    let mut previous_boundary = None;

    for layer in layers {
        if !layer.visible {
            continue;
        }

        validate_opacity(layer.opacity)?;

        if layer.opacity == 0.0 {
            continue;
        }

        let mut boundary = boundary_surface(target, layer.source)?;
        apply_boundary_coverage(&mut boundary, layer, previous_boundary.as_ref())?;
        composite_surface(target, &boundary, layer.blend_mode)?;
        previous_boundary = Some(boundary);
    }

    Ok(())
}

fn validate_opacity(opacity: f32) -> Result<(), RasterError> {
    if opacity.is_finite() && (0.0..=1.0).contains(&opacity) {
        Ok(())
    } else {
        Err(RasterError::InvalidOpacity)
    }
}

fn validate_dimensions(target: &Surface, source: &Surface) -> Result<(), RasterError> {
    if target.width() == source.width() && target.height() == source.height() {
        Ok(())
    } else {
        Err(RasterError::DimensionMismatch)
    }
}

fn boundary_surface(target: &Surface, source: LayerSource<'_>) -> Result<Surface, RasterError> {
    match source {
        LayerSource::Surface(surface) => {
            validate_dimensions(target, surface)?;
            Ok(surface.clone())
        }
        LayerSource::Group(layers) => compose(target.width(), target.height(), layers),
        LayerSource::Adjustment(adjustment) => adjustment.apply(target),
    }
}

fn apply_boundary_coverage(
    boundary: &mut Surface,
    layer: &Layer<'_>,
    previous_boundary: Option<&Surface>,
) -> Result<(), RasterError> {
    let mask = layer.mask;
    let mask_surface = mask.as_ref().map(|mask| mask.surface());

    if let Some(mask_surface) = mask_surface {
        validate_dimensions(boundary, mask_surface)?;
    }
    if let Some(previous_boundary) = previous_boundary {
        validate_dimensions(boundary, previous_boundary)?;
    }

    for y in 0..boundary.height() {
        for x in 0..boundary.width() {
            let mut coverage = layer.opacity;

            if let Some(mask) = mask {
                let mask_pixel = mask_surface
                    .and_then(|surface| surface.get(x, y))
                    .ok_or(RasterError::OutOfBounds)?;
                coverage *= mask.coverage(mask_pixel);
            }

            if layer.clipping {
                let clip_coverage = if let Some(previous_boundary) = previous_boundary {
                    previous_boundary
                        .get(x, y)
                        .ok_or(RasterError::OutOfBounds)?
                        .a()
                } else {
                    0.0
                };
                coverage *= clip_coverage;
            }

            let pixel = boundary.get(x, y).ok_or(RasterError::OutOfBounds)?;
            boundary.set(x, y, scale_pixel(pixel, coverage)?)?;
        }
    }

    Ok(())
}

fn composite_surface(
    target: &mut Surface,
    source: &Surface,
    blend_mode: BlendMode,
) -> Result<(), RasterError> {
    let width = target.width();
    let height = target.height();

    for y in 0..height {
        for x in 0..width {
            let backdrop = target.get(x, y).ok_or(RasterError::OutOfBounds)?;
            let source_pixel = source.get(x, y).ok_or(RasterError::OutOfBounds)?;
            let blended = blend_pixel(blend_mode, backdrop, source_pixel)?;
            target.set(x, y, blended)?;
        }
    }

    Ok(())
}

fn scale_pixel(pixel: LinearRgba, opacity: f32) -> Result<LinearRgba, RasterError> {
    LinearRgba::premultiplied(
        pixel.r() * opacity,
        pixel.g() * opacity,
        pixel.b() * opacity,
        pixel.a() * opacity,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::{Color, GradientStop};

    fn pixel(r: f32, g: f32, b: f32, a: f32) -> LinearRgba {
        LinearRgba::straight(r, g, b, a).unwrap()
    }

    fn one_pixel_surface(pixel: LinearRgba) -> Surface {
        Surface::filled(1, 1, pixel).unwrap()
    }

    fn stop(offset: f64, color: Color) -> GradientStop {
        GradientStop { offset, color }
    }

    fn black_to_white_stops() -> [GradientStop; 2] {
        [
            stop(0.0, Color::srgb(0, 0, 0, 255)),
            stop(1.0, Color::srgb(255, 255, 255, 255)),
        ]
    }

    fn assert_channels_close(actual: LinearRgba, expected: [f32; 4]) {
        let actual = actual.channels();
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!(
                (actual - expected).abs() < 0.000_001,
                "actual {actual} expected {expected}"
            );
        }
    }

    #[test]
    fn unmasked_unclipped_layer_matches_source_over() {
        let source_pixel = pixel(0.8, 0.1, 0.3, 0.25);
        let source = one_pixel_surface(source_pixel);
        let layers = [Layer::surface(&source)];

        let composed = compose(1, 1, &layers).unwrap();

        assert_eq!(composed.get(0, 0), Some(source_pixel));
    }

    #[test]
    fn empty_layers_return_transparent_surface() {
        let composed = compose(2, 1, &[]).unwrap();

        assert_eq!(composed.width(), 2);
        assert_eq!(composed.height(), 1);
        assert_eq!(
            composed.pixels(),
            &[LinearRgba::TRANSPARENT, LinearRgba::TRANSPARENT]
        );
    }

    #[test]
    fn invisible_and_zero_opacity_layers_are_skipped() {
        let base_pixel = pixel(0.1, 0.2, 0.3, 0.4);
        let base = one_pixel_surface(base_pixel);
        let red = one_pixel_surface(pixel(1.0, 0.0, 0.0, 1.0));
        let green = one_pixel_surface(pixel(0.0, 1.0, 0.0, 1.0));
        let layers = [
            Layer::surface(&red).hidden(),
            Layer::surface(&green).with_opacity(0.0),
        ];

        let composed = compose_onto(&base, &layers).unwrap();

        assert_eq!(composed.get(0, 0), Some(base_pixel));
        assert_eq!(red.get(0, 0), Some(pixel(1.0, 0.0, 0.0, 1.0)));
        assert_eq!(green.get(0, 0), Some(pixel(0.0, 1.0, 0.0, 1.0)));
    }

    #[test]
    fn layer_opacity_scales_source_at_boundary() {
        let source_pixel = pixel(1.0, 0.0, 0.0, 0.8);
        let source = one_pixel_surface(source_pixel);
        let layers = [Layer::surface(&source).with_opacity(0.25)];

        let composed = compose(1, 1, &layers).unwrap();

        assert_channels_close(composed.get(0, 0).unwrap(), [0.2, 0.0, 0.0, 0.2]);
        assert_eq!(source.get(0, 0), Some(source_pixel));
    }

    #[test]
    fn alpha_mask_scales_boundary_coverage() {
        let source_pixel = pixel(1.0, 0.0, 0.0, 0.8);
        let source = one_pixel_surface(source_pixel);
        let mask_pixel = pixel(0.0, 0.0, 0.0, 0.25);
        let mask = one_pixel_surface(mask_pixel);
        let layers = [Layer::surface(&source).with_mask(Mask::alpha(&mask))];

        let composed = compose(1, 1, &layers).unwrap();

        assert_channels_close(composed.get(0, 0).unwrap(), [0.2, 0.0, 0.0, 0.2]);
        assert_eq!(source.get(0, 0), Some(source_pixel));
        assert_eq!(mask.get(0, 0), Some(mask_pixel));
    }

    #[test]
    fn luminance_mask_uses_premultiplied_linear_channels() {
        let source = one_pixel_surface(pixel(1.0, 0.0, 0.0, 1.0));
        let mask = one_pixel_surface(pixel(1.0, 0.5, 0.0, 0.5));
        let layers = [Layer::surface(&source).with_mask(Mask::luminance(&mask))];

        let composed = compose(1, 1, &layers).unwrap();

        assert_channels_close(composed.get(0, 0).unwrap(), [0.2975, 0.0, 0.0, 0.2975]);
    }

    #[test]
    fn inverted_mask_uses_complement_coverage() {
        let source = one_pixel_surface(pixel(0.0, 1.0, 0.0, 1.0));
        let mask = one_pixel_surface(pixel(0.0, 0.0, 0.0, 0.25));
        let layers = [Layer::surface(&source).with_mask(Mask::alpha(&mask).inverted())];

        let composed = compose(1, 1, &layers).unwrap();

        assert_channels_close(composed.get(0, 0).unwrap(), [0.0, 0.75, 0.0, 0.75]);
    }

    #[test]
    fn clipped_layer_uses_previous_boundary_alpha() {
        let blue = one_pixel_surface(pixel(0.0, 0.0, 1.0, 0.4));
        let red = one_pixel_surface(pixel(1.0, 0.0, 0.0, 1.0));
        let layers = [Layer::surface(&blue), Layer::surface(&red).clipped()];

        let composed = compose(1, 1, &layers).unwrap();

        assert_channels_close(composed.get(0, 0).unwrap(), [0.4, 0.0, 0.24, 0.64]);
    }

    #[test]
    fn clipped_layer_without_previous_boundary_is_no_op() {
        let source = one_pixel_surface(pixel(1.0, 0.0, 0.0, 1.0));
        let layers = [Layer::surface(&source).clipped()];

        let composed = compose(1, 1, &layers).unwrap();

        assert_eq!(composed.get(0, 0), Some(LinearRgba::TRANSPARENT));
    }

    #[test]
    fn group_output_is_available_as_clipping_boundary() {
        let green = one_pixel_surface(pixel(0.0, 1.0, 0.0, 0.5));
        let red = one_pixel_surface(pixel(1.0, 0.0, 0.0, 1.0));
        let group_layers = [Layer::surface(&green)];
        let layers = [Layer::group(&group_layers), Layer::surface(&red).clipped()];

        let composed = compose(1, 1, &layers).unwrap();

        assert_channels_close(composed.get(0, 0).unwrap(), [0.5, 0.25, 0.0, 0.75]);
    }

    #[test]
    fn non_normal_blend_modes_route_through_pixel_blending() {
        let base = one_pixel_surface(pixel(0.5, 0.5, 0.5, 1.0));
        let source = one_pixel_surface(pixel(0.4, 0.8, 0.2, 1.0));
        let layers = [Layer::surface(&source).with_blend_mode(BlendMode::Multiply)];

        let composed = compose_onto(&base, &layers).unwrap();

        assert_channels_close(composed.get(0, 0).unwrap(), [0.2, 0.4, 0.1, 1.0]);
    }

    #[test]
    fn source_dimension_mismatch_is_reported() {
        let source = Surface::new(2, 1).unwrap();
        let layers = [Layer::surface(&source)];

        assert_eq!(compose(1, 1, &layers), Err(RasterError::DimensionMismatch));
    }

    #[test]
    fn mask_dimension_mismatch_is_reported() {
        let source = one_pixel_surface(pixel(1.0, 0.0, 0.0, 1.0));
        let mask = Surface::new(2, 1).unwrap();
        let layers = [Layer::surface(&source).with_mask(Mask::alpha(&mask))];

        assert_eq!(compose(1, 1, &layers), Err(RasterError::DimensionMismatch));
    }

    #[test]
    fn invalid_opacity_is_reported() {
        let source = one_pixel_surface(pixel(1.0, 0.0, 0.0, 1.0));
        let layers = [Layer::surface(&source).with_opacity(f32::NAN)];

        assert_eq!(compose(1, 1, &layers), Err(RasterError::InvalidOpacity));
    }

    #[test]
    fn hidden_layers_do_not_validate_opacity() {
        let source = one_pixel_surface(pixel(1.0, 0.0, 0.0, 1.0));
        let layers = [Layer::surface(&source).with_opacity(f32::NAN).hidden()];

        assert_eq!(compose(1, 1, &layers), Ok(Surface::new(1, 1).unwrap()));
    }

    #[test]
    fn nested_groups_compose_children_before_group_boundary() {
        let base = one_pixel_surface(pixel(0.0, 0.0, 1.0, 1.0));
        let red = one_pixel_surface(pixel(1.0, 0.0, 0.0, 0.5));
        let green = one_pixel_surface(pixel(0.0, 1.0, 0.0, 0.5));
        let group_layers = [Layer::surface(&red), Layer::surface(&green)];
        let layers = [Layer::group(&group_layers).with_opacity(0.5)];

        let composed = compose_onto(&base, &layers).unwrap();

        assert_channels_close(composed.get(0, 0).unwrap(), [0.125, 0.25, 0.625, 1.0]);
    }

    #[test]
    fn gradient_map_midpoint_remaps_snapshot_luminance() {
        let base = one_pixel_surface(pixel(0.5, 0.5, 0.5, 1.0));
        let stops = [
            stop(0.0, Color::srgb(0, 0, 0, 255)),
            stop(1.0, Color::srgb(255, 0, 0, 255)),
        ];
        let layers = [Layer::adjustment(Adjustment::gradient_map(&stops))];

        let composed = compose_onto(&base, &layers).unwrap();

        assert_channels_close(composed.get(0, 0).unwrap(), [0.5, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn gradient_map_clamps_samples_outside_stop_range() {
        let source = Surface::from_pixels(
            2,
            1,
            vec![pixel(0.0, 0.0, 0.0, 1.0), pixel(1.0, 1.0, 1.0, 1.0)],
        )
        .unwrap();
        let stops = [
            stop(0.25, Color::srgb(255, 0, 0, 255)),
            stop(0.75, Color::srgb(0, 0, 255, 255)),
        ];
        let layers = [Layer::adjustment(Adjustment::gradient_map(&stops))];

        let composed = compose_onto(&source, &layers).unwrap();

        assert_channels_close(composed.get(0, 0).unwrap(), [1.0, 0.0, 0.0, 1.0]);
        assert_channels_close(composed.get(1, 0).unwrap(), [0.0, 0.0, 1.0, 1.0]);
    }

    #[test]
    fn gradient_map_equal_offsets_use_later_stop_at_offset() {
        let base = one_pixel_surface(pixel(0.5, 0.5, 0.5, 1.0));
        let stops = [
            stop(0.0, Color::srgb(0, 0, 0, 255)),
            stop(0.5, Color::srgb(255, 0, 0, 255)),
            stop(0.5, Color::srgb(0, 255, 0, 255)),
            stop(1.0, Color::srgb(0, 0, 255, 255)),
        ];
        let layers = [Layer::adjustment(Adjustment::gradient_map(&stops))];

        let composed = compose_onto(&base, &layers).unwrap();

        assert_channels_close(composed.get(0, 0).unwrap(), [0.0, 1.0, 0.0, 1.0]);
    }

    #[test]
    fn gradient_map_duplicate_first_offset_uses_later_stop_at_offset() {
        let base = one_pixel_surface(pixel(0.0, 0.0, 0.0, 1.0));
        let stops = [
            stop(0.0, Color::srgb(255, 0, 0, 255)),
            stop(0.0, Color::srgb(0, 255, 0, 255)),
            stop(1.0, Color::srgb(0, 0, 255, 255)),
        ];
        let layers = [Layer::adjustment(Adjustment::gradient_map(&stops))];

        let composed = compose_onto(&base, &layers).unwrap();

        assert_channels_close(composed.get(0, 0).unwrap(), [0.0, 1.0, 0.0, 1.0]);
    }

    #[test]
    fn gradient_map_preserves_input_alpha() {
        let base = one_pixel_surface(pixel(0.5, 0.5, 0.5, 0.25));
        let stops = [
            stop(0.0, Color::srgb(0, 0, 0, 255)),
            stop(1.0, Color::srgb(255, 0, 0, 255)),
        ];
        let layers = [Layer::adjustment(Adjustment::gradient_map(&stops))];

        let composed = compose_onto(&base, &layers).unwrap();

        assert_channels_close(
            composed.get(0, 0).unwrap(),
            [0.21875, 0.09375, 0.09375, 0.4375],
        );
    }

    #[test]
    fn gradient_map_opacity_applies_after_remap() {
        let base = one_pixel_surface(pixel(0.5, 0.5, 0.5, 1.0));
        let stops = [
            stop(0.0, Color::srgb(0, 0, 0, 255)),
            stop(1.0, Color::srgb(255, 0, 0, 255)),
        ];
        let layers = [Layer::adjustment(Adjustment::gradient_map(&stops)).with_opacity(0.5)];

        let composed = compose_onto(&base, &layers).unwrap();

        assert_channels_close(composed.get(0, 0).unwrap(), [0.5, 0.25, 0.25, 1.0]);
    }

    #[test]
    fn gradient_map_clipping_uses_previous_boundary_alpha() {
        let blue = one_pixel_surface(pixel(0.0, 0.0, 1.0, 0.4));
        let stops = [
            stop(0.0, Color::srgb(255, 0, 0, 255)),
            stop(1.0, Color::srgb(255, 0, 0, 255)),
        ];
        let layers = [
            Layer::surface(&blue),
            Layer::adjustment(Adjustment::gradient_map(&stops)).clipped(),
        ];

        let composed = compose(1, 1, &layers).unwrap();

        assert_channels_close(composed.get(0, 0).unwrap(), [0.16, 0.0, 0.336, 0.496]);
    }

    #[test]
    fn gradient_map_blend_mode_routes_through_pixel_blending() {
        let base = one_pixel_surface(pixel(0.5, 0.5, 0.5, 1.0));
        let stops = black_to_white_stops();
        let layers = [Layer::adjustment(Adjustment::gradient_map(&stops))
            .with_blend_mode(BlendMode::Multiply)];

        let composed = compose_onto(&base, &layers).unwrap();

        assert_channels_close(composed.get(0, 0).unwrap(), [0.25, 0.25, 0.25, 1.0]);
    }

    #[test]
    fn gradient_map_rejects_invalid_stop_data() {
        let base = one_pixel_surface(pixel(0.5, 0.5, 0.5, 1.0));
        let too_few = [stop(0.0, Color::srgb(0, 0, 0, 255))];
        let non_finite = [
            stop(0.0, Color::srgb(0, 0, 0, 255)),
            stop(f64::NAN, Color::srgb(255, 255, 255, 255)),
        ];
        let out_of_range = [
            stop(0.0, Color::srgb(0, 0, 0, 255)),
            stop(1.1, Color::srgb(255, 255, 255, 255)),
        ];
        let unsorted = [
            stop(0.75, Color::srgb(0, 0, 0, 255)),
            stop(0.25, Color::srgb(255, 255, 255, 255)),
        ];

        for stops in [&too_few[..], &non_finite, &out_of_range, &unsorted] {
            let layers = [Layer::adjustment(Adjustment::gradient_map(stops))];
            assert_eq!(
                compose_onto(&base, &layers),
                Err(RasterError::InvalidGradientStops)
            );
        }
    }
}
