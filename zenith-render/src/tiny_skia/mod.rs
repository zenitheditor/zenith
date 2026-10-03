//! Concrete rasterization backend powered by `tiny-skia`.
//!
//! This is the **only** module in the crate that names `tiny_skia` types or
//! `ttf_parser` types.  All other modules see only the backend-neutral types
//! from `backend.rs`.
//!
//! The [`RasterBackend`](crate::backend::RasterBackend) implementation and the
//! command-dispatch render loop live in [`backend`]; per-drawing-command
//! rasterization lives in [`commands`] + the [`draw`] submodules. Self-contained
//! helpers live in focused submodules: image decoding ([`raster`]), gradient
//! shaders ([`gradient`]), rotated clips ([`clip`]), box blur ([`blur`]),
//! drop-shadow compositing ([`shadow`]), effect crop regions ([`crop`]),
//! geometry/path helpers ([`paths`]), per-pixel color filters ([`filter`]),
//! soft-mask attenuation ([`mask`]), dimension/pixel-format conversions ([`pixels`]), PNG encoding
//! ([`encode`]), and raster-time output scale ([`scale`]). Wiring only — no
//! business logic in this module root.

mod backend;
mod blur;
mod clip;
mod commands;
mod crop;
mod draw;
mod encode;
mod filter;
mod gradient;
mod mask;
mod paths;
mod pixels;
mod raster;
mod scale;
mod shadow;

pub(crate) use raster::decode_raster_image as decode_raster_to_pixmap;
pub(crate) use scale::scaled_px;

pub use backend::TinySkiaBackend;
