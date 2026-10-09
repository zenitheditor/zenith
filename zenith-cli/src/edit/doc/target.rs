//! [`Target`]: the document an editor session edits, and the directory tree
//! it may read.

use std::path::{Path, PathBuf};

use zenith_editor::EditorError;

/// A resolved document: canonical path, its directory, and the confinement
/// root (the directory, or a `--root` that contains it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Target {
    /// The canonical document path.
    pub(crate) path: PathBuf,
    /// The canonical document directory. Imports and assets resolve here.
    pub(crate) dir: PathBuf,
    /// The canonical directory every project read stays under.
    pub(crate) root: PathBuf,
}

impl Target {
    /// Resolve `path` and the optional `root`.
    ///
    /// # Errors
    ///
    /// - `edit.missing_file` when `path` does not exist.
    /// - `edit.not_a_file` when `path` is not a regular file.
    /// - `edit.bad_root` when `root` does not exist or does not hold the
    ///   document.
    pub(crate) fn resolve(path: &Path, root: Option<&Path>) -> Result<Target, EditorError> {
        let canonical = std::fs::canonicalize(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                EditorError::new(
                    "edit.missing_file",
                    format!(
                        "cannot open '{}': no such file; create it with `zenith new {}`, then \
                         run `zenith edit {}` again",
                        path.display(),
                        path.display(),
                        path.display()
                    ),
                )
            } else {
                EditorError::new(
                    "edit.read_failed",
                    format!(
                        "cannot open '{}': {e}; check the path and its permissions",
                        path.display()
                    ),
                )
            }
        })?;
        if !canonical.is_file() {
            return Err(EditorError::new(
                "edit.not_a_file",
                format!(
                    "cannot open '{}': it is not a regular file; pass the path of a .zen document",
                    path.display()
                ),
            ));
        }
        let dir = canonical.parent().map(Path::to_path_buf).ok_or_else(|| {
            EditorError::new(
                "edit.not_a_file",
                format!(
                    "cannot open '{}': it has no parent directory",
                    path.display()
                ),
            )
        })?;
        let root = match root {
            None => dir.clone(),
            Some(r) => {
                let canonical_root = std::fs::canonicalize(r).map_err(|e| {
                    EditorError::new(
                        "edit.bad_root",
                        format!(
                            "cannot use root '{}': {e}; pass an existing directory",
                            r.display()
                        ),
                    )
                })?;
                if !dir.starts_with(&canonical_root) {
                    return Err(EditorError::new(
                        "edit.bad_root",
                        format!(
                            "root '{}' does not contain '{}'; pass a root that holds the document",
                            r.display(),
                            path.display()
                        ),
                    ));
                }
                canonical_root
            }
        };
        Ok(Target {
            path: canonical,
            dir,
            root,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_names_the_path_and_the_next_step() {
        let err = Target::resolve(Path::new("/no/such/doc.zen"), None).expect_err("missing");
        assert_eq!(err.code, "edit.missing_file");
        assert!(err.message.contains("'/no/such/doc.zen'"), "{err}");
        assert!(err.message.contains("zenith new /no/such/doc.zen"), "{err}");
    }

    #[test]
    fn root_must_hold_the_document() {
        let dir = tempfile::tempdir().expect("tempdir");
        let doc = dir.path().join("a").join("d.zen");
        std::fs::create_dir_all(doc.parent().expect("parent")).expect("mkdir");
        std::fs::write(&doc, "x").expect("write");
        let t = Target::resolve(&doc, Some(dir.path())).expect("resolve");
        assert_eq!(t.root, std::fs::canonicalize(dir.path()).expect("canon"));
        assert_eq!(t.dir, t.path.parent().expect("parent"));
        let other = tempfile::tempdir().expect("other");
        let err = Target::resolve(&doc, Some(other.path())).expect_err("outside");
        assert_eq!(err.code, "edit.bad_root");
        let err = Target::resolve(dir.path(), None).expect_err("dir");
        assert_eq!(err.code, "edit.not_a_file");
    }
}
