//! Pure-ish logic for `zenith new`.
//!
//! Scaffolds a minimal, valid `.zen` document at a fresh path, with a `doc-id`
//! already minted + stamped and its initial Tier-2 history version recorded.
//! This gives an agent or GUI "File > New" a valid starting point without
//! hand-authoring boilerplate.
//!
//! The template is synthesized from a slug (derived from `--name`, else from the
//! target path's file stem) at the requested [`PageSpec`] geometry, canonicalized
//! through the engine formatter ([`crate::commands::fmt::run`]), then run through
//! the shared history pipeline ([`crate::history::record_edit_in`]) with op_kind
//! `"document.new"` so the written bytes carry the stamped identity and the first
//! version is durable.

use std::path::{Path, PathBuf};

use zenith_core::{Document, KdlAdapter, KdlSource as _};
use zenith_session::StorePaths;
use zenith_session::adapter::{OsClock, OsRng};

use super::page::PageSpec;
use crate::history::record_edit_in;
use crate::library::EMBEDDED_PACKS;

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
/// slug is derived from `path`'s file stem. `page` gives the per-page dimensions
/// and page count.
pub fn run(
    path: &Path,
    name: Option<&str>,
    page: PageSpec,
    theme: Option<&str>,
) -> Result<NewResult, NewErr> {
    let paths = match zenith_session::resolve_data_dir() {
        Ok(data_dir) => StorePaths::new(data_dir),
        Err(e) => {
            return Err(NewErr {
                message: format!("cannot resolve data directory: {}", e.message),
                exit_code: 2,
            });
        }
    };
    run_in(&paths, path, name, page, theme)
}

/// Same as [`run`] but with an explicit store root (used by tests).
///
/// Refuses to overwrite an existing `path`. On success, the file at `path` has
/// been written with a freshly minted + stamped `doc-id`, and the initial
/// version has been recorded into `paths`. When `theme` is given, it names an
/// embedded theme pack (`@zenith/theme.<name>`, e.g. `sunset`) whose full token
/// contract is copied in and whose `color.base.100` token becomes the page
/// background, in place of the bare default `color.bg` token.
pub fn run_in(
    paths: &StorePaths,
    path: &Path,
    name: Option<&str>,
    page: PageSpec,
    theme: Option<&str>,
) -> Result<NewResult, NewErr> {
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

    // Resolve the requested theme pack up front so a bad `--theme` name fails
    // fast, before any file is touched. `theme_bg_token` is `Some` exactly when
    // a theme was applied, driving `emit`'s choice of background token and
    // tokens-block shape explicitly (no string-sentinel comparison).
    let theme_pack = theme.map(resolve_theme_pack).transpose()?;
    let theme_bg_token = theme_pack.is_some().then_some("color.base.100");

    // Synthesize the template, then canonicalize through the engine formatter so
    // the output is valid + canonical and any emission bug surfaces as an error.
    let raw = emit(&slug, display_name, page, theme_bg_token);
    let canonical = crate::commands::fmt::run(&raw).map_err(|e| NewErr {
        message: format!(
            "internal: scaffolded document failed to format: {}",
            e.message
        ),
        exit_code: 2,
    })?;

    // When a theme was requested, splice its full token block into the
    // canonical document and re-format; without a theme this is a no-op and
    // the output is byte-identical to before `--theme` existed.
    let canonical_bytes = match theme_pack {
        Some(theme_doc) => {
            let mut doc = KdlAdapter.parse(&canonical.formatted).map_err(|e| NewErr {
                message: format!(
                    "internal: scaffolded document failed to parse: {}",
                    e.message
                ),
                exit_code: 2,
            })?;
            doc.tokens = theme_doc.tokens;
            KdlAdapter.format(&doc).map_err(|e| NewErr {
                message: format!("internal: failed to apply theme tokens: {}", e.message),
                exit_code: 2,
            })?
        }
        None => canonical.formatted,
    };

    // Mint + stamp the doc-id and record the initial version through the shared
    // history pipeline. The returned bytes carry the stamped identity.
    let recorded = record_edit_in(paths, &canonical_bytes, &target, "document.new");

    // History recording is best-effort and must never block creating the file.
    // In the normal path `record_edit_in` mints, stamps, and records a doc-id.
    // If the history store is unavailable (e.g. reconcile/parse failed) it comes
    // back with an empty id and a warning — but a brand-new document must still
    // be born with an identity. Mint + stamp one directly so `new` always yields
    // a valid, identity-carrying `.zen`, surfacing the recording failure as a
    // (non-fatal) warning rather than aborting.
    let (bytes, doc_id, warning) = if recorded.doc_id.is_empty() {
        let (stamped, minted) = mint_and_stamp(&canonical_bytes)?;
        let warning = Some(match recorded.warning {
            Some(w) => format!("history unavailable, doc-id minted locally: {w}"),
            None => "history unavailable; doc-id minted locally".to_string(),
        });
        (stamped, minted, warning)
    } else {
        (recorded.bytes, recorded.doc_id, recorded.warning)
    };

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

    std::fs::write(&target, &bytes).map_err(|e| NewErr {
        message: format!("error writing '{}': {}", target.display(), e),
        exit_code: 2,
    })?;

    Ok(NewResult {
        path: target,
        doc_id,
        warning,
    })
}

// ── Helpers ─────────────────────────────────────────────────────────────────────

/// Mint a fresh ULID doc-id and stamp it into `canonical` `.zen` bytes, returning
/// the re-formatted bytes and the minted id.
///
/// Used only as a fallback when the history pipeline could not attach an id (its
/// store was unavailable): a new document must still be created with a stable
/// identity even when local history recording is impossible.
fn mint_and_stamp(canonical: &[u8]) -> Result<(Vec<u8>, String), NewErr> {
    let id = zenith_session::mint_ulid(&OsClock, &OsRng).map_err(|e| NewErr {
        message: format!("internal: could not mint doc-id: {}", e.message),
        exit_code: 2,
    })?;
    let mut doc = KdlAdapter.parse(canonical).map_err(|e| NewErr {
        message: format!(
            "internal: scaffolded document failed to parse: {}",
            e.message
        ),
        exit_code: 2,
    })?;
    doc.doc_id = Some(id.clone());
    let bytes = KdlAdapter.format(&doc).map_err(|e| NewErr {
        message: format!("internal: failed to stamp doc-id: {}", e.message),
        exit_code: 2,
    })?;
    Ok((bytes, id))
}

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

/// Emit the minimal valid `.zen` source for `slug` / `name` at the given page
/// geometry, with each page's `background` referencing the resolved background
/// token.
///
/// Each of `page.pages` pages gets a stable `page.N` id (1-based) at the
/// requested pixel size. When `theme_bg_token` is `None` (no theme requested),
/// the background is the default `"color.bg"`, the tokens block carries the
/// white `color.bg` token, and a single default-square page reproduces the
/// original hard-coded template byte-for-byte after canonicalization. When
/// `theme_bg_token` is `Some` (a themed scaffold), every page's background
/// references that token instead and the tokens block is left empty — the
/// caller splices in a full theme token pack after canonicalization.
fn emit(slug: &str, name: &str, page: PageSpec, theme_bg_token: Option<&str>) -> String {
    // `name` may contain characters that need escaping inside a KDL string; the
    // formatter re-quotes canonically, so a backslash/quote escape here is enough
    // to keep the intermediate source parseable.
    let esc = escape_kdl_string(name);
    let PageSpec {
        width,
        height,
        pages,
    } = page;

    let bg_token = theme_bg_token.unwrap_or("color.bg");
    let mut page_nodes = String::new();
    for n in 1..=pages {
        page_nodes.push_str(&format!(
            "    page id=\"page.{n}\" w=(px){width} h=(px){height} background=(token)\"{bg_token}\" {{}}\n"
        ));
    }

    let tokens_block = if theme_bg_token.is_none() {
        "  tokens format=\"zenith-token-v1\" {\n    token id=\"color.bg\" type=\"color\" value=\"#ffffff\"\n  }\n"
            .to_string()
    } else {
        "  tokens format=\"zenith-token-v1\" {\n  }\n".to_string()
    };

    format!(
        r##"zenith version=1 {{
  project id="proj.{slug}" name="{esc}"
{tokens_block}  document id="doc.{slug}" title="{esc}" {{
{page_nodes}  }}
}}
"##
    )
}

/// Resolve `--theme <name>` to its embedded token pack's parsed [`Document`],
/// for splicing its token block into a themed scaffold.
///
/// Looks up `@zenith/theme.<name>` in [`EMBEDDED_PACKS`] by exact id match. On
/// a miss, returns a [`NewErr`] listing the available bare theme names (sorted,
/// prefix stripped), mirroring `unknown_package_error`'s convention in
/// `zenith-cli/src/library/add.rs`.
fn resolve_theme_pack(name: &str) -> Result<Document, NewErr> {
    let pkg_id = format!("@zenith/theme.{name}");
    let source = EMBEDDED_PACKS
        .iter()
        .find(|(id, _)| *id == pkg_id)
        .map(|(_, src)| *src)
        .ok_or_else(|| {
            let mut available: Vec<&str> = EMBEDDED_PACKS
                .iter()
                .filter_map(|(id, _)| id.strip_prefix("@zenith/theme."))
                .collect();
            available.sort_unstable();
            NewErr {
                message: format!(
                    "unknown theme '{}' (available: {})",
                    name,
                    available.join(", ")
                ),
                exit_code: 2,
            }
        })?;

    // Defensive: the source is embedded (bundled at build time), so a parse
    // failure here indicates an internal packaging bug, not bad user input.
    KdlAdapter.parse(source.as_bytes()).map_err(|e| NewErr {
        message: format!(
            "internal: embedded theme '{}' failed to parse: {}",
            name, e.message
        ),
        exit_code: 2,
    })
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
    use crate::commands::new::page::DEFAULT_PAGE;

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
        let raw = emit("demo", "Demo", DEFAULT_PAGE, None);
        let r = crate::commands::fmt::run(&raw).expect("template must format");
        let s = String::from_utf8(r.formatted).unwrap();
        assert!(s.contains("doc.demo"));
        assert!(s.contains("page.1"));
    }

    #[test]
    fn emit_honors_dimensions_and_page_count() {
        let spec = PageSpec {
            width: 794,
            height: 1123,
            pages: 3,
        };
        let raw = emit("demo", "Demo", spec, None);
        let r = crate::commands::fmt::run(&raw).expect("template must format");
        let s = String::from_utf8(r.formatted).unwrap();
        assert!(s.contains("(px)794"), "width honored; got:\n{s}");
        assert!(s.contains("(px)1123"), "height honored; got:\n{s}");
        assert!(s.contains("page.1") && s.contains("page.2") && s.contains("page.3"));
    }

    /// A `Some` `theme_bg_token` (the themed-scaffold path) references that
    /// token on every page's `background` and emits no `color.bg` token —
    /// the caller splices in the theme's own token block afterward.
    #[test]
    fn emit_with_theme_token_swaps_background_and_omits_default_tokens() {
        let raw = emit("demo", "Demo", DEFAULT_PAGE, Some("color.base.100"));
        let r = crate::commands::fmt::run(&raw).expect("template must format");
        let s = String::from_utf8(r.formatted).unwrap();
        assert!(
            s.contains("background=(token)\"color.base.100\""),
            "background must reference the theme token; got:\n{s}"
        );
        assert!(
            !s.contains("color.bg"),
            "the default color.bg token must be absent; got:\n{s}"
        );
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
