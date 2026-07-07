//! Deterministic raster surfaces and transfer math for Zenith render backends.

pub mod surface;
pub mod transfer;

pub use surface::{LinearRgba, RasterError, Surface};
pub use transfer::{color_to_linear_rgba, decode_srgb_u8, encode_linear_to_srgb_u8};
