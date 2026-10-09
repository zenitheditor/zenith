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
//! pipeline-free source-over of a pixmap ([`blend`]), reused effect captures
//! ([`pool`]),
//! geometry/path helpers ([`paths`]), per-pixel color filters ([`filter`]),
//! soft-mask attenuation ([`mask`]), dimension/pixel-format conversions ([`pixels`]), PNG encoding
//! ([`encode`]), raster-time output scale ([`scale`]), the render surface and
//! its origin-aware draw calls ([`surface`]), effect reach ([`reach`]), and
//! region renders ([`region`]). Wiring only — no business logic in this
//! module root.

mod backend;
mod blend;
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
mod pool;
mod raster;
mod reach;
mod region;
mod scale;
mod shadow;
mod surface;

pub(crate) use paths::build_rounded_rect_path;
pub(crate) use raster::{
    crop_raster_image, crop_raster_rect, decode_raster_image as decode_raster_to_pixmap,
};
pub(crate) use region::{MAX_SURFACE_SIDE, Window, rasterize_window, rasterize_window_png};
pub(crate) use scale::{check_scale, page_px, scaled_px};

pub use backend::TinySkiaBackend;
