//! Vector PDF export backend for Zenith.
//!
//! Translates a backend-neutral [`Scene`] display list into a deterministic,
//! print-ready vector PDF: real path/text/shading operators (not a rasterized
//! image), print box metadata (MediaBox / TrimBox / BleedBox / CropBox), and
//! native DeviceCMYK colors for CMYK-origin tokens.
//!
//! # Coordinate system
//!
//! The scene is y-DOWN with the origin at the top-left. PDF user space is y-UP
//! with the origin at the bottom-left. Every page content stream begins with
//! the scale CTM `0.75 0 0 0.75 0 0`, then the flip CTM `1 0 0 -1 0 H`
//! (H = canvas height in px). A scene point `(x, y)` in px then maps to PDF
//! user space with no per-primitive flipping or scaling.
//!
//! # Units
//!
//! One inch is 96 scene px ([`zenith_core::PX_PER_INCH`]) and 72 PDF points.
//! One px is [`zenith_core::PX_TO_PT`] = 0.75 pt.
//! An A4 page of 794 x 1123 px is a 595.5 x 842.25 pt PDF page.
//! MediaBox, TrimBox, BleedBox, CropBox, and link annotation `/Rect` values
//! are in points. Content stream operators stay in px under the scale CTM.
//!
//! # Determinism
//!
//! `pdf-writer` writes no `/CreationDate`, `/ModDate`, or document `/ID` unless
//! asked, and uses no time or randomness. Object ids are assigned in a fixed
//! ordered walk of the scene, so identical input yields byte-identical output.
//!
//! # Module layout
//!
//! - `document` — top-level orchestration ([`render_pdf`]); page boxes,
//!   resource collection, object-id allocation, trailer.
//! - `content`  — the scene-command → content-operator translator.
//! - `color`    — `Color` → DeviceRGB / DeviceCMYK fill+stroke ops, alpha
//!   ExtGState allocation.
//! - `geometry` — rounded-rect / ellipse bezier path builders and the glyph
//!   outline pen.
//! - `gradient` — linear and radial gradients → native PDF shading dictionaries.
//! - `image`    — raster image → FlateDecode RGB image XObject (+ alpha SMask).
//! - `svg`      — SVG asset → native PDF vector operators (paths + shadings).

mod color;
mod content;
mod document;
mod font;
mod geometry;
mod glyph;
mod gradient;
mod image;
mod numeric;
mod raster_embed;
mod report;
pub use report::{
    PdfExportOptions, PdfOutput, PdfRasterizationReason, PdfRasterizedRegion,
    render_pdf_multi_report, render_pdf_multi_report_with_options, render_pdf_report,
    render_pdf_report_with_options,
};
mod scopes;
mod svg;
mod svg_capability;
mod svg_parse;

#[cfg(test)]
mod alpha_tests;
#[cfg(test)]
mod image_tests;
#[cfg(test)]
mod report_tests;
#[cfg(test)]
mod scope_tests;
#[cfg(test)]
mod stroke_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod units_tests;

pub use document::{
    PdfOptions, render_pdf, render_pdf_multi, render_pdf_multi_with, render_pdf_with,
};

#[cfg(test)]
mod gradient_tests;

#[cfg(test)]
mod svg_asset_tests;
