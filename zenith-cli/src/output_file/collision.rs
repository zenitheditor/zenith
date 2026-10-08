//! Destination reservations protect inputs and already committed outputs.

use super::identity::destination_identity;
use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
};

pub(crate) struct OutputGuard {
    protected: Vec<PathBuf>,
    committed: BTreeMap<PathBuf, PathBuf>,
    pending: Vec<PathBuf>,
}
impl OutputGuard {
    pub(crate) fn new(protected: &[&Path]) -> io::Result<Self> {
        check_distinct(protected.iter().copied())?;
        Ok(Self {
            protected: protected.iter().map(|path| path.to_path_buf()).collect(),
            committed: BTreeMap::new(),
            pending: Vec::new(),
        })
    }
    pub(crate) fn check(&mut self, path: &Path) -> io::Result<()> {
        for committed in &self.pending {
            self.committed
                .insert(destination_identity(committed)?, committed.clone());
        }
        self.pending.clear();
        let identity = destination_identity(path)?;
        for protected in &self.protected {
            if destination_identity(protected)? == identity {
                return Err(collision(path, protected));
            }
        }
        if let Some(previous) = self.committed.get(&identity) {
            return Err(collision(path, previous));
        }
        Ok(())
    }
    pub(crate) fn write(&mut self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        self.check(path)?;
        super::replacement::write_bytes(path, bytes)?;
        self.pending.push(path.to_path_buf());
        Ok(())
    }
}

pub(crate) fn check_distinct<'a>(paths: impl IntoIterator<Item = &'a Path>) -> io::Result<()> {
    let mut identities = BTreeMap::new();
    for path in paths {
        let identity = destination_identity(path)?;
        if let Some(previous) = identities.insert(identity, path) {
            return Err(collision(path, previous));
        }
    }
    Ok(())
}
fn collision(path: &Path, previous: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!(
            "output destination collision: '{}' aliases '{}'. Choose distinct output and input paths",
            path.display(),
            previous.display()
        ),
    )
}
