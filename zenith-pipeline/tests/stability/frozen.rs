//! The frozen corpus on disk: its directory, its `FROZEN_AT` version, and its
//! documents.

use std::path::{Path, PathBuf};

/// The directory that holds exactly one `v<X.Y.Z>` corpus, relative to the
/// crate directory (the test cwd).
const CORPUS_ROOT: &str = "tests/corpus";

/// A release version as `(major, minor, patch)`.
pub type Version = (u64, u64, u64);

/// Parse `X.Y.Z` into a [`Version`], or `None` for any other shape.
pub fn parse_version(text: &str) -> Option<Version> {
    let mut parts = text.split('.').map(|p| p.parse::<u64>().ok());
    let version = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(version)
}

/// The single `tests/corpus/v<X.Y.Z>` directory.
pub fn corpus_dir() -> PathBuf {
    let dirs: Vec<PathBuf> = std::fs::read_dir(CORPUS_ROOT)
        .unwrap_or_else(|e| panic!("cannot read {CORPUS_ROOT}: {e}"))
        .map(|entry| entry.expect("corpus entry").path())
        .filter(|p| p.is_dir())
        .collect();
    assert_eq!(
        dirs.len(),
        1,
        "{CORPUS_ROOT} must hold exactly one frozen corpus directory, found {dirs:?}. \
         A refresh replaces the old directory"
    );
    dirs.into_iter().next().expect("one corpus directory")
}

/// The version in `<dir>/FROZEN_AT`. The directory name must be `v<version>`.
pub fn frozen_at(dir: &Path) -> String {
    let path = dir.join("FROZEN_AT");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let version = text.trim().to_owned();
    assert_eq!(
        text,
        format!("{version}\n"),
        "{} must be one line `X.Y.Z` and a newline",
        path.display()
    );
    assert!(
        parse_version(&version).is_some(),
        "{}: `{version}` is not `X.Y.Z`",
        path.display()
    );
    let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    assert_eq!(
        name,
        format!("v{version}"),
        "corpus directory `{name}` must be named after FROZEN_AT `{version}`"
    );
    version
}

/// Every `.zen` file under `dir`, recursively, sorted.
pub fn documents(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut work = vec![dir.to_path_buf()];
    while let Some(d) = work.pop() {
        for entry in std::fs::read_dir(&d).expect("read corpus dir") {
            let path = entry.expect("corpus entry").path();
            if path.is_dir() {
                work.push(path);
            } else if path.extension().is_some_and(|e| e == "zen") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// The `/`-separated path of `path` under `dir`, as the pins key it.
pub fn relative_name(dir: &Path, path: &Path) -> String {
    path.strip_prefix(dir)
        .expect("document under the corpus dir")
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}
