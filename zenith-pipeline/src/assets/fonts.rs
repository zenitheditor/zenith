//! Font provider construction: bundled fonts, project font assets, imported
//! font assets, and machine-local fonts.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use zenith_core::{
    AssetKind, BytesFontProvider, Document, FontProvider, FontSource, FontStyle, TokenLiteral,
    TokenType, TokenValue, default_provider,
};

use super::lock::verify_locked_sha256;
use crate::error::PipelineError;
use crate::host::Host;
use crate::imports::LoadedImportGraph;

/// The bundled provider layered with the host's extra faces and miss log.
///
/// [`default_provider`] clones one process-wide registry and shares its font
/// bytes, so this copies no font. Registering on the clone leaves the shared
/// base unchanged.
fn base_provider(host: Host<'_>) -> BytesFontProvider {
    let mut provider = default_provider();
    for font in host.extra_fonts {
        provider.register(
            &font.family,
            font.weight,
            font.style,
            Arc::clone(&font.bytes),
            font.index,
            font.source,
        );
    }
    match host.font_log {
        Some(log) => provider.with_miss_log(log.clone()),
        None => provider,
    }
}

/// Build a [`BytesFontProvider`] with the bundled fonts, the document's
/// `font`-kind assets, and the local fonts it needs.
///
/// When `project_dir` is `None`, no project font is read. When `Some`, each
/// `font`-kind asset is read through `host.fs` and registered under the
/// family name in its own metadata, so a `fontFamily` token naming that family
/// resolves to the real face instead of falling back to Noto.
///
/// Without `locked`, an unreadable asset is skipped (a missing file is
/// reported as `asset.missing` by
/// [`collect_missing_asset_diagnostics`](super::collect_missing_asset_diagnostics))
/// and an unparseable one is skipped with a warning to `host.warnings`.
///
/// # Errors
///
/// With `locked`, exit code 2 on a read failure, a parse failure, or a
/// missing or mismatched `sha256`.
pub fn build_font_provider(
    host: Host<'_>,
    doc: &Document,
    project_dir: Option<&Path>,
    locked: bool,
) -> Result<BytesFontProvider, PipelineError> {
    let mut provider = base_provider(host);
    if let Some(dir) = project_dir {
        register_project_fonts(host, &mut provider, doc, dir, None, locked)?;
    }
    register_local_fonts(host, &mut provider, doc);
    Ok(provider)
}

/// [`build_font_provider`] plus the font assets of every imported document,
/// each read from its own directory and registered under the namespaced
/// family `"<import-id>/<family>"`.
///
/// # Errors
///
/// As [`build_font_provider`], for host and imported assets alike.
pub fn build_font_provider_with_imports(
    host: Host<'_>,
    doc: &Document,
    project_dir: Option<&Path>,
    imports: &LoadedImportGraph,
    locked: bool,
) -> Result<BytesFontProvider, PipelineError> {
    let mut provider = base_provider(host);
    if let Some(dir) = project_dir {
        register_project_fonts(host, &mut provider, doc, dir, None, locked)?;
        for (import_id, imported, import_dir) in imports.documents_with_dirs() {
            register_project_fonts(
                host,
                &mut provider,
                imported,
                import_dir,
                Some(import_id),
                locked,
            )?;
        }
    }
    register_local_fonts(host, &mut provider, doc);
    Ok(provider)
}

/// Register every `font`-kind asset of `doc` into `provider` with
/// [`FontSource::Project`].
///
/// With `family_prefix = Some(import_id)` the face registers under
/// `"{import_id}/{family}"`. An imported document's fonts then keep their own
/// family namespace, so a host font and an imported font with the same real
/// family neither shadow nor merge (the imported subtree compiles through a
/// namespaced provider with the same prefix). The host passes `None`.
fn register_project_fonts(
    host: Host<'_>,
    provider: &mut BytesFontProvider,
    doc: &Document,
    dir: &Path,
    family_prefix: Option<&str>,
    locked: bool,
) -> Result<(), PipelineError> {
    for decl in &doc.assets.assets {
        if decl.kind != AssetKind::Font {
            continue;
        }
        let path = dir.join(&decl.src);
        let bytes = match host.fs.read(&path) {
            Ok(b) => b,
            Err(e) => {
                if locked {
                    return Err(PipelineError::new(
                        "asset.read_failed",
                        format!(
                            "--locked: could not read font asset '{}' from '{}': {}",
                            decl.id,
                            path.display(),
                            e
                        ),
                        2,
                    ));
                }
                // A missing or unreadable file is reported as `asset.missing`
                // by `collect_missing_asset_diagnostics`; skip here.
                continue;
            }
        };

        if locked {
            verify_locked_sha256(&decl.id, "font asset", decl.sha256.as_deref(), &bytes)?;
        }

        let arc: Arc<[u8]> = Arc::from(bytes.as_slice());
        match zenith_layout::face_metadata(&arc, 0) {
            Ok(m) => {
                let family = match family_prefix {
                    Some(prefix) => format!("{prefix}/{}", m.family),
                    None => m.family.clone(),
                };
                provider.register(&family, m.weight, m.style, arc, 0, FontSource::Project);
            }
            Err(e) => {
                if locked {
                    return Err(PipelineError::new(
                        "font.parse_failed",
                        format!(
                            "--locked: font asset '{}' could not be parsed: {}",
                            decl.id, e
                        ),
                        2,
                    ));
                }
                host.warnings.warn(&format!(
                    "font asset '{}' could not be parsed: {} — skipping",
                    decl.id, e
                ));
            }
        }
    }
    Ok(())
}

/// Register machine-local fonts as a last-resort source.
///
/// Asks `host.local_fonts` for the faces whose family the document names and
/// registers them with [`FontSource::Local`]. Unrelated faces are never
/// loaded, so they never join per-glyph fallback.
///
/// A face is skipped when bundled or project fonts already supply its family,
/// so local fonts never shadow them: a document that uses only bundled fonts
/// resolves to the same bytes as with no local fonts. Among local faces, the
/// first in scan order wins a `(family, weight, style)` slot. A face that
/// resolves from here trips a `font.local` advisory at compile time.
///
/// Read failures skip the file's faces silently: a local font is a
/// best-effort convenience, not a required asset.
fn register_local_fonts(host: Host<'_>, provider: &mut BytesFontProvider, doc: &Document) {
    // The host scan is the costly step, so run it only when the document
    // needs a family that bundled/project fonts cannot supply. In a valid
    // document every `font-family` resolves through a `fontFamily` token, so
    // those token values are every family the document can request.
    let wanted: BTreeSet<String> = doc
        .tokens
        .tokens
        .iter()
        .filter(|t| t.token_type == TokenType::FontFamily)
        .filter_map(|t| match &t.value {
            TokenValue::Literal(TokenLiteral::String(s)) => Some(s.clone()),
            _ => None,
        })
        .collect();
    let needs_scan = wanted.iter().any(|fam| {
        provider
            .resolve(std::slice::from_ref(fam), 400, FontStyle::Normal)
            .is_none()
    });
    if !needs_scan {
        return;
    }

    // One read per file, shared by every face of a collection.
    let mut loaded: BTreeMap<PathBuf, Arc<[u8]>> = BTreeMap::new();
    let mut taken: BTreeSet<(String, u16, FontStyle)> = BTreeSet::new();
    for entry in host.local_fonts.faces(&wanted) {
        let slot = (entry.family.to_lowercase(), entry.weight, entry.style);
        if taken.contains(&slot) {
            continue;
        }
        // Bundled/project always win: skip a face whose family a non-local
        // source already supplies.
        let supplied = provider
            .resolve(
                std::slice::from_ref(&entry.family),
                entry.weight,
                entry.style,
            )
            .is_some_and(|d| d.source != FontSource::Local);
        if supplied {
            continue;
        }
        let arc = match loaded.get(&entry.path) {
            Some(a) => a.clone(),
            None => {
                let Some(bytes) = host.local_fonts.read(&entry.path) else {
                    continue;
                };
                let a: Arc<[u8]> = Arc::from(bytes);
                loaded.insert(entry.path.clone(), a.clone());
                a
            }
        };
        provider.register(
            &entry.family,
            entry.weight,
            entry.style,
            arc,
            entry.index,
            FontSource::Local,
        );
        taken.insert(slot);
    }
}
