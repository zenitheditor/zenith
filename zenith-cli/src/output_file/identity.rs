//! Side-effect-free output identity resolution, including missing ancestors.

use std::{
    collections::BTreeSet,
    fs, io,
    path::{Component, Path, PathBuf},
};

pub(super) fn destination_identity(path: &Path) -> io::Result<PathBuf> {
    resolve(path, true)
}

pub(super) fn replacement_destination(path: &Path) -> io::Result<PathBuf> {
    resolve(path, false)
}

fn resolve(path: &Path, missing_ancestors: bool) -> io::Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut pending = absolute;
    let mut visited = BTreeSet::new();
    for _ in 0..64 {
        match fs::symlink_metadata(&pending) {
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        if !visited.insert(pending.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("output '{}' contains a symlink cycle", path.display()),
            ));
        }
        let mut resolved = PathBuf::new();
        let mut redirected = None;
        let mut components = pending.components();
        while let Some(component) = components.next() {
            match component {
                Component::Prefix(prefix) => resolved.push(prefix.as_os_str()),
                Component::RootDir => resolved.push(component.as_os_str()),
                Component::CurDir => {}
                Component::ParentDir => {
                    resolved.pop();
                }
                Component::Normal(name) => {
                    resolved.push(name);
                    match fs::symlink_metadata(&resolved) {
                        Ok(metadata) if metadata.file_type().is_symlink() => {
                            let target = fs::read_link(&resolved)?;
                            resolved.pop();
                            let mut target = if target.is_absolute() {
                                target
                            } else {
                                resolved.join(target)
                            };
                            target.extend(components);
                            redirected = Some(target);
                            break;
                        }
                        Ok(metadata) => {
                            if !metadata.is_dir() && components.clone().next().is_some() {
                                return Err(io::Error::new(
                                    io::ErrorKind::NotADirectory,
                                    format!(
                                        "output '{}' traverses non-directory '{}'",
                                        path.display(),
                                        resolved.display()
                                    ),
                                ));
                            }
                            resolved = fs::canonicalize(&resolved)?;
                        }
                        Err(error) if error.kind() == io::ErrorKind::NotFound => {
                            if !missing_ancestors && components.clone().next().is_some() {
                                return Err(error);
                            }
                        }
                        Err(error) => return Err(error),
                    }
                }
            }
        }
        match redirected {
            Some(target) => pending = target,
            None => return Ok(resolved),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("output '{}' exceeds 64 symlink targets", path.display()),
    ))
}
