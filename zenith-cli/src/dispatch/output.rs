//! Small output helpers shared by the dispatch submodules.

use crate::report::CliError;

pub(super) use crate::edit_io::{apply_edit, write_error};

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
