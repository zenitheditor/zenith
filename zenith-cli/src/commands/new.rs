//! Pure-ish logic for `zenith new`.
//!
//! Scaffolds a minimal, valid `.zen` document at a fresh path, with a `doc-id`
//! already minted + stamped and its initial Tier-2 history version recorded.
//! This gives an agent or GUI "File > New" a valid starting point without
//! hand-authoring boilerplate.
//!
//! The template is synthesized from a slug (derived from `--name`, else from the
//! target path's file stem), canonicalized through the engine formatter
//! ([`crate::commands::fmt::run`]), then run through the shared history pipeline
//! ([`crate::history::record_edit_in`]) with op_kind `"document.new"` so the
//! written bytes carry the stamped identity and the first version is durable.

use std::path::{Path, PathBuf};

use zenith_session::StorePaths;

use crate::history::record_edit_in;

// ── Result / error types ────────────────────────────────────────────────────────

/// Error from `zenith new`: a message plus a process exit code.
#[derive(Debug)]
pub struct NewErr {
    /// Human-readable message.
    pub message: String,
    /// Exit code (2 for refusal / input / internal failure).
    pub exit_code: u8,
}

/// The outcome of a successful `zenith new` run.
#[derive(Debug)]
pub struct NewResult {
    /// The path the document was actually written to (may differ from the
    /// requested path when a default `.zen` extension was appended).
    pub path: PathBuf,
    /// The stamped `doc-id` of the freshly created document.
    pub doc_id: String,
    /// Non-fatal warning produced during history recording, if any.
    pub warning: Option<String>,
}

// ── Public API ──────────────────────────────────────────────────────────────────

/// Scaffold a new minimal valid `.zen` document at `path`, resolving the real
/// data directory for history recording.
///
/// Refuses to overwrite an existing `path` (returns `Err` without touching it).
/// `name` is the optional display name; when absent, "Untitled" is used and the
/// slug is derived from `path`'s file stem.
pub fn run(path: &Path, name: Option<&str>) -> Result<NewResult, NewErr> {
    let paths = match zenith_session::resolve_data_dir() {
        Ok(data_dir) => StorePaths::new(data_dir),
        Err(e) => {
            return Err(NewErr {
                message: format!("cannot resolve data directory: {}", e.message),
                exit_code: 2,
            });
        }
    };
    run_in(&paths, path, name)
}

/// Same as [`run`] but with an explicit store root (used by tests).
///
/// Refuses to overwrite an existing `path`. On success, the file at `path` has
/// been written with a freshly minted + stamped `doc-id`, and the initial
/// version has been recorded into `paths`.
pub fn run_in(paths: &StorePaths, path: &Path, name: Option<&str>) -> Result<NewResult, NewErr> {
    // A directory can't be a document target.
    if path.is_dir() {
        return Err(NewErr {
            message: format!("'{}' is a directory; provide a file path", path.display()),
            exit_code: 2,
        });
    }

    // Default the `.zen` extension when the path has none (`poster` → `poster.zen`);
    // an explicit extension is respected as given.
    let target = target_path(path);

    // Bright-line safety: never overwrite an existing file.
    if target.exists() {
        return Err(NewErr {
            message: format!("refusing to overwrite existing file '{}'", target.display()),
            exit_code: 2,
        });
    }

    let display_name = name.unwrap_or("Untitled");
    let slug = slug_for(name, &target);

    // Synthesize the template, then canonicalize through the engine formatter so
    // the output is valid + canonical and any emission bug surfaces as an error.
    let raw = emit(&slug, display_name);
    let canonical = crate::commands::fmt::run(&raw).map_err(|e| NewErr {
        message: format!(
            "internal: scaffolded document failed to format: {}",
            e.message
        ),
        exit_code: 2,
    })?;

    // Mint + stamp the doc-id and record the initial version through the shared
    // history pipeline. The returned bytes carry the stamped identity.
    let recorded = record_edit_in(paths, &canonical.formatted, &target, "document.new");

    if recorded.doc_id.is_empty() {
        return Err(NewErr {
            message: "internal: no doc-id present after recording".to_string(),
            exit_code: 2,
        });
    }

    // Create any missing parent directories so `new sub/dir/doc.zen` just works.
    if let Some(parent) = target.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        std::fs::create_dir_all(parent).map_err(|e| NewErr {
            message: format!("cannot create directory '{}': {}", parent.display(), e),
            exit_code: 2,
        })?;
    }

    std::fs::write(&target, &recorded.bytes).map_err(|e| NewErr {
        message: format!("error writing '{}': {}", target.display(), e),
        exit_code: 2,
    })?;

    Ok(NewResult {
        path: target,
        doc_id: recorded.doc_id,
        warning: recorded.warning,
    })
}

// ── Helpers ─────────────────────────────────────────────────────────────────────

/// The on-disk target path: append a default `.zen` extension when `path` has
/// none, else respect the path exactly as given.
///
/// This relies only on `std::path`, which treats `/` as a separator on every OS
/// (including Windows) — so a Unix-style path typed by an agent on Windows
/// parses and extends correctly.
fn target_path(path: &Path) -> PathBuf {
    if path.extension().is_none() {
        path.with_extension("zen")
    } else {
        path.to_path_buf()
    }
}

/// Derive an id slug from the explicit `name` if present, else from `path`'s
/// file stem, falling back to `"untitled"` when neither yields usable text.
fn slug_for(name: Option<&str>, path: &Path) -> String {
    if let Some(n) = name {
        let s = slugify(n);
        if !s.is_empty() {
            return s;
        }
    }
    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
        let s = slugify(stem);
        if !s.is_empty() {
            return s;
        }
    }
    "untitled".to_string()
}

/// Reduce arbitrary text to a lowercase `[a-z0-9-]` slug: alphanumerics are
/// lowercased, every other run collapses to a single `-`, and leading/trailing
/// `-` are trimmed.
fn slugify(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut prev_dash = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// Emit the minimal valid `.zen` source for `slug` / `name`.
fn emit(slug: &str, name: &str) -> String {
    // `name` may contain characters that need escaping inside a KDL string; the
    // formatter re-quotes canonically, so a backslash/quote escape here is enough
    // to keep the intermediate source parseable.
    let esc = escape_kdl_string(name);
    format!(
        r##"zenith version=1 {{
  project id="proj.{slug}" name="{esc}"
  tokens format="zenith-token-v1" {{
    token id="color.bg" type="color" value="#ffffff"
  }}
  document id="doc.{slug}" title="{esc}" {{
    page id="page.1" w=(px)1080 h=(px)1080 background=(token)"color.bg" {{}}
  }}
}}
"##
    )
}

/// Escape characters that require backslash encoding inside a double-quoted KDL string.
fn escape_kdl_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out
}

// ── Tests ───────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_from_name_takes_precedence() {
        let p = Path::new("/x/poster.zen");
        assert_eq!(slug_for(Some("Launch Poster!"), p), "launch-poster");
    }

    #[test]
    fn slug_from_stem_when_no_name() {
        let p = Path::new("/x/My Cool Doc.zen");
        assert_eq!(slug_for(None, p), "my-cool-doc");
    }

    #[test]
    fn slug_falls_back_to_untitled() {
        let p = Path::new("/x/___.zen");
        assert_eq!(slug_for(Some("!!!"), p), "untitled");
    }

    #[test]
    fn template_formats_clean() {
        let raw = emit("demo", "Demo");
        let r = crate::commands::fmt::run(&raw).expect("template must format");
        let s = String::from_utf8(r.formatted).unwrap();
        assert!(s.contains("doc.demo"));
        assert!(s.contains("page.1"));
    }

    #[test]
    fn target_appends_zen_when_extension_absent() {
        assert_eq!(
            target_path(Path::new("poster")),
            PathBuf::from("poster.zen")
        );
        // An explicit extension (even a non-`.zen` one) is respected as given.
        assert_eq!(
            target_path(Path::new("poster.zen")),
            PathBuf::from("poster.zen")
        );
        assert_eq!(
            target_path(Path::new("notes.txt")),
            PathBuf::from("notes.txt")
        );
    }

    // `/` is a path separator on EVERY OS, including Windows. These assertions
    // lock the cross-platform property the command relies on: a Unix-style path
    // an agent types on Windows still parses into the right parent + extension.
    // (A backslash is only a separator on Windows, so it cannot be asserted
    // portably here — it would be a literal filename char on Unix.)
    #[test]
    fn forward_slash_path_parses_cross_platform() {
        let p = Path::new("made/by/agent/poster");
        assert_eq!(p.extension(), None, "no extension on the slashed path");
        assert_eq!(
            p.parent(),
            Some(Path::new("made/by/agent")),
            "`/` splits into parent components on every OS"
        );
        // `.zen` is appended onto the final component only, parents preserved.
        assert_eq!(target_path(p), PathBuf::from("made/by/agent/poster.zen"));
    }
}
