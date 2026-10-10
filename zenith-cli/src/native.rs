//! The native host of the render pipeline: `std::fs` files, `$HOME` config,
//! OS font directories, a bounded page thread pool, and stderr warnings.
//!
//! Every CLI command that parses, validates, compiles, or renders passes
//! [`host`] to `zenith-pipeline`. The browser editor passes an in-memory
//! host instead, so both report the same diagnostics and bytes.

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use rayon::prelude::*;
use zenith_core::LocalFontEntry;
use zenith_pipeline::{
    ConfigFile, ConfigSource, FsConfig, FsError, Host, LocalFontSource, PageRunner, SourceFs,
    WarningSink,
};

use crate::commands::fonts::{font_index_path, os_font_dirs};

/// Upper bound on page worker threads.
const MAX_PAGE_THREADS: usize = 8;

/// The native host: [`NativeFs`], [`NativeConfig`], [`NativeLocalFonts`],
/// [`ThreadPoolRunner`], and [`StderrWarnings`].
#[must_use]
pub fn host() -> Host<'static> {
    Host::new(&NativeFs, &NativeConfig)
        .with_local_fonts(&NativeLocalFonts)
        .with_runner(&ThreadPoolRunner)
        .with_warnings(&StderrWarnings)
}

/// The directory every [`NativeFs`] read of this process is confined to,
/// when one is set (`zenith mcp --root`).
static READ_ROOT: OnceLock<PathBuf> = OnceLock::new();

/// Confine every [`NativeFs`] read of this process (documents, imports,
/// assets, data, config) to `root`, a canonical directory. Call once,
/// before serving.
///
/// # Errors
///
/// A message when a root was set before.
pub(crate) fn confine_reads(root: PathBuf) -> Result<(), String> {
    READ_ROOT
        .set(root)
        .map_err(|_| "the read root is already set".to_owned())
}

/// Project files on the local disk through `std::fs`. Error messages are the
/// OS error text. Under a read root (`zenith mcp --root`) a read of a path
/// outside it is an error that names the path, the root, and the next
/// action. `exists` and `is_file` still report the file, and `refusal`
/// gives that error, so callers report a refused file, not a missing one.
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeFs;

impl SourceFs for NativeFs {
    fn read(&self, path: &Path) -> Result<Vec<u8>, FsError> {
        std::fs::read(allowed(path)?).map_err(fs_error)
    }

    fn read_to_string(&self, path: &Path) -> Result<String, FsError> {
        std::fs::read_to_string(allowed(path)?).map_err(fs_error)
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }

    fn refusal(&self, path: &Path) -> Option<FsError> {
        allowed(path)
            .err()
            .filter(|e| e.kind != zenith_pipeline::FsErrorKind::NotFound)
    }
}

/// The user's own files on the local disk, never confined: the global
/// config at a fixed path under `$HOME`, which no document names.
struct UserFs;

impl SourceFs for UserFs {
    fn read(&self, path: &Path) -> Result<Vec<u8>, FsError> {
        std::fs::read(path).map_err(fs_error)
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }
}

/// `path` when this process may read it: always without a read root, and
/// with one, its canonical form when that lies under the root (checked
/// lexically and after links resolve, so a link out of the tree does not
/// escape).
fn allowed(path: &Path) -> Result<Cow<'_, Path>, FsError> {
    let Some(root) = READ_ROOT.get() else {
        return Ok(Cow::Borrowed(path));
    };
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| FsError::other(e.to_string()))?
            .join(path)
    };
    let lexical = zenith_pipeline::path::normalize_lexically(&absolute);
    let canonical = crate::edit::doc::canonicalize(&lexical).map_err(fs_error)?;
    if lexical.starts_with(root) && canonical.starts_with(root) {
        Ok(Cow::Owned(canonical))
    } else {
        Err(FsError::other(format!(
            "'{}' is outside the MCP root '{}': the MCP server reads only files under it; move \
             the file under the root, or start `zenith mcp --root <DIR>` with a root that \
             holds it",
            path.display(),
            root.display()
        )))
    }
}

fn fs_error(e: std::io::Error) -> FsError {
    if e.kind() == ErrorKind::NotFound {
        FsError::not_found(e.to_string())
    } else {
        FsError::other(e.to_string())
    }
}

/// The global config at `$HOME/.config/zenith/config.kdl`, or `None` when
/// `$HOME` is unset.
#[must_use]
pub fn global_config_path() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".config").join("zenith").join("config.kdl"))
}

/// Config on the local disk: the global file at [`global_config_path`] and
/// the nearest `.zenith.kdl` walking up from the document. `$HOME` is read on
/// every call.
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeConfig;

impl ConfigSource for NativeConfig {
    fn global(&self) -> Result<Option<ConfigFile>, String> {
        FsConfig::new(&UserFs, global_config_path()).global()
    }

    fn local(&self, start_dir: &Path) -> Result<Option<ConfigFile>, String> {
        FsConfig::new(&NativeFs, None).local(start_dir)
    }
}

/// Fonts in the OS font directories, scanned through the metadata index in
/// the user cache directory.
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeLocalFonts;

impl LocalFontSource for NativeLocalFonts {
    fn faces(&self, wanted: &BTreeSet<String>) -> Vec<LocalFontEntry> {
        zenith_core::filter_wanted_families(
            zenith_core::scan_font_dirs(&os_font_dirs(), font_index_path().as_deref()),
            wanted,
        )
    }

    fn read(&self, path: &Path) -> Option<Vec<u8>> {
        std::fs::read(path).ok()
    }
}

/// Runs page jobs on a scoped pool of `min(available_parallelism, 8, count)`
/// threads. One thread, or a pool that fails to start, runs them in order on
/// the calling thread.
#[derive(Debug, Clone, Copy, Default)]
pub struct ThreadPoolRunner;

impl PageRunner for ThreadPoolRunner {
    fn run(&self, count: usize, job: &(dyn Fn(usize) + Sync)) {
        let threads = std::thread::available_parallelism()
            .map(std::num::NonZeroUsize::get)
            .unwrap_or(1)
            .min(MAX_PAGE_THREADS)
            .min(count);
        if threads <= 1 {
            (0..count).for_each(job);
            return;
        }
        match rayon::ThreadPoolBuilder::new().num_threads(threads).build() {
            Ok(pool) => pool.install(|| (0..count).into_par_iter().for_each(job)),
            Err(_) => (0..count).for_each(job),
        }
    }
}

/// Prints each warning to stderr as `warning: <message>`.
#[derive(Debug, Clone, Copy, Default)]
pub struct StderrWarnings;

impl WarningSink for StderrWarnings {
    fn warn(&self, message: &str) {
        eprintln!("warning: {message}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use zenith_pipeline::FsErrorKind;

    #[test]
    fn missing_file_is_not_found_with_os_text() {
        let err = NativeFs
            .read(Path::new("/no/such/zenith/file.zen"))
            .expect_err("missing");
        assert_eq!(err.kind, FsErrorKind::NotFound);
        let os_text = std::fs::read("/no/such/zenith/file.zen")
            .expect_err("missing")
            .to_string();
        assert_eq!(err.message, os_text);
    }

    #[test]
    fn directory_read_is_other() {
        let dir = tempfile::tempdir().expect("tempdir");
        let err = NativeFs.read(dir.path()).expect_err("directory");
        assert_eq!(err.kind, FsErrorKind::Other);
        assert!(NativeFs.exists(dir.path()));
        assert!(!NativeFs.is_file(dir.path()));
    }

    #[test]
    fn local_config_walks_up_on_disk() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join(".zenith.kdl"), b"diagnostics {}\n").expect("write");
        let nested = dir.path().join("a").join("b");
        std::fs::create_dir_all(&nested).expect("mkdir");
        let found = NativeConfig.local(&nested).expect("read").expect("found");
        assert_eq!(found.path, dir.path().join(".zenith.kdl"));
    }

    #[test]
    fn thread_pool_runs_every_job_once() {
        let hits: Vec<AtomicUsize> = (0..32).map(|_| AtomicUsize::new(0)).collect();
        ThreadPoolRunner.run(32, &|i| {
            if let Some(h) = hits.get(i) {
                h.fetch_add(1, Ordering::SeqCst);
            }
        });
        assert!(hits.iter().all(|h| h.load(Ordering::SeqCst) == 1));
    }
}
