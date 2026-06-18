//! Command-line interface library for Zenith.
//!
//! Owns all command logic, argument parsing, the full command set, JSON I/O
//! shaping, and human-readable stdout/stderr formatting. `src/main.rs` is
//! kept thin — it only dispatches to this library. `zenith-layout` is reached
//! transitively through `zenith-scene`; the CLI never constructs layout types
//! directly.

/// Scaffold entry point — later units will grow this into the full command dispatcher.
///
/// Currently a no-op that returns success. All argument parsing and command
/// dispatch will be wired in here as subsequent units are implemented.
pub fn run() -> std::process::ExitCode {
    std::process::ExitCode::SUCCESS
}
