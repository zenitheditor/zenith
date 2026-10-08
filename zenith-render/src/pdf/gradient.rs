//! Native axial and radial gradient geometry and RGB stops.

use zenith_scene::GradientPaint;

/// A gradient resolved to native PDF shading geometry plus ordered RGB stops.
///
/// Built once per gradient draw by [`resolve`]; consumed by the document writer
/// which materializes the function + shading indirect objects and clips the
/// shading to the shape via `W n` + `sh`.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PdfGradient {
    /// Axial endpoints or radial circles in scene coordinates.
    pub(super) geometry: GradientGeometry,
    /// Ordered stops: `(offset 0..=1, [r, g, b] each 0..=1)`. At least two.
    pub(super) stops: Vec<(f32, [f32; 3])>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum GradientGeometry {
    Axial([f32; 4]),
    Radial([f32; 6]),
}

/// Native shading requires opaque, strictly ordered stops spanning the full domain.
pub(super) fn requires_raster(gradient: &GradientPaint) -> bool {
    gradient.stops.len() < 2
        || gradient.stops.iter().any(|stop| stop.color.a != 255)
        || gradient
            .stops
            .first()
            .is_some_and(|stop| stop.offset != 0.0)
        || gradient.stops.last().is_some_and(|stop| stop.offset != 1.0)
        || gradient.stops.windows(2).any(|pair| {
            pair.first().zip(pair.get(1)).is_some_and(|(a, b)| {
                (a.offset as f32).partial_cmp(&(b.offset as f32)) != Some(std::cmp::Ordering::Less)
            })
        })
}

/// Resolve a [`GradientPaint`] over the box `[x, y, w, h]` into an
/// [`PdfGradient`], or `None` when it has fewer than two stops.
///
/// The axial gradient line runs through the box center at `angle_deg` (clockwise from
/// +x in screen coordinates), with the CSS gradient-line length
/// `|w·cosθ| + |h·sinθ|`, identical to the raster backend's `gradient_shader`.
pub(super) fn resolve(
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    gradient: &GradientPaint,
) -> Option<PdfGradient> {
    if gradient.stops.len() < 2 {
        return None;
    }
    let stops: Vec<(f32, [f32; 3])> = gradient
        .stops
        .iter()
        .map(|s| {
            (
                (s.offset as f32).clamp(0.0, 1.0),
                [
                    f32::from(s.color.r) / 255.0,
                    f32::from(s.color.g) / 255.0,
                    f32::from(s.color.b) / 255.0,
                ],
            )
        })
        .collect();
    let geometry = geometry_coordinates(x, y, w, h, gradient);
    let valid = match geometry {
        GradientGeometry::Axial(coords) => coords.iter().all(|value| value.is_finite()),
        GradientGeometry::Radial(coords) => {
            coords.iter().all(|value| value.is_finite())
                && coords.last().is_some_and(|radius| *radius > 0.0)
        }
    };
    valid.then_some(PdfGradient { geometry, stops })
}

/// Resolve raw coordinates without conflating unsupported numbers and degenerate geometry.
pub(super) fn geometry_coordinates(
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    gradient: &GradientPaint,
) -> GradientGeometry {
    if gradient.radial {
        let cx = (x + w * gradient.center_x.unwrap_or(0.5)) as f32;
        let cy = (y + h * gradient.center_y.unwrap_or(0.5)) as f32;
        let radius = (gradient.radius_frac.unwrap_or(1.0) * (w / 2.0).hypot(h / 2.0)) as f32;
        GradientGeometry::Radial([cx, cy, 0.0, cx, cy, radius])
    } else {
        let theta = gradient.angle_deg.to_radians();
        let (dir_x, dir_y) = (theta.cos(), theta.sin());
        let (cx, cy) = (x + w / 2.0, y + h / 2.0);
        let line_len = (w * dir_x).abs() + (h * dir_y).abs();
        let half = line_len / 2.0;
        let coords = [
            (cx - dir_x * half) as f32,
            (cy - dir_y * half) as f32,
            (cx + dir_x * half) as f32,
            (cy + dir_y * half) as f32,
        ];
        GradientGeometry::Axial(coords)
    }
}
