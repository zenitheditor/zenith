//! [`Env`]: the project a command runs against, and [`MemProject`], an
//! in-memory project that owns its files and fonts.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use zenith_core::DataContext;
use zenith_pipeline::assets::load_data_context;
use zenith_pipeline::{ExtraFont, FsConfig, Host, MemFs, PolicyFlags};

use crate::error::EditorError;
use crate::execute::{Outcome, Request, execute};
use crate::fonts::bundled_font_files;
use crate::session::Session;

/// The document path when a project names none.
pub const DEFAULT_DOCUMENT_PATH: &str = "document.zen";

/// The project a command runs against: the pipeline host (files, config,
/// fonts), the document's directory, the policy flags, and the data
/// context.
///
/// The native server builds it over the CLI's native host; the browser
/// module and tests use [`MemProject`].
#[derive(Clone, Copy)]
pub struct Env<'a> {
    /// Files, config, and fonts.
    pub host: Host<'a>,
    /// The document's directory. Imports, assets, project fonts, text
    /// sources, and the local `.zenith.kdl` resolve against it. `None`
    /// uses only bundled fonts and no project files.
    pub project_dir: Option<&'a Path>,
    /// `--allow` / `--warn` / `--deny` policy overrides.
    pub flags: &'a PolicyFlags,
    /// The context for `(data)` references, when any.
    pub data: Option<&'a DataContext>,
}

impl<'a> Env<'a> {
    /// An env over `host` with the document in `project_dir`, no flags, and
    /// no data.
    #[must_use]
    pub fn new(host: Host<'a>, project_dir: Option<&'a Path>, flags: &'a PolicyFlags) -> Self {
        Self {
            host,
            project_dir,
            flags,
            data: None,
        }
    }

    /// This env with `data` as the data context.
    #[must_use]
    pub fn with_data(mut self, data: Option<&'a DataContext>) -> Self {
        self.data = data;
        self
    }
}

/// An in-memory project: files keyed by path, extra bundled fonts, the
/// global config path, policy flags, and an optional data context.
///
/// It holds no session. [`MemProject::execute`] builds an [`Env`] over its
/// files and runs one request.
#[derive(Clone, Default)]
pub struct MemProject {
    fs: MemFs,
    dir: PathBuf,
    global_config: Option<PathBuf>,
    fonts: Vec<ExtraFont>,
    flags: PolicyFlags,
    data: Option<DataContext>,
}

impl std::fmt::Debug for MemProject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemProject")
            .field("files", &self.fs.paths().collect::<Vec<_>>())
            .field("dir", &self.dir)
            .field("global_config", &self.global_config)
            .field("fonts", &self.fonts)
            .field("has_data", &self.data.is_some())
            .finish()
    }
}

impl MemProject {
    /// An empty project whose document sits at `document_path` (for
    /// example `site/page.zen`). Its parent is the project directory.
    #[must_use]
    pub fn new(document_path: &str) -> Self {
        let path = Path::new(document_path);
        Self {
            dir: path.parent().map(Path::to_path_buf).unwrap_or_default(),
            ..Self::default()
        }
    }

    /// Store `bytes` at `path`.
    pub fn insert_file(&mut self, path: impl AsRef<Path>, bytes: impl Into<Vec<u8>>) {
        self.fs.insert(path, bytes);
    }

    /// This project with `bytes` at `path`.
    #[must_use]
    pub fn with_file(mut self, path: impl AsRef<Path>, bytes: impl Into<Vec<u8>>) -> Self {
        self.insert_file(path, bytes);
        self
    }

    /// This project with `files` (path → bytes) stored.
    #[must_use]
    pub fn with_files(mut self, files: BTreeMap<String, Vec<u8>>) -> Self {
        for (path, bytes) in files {
            self.insert_file(path, bytes);
        }
        self
    }

    /// This project with bundled font files the build does not embed
    /// (file name → bytes).
    ///
    /// # Errors
    ///
    /// `editor.invalid_font` when a name is not a bundled font file.
    pub fn with_fonts(mut self, fonts: BTreeMap<String, Vec<u8>>) -> Result<Self, EditorError> {
        self.fonts = bundled_font_files(fonts)?;
        Ok(self)
    }

    /// This project with the global config read from `path` in its files.
    #[must_use]
    pub fn with_global_config(mut self, path: impl Into<PathBuf>) -> Self {
        self.global_config = Some(path.into());
        self
    }

    /// This project with `flags` as the policy overrides.
    #[must_use]
    pub fn with_flags(mut self, flags: PolicyFlags) -> Self {
        self.flags = flags;
        self
    }

    /// This project with the data context read from `path` in its files.
    ///
    /// # Errors
    ///
    /// `editor.data_load_failed` when the file is missing or not valid JSON
    /// or CSV.
    pub fn with_data_file(mut self, path: &str) -> Result<Self, EditorError> {
        let data = load_data_context(&self.fs, Path::new(path)).map_err(|e| {
            EditorError::new(
                "editor.data_load_failed",
                format!("{e}; check files['{path}'] is valid JSON or CSV"),
            )
        })?;
        self.data = Some(data);
        Ok(self)
    }

    /// The project directory.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Run `request` against `session` over this project.
    #[must_use]
    pub fn execute(&self, session: Session, request: &Request) -> Outcome {
        let config = FsConfig::new(&self.fs, self.global_config.clone());
        let host = Host::new(&self.fs, &config).with_extra_fonts(&self.fonts);
        let env = Env::new(host, Some(&self.dir), &self.flags).with_data(self.data.as_ref());
        execute(&env, session, request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_path_sets_the_directory() {
        assert_eq!(MemProject::new("site/a.zen").dir(), Path::new("site"));
        assert_eq!(MemProject::new("a.zen").dir(), Path::new(""));
    }

    #[test]
    fn missing_data_file_is_an_error() {
        let err = MemProject::new("a.zen")
            .with_data_file("rows.csv")
            .expect_err("missing");
        assert_eq!(err.code, "editor.data_load_failed");
    }
}
