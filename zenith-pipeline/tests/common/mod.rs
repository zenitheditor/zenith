//! Shared test helpers: a disk-backed `SourceFs` for parity checks, loaders
//! that copy files into a `MemFs`, and the example list.

use std::path::{Path, PathBuf};

use zenith_pipeline::{FsError, MemFs, SourceFs};

/// The repository `examples/` directory, relative to this crate.
pub const EXAMPLES: &str = "../examples";

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

/// Every file under `dir`, recursively, sorted.
pub fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut work = vec![dir.to_path_buf()];
    while let Some(d) = work.pop() {
        for entry in std::fs::read_dir(&d).expect("read dir") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                work.push(path);
            } else {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// A `MemFs` holding every file under `dir`, keyed by the same paths.
pub fn mem_copy_of(dir: &Path) -> MemFs {
    let mut fs = MemFs::new();
    for path in files_under(dir) {
        let bytes = std::fs::read(&path).expect("read file");
        fs.insert(&path, bytes);
    }
    fs
}

/// The `.zen` files directly in `examples/`, sorted.
pub fn example_documents() -> Vec<PathBuf> {
    let mut docs: Vec<PathBuf> = std::fs::read_dir(EXAMPLES)
        .expect("examples dir")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "zen"))
        .collect();
    docs.sort();
    docs
}
