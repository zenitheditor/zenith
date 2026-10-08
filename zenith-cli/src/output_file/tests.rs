//! Collision checks use destination entries rather than shared hardlink inodes.

use super::{OutputGuard, check_distinct, identity::destination_identity};
use std::fs;

#[test]
fn missing_ancestors_and_relative_components_have_one_identity() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("missing/nested/output.svg");
    let second = directory.path().join("missing/other/../nested/output.svg");
    assert_eq!(
        destination_identity(&first).unwrap(),
        destination_identity(&second).unwrap()
    );
    assert!(check_distinct([first.as_path(), second.as_path()]).is_err());
    assert!(!directory.path().join("missing").exists());
}

#[test]
fn committed_destinations_reject_native_case_aliases_when_supported() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("capture.svg");
    let alias = directory.path().join("CAPTURE.svg");
    let mut guard = OutputGuard::new(&[]).unwrap();
    guard.write(&first, b"first").unwrap();
    if !alias.exists() {
        return;
    }
    assert!(guard.write(&alias, b"replacement").is_err());
    assert_eq!(fs::read(first).unwrap(), b"first");
}

#[test]
fn separate_existing_outputs_remain_replaceable() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("first.svg");
    let second = directory.path().join("second.svg");
    fs::write(&first, b"previous first").unwrap();
    fs::write(&second, b"previous second").unwrap();
    let mut guard = OutputGuard::new(&[]).unwrap();
    guard.write(&first, b"new first").unwrap();
    guard.write(&second, b"new second").unwrap();
    assert_eq!(fs::read(first).unwrap(), b"new first");
    assert_eq!(fs::read(second).unwrap(), b"new second");
}

#[cfg(unix)]
#[test]
fn changed_final_symlink_cannot_replace_a_committed_output() {
    use std::os::unix::fs::symlink;
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("first.svg");
    let second = directory.path().join("second.svg");
    let mut guard = OutputGuard::new(&[]).unwrap();
    guard.check(&first).unwrap();
    guard.check(&second).unwrap();
    guard.write(&first, b"first").unwrap();
    symlink("first.svg", &second).unwrap();
    assert!(guard.write(&second, b"replacement").is_err());
    assert_eq!(fs::read(first).unwrap(), b"first");
}

#[cfg(unix)]
#[test]
fn distinct_hardlink_entries_keep_independent_replacement_semantics() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("first.svg");
    let second = directory.path().join("second.svg");
    fs::write(&first, b"previous").unwrap();
    fs::hard_link(&first, &second).unwrap();
    let mut guard = OutputGuard::new(&[first.as_path()]).unwrap();
    guard.write(&second, b"new").unwrap();
    assert_eq!(fs::read(first).unwrap(), b"previous");
    assert_eq!(fs::read(second).unwrap(), b"new");
}

#[test]
fn regular_file_intermediate_cannot_be_removed_by_parent_components() {
    let directory = tempfile::tempdir().unwrap();
    let intermediate = directory.path().join("file");
    let output = directory.path().join("output.svg");
    fs::write(&intermediate, b"regular file").unwrap();
    fs::write(&output, b"previous").unwrap();
    let path = intermediate.join("..").join("output.svg");
    assert_eq!(
        destination_identity(&path).unwrap_err().kind(),
        std::io::ErrorKind::NotADirectory
    );
    assert_eq!(
        super::write_bytes(&path, b"replacement")
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::NotADirectory
    );
    assert_eq!(fs::read(&output).unwrap(), b"previous");
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
}

#[cfg(unix)]
#[test]
fn symlink_to_regular_file_intermediate_cannot_be_removed_by_parent_components() {
    use std::os::unix::fs::symlink;
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("file"), b"regular file").unwrap();
    let link = directory.path().join("link");
    symlink("file", &link).unwrap();
    let output = directory.path().join("output.svg");
    fs::write(&output, b"previous").unwrap();
    let path = link.join("..").join("output.svg");
    assert_eq!(
        destination_identity(&path).unwrap_err().kind(),
        std::io::ErrorKind::NotADirectory
    );
    assert_eq!(
        super::write_bytes(&path, b"replacement")
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::NotADirectory
    );
    assert_eq!(fs::read(output).unwrap(), b"previous");
    assert_eq!(fs::read_link(link).unwrap(), std::path::Path::new("file"));
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 3);
}

#[test]
fn missing_intermediate_parent_components_do_not_redirect_replacement() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("output.svg");
    fs::write(&output, b"previous").unwrap();
    let path = directory
        .path()
        .join("missing")
        .join("..")
        .join("output.svg");
    assert_eq!(
        destination_identity(&path).unwrap(),
        destination_identity(&output).unwrap()
    );
    assert_eq!(
        super::write_bytes(&path, b"replacement")
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::NotFound
    );
    assert_eq!(fs::read(output).unwrap(), b"previous");
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn regular_file_current_directory_suffix_does_not_redirect_replacement() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("file");
    fs::write(&file, b"previous").unwrap();
    let path = file.join(".");
    assert_eq!(
        super::write_bytes(&path, b"replacement")
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::NotADirectory
    );
    assert_eq!(fs::read(file).unwrap(), b"previous");
}
