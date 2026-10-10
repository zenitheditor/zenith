//! Document write path shared by the CLI dispatchers, the MCP server, and the
//! `zenith edit` server.
//!
//! Every write to a `.zen` document goes through [`write_document`] so the
//! edit is recorded in version history before the bytes reach disk. The bytes
//! replace the file atomically through the protected output path: a sibling
//! temporary is written and flushed, then renamed over the destination. A
//! symlinked destination keeps its link and replaces the target.

use std::path::Path;

use crate::history;
use crate::output_file;
use crate::report::CliError;

/// The bytes one document write put on disk.
pub(crate) struct Written {
    /// The bytes written. They differ from the input when history stamps a
    /// `doc-id` into the document.
    pub(crate) bytes: Vec<u8>,
    /// A non-fatal history warning.
    pub(crate) warning: Option<String>,
}

/// Record `bytes` in history under `label`, then replace `path` with them.
///
/// A history error never blocks the write. It comes back as
/// [`Written::warning`].
pub(crate) fn write_document(path: &Path, bytes: &[u8], label: &str) -> Result<Written, CliError> {
    write_document_io(path, bytes, label).map_err(|e| write_error(path, &e))
}

/// [`write_document`] with the I/O error. A file that cannot be replaced
/// (read-only, not a regular file) fails before history records anything,
/// so history never holds a version that did not reach the disk.
pub(crate) fn write_document_io(
    path: &Path,
    bytes: &[u8],
    label: &str,
) -> std::io::Result<Written> {
    output_file::check_replaceable(path)?;
    let recorded = history::record_edit(bytes, path, label);
    output_file::write_bytes(path, &recorded.bytes)?;
    Ok(Written {
        bytes: recorded.bytes,
        warning: recorded.warning,
    })
}

/// [`write_document`], printing a history warning to stderr.
///
/// Returns the bytes written.
pub(crate) fn apply_edit(path: &Path, bytes: &[u8], label: &str) -> Result<Vec<u8>, CliError> {
    let written = write_document(path, bytes, label)?;
    if let Some(w) = &written.warning {
        eprintln!("warning: {w}");
    }
    Ok(written.bytes)
}

/// An `io.write_failed` error for `path` (exit code 2).
pub(crate) fn write_error(path: &Path, e: &std::io::Error) -> CliError {
    CliError::new(
        "io.write_failed",
        format!(
            "error[io.write_failed]: {}",
            output_file::write_failure(path, e)
        ),
        2,
    )
}
