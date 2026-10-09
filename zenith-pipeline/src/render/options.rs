//! Render options shared by every entry point.

use zenith_core::{DataContext, Document};

use crate::policy::PolicyFlags;

/// Options of one render call. Each field has the meaning of the CLI flag
/// named beside it.
#[derive(Clone, Copy, Debug)]
pub struct RenderOptions<'a> {
    /// `--locked`: verify asset and font `sha256` and fail on a mismatch.
    pub locked: bool,
    /// Subset PDF fonts to used glyphs (`--embed-full-fonts` turns it off).
    pub subset: bool,
    /// `--allow` / `--warn` / `--deny` policy overrides.
    pub flags: &'a PolicyFlags,
    /// `--data`: the context for `(data)` references.
    pub data: Option<&'a DataContext>,
    /// `--construction-overlay`: append page construction guides to the
    /// scene after the canonical compile.
    pub construction_overlay: bool,
    /// `--scale`: raster output scale for PNG outputs (`1.0` = page pixels).
    /// Each axis is `max(1, round(page × scale))` pixels. Scene JSON, SVG,
    /// and PDF ignore it.
    pub scale: f64,
    /// `--raster-scale`: raster fallback resolution for SVG and PDF
    /// (`1.0` = page pixels).
    pub raster_scale: f64,
    /// The parse of the source the caller already holds. The render uses it
    /// instead of parsing the source again. It must be the parse of that
    /// source: the output then equals a render without it.
    pub parsed: Option<&'a Document>,
}

impl<'a> RenderOptions<'a> {
    /// Defaults with `flags`: unlocked, subset fonts, no data, no overlay,
    /// scale 1, raster scale 1.
    #[must_use]
    pub fn new(flags: &'a PolicyFlags) -> Self {
        Self {
            locked: false,
            subset: true,
            flags,
            data: None,
            construction_overlay: false,
            scale: 1.0,
            raster_scale: 1.0,
            parsed: None,
        }
    }

    /// This options set with `locked`.
    #[must_use]
    pub fn with_locked(mut self, locked: bool) -> Self {
        self.locked = locked;
        self
    }

    /// This options set with PDF font subsetting on or off.
    #[must_use]
    pub fn with_subset(mut self, subset: bool) -> Self {
        self.subset = subset;
        self
    }

    /// This options set with `data` as the data context.
    #[must_use]
    pub fn with_data(mut self, data: Option<&'a DataContext>) -> Self {
        self.data = data;
        self
    }

    /// This options set with the construction overlay on or off.
    #[must_use]
    pub fn with_construction_overlay(mut self, construction_overlay: bool) -> Self {
        self.construction_overlay = construction_overlay;
        self
    }

    /// This options set with raster output `scale` (see [`Self::scale`]).
    #[must_use]
    pub fn with_scale(mut self, scale: f64) -> Self {
        self.scale = scale;
        self
    }

    /// This options set with `parsed` as the parse of the source (see
    /// [`Self::parsed`]).
    #[must_use]
    pub fn with_parsed(mut self, parsed: Option<&'a Document>) -> Self {
        self.parsed = parsed;
        self
    }

    /// This options set with vector fallback `raster_scale`.
    #[must_use]
    pub fn with_raster_scale(mut self, raster_scale: f64) -> Self {
        self.raster_scale = raster_scale;
        self
    }
}

/// Options of [`render_png_spread`](super::render_png_spread).
#[derive(Clone, Copy, Debug)]
pub struct SpreadOptions<'a> {
    /// Verify asset `sha256` and fail on a mismatch.
    pub locked: bool,
    /// Policy overrides.
    pub flags: &'a PolicyFlags,
    /// The context for `(data)` references.
    pub data: Option<&'a DataContext>,
    /// Append page construction guides to both compiled scenes.
    pub construction_overlay: bool,
    /// Raster output scale (`1.0` = page pixels); the gutter scales with it.
    pub scale: f64,
}
