//! [`canonicalize`]: canonical paths without the Windows verbatim prefix.
//!
//! On Windows `std::fs::canonicalize` returns `\\?\C:\dir\file` or
//! `\\?\UNC\server\share\file`. Under that prefix `/` is no separator and
//! `..` does not collapse, so `root.join("brand/logo.png")` names no file,
//! and a plain `C:\dir` never `starts_with` the verbatim form. Every
//! canonical path the editor compares or joins goes through [`canonicalize`],
//! which drops the prefix when the plain form names the same file. On other
//! systems a canonical path starts with `/` and passes unchanged.

use std::path::{Path, PathBuf};

/// `std::fs::canonicalize`, then the plain form of a verbatim result.
///
/// # Errors
///
/// The error of `std::fs::canonicalize`.
pub(crate) fn canonicalize(path: &Path) -> std::io::Result<PathBuf> {
    std::fs::canonicalize(path).map(simplify)
}

/// `path` without a `\\?\` drive or `\\?\UNC\` prefix, when the plain form
/// names the same file. Any other path comes back unchanged.
fn simplify(path: PathBuf) -> PathBuf {
    let Some(text) = path.to_str() else {
        return path;
    };
    let (plain, skip) = if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        // `server` and `share` lead the rest.
        (format!(r"\\{rest}"), 2)
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        if !is_drive_root(rest) {
            return path;
        }
        (rest.to_owned(), 1)
    } else {
        return path;
    };
    let names = plain.trim_start_matches('\\').split('\\').skip(skip);
    let mut names = names.filter(|n| !n.is_empty());
    if names.any(|n| !plain_name(n)) {
        return path;
    }
    PathBuf::from(plain)
}

/// `true` for `X:\…`.
fn is_drive_root(rest: &str) -> bool {
    matches!(rest.as_bytes(), [drive, b':', b'\\', ..] if drive.is_ascii_alphabetic())
}

/// `true` when the plain path syntax keeps file name `name` as is: no `/`,
/// no trailing dot or space, and no DOS device name (`CON`, `NUL`, `COM1`, …).
fn plain_name(name: &str) -> bool {
    if name.contains('/') || name.ends_with('.') || name.ends_with(' ') {
        return false;
    }
    let stem = name.split('.').next().unwrap_or(name).trim_end();
    let upper = stem.to_ascii_uppercase();
    let device = match upper.as_str() {
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$" => true,
        _ => {
            let numbered = upper
                .strip_prefix("COM")
                .or_else(|| upper.strip_prefix("LPT"));
            numbered.is_some_and(|n| matches!(n.as_bytes(), [b'1'..=b'9']))
        }
    };
    !device
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(p: &str) -> String {
        simplify(PathBuf::from(p)).to_string_lossy().into_owned()
    }

    #[test]
    fn drive_and_unc_prefixes_drop() {
        assert_eq!(s(r"\\?\C:\work\doc.zen"), r"C:\work\doc.zen");
        assert_eq!(s(r"\\?\d:\"), r"d:\");
        assert_eq!(
            s(r"\\?\UNC\server\share\doc.zen"),
            r"\\server\share\doc.zen"
        );
    }

    #[test]
    fn names_the_plain_form_changes_keep_the_prefix() {
        assert_eq!(s(r"\\?\C:\work\con.zen"), r"\\?\C:\work\con.zen");
        assert_eq!(s(r"\\?\C:\work\LPT1"), r"\\?\C:\work\LPT1");
        assert_eq!(s(r"\\?\C:\work\trailing."), r"\\?\C:\work\trailing.");
        assert_eq!(s(r"\\?\C:\work\space \x"), r"\\?\C:\work\space \x");
        assert_eq!(s(r"\\?\C:\a/b"), r"\\?\C:\a/b");
        assert_eq!(s(r"\\?\Volume{1234}\x"), r"\\?\Volume{1234}\x");
    }

    #[test]
    fn plain_paths_pass_unchanged() {
        assert_eq!(s("/home/u/doc.zen"), "/home/u/doc.zen");
        assert_eq!(s(r"C:\work\com10.zen"), r"C:\work\com10.zen");
        assert_eq!(s(r"\\?\C:\work\com10\lpt0"), r"C:\work\com10\lpt0");
    }

    #[test]
    fn canonical_paths_resolve() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = canonicalize(dir.path()).expect("canonical");
        assert!(path.is_absolute());
        assert!(!path.to_string_lossy().starts_with(r"\\?\"));
        assert_eq!(canonicalize(&path.join(".")).expect("again"), path);
    }
}
