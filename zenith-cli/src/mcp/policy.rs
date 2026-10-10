//! The file-access policy of the MCP server.
//!
//! - **Root.** `zenith mcp --root <DIR>` confines every path a tool takes
//!   (documents, outputs, data, bundles) to `DIR`. The HTTP transport
//!   defaults the root to the working directory. Stdio has no root unless
//!   one is given: its client already runs as the user.
//! - **Editor documents.** `zenith_editor_*` open and save only `.zen`
//!   files (the canonical path too, so a `.zen` link to another file is
//!   refused). The read root is the document's directory, or the `root`
//!   argument. A filesystem root (`/`, `C:\`) is refused as a read root
//!   unless the server root is that filesystem root.

use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

use crate::edit::doc::{Target, canonicalize};

static ROOT: OnceLock<PathBuf> = OnceLock::new();

/// Set the server root to `dir`. Call once, before serving.
///
/// # Errors
///
/// A message when `dir` is not an existing directory, or a root was set
/// before.
pub fn set_root(dir: &Path) -> Result<PathBuf, String> {
    let canonical = canonicalize(dir).map_err(|e| {
        format!(
            "--root '{}': {e}; pass an existing directory",
            dir.display()
        )
    })?;
    if !canonical.is_dir() {
        return Err(format!(
            "--root '{}' is not a directory; pass the project directory",
            dir.display()
        ));
    }
    ROOT.set(canonical.clone())
        .map_err(|_| "the MCP root is already set".to_owned())?;
    // Every file the tools read (a document's imports, assets, data, and
    // config too) stays under the root.
    crate::native::confine_reads(canonical.clone())?;
    Ok(canonical)
}

/// The server root, when one is set.
fn root() -> Option<&'static Path> {
    ROOT.get().map(PathBuf::as_path)
}

/// Check that `path` (which may not exist yet) lies under the server root.
/// Always `Ok` without a root.
///
/// # Errors
///
/// A message naming the path, the root, and the next action.
pub(crate) fn confine(path: &Path) -> Result<(), String> {
    let Some(root) = root() else {
        return Ok(());
    };
    confine_to(root, path)
}

fn confine_to(root: &Path, path: &Path) -> Result<(), String> {
    let resolved = resolve_lenient(path).map_err(|e| {
        format!(
            "cannot check '{}' against the MCP root '{}': {e}",
            path.display(),
            root.display()
        )
    })?;
    if resolved.starts_with(root) {
        Ok(())
    } else {
        Err(outside(path, root))
    }
}

fn outside(path: &Path, root: &Path) -> String {
    format!(
        "'{}' is outside the MCP root '{}'; pass a path under it, or start `zenith mcp --root \
         <DIR>` with a root that holds it",
        path.display(),
        root.display()
    )
}

/// `path` made absolute, with its longest existing ancestor canonical (links
/// resolved) and the rest appended. The rest may hold no `..`.
fn resolve_lenient(path: &Path) -> std::io::Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut rest: Vec<&std::ffi::OsStr> = Vec::new();
    let mut base = absolute.as_path();
    loop {
        match canonicalize(base) {
            Ok(canonical) => {
                let mut out = canonical;
                for part in rest.iter().rev() {
                    out.push(part);
                }
                return Ok(out);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        let name = match base.components().next_back() {
            Some(Component::Normal(name)) => name,
            Some(Component::CurDir) => std::ffi::OsStr::new(""),
            Some(Component::ParentDir) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "a '..' after a missing directory cannot be checked; pass a plain path",
                ));
            }
            Some(Component::RootDir | Component::Prefix(_)) | None => {
                return Err(std::io::Error::from(std::io::ErrorKind::NotFound));
            }
        };
        if !name.is_empty() {
            rest.push(name);
        }
        base = base
            .parent()
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))?;
    }
}

/// `true` when `path` ends in `.zen` (any case).
fn is_zen(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("zen"))
}

/// `true` for `/` or a drive root: a path with no parent.
fn is_filesystem_root(path: &Path) -> bool {
    path.parent().is_none()
}

/// Resolve the document an editor tool opens: `.zen`, under the server
/// root, with a read root that is not a filesystem root.
///
/// # Errors
///
/// A message that names the refused path and the next action.
pub(crate) fn editor_target(path: &str, read_root: Option<&str>) -> Result<Target, String> {
    editor_target_in(root(), path, read_root)
}

fn editor_target_in(
    server_root: Option<&Path>,
    path: &str,
    read_root: Option<&str>,
) -> Result<Target, String> {
    let given = Path::new(path);
    if !is_zen(given) {
        return Err(not_zen(path));
    }
    let target = Target::resolve(given, read_root.map(Path::new))
        .map_err(|e| format!("{}: {}", e.code, e.message))?;
    if !is_zen(&target.path) {
        return Err(format!(
            "'{path}' links to '{}', which is not a .zen document; open the .zen file itself",
            target.path.display()
        ));
    }
    let allowed_root = server_root.is_some_and(is_filesystem_root);
    if is_filesystem_root(&target.root) && !allowed_root {
        return Err(format!(
            "the read root '{}' is the whole filesystem; pass `root` set to the project \
             directory, or start `zenith mcp --root {}` to allow it",
            target.root.display(),
            target.root.display()
        ));
    }
    if let Some(server_root) = server_root
        && !target.root.starts_with(server_root)
    {
        return Err(outside(&target.root, server_root));
    }
    Ok(target)
}

fn not_zen(path: &str) -> String {
    format!(
        "'{path}' is not a .zen document; the zenith_editor tools open and save only .zen files"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "zenith version=1 {\n}\n";

    #[test]
    fn editor_documents_must_be_zen() {
        let dir = tempfile::tempdir().expect("dir");
        let rc = dir.path().join(".bashrc");
        std::fs::write(&rc, "x").expect("write");
        let err = editor_target_in(None, rc.to_str().expect("utf8"), None).expect_err("rc");
        assert!(err.contains("not a .zen document"), "{err}");
        let doc = dir.path().join("a.ZEN");
        std::fs::write(&doc, DOC).expect("write");
        assert!(editor_target_in(None, doc.to_str().expect("utf8"), None).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn a_zen_link_to_another_file_is_refused() {
        let dir = tempfile::tempdir().expect("dir");
        let rc = dir.path().join("rc");
        std::fs::write(&rc, "x").expect("write");
        let link = dir.path().join("evil.zen");
        std::os::unix::fs::symlink(&rc, &link).expect("link");
        let err = editor_target_in(None, link.to_str().expect("utf8"), None).expect_err("link");
        assert!(err.contains("not a .zen document"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn filesystem_root_needs_the_server_root() {
        let dir = tempfile::tempdir().expect("dir");
        let doc = dir.path().join("d.zen");
        std::fs::write(&doc, DOC).expect("write");
        let path = doc.to_str().expect("utf8");
        let err = editor_target_in(None, path, Some("/")).expect_err("root /");
        assert!(err.contains("whole filesystem"), "{err}");
        assert!(editor_target_in(Some(Path::new("/")), path, Some("/")).is_ok());
    }

    #[test]
    fn the_server_root_bounds_documents_and_outputs() {
        let root = tempfile::tempdir().expect("root");
        let root_path = canonicalize(root.path()).expect("canon");
        let inside = root.path().join("d.zen");
        std::fs::write(&inside, DOC).expect("write");
        let other = tempfile::tempdir().expect("other");
        let outside_doc = other.path().join("d.zen");
        std::fs::write(&outside_doc, DOC).expect("write");
        assert!(editor_target_in(Some(&root_path), inside.to_str().expect("utf8"), None).is_ok());
        let err = editor_target_in(Some(&root_path), outside_doc.to_str().expect("utf8"), None)
            .expect_err("outside");
        assert!(err.contains("outside the MCP root"), "{err}");
        assert!(confine_to(&root_path, &root.path().join("new/dir/out.png")).is_ok());
        assert!(confine_to(&root_path, &other.path().join("out.png")).is_err());
        let sneaky = root.path().join("missing/../../escape.png");
        assert!(confine_to(&root_path, &sneaky).is_err());
    }
}
