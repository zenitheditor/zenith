//! The render entry points of `zenith-pipeline`, bound to the native host
//! ([`crate::native::host`]). Every function here is one pipeline call.

use std::path::Path;

use zenith_core::DataContext;
use zenith_pipeline::assets::DataInputError;
use zenith_pipeline::render::{
    ContactSheetArtifact, PdfArtifact, PngArtifact, PngPagesArtifact, RenderOptions, SceneArtifact,
    SpreadOptions, SvgArtifact, SvgPagesArtifact,
};
use zenith_pipeline::{PipelineError, PolicyFlags, render};

use crate::native::{self, NativeFs};

/// Load a data context for `--data` from a JSON or CSV file on disk. See
/// [`zenith_pipeline::assets::load_data_context`].
///
/// # Errors
///
/// Any read, parse, or shape failure, with a message naming the file.
pub fn load_data_context(path: &Path) -> Result<DataContext, DataInputError> {
    zenith_pipeline::assets::load_data_context(&NativeFs, path)
}

/// Scene JSON of 1-based `page` with default options plus `flags` and
/// `data`. See [`render::render_scene_json`].
///
/// # Errors
///
/// As [`render::render_scene_json`].
pub fn to_scene_json(
    src: &str,
    project_dir: Option<&Path>,
    page: usize,
    flags: &PolicyFlags,
    data: Option<&DataContext>,
) -> Result<SceneArtifact, PipelineError> {
    to_scene_json_with_options(
        src,
        project_dir,
        page,
        RenderOptions::new(flags).with_data(data),
    )
}

/// [`render::render_scene_json`] on the native host.
///
/// # Errors
///
/// As [`render::render_scene_json`].
pub fn to_scene_json_with_options(
    src: &str,
    project_dir: Option<&Path>,
    page: usize,
    opts: RenderOptions<'_>,
) -> Result<SceneArtifact, PipelineError> {
    render::render_scene_json(native::host(), src, project_dir, page, opts)
}

/// PNG of 1-based `page` with no project directory, no flags, and no data.
///
/// # Errors
///
/// As [`render::render_png`].
pub fn to_png(src: &str, page: usize) -> Result<PngArtifact, PipelineError> {
    to_png_with_dir(src, None, page, false, &PolicyFlags::default(), None)
}

/// PNG of 1-based `page` with default options plus `locked`, `flags`, and
/// `data`.
///
/// # Errors
///
/// As [`render::render_png`].
pub fn to_png_with_dir(
    src: &str,
    project_dir: Option<&Path>,
    page: usize,
    locked: bool,
    flags: &PolicyFlags,
    data: Option<&DataContext>,
) -> Result<PngArtifact, PipelineError> {
    to_png_with_dir_options(
        src,
        project_dir,
        page,
        RenderOptions::new(flags)
            .with_locked(locked)
            .with_data(data),
    )
}

/// [`render::render_png`] on the native host.
///
/// # Errors
///
/// As [`render::render_png`].
pub fn to_png_with_dir_options(
    src: &str,
    project_dir: Option<&Path>,
    page: usize,
    opts: RenderOptions<'_>,
) -> Result<PngArtifact, PipelineError> {
    render::render_png(native::host(), src, project_dir, page, opts)
}

/// PNG of every page with default options plus `locked`, `flags`, and
/// `data`.
///
/// # Errors
///
/// As [`render::render_png_pages`].
pub fn to_png_all_pages(
    src: &str,
    project_dir: Option<&Path>,
    locked: bool,
    flags: &PolicyFlags,
    data: Option<&DataContext>,
) -> Result<PngPagesArtifact, PipelineError> {
    to_png_all_pages_options(
        src,
        project_dir,
        RenderOptions::new(flags)
            .with_locked(locked)
            .with_data(data),
    )
}

/// [`render::render_png_pages`] on the native host.
///
/// # Errors
///
/// As [`render::render_png_pages`].
pub fn to_png_all_pages_options(
    src: &str,
    project_dir: Option<&Path>,
    opts: RenderOptions<'_>,
) -> Result<PngPagesArtifact, PipelineError> {
    render::render_png_pages(native::host(), src, project_dir, opts)
}

/// [`render::render_png_spread`] on the native host.
///
/// # Errors
///
/// As [`render::render_png_spread`].
pub fn to_png_spread(
    src: &str,
    project_dir: Option<&Path>,
    page_a: usize,
    page_b: usize,
    gutter_override: Option<u32>,
    opts: SpreadOptions<'_>,
) -> Result<PngArtifact, PipelineError> {
    render::render_png_spread(
        native::host(),
        src,
        project_dir,
        page_a,
        page_b,
        gutter_override,
        opts,
    )
}

/// SVG of 1-based `page` with default options plus `locked`, `flags`, and
/// `data`.
///
/// # Errors
///
/// As [`render::render_svg`].
pub fn to_svg_with_dir(
    src: &str,
    project_dir: Option<&Path>,
    page: usize,
    locked: bool,
    flags: &PolicyFlags,
    data: Option<&DataContext>,
) -> Result<SvgArtifact, PipelineError> {
    to_svg_with_dir_options(
        src,
        project_dir,
        page,
        RenderOptions::new(flags)
            .with_locked(locked)
            .with_data(data),
    )
}

/// [`render::render_svg`] on the native host.
///
/// # Errors
///
/// As [`render::render_svg`].
pub fn to_svg_with_dir_options(
    src: &str,
    project_dir: Option<&Path>,
    page: usize,
    opts: RenderOptions<'_>,
) -> Result<SvgArtifact, PipelineError> {
    render::render_svg(native::host(), src, project_dir, page, opts)
}

/// SVG of every page with default options plus `locked`, `flags`, and
/// `data`.
///
/// # Errors
///
/// As [`render::render_svg_pages`].
pub fn to_svg_all_pages_with_dir(
    src: &str,
    project_dir: Option<&Path>,
    locked: bool,
    flags: &PolicyFlags,
    data: Option<&DataContext>,
) -> Result<SvgPagesArtifact, PipelineError> {
    to_svg_all_pages_with_dir_options(
        src,
        project_dir,
        RenderOptions::new(flags)
            .with_locked(locked)
            .with_data(data),
    )
}

/// [`render::render_svg_pages`] on the native host.
///
/// # Errors
///
/// As [`render::render_svg_pages`].
pub fn to_svg_all_pages_with_dir_options(
    src: &str,
    project_dir: Option<&Path>,
    opts: RenderOptions<'_>,
) -> Result<SvgPagesArtifact, PipelineError> {
    render::render_svg_pages(native::host(), src, project_dir, opts)
}

/// PDF of 1-based `page` with default options plus `locked`, `subset`,
/// `flags`, and `data`.
///
/// # Errors
///
/// As [`render::render_pdf`].
pub fn to_pdf_with_dir(
    src: &str,
    project_dir: Option<&Path>,
    page: usize,
    locked: bool,
    subset: bool,
    flags: &PolicyFlags,
    data: Option<&DataContext>,
) -> Result<PdfArtifact, PipelineError> {
    to_pdf_with_dir_options(
        src,
        project_dir,
        page,
        RenderOptions::new(flags)
            .with_locked(locked)
            .with_subset(subset)
            .with_data(data),
    )
}

/// [`render::render_pdf`] on the native host.
///
/// # Errors
///
/// As [`render::render_pdf`].
pub fn to_pdf_with_dir_options(
    src: &str,
    project_dir: Option<&Path>,
    page: usize,
    opts: RenderOptions<'_>,
) -> Result<PdfArtifact, PipelineError> {
    render::render_pdf(native::host(), src, project_dir, page, opts)
}

/// One PDF of every page with default options plus `locked`, `subset`,
/// `flags`, and `data`.
///
/// # Errors
///
/// As [`render::render_pdf_pages`].
pub fn to_pdf_all_pages_with_dir(
    src: &str,
    project_dir: Option<&Path>,
    locked: bool,
    subset: bool,
    flags: &PolicyFlags,
    data: Option<&DataContext>,
) -> Result<PdfArtifact, PipelineError> {
    to_pdf_all_pages_with_dir_options(
        src,
        project_dir,
        RenderOptions::new(flags)
            .with_locked(locked)
            .with_subset(subset)
            .with_data(data),
    )
}

/// [`render::render_pdf_pages`] on the native host.
///
/// # Errors
///
/// As [`render::render_pdf_pages`].
pub fn to_pdf_all_pages_with_dir_options(
    src: &str,
    project_dir: Option<&Path>,
    opts: RenderOptions<'_>,
) -> Result<PdfArtifact, PipelineError> {
    render::render_pdf_pages(native::host(), src, project_dir, opts)
}

/// [`render::render_contact_sheet`] on the native host.
///
/// # Errors
///
/// As [`render::render_contact_sheet`].
pub fn to_contact_sheet(
    src: &str,
    project_dir: Option<&Path>,
    page: Option<usize>,
    scale: Option<f64>,
    opts: RenderOptions<'_>,
) -> Result<ContactSheetArtifact, PipelineError> {
    render::render_contact_sheet(native::host(), src, project_dir, page, scale, opts)
}
