//! [`MemFs`]: an in-memory [`SourceFs`] that a browser host fills.

use std::collections::BTreeMap;
use std::path::{Component, Path};

use super::source_fs::{FsError, SourceFs};
use crate::path::normalize_lexically;

/// The message a [`MemFs`] read reports for an absent path.
pub const MEM_NOT_FOUND_MESSAGE: &str = "no such file in the in-memory file set";

/// An in-memory file set keyed by normalized path.
///
/// Keys are normalized lexically (`.` and `..` collapse, separators become
/// `/`), so `brand/../logo.png` and `logo.png` name the same file. A leading
/// `/` is kept, so `/logo.png` and `logo.png` are distinct. A path is a
/// directory when some file lies under it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemFs {
    files: BTreeMap<String, Vec<u8>>,
}

impl MemFs {
    /// An empty file set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Store `bytes` at `path`, replacing any file there.
    pub fn insert(&mut self, path: impl AsRef<Path>, bytes: impl Into<Vec<u8>>) {
        self.files.insert(key(path.as_ref()), bytes.into());
    }

    /// This file set with `bytes` stored at `path`.
    #[must_use]
    pub fn with(mut self, path: impl AsRef<Path>, bytes: impl Into<Vec<u8>>) -> Self {
        self.insert(path, bytes);
        self
    }

    /// The bytes stored at `path`, when present.
    #[must_use]
    pub fn get(&self, path: impl AsRef<Path>) -> Option<&[u8]> {
        self.files.get(&key(path.as_ref())).map(Vec::as_slice)
    }

    /// Every stored path, in sorted order.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.files.keys().map(String::as_str)
    }

    /// The number of stored files.
    #[must_use]
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// `true` when no file is stored.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

impl SourceFs for MemFs {
    fn read(&self, path: &Path) -> Result<Vec<u8>, FsError> {
        self.files
            .get(&key(path))
            .cloned()
            .ok_or_else(|| FsError::not_found(MEM_NOT_FOUND_MESSAGE))
    }

    fn exists(&self, path: &Path) -> bool {
        let k = key(path);
        if self.files.contains_key(&k) {
            return true;
        }
        let prefix = if k.is_empty() || k.ends_with('/') {
            k
        } else {
            format!("{k}/")
        };
        self.files
            .range(prefix.clone()..)
            .next()
            .is_some_and(|(stored, _)| stored.starts_with(&prefix))
    }

    fn is_file(&self, path: &Path) -> bool {
        self.files.contains_key(&key(path))
    }
}

/// The storage key of `path`: lexically normalized, `/`-separated, with `.`
/// as the empty key.
fn key(path: &Path) -> String {
    let normalized = normalize_lexically(path);
    let mut out = String::new();
    for component in normalized.components() {
        match component {
            Component::Prefix(prefix) => out.push_str(&prefix.as_os_str().to_string_lossy()),
            Component::RootDir => out.push('/'),
            Component::CurDir => {}
            Component::ParentDir => push_part(&mut out, ".."),
            Component::Normal(part) => push_part(&mut out, &part.to_string_lossy()),
        }
    }
    out
}

fn push_part(out: &mut String, part: &str) {
    if !out.is_empty() && !out.ends_with('/') {
        out.push('/');
    }
    out.push_str(part);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::source_fs::FsErrorKind;

    #[test]
    fn paths_normalize_to_one_key() {
        let fs = MemFs::new().with("brand/../logo.png", b"a".to_vec());
        assert_eq!(fs.get("./logo.png"), Some(&b"a"[..]));
        assert!(fs.is_file(Path::new("logo.png")));
        assert_eq!(fs.paths().collect::<Vec<_>>(), vec!["logo.png"]);
    }

    #[test]
    fn rooted_and_relative_paths_differ() {
        let fs = MemFs::new().with("/p/a.zen", b"x".to_vec());
        assert!(fs.is_file(Path::new("/p/a.zen")));
        assert!(!fs.is_file(Path::new("p/a.zen")));
    }

    #[test]
    fn directories_exist_but_are_not_files() {
        let fs = MemFs::new().with("fonts/a.ttf", b"x".to_vec());
        assert!(fs.exists(Path::new("fonts")));
        assert!(!fs.is_file(Path::new("fonts")));
        assert!(!fs.exists(Path::new("font")));
        assert!(fs.exists(Path::new("")));
    }

    #[test]
    fn absent_read_is_not_found() {
        let err = MemFs::new().read(Path::new("x")).expect_err("absent");
        assert_eq!(err.kind, FsErrorKind::NotFound);
        assert_eq!(err.message, MEM_NOT_FOUND_MESSAGE);
    }

    #[test]
    fn read_to_string_rejects_invalid_utf8() {
        let fs = MemFs::new().with("t.txt", vec![0xff, 0xfe]);
        let err = fs.read_to_string(Path::new("t.txt")).expect_err("bad utf8");
        assert_eq!(err.message, crate::io::source_fs::INVALID_UTF8_MESSAGE);
    }
}
