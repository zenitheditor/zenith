//! A disk-backed `SourceFs` that reads files the way the CLI reads them.

use std::path::Path;

use zenith_pipeline::{FsError, SourceFs};

/// Project files read from disk with `std::fs`, the way the CLI reads them.
pub struct DiskFs;

impl SourceFs for DiskFs {
    fn read(&self, path: &Path) -> Result<Vec<u8>, FsError> {
        std::fs::read(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                FsError::not_found(e.to_string())
            } else {
                FsError::other(e.to_string())
            }
        })
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }
}
