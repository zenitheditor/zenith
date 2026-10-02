//! Font discovery and read-only OpenType craft inspection for the CLI.
//!
//! - `list` enumerates bundled + local/system families (same discovery as render).
//! - `features` / `alternates` resolve a face and expose GSUB/GPOS table data
//!   via [`zenith_layout::list_layout_features`] /
//!   [`zenith_layout::list_glyph_alternates`].
//!
//! OS-specific font-directory enumeration lives here — NOT in `zenith-core` —
//! so the core stays free of machine-specific assumptions.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use zenith_core::{
    AssetKind, BytesFontProvider, FontProvider, FontSource, FontStyle, KdlAdapter, KdlSource,
    default_provider, scan_font_dirs,
};
use zenith_layout::{FeatureList, GlyphAlternates, list_glyph_alternates, list_layout_features};

use crate::commands::serialize_pretty;
use crate::json_types::{
    FontsAlternatesOutput, FontsFeatureEntry, FontsFeaturesOutput, FontsOutput,
};

const FEATURES_SCHEMA: &str = "zenith-fonts-features-v1";
const ALTERNATES_SCHEMA: &str = "zenith-fonts-alternates-v1";

// ── list ──────────────────────────────────────────────────────────────────────

/// List available fonts in two sections: bundled (portable) and local (this
/// machine only).
///
/// Uses the same discovery code as the renderer so there is no drift:
/// - Bundled families are the faces in `zenith_core::default_provider()`, named
///   in their proper (name-table) case via `zenith_layout::face_metadata`.
/// - Local families come from `zenith_core::scan_font_dirs`, which reads only
///   metadata (no font bytes are loaded) and reuses the on-disk index,
///   with any family already in the bundled set excluded (case-insensitively) so
///   the local section shows only genuinely machine-specific families.
///
/// Note: the first run parses every system font file and writes the metadata
/// index ([`font_index_path`]). Later runs re-parse only changed files.
///
/// Returns `(output_string, exit_code)` following the convention used by
/// `commands::schema::*` and other discovery commands.
pub fn list(json: bool) -> (String, u8) {
    // Bundled families in their proper (name-table) case, deduped
    // case-insensitively. The provider keys faces by a lowercased family, so the
    // display name is recovered from each face's own metadata (same source the
    // renderer and `scan_font_dirs` use). The map is keyed by the lowercase name
    // for case-insensitive dedup; iteration order (lowercase-sorted) is stable.
    let mut bundled: BTreeMap<String, String> = BTreeMap::new();
    for face in default_provider().all_faces() {
        if let Ok(meta) = zenith_layout::face_metadata(&face.bytes, face.index) {
            bundled
                .entry(meta.family.to_lowercase())
                .or_insert(meta.family);
        }
    }

    // Local families (proper case), excluding any family already bundled
    // (compared case-insensitively).
    let mut local: BTreeMap<String, String> = BTreeMap::new();
    for entry in scan_font_dirs(&os_font_dirs(), font_index_path().as_deref()) {
        let key = entry.family.to_lowercase();
        if bundled.contains_key(&key) {
            continue;
        }
        local.entry(key).or_insert(entry.family);
    }

    let bundled_vec: Vec<String> = bundled.into_values().collect();
    let local_vec: Vec<String> = local.into_values().collect();

    if json {
        let out = FontsOutput {
            schema: "zenith-fonts-v1",
            bundled: bundled_vec,
            local: local_vec,
        };
        (serialize_pretty(&out), 0)
    } else {
        let mut lines: Vec<String> = Vec::new();

        lines.push("Bundled (portable)".to_owned());
        lines.push("──────────────────".to_owned());
        if bundled_vec.is_empty() {
            lines.push("  (none)".to_owned());
        } else {
            for family in &bundled_vec {
                lines.push(format!("  {family}"));
            }
        }

        lines.push(String::new());
        lines.push("Local / system (this machine only)".to_owned());
        lines.push("──────────────────────────────────".to_owned());
        if local_vec.is_empty() {
            lines.push("  (none found)".to_owned());
        } else {
            for family in &local_vec {
                lines.push(format!("  {family}"));
            }
            lines.push(String::new());
            lines.push(
                "Note: local fonts are not portable — renders that use them may differ on \
another machine and trip a `font.local` advisory."
                    .to_owned(),
            );
        }

        (lines.join("\n"), 0)
    }
}

// ── features / alternates ─────────────────────────────────────────────────────

/// `zenith fonts features <target>`.
///
/// Returns `(output, exit_code)`. Exit 2 on resolution/parse errors.
pub fn features(
    target: &str,
    weight: u16,
    style: &str,
    doc: Option<&Path>,
    json: bool,
) -> (String, u8) {
    let style_enum = match parse_style(style) {
        Ok(s) => s,
        Err(msg) => return (msg, 2),
    };
    let resolved = match resolve_target(target, weight, style_enum, doc) {
        Ok(r) => r,
        Err(msg) => return (msg, 2),
    };
    let list = match list_layout_features(&resolved.bytes, resolved.face_index) {
        Ok(l) => l,
        Err(e) => return (format!("error[font.parse]: {e}"), 2),
    };

    let style_label = style_str(style_enum);
    if json {
        let out = FontsFeaturesOutput {
            schema: FEATURES_SCHEMA,
            target: resolved.label,
            weight: resolved.weight,
            style: style_label,
            has_kern_table: list.has_kern_table,
            features: list
                .features
                .into_iter()
                .map(|f| FontsFeatureEntry {
                    tag: f.tag,
                    tables: f.tables,
                })
                .collect(),
        };
        (serialize_pretty(&out), 0)
    } else {
        (
            render_features_human(&resolved.label, resolved.weight, style_label, &list),
            0,
        )
    }
}

/// `zenith fonts alternates <target> [--char …]`.
///
/// Returns `(output, exit_code)`. Exit 2 on resolution/parse errors.
pub fn alternates(
    target: &str,
    char_arg: &str,
    weight: u16,
    style: &str,
    doc: Option<&Path>,
    json: bool,
) -> (String, u8) {
    let style_enum = match parse_style(style) {
        Ok(s) => s,
        Err(msg) => return (msg, 2),
    };
    let ch = match parse_char_arg(char_arg) {
        Ok(c) => c,
        Err(msg) => return (msg, 2),
    };
    let resolved = match resolve_target(target, weight, style_enum, doc) {
        Ok(r) => r,
        Err(msg) => return (msg, 2),
    };
    let alts = match list_glyph_alternates(&resolved.bytes, resolved.face_index, ch) {
        Ok(a) => a,
        Err(e) => return (format!("error[font.parse]: {e}"), 2),
    };

    let style_label = style_str(style_enum);
    let codepoint = format!("U+{:04X}", u32::from(ch));
    if json {
        let out = FontsAlternatesOutput {
            schema: ALTERNATES_SCHEMA,
            target: resolved.label,
            weight: resolved.weight,
            style: style_label,
            char: ch.to_string(),
            codepoint,
            glyph_index: alts.glyph_index,
            alternate_glyph_ids: alts.alternate_glyph_ids,
            limits: alts.limits,
        };
        (serialize_pretty(&out), 0)
    } else {
        (
            render_alternates_human(&resolved.label, resolved.weight, style_label, &alts),
            0,
        )
    }
}

// ── target resolution ─────────────────────────────────────────────────────────

struct ResolvedFace {
    bytes: Arc<[u8]>,
    face_index: u32,
    /// Display label: family name or path string.
    label: String,
    /// Effective weight used for family resolution (or metadata weight for paths).
    weight: u16,
}

/// Resolve `target` as a `.ttf`/`.otf` path, else as a font family.
///
/// Family resolution uses [`default_provider`] plus project fonts when `doc` is
/// set. Local/system fonts are not scanned here.
fn resolve_target(
    target: &str,
    weight: u16,
    style: FontStyle,
    doc: Option<&Path>,
) -> Result<ResolvedFace, String> {
    if is_font_file_target(target) {
        let path = Path::new(target);
        let bytes = std::fs::read(path).map_err(|e| {
            format!(
                "error[font.missing]: could not read font file '{}': {e}",
                path.display()
            )
        })?;
        // Prefer metadata weight when available; fall back to the requested weight.
        let meta_weight = zenith_layout::face_metadata(&bytes, 0)
            .map(|m| m.weight)
            .unwrap_or(weight);
        return Ok(ResolvedFace {
            bytes: Arc::from(bytes),
            face_index: 0,
            label: target.to_owned(),
            weight: meta_weight,
        });
    }

    let provider = build_family_provider(doc)?;
    let families = [target.to_owned()];
    let face = provider.resolve(&families, weight, style).ok_or_else(|| {
        format!(
            "error[font.unresolved]: no face for family '{target}' \
(weight={weight}, style={}); use a bundled family, a .ttf/.otf path, \
or --doc with a project font asset",
            style_str(style)
        )
    })?;

    Ok(ResolvedFace {
        bytes: face.bytes,
        face_index: face.index,
        label: target.to_owned(),
        weight,
    })
}

fn build_family_provider(doc: Option<&Path>) -> Result<BytesFontProvider, String> {
    let mut provider = default_provider();
    if let Some(path) = doc {
        register_doc_project_fonts(&mut provider, path)?;
    }
    Ok(provider)
}

/// Register `font`-kind assets from a document into `provider` as project faces.
fn register_doc_project_fonts(
    provider: &mut BytesFontProvider,
    doc_path: &Path,
) -> Result<(), String> {
    let src = std::fs::read_to_string(doc_path).map_err(|e| {
        format!(
            "error[io]: could not read document '{}': {e}",
            doc_path.display()
        )
    })?;
    let doc = KdlAdapter
        .parse(src.as_bytes())
        .map_err(|e| format!("error[parse.error]: {}", e.message))?;
    let dir = doc_path.parent().unwrap_or_else(|| Path::new("."));

    for decl in &doc.assets.assets {
        if decl.kind != AssetKind::Font {
            continue;
        }
        let path = dir.join(&decl.src);
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        if let Ok(meta) = zenith_layout::face_metadata(&bytes, 0) {
            provider.register(
                &meta.family,
                meta.weight,
                meta.style,
                Arc::from(bytes),
                0,
                FontSource::Project,
            );
        }
    }
    Ok(())
}

fn is_font_file_target(target: &str) -> bool {
    let lower = target.to_ascii_lowercase();
    lower.ends_with(".ttf") || lower.ends_with(".otf")
}

fn parse_style(style: &str) -> Result<FontStyle, String> {
    match style {
        "normal" => Ok(FontStyle::Normal),
        "italic" => Ok(FontStyle::Italic),
        other => Err(format!(
            "error[arg]: invalid --style '{other}'; expected normal or italic"
        )),
    }
}

fn style_str(style: FontStyle) -> &'static str {
    match style {
        FontStyle::Normal => "normal",
        FontStyle::Italic => "italic",
    }
}

/// Parse `--char U+0041` or a single scalar like `A`.
fn parse_char_arg(s: &str) -> Result<char, String> {
    let trimmed = s.trim();
    if let Some(hex) = trimmed
        .strip_prefix("U+")
        .or_else(|| trimmed.strip_prefix("u+"))
    {
        if hex.is_empty() {
            return Err("error[arg]: --char U+ form requires hex digits (e.g. U+0041)".to_owned());
        }
        let cp = u32::from_str_radix(hex, 16).map_err(|_| {
            format!("error[arg]: invalid --char codepoint 'U+{hex}'; expected hex digits")
        })?;
        return char::from_u32(cp).ok_or_else(|| {
            format!("error[arg]: --char U+{hex:0>4} is not a valid Unicode scalar value")
        });
    }

    let mut chars = trimmed.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Ok(c),
        (None, _) => Err("error[arg]: --char must not be empty".to_owned()),
        (Some(_), Some(_)) => Err(format!(
            "error[arg]: --char '{trimmed}' must be a single character or U+XXXX form"
        )),
    }
}

// ── human rendering ───────────────────────────────────────────────────────────

fn render_features_human(target: &str, weight: u16, style: &str, list: &FeatureList) -> String {
    let mut lines = Vec::new();
    lines.push(format!("Font features — {target} ({weight}, {style})"));
    lines.push("────────────────────────────────────────".to_owned());
    lines.push(format!(
        "Classic kern table: {}",
        if list.has_kern_table { "yes" } else { "no" }
    ));
    if list.features.is_empty() {
        lines.push("Features: (none)".to_owned());
    } else {
        lines.push(format!("Features ({}):", list.features.len()));
        for entry in &list.features {
            let tables = entry.tables.join("+");
            lines.push(format!("  {:4}  [{}]", entry.tag, tables));
        }
    }
    lines.join("\n")
}

fn render_alternates_human(
    target: &str,
    weight: u16,
    style: &str,
    alts: &GlyphAlternates,
) -> String {
    let mut lines = Vec::new();
    let cp = format!("U+{:04X}", u32::from(alts.codepoint));
    lines.push(format!(
        "Font alternates — {target} ({weight}, {style}) char={} ({cp})",
        alts.codepoint
    ));
    lines.push("────────────────────────────────────────".to_owned());
    match alts.glyph_index {
        Some(gid) => lines.push(format!("Glyph index: {gid}")),
        None => lines.push("Glyph index: (not in cmap)".to_owned()),
    }
    if alts.alternate_glyph_ids.is_empty() {
        lines.push("Alternate glyph IDs: (none)".to_owned());
    } else {
        lines.push(format!(
            "Alternate glyph IDs ({}):",
            alts.alternate_glyph_ids.len()
        ));
        for gid in &alts.alternate_glyph_ids {
            lines.push(format!("  {gid}"));
        }
    }
    lines.push(String::new());
    lines.push(format!("Limits: {}", alts.limits));
    lines.join("\n")
}

// ── OS font dirs ──────────────────────────────────────────────────────────────

/// Resolve `$HOME` as a [`PathBuf`].
///
/// Mirrors the pattern used by the plugin-paths module: `var_os` returns `None`
/// when the variable is unset, so no panic is possible. Only the unix-family
/// targets (linux/macos) consult `$HOME` for per-user font dirs.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Path of the local-font metadata index file, or `None` when the user cache
/// directory cannot be named.
///
/// Linux: `$XDG_CACHE_HOME/zenith/font-index-v1.txt`, else
/// `$HOME/.cache/zenith/font-index-v1.txt`. macOS: `$HOME/Library/Caches/zenith/`.
/// Windows: `%LOCALAPPDATA%\zenith\`.
#[must_use]
pub fn font_index_path() -> Option<PathBuf> {
    cache_root().map(|d| d.join("zenith").join("font-index-v1.txt"))
}

#[cfg(target_os = "linux")]
fn cache_root() -> Option<PathBuf> {
    match std::env::var_os("XDG_CACHE_HOME") {
        Some(x) if !x.is_empty() => Some(PathBuf::from(x)),
        _ => home_dir().map(|h| h.join(".cache")),
    }
}

#[cfg(target_os = "macos")]
fn cache_root() -> Option<PathBuf> {
    home_dir().map(|h| h.join("Library/Caches"))
}

#[cfg(target_os = "windows")]
fn cache_root() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn cache_root() -> Option<PathBuf> {
    None
}

/// The OS font directories to scan for local/system fonts, most-canonical first.
///
/// Only directories that can be named without panicking are included; entries
/// that depend on an unset environment variable are simply omitted. The returned
/// list may contain directories that do not exist — the scanner skips those.
#[cfg(target_os = "linux")]
#[must_use]
pub fn os_font_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("/usr/share/fonts"),
        PathBuf::from("/usr/local/share/fonts"),
    ];
    if let Some(home) = home_dir() {
        dirs.push(home.join(".fonts"));
        dirs.push(home.join(".local/share/fonts"));
    }
    dirs
}

/// The OS font directories to scan for local/system fonts, most-canonical first.
#[cfg(target_os = "macos")]
#[must_use]
pub fn os_font_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("/System/Library/Fonts"),
        PathBuf::from("/Library/Fonts"),
    ];
    if let Some(home) = home_dir() {
        dirs.push(home.join("Library/Fonts"));
    }
    dirs
}

/// The OS font directories to scan for local/system fonts, most-canonical first.
#[cfg(target_os = "windows")]
#[must_use]
pub fn os_font_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(windir) = std::env::var_os("WINDIR") {
        dirs.push(PathBuf::from(windir).join("Fonts"));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        dirs.push(PathBuf::from(local).join("Microsoft/Windows/Fonts"));
    }
    dirs
}

/// Fallback for any other target OS: no known system font locations.
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
#[must_use]
pub fn os_font_dirs() -> Vec<PathBuf> {
    Vec::new()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_font_dirs_is_callable_and_paths_are_absolute_or_under_home() {
        // The list may legitimately be empty on exotic targets, but every entry
        // it does contain must be a non-empty path.
        for dir in os_font_dirs() {
            assert!(
                !dir.as_os_str().is_empty(),
                "an os font dir entry must not be empty"
            );
        }
    }

    #[test]
    fn list_human_returns_exit_0_and_contains_bundled_section() {
        let (output, code) = list(false);
        assert_eq!(code, 0, "exit code must be 0");
        assert!(
            output.contains("Bundled"),
            "human output must contain a 'Bundled' section header"
        );
        // "Noto Sans" is always bundled — verify it appears in proper case.
        assert!(
            output.contains("Noto Sans"),
            "bundled section must include 'Noto Sans'"
        );
        assert!(
            output.contains("Local / system"),
            "human output must contain a 'Local / system' section header"
        );
    }

    #[test]
    fn list_json_returns_exit_0_and_valid_envelope() {
        let (output, code) = list(true);
        assert_eq!(code, 0, "exit code must be 0");
        let parsed: serde_json::Value =
            serde_json::from_str(&output).expect("--json output must be valid JSON");
        assert_eq!(
            parsed["schema"], "zenith-fonts-v1",
            "JSON envelope must carry schema = 'zenith-fonts-v1'"
        );
        let bundled = parsed["bundled"]
            .as_array()
            .expect("'bundled' must be an array");
        assert!(
            bundled.iter().any(|v| v.as_str() == Some("Noto Sans")),
            "bundled array must include 'Noto Sans'"
        );
        // 'local' key must be present (may be empty on CI, that is fine).
        assert!(
            parsed["local"].is_array(),
            "'local' key must be present and be an array"
        );
    }

    #[test]
    fn features_json_for_bundled_family() {
        let (output, code) = features("Noto Sans", 400, "normal", None, true);
        assert_eq!(code, 0, "features must succeed: {output}");
        let parsed: serde_json::Value =
            serde_json::from_str(&output).expect("features --json must be valid JSON");
        assert_eq!(parsed["schema"], FEATURES_SCHEMA);
        assert_eq!(parsed["target"], "Noto Sans");
        assert_eq!(parsed["weight"], 400);
        assert_eq!(parsed["style"], "normal");
        assert!(parsed["has_kern_table"].is_boolean());
        assert!(parsed["features"].is_array(), "features must be an array");
    }

    #[test]
    fn features_unresolved_family_exits_2() {
        let (output, code) = features("DefinitelyNotARealFontFamilyXYZ", 400, "normal", None, true);
        assert_eq!(code, 2);
        assert!(
            output.contains("font.unresolved"),
            "expected unresolved error, got: {output}"
        );
    }

    #[test]
    fn alternates_json_for_bundled_family() {
        let (output, code) = alternates("Noto Sans", "A", 400, "normal", None, true);
        assert_eq!(code, 0, "alternates must succeed: {output}");
        let parsed: serde_json::Value =
            serde_json::from_str(&output).expect("alternates --json must be valid JSON");
        assert_eq!(parsed["schema"], ALTERNATES_SCHEMA);
        assert_eq!(parsed["char"], "A");
        assert_eq!(parsed["codepoint"], "U+0041");
        assert!(parsed["glyph_index"].is_number() || parsed["glyph_index"].is_null());
        assert!(parsed["alternate_glyph_ids"].is_array());
        assert!(
            parsed["limits"]
                .as_str()
                .is_some_and(|s| s.contains("AlternateSubstitution")),
            "limits must mention AlternateSubstitution"
        );
    }

    #[test]
    fn parse_char_accepts_u_plus_and_scalar() {
        assert_eq!(parse_char_arg("A").unwrap(), 'A');
        assert_eq!(parse_char_arg("U+0041").unwrap(), 'A');
        assert_eq!(parse_char_arg("u+0041").unwrap(), 'A');
        assert_eq!(parse_char_arg("U+1F600").unwrap(), '😀');
        assert!(parse_char_arg("AB").is_err());
        assert!(parse_char_arg("U+").is_err());
        assert!(parse_char_arg("U+ZZZZ").is_err());
    }

    #[test]
    fn is_font_file_target_detects_extensions() {
        assert!(is_font_file_target("Foo.ttf"));
        assert!(is_font_file_target("/path/Bar.OTF"));
        assert!(!is_font_file_target("Noto Sans"));
        assert!(!is_font_file_target("something.ttc"));
    }

    #[test]
    fn features_from_ttf_path() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../zenith-core/assets/fonts/NotoSans-Regular.ttf");
        let path_str = path.to_string_lossy();
        let (output, code) = features(&path_str, 400, "normal", None, true);
        assert_eq!(code, 0, "path features must succeed: {output}");
        let parsed: serde_json::Value = serde_json::from_str(&output).expect("json");
        assert_eq!(parsed["schema"], FEATURES_SCHEMA);
        assert!(parsed["features"].is_array());
    }
}
