//! `merge --combine`: one PDF that holds every successful row's pages.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use zenith_core::{AssetKind, BytesAssetProvider, FontProvider};
use zenith_scene::Scene;

use super::run::RowResult;
use crate::commands::render::{BatchExportOptions, BatchFormat, encode_batch_pdf};

/// The outcome of `--combine`.
#[derive(Debug)]
pub struct CombinedResult {
    /// The combined file path as given.
    pub path: PathBuf,
    /// Pages in the written file. Zero when nothing is written.
    pub pages: usize,
    /// 0-based CSV rows in the file, in CSV order.
    pub rows: Vec<usize>,
    /// 0-based CSV rows left out because they failed, in CSV order.
    pub excluded: Vec<usize>,
    /// `None` = written. `Some(reason)` = nothing written.
    pub failure: Option<String>,
}

/// Reject `--combine` for a format other than PDF.
pub(super) fn check_combine(format: BatchFormat, combine: Option<&Path>) -> Result<(), String> {
    match (format, combine) {
        (_, None) | (BatchFormat::Pdf, Some(_)) => Ok(()),
        (BatchFormat::Png | BatchFormat::Svg, Some(path)) => Err(format!(
            "--combine '{}' requires PDF output. Set format to pdf",
            path.display()
        )),
    }
}

/// Collects the compiled pages and row images of each written row.
pub(super) struct Combiner {
    path: PathBuf,
    assets: BytesAssetProvider,
    scenes: Vec<Scene>,
    rows: Vec<usize>,
}

impl Combiner {
    /// Start with the template assets. Row images layer on top.
    pub(super) fn new(path: &Path, assets: BytesAssetProvider) -> Self {
        Self {
            path: path.to_path_buf(),
            assets,
            scenes: Vec::new(),
            rows: Vec::new(),
        }
    }

    /// Add the pages and images of one written row.
    ///
    /// Row image ids hold the row index, so rows never share an id.
    pub(super) fn add(&mut self, row: usize, scenes: Vec<Scene>, images: Vec<(String, Arc<[u8]>)>) {
        for (id, bytes) in images {
            self.assets.register(&id, AssetKind::Image, bytes);
        }
        self.scenes.extend(scenes);
        self.rows.push(row);
    }

    /// Encode and write the combined PDF.
    ///
    /// The write checks `reserved` so it never replaces an input or the manifest.
    /// Row reports already carry the export advisories, so this drops them.
    pub(super) fn finish(
        self,
        options: BatchExportOptions,
        fonts: &dyn FontProvider,
        reserved: &[&Path],
        report_rows: &[RowResult],
    ) -> CombinedResult {
        let excluded = report_rows
            .iter()
            .filter(|row| row.failure.is_some())
            .map(|row| row.row)
            .collect();
        let mut result = CombinedResult {
            path: self.path,
            pages: 0,
            rows: self.rows,
            excluded,
            failure: None,
        };
        if self.scenes.is_empty() {
            result.failure = Some(format!(
                "combined PDF '{}' not written: no row succeeded. Fix the failed rows and rerun",
                result.path.display()
            ));
            return result;
        }
        let pages: Vec<usize> = (1..=self.scenes.len()).collect();
        let bytes = match encode_batch_pdf(options, &self.scenes, &pages, fonts, &self.assets, None)
        {
            Ok((bytes, _advisories)) => bytes,
            Err(diagnostics) => {
                let messages: Vec<&str> = diagnostics.iter().map(|d| d.message.as_str()).collect();
                result.failure = Some(format!(
                    "combined PDF '{}' not written: {}",
                    result.path.display(),
                    messages.join("; ")
                ));
                return result;
            }
        };
        if let Err(error) = write(&result.path, &bytes, reserved) {
            result.failure = Some(crate::output_file::write_failure(&result.path, &error));
            return result;
        }
        result.pages = self.scenes.len();
        result
    }
}

fn write(path: &Path, bytes: &[u8], reserved: &[&Path]) -> std::io::Result<()> {
    let mut guard = crate::output_file::OutputGuard::new(reserved)?;
    guard.check(path)?;
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    guard.write(path, bytes)
}
