//! Document write path shared by the CLI dispatchers and the MCP server.
//!
//! Every write to a `.zen` document goes through [`apply_edit`] so the edit is
//! recorded in version history before the bytes reach disk.

use std::path::Path;

use crate::history;
use crate::report::CliError;

/// Record `bytes` in history, then write them to `path`.
///
/// A history warning is printed to stderr and never blocks the write.
/// Returns the bytes written, which differ from `bytes` when history stamps a
/// `doc-id` into the document.
pub(crate) fn apply_edit(path: &Path, bytes: &[u8], label: &str) -> Result<Vec<u8>, CliError> {
    let recorded = history::record_edit(bytes, path, label);
    if let Some(w) = &recorded.warning {
        eprintln!("warning: {w}");
    }
    std::fs::write(path, &recorded.bytes).map_err(|e| write_error(path, &e))?;
    Ok(recorded.bytes)
}

/// An `io.write_failed` error for `path` (exit code 2).
pub(crate) fn write_error(path: &Path, e: &std::io::Error) -> CliError {
    CliError::new(
        "io.write_failed",
        format!(
            "error[io.write_failed]: cannot write '{}': {e}; check the directory exists and is writable",
            path.display()
        ),
        2,
    )
}
