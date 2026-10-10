//! The text of a failed output write: what failed and the next action.

use std::io;
use std::path::Path;

/// What went wrong writing `path`, and what to do next. No code prefix.
///
/// - A read-only file: name it as read-only, and say how to make it
///   writable or to write a copy under another name.
/// - Another permission error: name the directory to check.
/// - Anything else: the error, and the directory to check.
pub(crate) fn write_failure(path: &Path, error: &io::Error) -> String {
    let shown = path.display();
    let readonly = std::fs::metadata(path).is_ok_and(|m| m.permissions().readonly());
    if error.kind() == io::ErrorKind::PermissionDenied && readonly {
        return format!(
            "cannot write '{shown}': the file is read-only; make it writable ({}) and try \
             again, or write a copy under another name",
            make_writable(path)
        );
    }
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map_or_else(|| ".".to_owned(), |p| p.display().to_string());
    if error.kind() == io::ErrorKind::PermissionDenied {
        return format!(
            "cannot write '{shown}': {error}; check that you may create files in '{dir}'"
        );
    }
    format!("cannot write '{shown}': {error}; check that '{dir}' exists and is writable")
}

#[cfg(windows)]
fn make_writable(path: &Path) -> String {
    format!("attrib -R \"{}\"", path.display())
}

#[cfg(not(windows))]
fn make_writable(path: &Path) -> String {
    format!("chmod u+w '{}'", path.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_files_name_the_fix() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("d.zen");
        std::fs::write(&path, "x").unwrap();
        let original = std::fs::metadata(&path).unwrap().permissions();
        let mut perms = original.clone();
        perms.set_readonly(true);
        std::fs::set_permissions(&path, perms).unwrap();
        let error = super::super::write_bytes(&path, b"y").unwrap_err();
        let text = write_failure(&path, &error);
        std::fs::set_permissions(&path, original).unwrap();
        assert!(text.contains("is read-only"), "{text}");
        assert!(text.contains("write a copy under another name"), "{text}");
        assert!(!text.contains("directory exists"), "{text}");
        #[cfg(unix)]
        assert!(text.contains("chmod u+w"), "{text}");
    }

    #[test]
    fn other_errors_name_the_directory() {
        let path = Path::new("/no/such/dir/out.svg");
        let error = io::Error::from(io::ErrorKind::NotFound);
        let text = write_failure(path, &error);
        assert!(
            text.contains("'/no/such/dir' exists and is writable"),
            "{text}"
        );
        let denied = io::Error::from(io::ErrorKind::PermissionDenied);
        let text = write_failure(path, &denied);
        assert!(
            text.contains("may create files in '/no/such/dir'"),
            "{text}"
        );
    }
}
