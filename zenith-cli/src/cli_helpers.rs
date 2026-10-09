use crate::report::CliError;
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

/// Print diagnostics to stderr, one line each, with same-cause advisories and
/// warnings grouped (see [`crate::report::human_diagnostic_lines`]). Does
/// nothing when there are no diagnostics.
pub(crate) fn print_diagnostics_stderr(
    diagnostics: &[zenith_core::Diagnostic],
    src: &str,
    files: &zenith_pipeline::imports::ImportFiles,
) {
    let mut locator = crate::report::Locator::with_files(src, files);
    for line in crate::report::human_diagnostic_lines(diagnostics, &mut locator) {
        eprintln!("{line}");
    }
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
/// Returns an `io.read_failed` or `io.not_utf8` [`CliError`] (exit code 2) on
/// failure. Never panics.
pub(crate) fn read_file(path: &std::path::Path) -> Result<String, CliError> {
    let bytes = std::fs::read(path).map_err(|e| {
        CliError::new(
            "io.read_failed",
            format!(
                "error[io.read_failed]: cannot read '{}': {e}; check the path exists and is readable",
                path.display()
            ),
            2,
        )
    })?;
    String::from_utf8(bytes).map_err(|_| {
        CliError::new(
            "io.not_utf8",
            format!(
                "error[io.not_utf8]: '{}' is not valid UTF-8; save it as UTF-8 text",
                path.display()
            ),
            2,
        )
    })
}
