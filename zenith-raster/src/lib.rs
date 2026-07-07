//! Deterministic raster surfaces and transfer math for Zenith render backends.

pub mod adjustment;
pub mod blend;
pub mod compositor;
pub mod mask;
pub mod surface;
pub mod transfer;

pub use adjustment::Adjustment;
pub use blend::blend_pixel;
pub use compositor::{Layer, LayerSource, compose, compose_onto};
pub use mask::{Mask, MaskSource};
pub use surface::{LinearRgba, RasterError, Surface};
pub use transfer::{color_to_linear_rgba, decode_srgb_u8, encode_linear_to_srgb_u8};
