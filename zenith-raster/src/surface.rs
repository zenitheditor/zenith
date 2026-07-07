//! Premultiplied linear floating-point raster surfaces.

/// Errors returned by raster substrate constructors and conversions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RasterError {
    EmptySurface,
    SurfaceTooLarge,
    PixelBufferLengthMismatch,
    OutOfBounds,
    NonFiniteChannel,
    ChannelOutOfRange,
    NotPremultiplied,
}

/// A premultiplied linear RGBA pixel.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinearRgba {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

impl LinearRgba {
    /// Transparent premultiplied black.
    pub const TRANSPARENT: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    /// Construct a premultiplied linear pixel.
    pub fn premultiplied(r: f32, g: f32, b: f32, a: f32) -> Result<Self, RasterError> {
        validate_unit_channel(r)?;
        validate_unit_channel(g)?;
        validate_unit_channel(b)?;
        validate_unit_channel(a)?;

        if r > a || g > a || b > a {
            return Err(RasterError::NotPremultiplied);
        }

        Ok(Self { r, g, b, a })
    }

    /// Construct a straight-alpha linear pixel and premultiply it.
    pub fn straight(r: f32, g: f32, b: f32, a: f32) -> Result<Self, RasterError> {
        validate_unit_channel(r)?;
        validate_unit_channel(g)?;
        validate_unit_channel(b)?;
        validate_unit_channel(a)?;

        Ok(Self {
            r: r * a,
            g: g * a,
            b: b * a,
            a,
        })
    }

    pub const fn r(self) -> f32 {
        self.r
    }

    pub const fn g(self) -> f32 {
        self.g
    }

    pub const fn b(self) -> f32 {
        self.b
    }

    pub const fn a(self) -> f32 {
        self.a
    }

    pub const fn channels(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

/// A deterministic premultiplied linear RGBA raster surface.
#[derive(Debug, Clone, PartialEq)]
pub struct Surface {
    width: u32,
    height: u32,
    pixels: Vec<LinearRgba>,
}

impl Surface {
    /// Create a surface filled with transparent premultiplied black.
    pub fn new(width: u32, height: u32) -> Result<Self, RasterError> {
        Self::filled(width, height, LinearRgba::TRANSPARENT)
    }

    /// Create a surface filled with `pixel`.
    pub fn filled(width: u32, height: u32, pixel: LinearRgba) -> Result<Self, RasterError> {
        let len = pixel_len(width, height)?;
        Ok(Self {
            width,
            height,
            pixels: vec![pixel; len],
        })
    }

    /// Create a surface from an existing contiguous pixel buffer.
    pub fn from_pixels(
        width: u32,
        height: u32,
        pixels: Vec<LinearRgba>,
    ) -> Result<Self, RasterError> {
        let len = pixel_len(width, height)?;
        if pixels.len() != len {
            return Err(RasterError::PixelBufferLengthMismatch);
        }

        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub fn pixels(&self) -> &[LinearRgba] {
        &self.pixels
    }

    pub fn get(&self, x: u32, y: u32) -> Option<LinearRgba> {
        self.index_of(x, y)
            .and_then(|index| self.pixels.get(index).copied())
    }

    pub fn set(&mut self, x: u32, y: u32, pixel: LinearRgba) -> Result<(), RasterError> {
        let index = self.index_of(x, y).ok_or(RasterError::OutOfBounds)?;
        if let Some(slot) = self.pixels.get_mut(index) {
            *slot = pixel;
            Ok(())
        } else {
            Err(RasterError::OutOfBounds)
        }
    }

    fn index_of(&self, x: u32, y: u32) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }

        let row = (y as usize).checked_mul(self.width as usize)?;
        row.checked_add(x as usize)
    }
}

fn validate_unit_channel(channel: f32) -> Result<(), RasterError> {
    if !channel.is_finite() {
        return Err(RasterError::NonFiniteChannel);
    }
    if !(0.0..=1.0).contains(&channel) {
        return Err(RasterError::ChannelOutOfRange);
    }
    Ok(())
}

fn pixel_len(width: u32, height: u32) -> Result<usize, RasterError> {
    if width == 0 || height == 0 {
        return Err(RasterError::EmptySurface);
    }

    let len = (width as usize)
        .checked_mul(height as usize)
        .ok_or(RasterError::SurfaceTooLarge)?;
    let bytes = len
        .checked_mul(std::mem::size_of::<LinearRgba>())
        .ok_or(RasterError::SurfaceTooLarge)?;
    if bytes > isize::MAX as usize {
        return Err(RasterError::SurfaceTooLarge);
    }

    Ok(len)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn premultiplied_pixels_reject_invalid_channels() {
        assert_eq!(
            LinearRgba::premultiplied(f32::NAN, 0.0, 0.0, 1.0),
            Err(RasterError::NonFiniteChannel)
        );
        assert_eq!(
            LinearRgba::premultiplied(1.1, 0.0, 0.0, 1.0),
            Err(RasterError::ChannelOutOfRange)
        );
        assert_eq!(
            LinearRgba::premultiplied(0.5, 0.0, 0.0, 0.25),
            Err(RasterError::NotPremultiplied)
        );
    }

    #[test]
    fn straight_pixels_are_premultiplied() {
        let pixel = LinearRgba::straight(0.8, 0.4, 0.2, 0.5).unwrap();
        assert_eq!(pixel.channels(), [0.4, 0.2, 0.1, 0.5]);
    }

    #[test]
    fn surface_rejects_zero_and_overflow_dimensions() {
        assert_eq!(Surface::new(0, 1), Err(RasterError::EmptySurface));
        assert_eq!(Surface::new(1, 0), Err(RasterError::EmptySurface));
        assert_eq!(
            Surface::new(u32::MAX, u32::MAX),
            Err(RasterError::SurfaceTooLarge)
        );
    }

    #[test]
    fn surface_get_set_are_bounds_checked() {
        let red = LinearRgba::premultiplied(1.0, 0.0, 0.0, 1.0).unwrap();
        let mut surface = Surface::new(2, 2).unwrap();

        assert_eq!(surface.get(2, 0), None);
        assert_eq!(surface.set(2, 0, red), Err(RasterError::OutOfBounds));

        assert_eq!(surface.set(1, 1, red), Ok(()));
        assert_eq!(surface.get(1, 1), Some(red));
    }

    #[test]
    fn surface_from_pixels_checks_buffer_length() {
        let pixel = LinearRgba::TRANSPARENT;
        assert_eq!(
            Surface::from_pixels(2, 2, vec![pixel; 3]),
            Err(RasterError::PixelBufferLengthMismatch)
        );
        assert_eq!(
            Surface::from_pixels(2, 2, vec![pixel; 4]).map(|s| s.pixels().len()),
            Ok(4)
        );
    }
}
