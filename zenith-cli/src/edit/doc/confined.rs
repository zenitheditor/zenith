//! [`ConfinedFs`]: project file reads limited to one directory tree.

use std::path::{Path, PathBuf};

use zenith_pipeline::path::normalize_lexically;
use zenith_pipeline::{FsError, SourceFs};

use crate::native::NativeFs;

/// Reads project files through [`NativeFs`], but only under `root`.
///
/// A path is checked twice: lexically (after `.` / `..` collapse) and after
/// the OS resolves every symlink. A path outside `root` by either check
/// reads as an error, so a symlink inside the tree that points out of it
/// does not escape.
pub(crate) struct ConfinedFs<'a> {
    root: &'a Path,
}

impl<'a> ConfinedFs<'a> {
    /// Confine reads to `root`, a canonical directory.
    pub(crate) fn new(root: &'a Path) -> Self {
        Self { root }
    }

    /// The canonical path of `path` when it is inside the root.
    fn check(&self, path: &Path) -> Result<PathBuf, FsError> {
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        };
        let lexical = normalize_lexically(&absolute);
        if !lexical.starts_with(self.root) {
            return Err(self.outside(path));
        }
        let canonical = std::fs::canonicalize(&lexical).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                FsError::not_found(e.to_string())
            } else {
                FsError::other(e.to_string())
            }
        })?;
        if canonical.starts_with(self.root) {
            Ok(canonical)
        } else {
            Err(self.outside(path))
        }
    }

    fn outside(&self, path: &Path) -> FsError {
        FsError::other(format!(
            "'{}' is outside the editor root '{}'; move the file under the root, or restart \
             `zenith edit` with `--root <dir>` set to a directory that holds both",
            path.display(),
            self.root.display()
        ))
    }
}

impl SourceFs for ConfinedFs<'_> {
    fn read(&self, path: &Path) -> Result<Vec<u8>, FsError> {
        let canonical = self.check(path)?;
        NativeFs.read(&canonical)
    }

    fn exists(&self, path: &Path) -> bool {
        self.check(path).is_ok_and(|p| p.exists())
    }

    fn is_file(&self, path: &Path) -> bool {
        self.check(path).is_ok_and(|p| p.is_file())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = std::fs::canonicalize(dir.path()).expect("canonical");
        (dir, root)
    }

    #[test]
    fn reads_inside_the_root() {
        let (_dir, root) = root();
        std::fs::write(root.join("a.txt"), b"hi").expect("write");
        let fs = ConfinedFs::new(&root);
        assert_eq!(fs.read(&root.join("a.txt")).expect("read"), b"hi");
        assert_eq!(fs.read(Path::new("a.txt")).expect("relative"), b"hi");
        assert!(fs.is_file(&root.join("a.txt")));
        assert!(fs.exists(&root));
    }

    #[test]
    fn dot_dot_out_of_the_root_is_refused() {
        let (_dir, root) = root();
        let fs = ConfinedFs::new(&root);
        let err = fs.read(&root.join("../etc/passwd")).expect_err("outside");
        assert!(err.message.contains("outside the editor root"), "{err}");
        assert!(!fs.exists(&root.join("..")));
    }

    #[cfg(unix)]
    #[test]
    fn symlink_escape_is_refused() {
        let (_dir, root) = root();
        let (_other, outside) = self::root();
        std::fs::write(outside.join("secret.txt"), b"secret").expect("write");
        std::os::unix::fs::symlink(outside.join("secret.txt"), root.join("link.txt"))
            .expect("symlink");
        let fs = ConfinedFs::new(&root);
        let err = fs.read(&root.join("link.txt")).expect_err("escape");
        assert!(err.message.contains("outside the editor root"), "{err}");
        assert!(!fs.is_file(&root.join("link.txt")));
    }
}
