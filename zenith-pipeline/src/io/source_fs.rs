//! Project file access: the [`SourceFs`] trait and its error type.

use std::fmt;
use std::path::Path;

/// Why a [`SourceFs`] read failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsErrorKind {
    /// No file exists at the path.
    NotFound,
    /// The file exists but cannot be read, or the path names no file.
    Other,
}

/// A failed [`SourceFs`] read: its kind and the host's message.
///
/// The message is the host's own wording (a native host passes the OS error
/// text). The pipeline embeds it verbatim in diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FsError {
    /// Why the read failed.
    pub kind: FsErrorKind,
    /// Host text for the error, without the path.
    pub message: String,
}

impl FsError {
    /// A [`FsErrorKind::NotFound`] error with `message`.
    #[must_use]
    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            kind: FsErrorKind::NotFound,
            message: message.into(),
        }
    }

    /// A [`FsErrorKind::Other`] error with `message`.
    #[must_use]
    pub fn other(message: impl Into<String>) -> Self {
        Self {
            kind: FsErrorKind::Other,
            message: message.into(),
        }
    }
}

impl fmt::Display for FsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

/// The message [`SourceFs::read_to_string`] reports for bytes that are not
/// UTF-8. It matches the standard library's text for the same error.
pub const INVALID_UTF8_MESSAGE: &str = "stream did not contain valid UTF-8";

/// Read access to the files of a project.
///
/// The pipeline resolves every project path (`.zen` imports, assets, font
/// assets, text sources, data files, config files) by joining it to the
/// document directory, then reads it here. Paths are never canonicalized:
/// the pipeline passes them exactly as the native CLI would.
pub trait SourceFs {
    /// Read every byte of the file at `path`.
    ///
    /// # Errors
    ///
    /// Returns [`FsError`] when the file is absent or unreadable.
    fn read(&self, path: &Path) -> Result<Vec<u8>, FsError>;

    /// Read the file at `path` as UTF-8 text.
    ///
    /// The default reads the bytes and reports [`INVALID_UTF8_MESSAGE`] for
    /// bytes that are not UTF-8.
    ///
    /// # Errors
    ///
    /// Returns [`FsError`] when the file is absent, unreadable, or not UTF-8.
    fn read_to_string(&self, path: &Path) -> Result<String, FsError> {
        let bytes = self.read(path)?;
        String::from_utf8(bytes).map_err(|_| FsError::other(INVALID_UTF8_MESSAGE))
    }

    /// `true` when a file or directory exists at `path`.
    fn exists(&self, path: &Path) -> bool;

    /// `true` when a regular file exists at `path`.
    fn is_file(&self, path: &Path) -> bool;

    /// Why this source refuses to read `path`, when it does: a confined
    /// source refuses a path outside its root, even when the file exists.
    /// The default refuses nothing.
    fn refusal(&self, _path: &Path) -> Option<FsError> {
        None
    }
}
