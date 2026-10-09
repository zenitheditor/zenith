//! Config discovery: the [`ConfigSource`] trait and the [`FsConfig`] and
//! [`NoConfig`] sources.

use std::path::{Path, PathBuf};

use super::source_fs::{FsErrorKind, SourceFs};

/// The file name of a local (per-project / per-directory) config.
pub const LOCAL_CONFIG_NAME: &str = ".zenith.kdl";

/// One config file: where it was found and its bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigFile {
    /// The path the file was read from, for error messages.
    pub path: PathBuf,
    /// The raw KDL bytes.
    pub bytes: Vec<u8>,
}

/// Where the global and local config files come from.
///
/// The pipeline parses both files for a `diagnostics` policy block and a
/// `brand` contract block. A missing file is not an error.
pub trait ConfigSource {
    /// The global config file, or `None` when there is none.
    ///
    /// # Errors
    ///
    /// Returns a message naming the file when it exists but cannot be read.
    fn global(&self) -> Result<Option<ConfigFile>, String>;

    /// The nearest local config found by walking up from `start_dir` to the
    /// root, or `None` when no directory holds one.
    ///
    /// # Errors
    ///
    /// Returns a message naming the file when it exists but cannot be read.
    fn local(&self, start_dir: &Path) -> Result<Option<ConfigFile>, String>;
}

/// A [`ConfigSource`] over a [`SourceFs`]: the global config at a fixed path
/// and the nearest [`LOCAL_CONFIG_NAME`] walking up from the document.
pub struct FsConfig<'a> {
    fs: &'a dyn SourceFs,
    global: Option<PathBuf>,
}

impl<'a> FsConfig<'a> {
    /// Read config through `fs`. `global` is the global config path, or
    /// `None` for no global config.
    #[must_use]
    pub fn new(fs: &'a dyn SourceFs, global: Option<PathBuf>) -> Self {
        Self { fs, global }
    }
}

impl ConfigSource for FsConfig<'_> {
    fn global(&self) -> Result<Option<ConfigFile>, String> {
        match &self.global {
            Some(path) => read_config(self.fs, path),
            None => Ok(None),
        }
    }

    fn local(&self, start_dir: &Path) -> Result<Option<ConfigFile>, String> {
        let mut dir: Option<&Path> = Some(start_dir);
        while let Some(current) = dir {
            let candidate = current.join(LOCAL_CONFIG_NAME);
            if self.fs.is_file(&candidate) {
                return read_config(self.fs, &candidate);
            }
            dir = current.parent();
        }
        Ok(None)
    }
}

/// A [`ConfigSource`] with no config files.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoConfig;

impl ConfigSource for NoConfig {
    fn global(&self) -> Result<Option<ConfigFile>, String> {
        Ok(None)
    }

    fn local(&self, _start_dir: &Path) -> Result<Option<ConfigFile>, String> {
        Ok(None)
    }
}

/// Read the config at `path`. Absent is `None`. Any other read error names
/// the file.
fn read_config(fs: &dyn SourceFs, path: &Path) -> Result<Option<ConfigFile>, String> {
    match fs.read(path) {
        Ok(bytes) => Ok(Some(ConfigFile {
            path: path.to_path_buf(),
            bytes,
        })),
        Err(e) if e.kind == FsErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("cannot read config '{}': {e}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MemFs;

    #[test]
    fn local_walks_up_to_the_nearest_file() {
        let fs = MemFs::new()
            .with("p/.zenith.kdl", b"outer".to_vec())
            .with("p/a/.zenith.kdl", b"inner".to_vec());
        let config = FsConfig::new(&fs, None);
        let found = config
            .local(Path::new("p/a/b"))
            .expect("read")
            .expect("found");
        assert_eq!(found.bytes, b"inner");
        assert_eq!(found.path, PathBuf::from("p/a/.zenith.kdl"));
    }

    #[test]
    fn local_walk_ends_at_root_without_a_file() {
        let fs = MemFs::new();
        let config = FsConfig::new(&fs, None);
        assert_eq!(config.local(Path::new("/x/y")).expect("read"), None);
    }

    #[test]
    fn missing_global_is_none() {
        let fs = MemFs::new();
        let config = FsConfig::new(&fs, Some(PathBuf::from("cfg/config.kdl")));
        assert_eq!(config.global().expect("read"), None);
        assert_eq!(NoConfig.global().expect("read"), None);
    }
}
