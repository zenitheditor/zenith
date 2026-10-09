//! The native host of the render pipeline: `std::fs` files, `$HOME` config,
//! OS font directories, a bounded page thread pool, and stderr warnings.
//!
//! Every CLI command that parses, validates, compiles, or renders passes
//! [`host`] to `zenith-pipeline`. The browser editor passes an in-memory
//! host instead, so both report the same diagnostics and bytes.

use std::collections::BTreeSet;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

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

/// Project files on the local disk through `std::fs`. Error messages are the
/// OS error text.
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeFs;

impl SourceFs for NativeFs {
    fn read(&self, path: &Path) -> Result<Vec<u8>, FsError> {
        std::fs::read(path).map_err(fs_error)
    }

    fn read_to_string(&self, path: &Path) -> Result<String, FsError> {
        std::fs::read_to_string(path).map_err(fs_error)
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
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
        FsConfig::new(&NativeFs, global_config_path()).global()
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
