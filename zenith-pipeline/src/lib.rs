//! Zenith render orchestration over host I/O.
//!
//! One pipeline serves the `zenith` CLI and the browser editor, so both
//! report the same diagnostics and render the same bytes. The crate performs
//! no I/O: every file, config, and local font comes through the traits in
//! [`io`], bundled in a [`Host`].
//!
//! - [`policy`] — config tiers, [`PolicyFlags`], and the policy merge.
//! - [`imports`] — the composition import graph.
//! - [`assets`] — fonts, images, text sources, data contexts, and
//!   missing-file diagnostics.
//! - [`prepare`] — parse and validate with the merged policy.
//! - [`validate_source`] — the full validate pipeline.
//! - [`render`] — scene JSON, PNG, SVG, PDF, contact sheet, and compiled
//!   pages with node boxes.
//! - [`resolved_boxes`] — final node geometry per page.
//!
//! Two hosts ship: the CLI's native host (`std::fs`, a thread pool, the OS
//! font directories) and [`MemFs`] with [`NoLocalFonts`] for the browser.

pub mod assets;
mod boxes;
mod error;
mod host;
pub mod imports;
pub mod io;
pub mod path;
pub mod policy;
pub mod prepare;
pub mod render;
mod validate;

pub use boxes::resolved_boxes;
pub use error::{PipelineError, format_error_diag};
pub use host::{ExtraFont, Host};
pub use io::{
    CollectWarnings, ConfigFile, ConfigSource, FsConfig, FsError, FsErrorKind, IgnoreWarnings,
    LocalFontSource, MemFs, NoConfig, NoLocalFonts, PageRunner, Sequential, SourceFs, WarningSink,
};
pub use policy::PolicyFlags;
pub use render::{
    CompiledPage, CompiledPages, ContactSheetArtifact, DeviceRect, PageRect, PageSelection,
    PageView, PdfArtifact, PngArtifact, PngImage, PngPagesArtifact, RegionArtifact, RegionView,
    RenderOptions, SceneArtifact, SpreadOptions, SvgArtifact, SvgPageArtifact, SvgPagesArtifact,
    ViewOptions, render_png_region, view_page,
};
pub use validate::{Validation, compile_check_diagnostics, validate_parsed, validate_source};
