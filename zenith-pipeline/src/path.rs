//! Lexical path math shared by import resolution and [`MemFs`](crate::MemFs).
//! No filesystem access.

use std::path::{Component, Path, PathBuf};

/// Resolve an import `src` against the importing document's directory and
/// collapse `.` / `..` lexically. An absolute `src` ignores `base_dir`.
#[must_use]
pub fn normalize_import_path(base_dir: &Path, src: &str) -> PathBuf {
    let raw = Path::new(src);
    let joined = if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        base_dir.join(raw)
    };
    normalize_lexically(&joined)
}

/// Collapse `.` and `..` components of `path` without touching the disk.
///
/// A `..` that cannot pop a normal component is kept. An empty result is
/// `"."`.
#[must_use]
pub fn normalize_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                let can_pop = normalized
                    .components()
                    .next_back()
                    .is_some_and(|last| matches!(last, Component::Normal(_)));
                if can_pop {
                    normalized.pop();
                } else {
                    normalized.push("..");
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    if normalized.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        normalized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapses_dot_and_dotdot() {
        assert_eq!(
            normalize_import_path(Path::new("a/b"), "../c/./d.zen"),
            PathBuf::from("a/c/d.zen")
        );
    }

    #[test]
    fn keeps_leading_dotdot_and_empty_is_dot() {
        assert_eq!(
            normalize_lexically(Path::new("../x")),
            PathBuf::from("../x")
        );
        assert_eq!(normalize_lexically(Path::new("")), PathBuf::from("."));
    }

    #[test]
    fn absolute_src_ignores_base() {
        assert_eq!(
            normalize_import_path(Path::new("a"), "/r/x.zen"),
            PathBuf::from("/r/x.zen")
        );
    }
}
