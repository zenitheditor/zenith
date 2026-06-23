use std::io::Write as _;

use crate::{cli, commands};

/// Map the CLI scope flag to the plugin module's [`Scope`](commands::plugin::Scope).
pub(crate) fn scope_from_arg(scope: cli::ScopeArg) -> commands::plugin::Scope {
    match scope {
        cli::ScopeArg::User => commands::plugin::Scope::User,
        cli::ScopeArg::Project => commands::plugin::Scope::Project,
    }
}

/// Translate the per-agent boolean flags into a [`Targets`](commands::plugin::Targets)
/// selection. `--all` wins; no flag set means auto-detect.
pub(crate) fn targets_from_flags(f: &cli::AgentFlags) -> commands::plugin::Targets {
    use commands::plugin::{Agent, Targets};
    if f.all {
        return Targets::All;
    }
    let mut agents = Vec::new();
    let mut push = |on: bool, a: Agent| {
        if on {
            agents.push(a);
        }
    };
    push(f.claude, Agent::ClaudeCode);
    push(f.codex, Agent::Codex);
    push(f.opencode, Agent::OpenCode);
    push(f.cursor, Agent::Cursor);
    push(f.windsurf, Agent::Windsurf);
    push(f.aider, Agent::Aider);
    push(f.zed, Agent::Zed);
    push(f.gemini, Agent::Gemini);
    push(f.copilot, Agent::Copilot);
    push(f.continue_dev, Agent::Continue);
    push(f.kiro, Agent::Kiro);
    push(f.antigravity, Agent::Antigravity);
    if agents.is_empty() {
        Targets::Auto
    } else {
        Targets::Agents(agents)
    }
}

// ── Diagnostics ───────────────────────────────────────────────────────────────

/// Print compile-stage diagnostics (advisories/warnings) to stderr, one per
/// line, so they are surfaced without polluting the stdout success message.
/// Does nothing when there are no diagnostics.
pub(crate) fn print_diagnostics_stderr(diagnostics: &[zenith_core::Diagnostic]) {
    for d in diagnostics {
        eprintln!("{}", commands::format_diagnostic_line(d));
    }
}

/// Count diagnostics with [`Severity::Error`].
pub(crate) fn count_hard_diagnostics(diagnostics: &[zenith_core::Diagnostic]) -> usize {
    diagnostics
        .iter()
        .filter(|d| d.severity == zenith_core::Severity::Error)
        .count()
}

/// Parse a `--spread` spec of the form `"A-B"` (two 1-based page numbers) into
/// `(a, b)`.
///
/// Returns a human-readable error message (never panics) when the spec is not
/// exactly two dash-separated positive integers.
pub(crate) fn parse_spread_spec(spec: &str) -> Result<(usize, usize), String> {
    let err = || {
        format!(
            "error: invalid --spread value {:?} (expected two 1-based page \
             numbers like \"10-11\")",
            spec
        )
    };
    let (a_str, b_str) = spec.split_once('-').ok_or_else(err)?;
    let a: usize = a_str.trim().parse().map_err(|_| err())?;
    let b: usize = b_str.trim().parse().map_err(|_| err())?;
    if a == 0 || b == 0 {
        return Err(err());
    }
    Ok((a, b))
}

/// Parse an `--at` spec of the form `"X,Y"` (two finite floats) into `(x, y)`.
///
/// `None` (the flag was omitted) defaults to `(0.0, 0.0)`. Returns a human-
/// readable error (never panics) when the value is not exactly two
/// comma-separated finite numbers.
pub(crate) fn parse_at_spec(spec: Option<&str>) -> Result<(f64, f64), String> {
    let spec = match spec {
        None => return Ok((0.0, 0.0)),
        Some(s) => s,
    };
    let err = || {
        format!(
            "error: invalid --at value {:?} (expected two comma-separated \
             numbers like \"120,80\")",
            spec
        )
    };
    let (x_str, y_str) = spec.split_once(',').ok_or_else(err)?;
    let x: f64 = x_str.trim().parse().map_err(|_| err())?;
    let y: f64 = y_str.trim().parse().map_err(|_| err())?;
    if !x.is_finite() || !y.is_finite() {
        return Err(err());
    }
    Ok((x, y))
}

/// Resolve the project directory for the library subsystem from an optional
/// `--path` argument.
///
/// - `None` → the current working directory (`.`).
/// - a path to an existing FILE (e.g. a `.zen`) → its parent directory.
/// - a path to an existing DIRECTORY → that directory.
/// - a non-existent path → its parent if it has one, else the path itself
///   (so a bare name like `proj.zen` still resolves to `.`).
///
/// Never panics; returns `None` only when no usable directory can be derived.
pub(crate) fn resolve_project_dir(path: Option<&std::path::Path>) -> Option<std::path::PathBuf> {
    use std::path::Path;
    match path {
        None => Some(std::path::PathBuf::from(".")),
        Some(p) if p.is_dir() => Some(p.to_path_buf()),
        // An existing file (e.g. a `.zen`) or a bare/non-existent name: use the
        // parent directory, falling back to `.` when there is none.
        Some(p) => Some(
            p.parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or(Path::new("."))
                .to_path_buf(),
        ),
    }
}

// ── I/O helpers ───────────────────────────────────────────────────────────────

/// Read a file to a UTF-8 string.
///
/// Returns a human-readable error message on failure (never panics).
pub(crate) fn read_file(path: &std::path::Path) -> Result<String, String> {
    std::fs::read(path)
        .map_err(|e| format!("error reading '{}': {}", path.display(), e))
        .and_then(|bytes| {
            String::from_utf8(bytes)
                .map_err(|_| format!("error: '{}' is not valid UTF-8", path.display()))
        })
}

/// Write raw bytes to a file.
///
/// Returns a `std::io::Error` on failure.
pub(crate) fn write_bytes(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut f = std::fs::File::create(path)?;
    f.write_all(bytes)
}
