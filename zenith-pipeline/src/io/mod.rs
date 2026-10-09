//! Host I/O traits and the in-crate implementations.
//!
//! The pipeline reads nothing itself. A host supplies:
//! - [`SourceFs`] — project files (documents, imports, assets, fonts, text).
//! - [`ConfigSource`] — the global and local `.zenith.kdl` config files.
//! - [`LocalFontSource`] — fonts installed on the host machine.
//! - [`PageRunner`] — how per-page jobs are scheduled.
//! - [`WarningSink`] — where non-diagnostic warnings go.

mod config_source;
mod local_fonts;
mod mem_fs;
mod page_runner;
mod source_fs;
mod warnings;

pub use config_source::{ConfigFile, ConfigSource, FsConfig, LOCAL_CONFIG_NAME, NoConfig};
pub use local_fonts::{LocalFontSource, NoLocalFonts};
pub use mem_fs::{MEM_NOT_FOUND_MESSAGE, MemFs};
pub use page_runner::{PageRunner, Sequential};
pub(crate) use page_runner::{map_pages, map_slice};
pub use source_fs::{FsError, FsErrorKind, INVALID_UTF8_MESSAGE, SourceFs};
pub use warnings::{CollectWarnings, IgnoreWarnings, WarningSink};
