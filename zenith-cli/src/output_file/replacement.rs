//! Complete sibling writes replace outputs only after writing, flushing, and
//! syncing the new bytes to disk. After the rename, the directory is synced
//! too (Unix), so a crash leaves either the old file or the whole new one.
//! Directory creation remains caller-owned.

use std::{
    fs::{self, File},
    io::{self, Write},
    path::Path,
};

pub(crate) fn write_bytes(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_with(path, |file| file.write_all(bytes))
}

/// Check that [`write_bytes`] may replace `path`: it is a regular file that
/// is not read-only, or it does not exist yet. A caller runs this before
/// work that must not happen for a write that cannot land (history).
///
/// # Errors
///
/// `InvalidInput` for a non-file, `PermissionDenied` for a read-only file,
/// or the metadata error.
pub(crate) fn check_replaceable(path: &Path) -> io::Result<()> {
    let destination = super::identity::replacement_destination(path)?;
    existing_permissions(path, &destination).map(|_| ())
}

/// The permissions of an existing `destination`, `None` when it is missing.
fn existing_permissions(path: &Path, destination: &Path) -> io::Result<Option<fs::Permissions>> {
    match fs::metadata(destination) {
        Ok(metadata) => {
            if !metadata.is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("output '{}' is not a regular file", path.display()),
                ));
            }
            let permissions = metadata.permissions();
            if permissions.readonly() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!("output '{}' is read-only", path.display()),
                ));
            }
            Ok(Some(permissions))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn write_with(path: &Path, write: impl FnOnce(&mut File) -> io::Result<()>) -> io::Result<()> {
    let destination = super::identity::replacement_destination(path)?;
    let permissions = existing_permissions(path, &destination)?;
    let parent = destination.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "output lacks a parent directory",
        )
    })?;
    let mut builder = tempfile::Builder::new();
    builder.prefix(".zenith-output-");
    if let Some(permissions) = &permissions {
        // Restrict the temporary before writing any destination contents.
        builder.permissions(permissions.clone());
    } else {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // Apply the process umask through file creation, matching File::create.
            builder.permissions(fs::Permissions::from_mode(0o666));
        }
    }
    let mut temporary = builder.tempfile_in(parent)?;
    write(temporary.as_file_mut())?;
    temporary.as_file_mut().flush()?;
    if let Some(permissions) = permissions {
        temporary.as_file().set_permissions(permissions)?;
    }
    // The bytes reach the disk before the rename can expose them.
    temporary.as_file().sync_all()?;
    temporary.persist(&destination).map_err(|error| {
        let tempfile::PersistError { error, file } = error;
        drop(file);
        error
    })?;
    sync_directory(parent);
    Ok(())
}

/// Make the rename in `dir` durable. Best effort: some filesystems refuse
/// to sync a directory, and the new bytes are already on disk.
#[cfg(unix)]
fn sync_directory(dir: &Path) {
    if let Ok(handle) = File::open(dir) {
        let _ = handle.sync_all();
    }
}

/// Windows cannot open a directory as a file; the rename is journaled by
/// NTFS.
#[cfg(not(unix))]
fn sync_directory(_dir: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn entries(path: &Path) -> Vec<PathBuf> {
        let mut paths: Vec<_> = fs::read_dir(path)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        paths.sort();
        paths
    }

    #[test]
    fn replacement_writes_complete_bytes_without_sibling_remnants() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("output.svg");
        fs::write(&path, b"previous").unwrap();
        write_bytes(&path, b"complete replacement").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"complete replacement");
        assert_eq!(entries(directory.path()), vec![path]);
    }

    #[test]
    fn interrupted_write_retains_existing_output_and_cleans_temporary() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("output.svg");
        fs::write(&path, b"previous").unwrap();
        let error = write_with(&path, |file| {
            file.write_all(b"incomplete replacement")?;
            Err(io::Error::other("injected write error"))
        })
        .unwrap_err();
        assert_eq!(error.to_string(), "injected write error");
        assert_eq!(fs::read(&path).unwrap(), b"previous");
        assert_eq!(entries(directory.path()), vec![path]);
    }

    #[test]
    fn directory_destinations_and_missing_parents_remain_intact() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("output.svg");
        fs::create_dir(&path).unwrap();
        assert!(write_bytes(&path, b"replacement").is_err());
        assert!(path.is_dir());
        assert!(write_bytes(&path.join("missing/output.svg"), b"replacement").is_err());
        assert_eq!(entries(directory.path()), vec![path]);
    }

    #[test]
    fn read_only_outputs_reject_replacement() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("output.svg");
        fs::write(&path, b"previous").unwrap();
        let original = fs::metadata(&path).unwrap().permissions();
        let mut permissions = original.clone();
        permissions.set_readonly(true);
        fs::set_permissions(&path, permissions).unwrap();
        let result = write_bytes(&path, b"replacement");
        fs::set_permissions(&path, original).unwrap();
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(fs::read(&path).unwrap(), b"previous");
        assert_eq!(entries(directory.path()), vec![path]);
    }

    #[cfg(unix)]
    #[test]
    fn replacement_preserves_modes_and_creation_matches_normal_umask() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let normal = directory.path().join("normal");
        fs::write(&normal, b"normal").unwrap();
        let path = directory.path().join("output.svg");
        write_bytes(&path, b"new").unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode(),
            fs::metadata(&normal).unwrap().permissions().mode()
        );
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        write_bytes(&path, b"replacement").unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o640
        );
    }

    #[cfg(unix)]
    #[test]
    fn temporary_permissions_restrict_private_contents_before_writing() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("private.svg");
        fs::write(&path, b"previous private content").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        write_with(&path, |file| {
            let mode = file.metadata()?.permissions().mode() & 0o777;
            assert_eq!(
                mode & !0o600,
                0,
                "temporary exposes additional permission bits"
            );
            file.write_all(b"replacement private content")
        })
        .unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"replacement private content");
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(entries(directory.path()), vec![path]);
    }

    #[cfg(unix)]
    #[test]
    fn relative_links_and_dangling_link_targets_retain_links() {
        use std::os::unix::fs::symlink;
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("target.svg");
        let link = directory.path().join("output.svg");
        symlink("target.svg", &link).unwrap();
        write_bytes(&link, b"new target").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"new target");
        assert_eq!(fs::read_link(&link).unwrap(), Path::new("target.svg"));
        write_bytes(&link, b"replacement target").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"replacement target");
        assert_eq!(entries(directory.path()), vec![link, target]);
    }

    #[cfg(unix)]
    #[test]
    fn link_cycles_return_errors_without_replacing_links() {
        use std::os::unix::fs::symlink;
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("first.svg");
        let second = directory.path().join("second.svg");
        symlink("second.svg", &first).unwrap();
        symlink("./first.svg", &second).unwrap();
        assert!(
            write_bytes(&first, b"replacement")
                .unwrap_err()
                .to_string()
                .contains("symlink cycle")
        );
        assert_eq!(fs::read_link(&first).unwrap(), Path::new("second.svg"));
        assert_eq!(fs::read_link(&second).unwrap(), Path::new("./first.svg"));
        assert_eq!(entries(directory.path()), vec![first, second]);
    }

    #[test]
    fn check_replaceable_matches_what_a_write_accepts() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("output.svg");
        check_replaceable(&path).unwrap();
        fs::write(&path, b"previous").unwrap();
        check_replaceable(&path).unwrap();
        let original = fs::metadata(&path).unwrap().permissions();
        let mut permissions = original.clone();
        permissions.set_readonly(true);
        fs::set_permissions(&path, permissions).unwrap();
        let result = check_replaceable(&path);
        fs::set_permissions(&path, original).unwrap();
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(
            check_replaceable(directory.path()).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
    }

    #[test]
    fn persistence_errors_clean_completed_temporary_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("output.svg");
        let result = write_with(&path, |file| {
            file.write_all(b"complete")?;
            fs::create_dir(&path)
        });
        assert!(result.is_err());
        assert!(path.is_dir());
        assert_eq!(entries(directory.path()), vec![path]);
    }
}
