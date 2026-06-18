//! CPU PNG reference renderer for Zenith.
//!
//! Owns the raster backend adapter trait (tiny-skia is the first engine),
//! deterministic PNG production from a scene display list, SVG and raster
//! image decode, glyph rasterization, and enforcement of all raster-time
//! determinism rules. Backend types never appear in the public API.
