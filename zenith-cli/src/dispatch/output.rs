//! Small output helpers shared by the dispatch submodules.

use std::path::Path;

use crate::history;
use crate::report::CliError;

/// Print a non-fatal warning line to stderr.
pub(super) fn warn(msg: impl std::fmt::Display) {
    eprintln!("warning: {msg}");
}

/// Print `json_str` under `--json`, else `human`.
pub(super) fn print_outcome(json: bool, json_str: &str, human: &str) {
    if json {
        println!("{json_str}");
    } else {
        println!("{human}");
    }
}

/// Record `bytes` in history, then write them to `path`.
pub(super) fn apply_edit(path: &Path, bytes: &[u8], label: &str) -> Result<(), CliError> {
    let recorded = history::record_edit(bytes, path, label);
    if let Some(w) = &recorded.warning {
        warn(w);
    }
    std::fs::write(path, &recorded.bytes).map_err(|e| write_error(path, &e))
}

/// An `io.write_failed` error for `path` (exit code 2).
pub(super) fn write_error(path: &std::path::Path, e: &std::io::Error) -> CliError {
    CliError::new(
        "io.write_failed",
        format!(
            "error[io.write_failed]: cannot write '{}': {e}; check the directory exists and is writable",
            path.display()
        ),
        2,
    )
}

/// An `io.write_failed` error for a directory that cannot be created.
pub(super) fn create_dir_error(path: &std::path::Path, e: &std::io::Error) -> CliError {
    CliError::new(
        "io.write_failed",
        format!(
            "error[io.write_failed]: cannot create directory '{}': {e}; check the parent is writable",
            path.display()
        ),
        2,
    )
}
