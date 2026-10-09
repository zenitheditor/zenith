//! Image and SVG asset provider construction and intrinsic image sizes.

use std::collections::BTreeMap;
use std::path::Path;

use zenith_core::{AssetKind, BytesAssetProvider, Document, subtree_uses_layout};

use super::lock::verify_locked_sha256;
use crate::error::PipelineError;
use crate::imports::LoadedImportGraph;
use crate::io::SourceFs;

/// Build a [`BytesAssetProvider`] with the `image`- and `svg`-kind assets of
/// `doc`, read through `fs` relative to `project_dir`.
///
/// `font`-kind assets go to [`build_font_provider`](super::build_font_provider).
/// Without `locked`, a read failure skips the asset (a missing file is
/// reported as `asset.missing` separately).
///
/// # Errors
///
/// With `locked`, exit code 2 on a read failure or a missing or mismatched
/// `sha256`.
pub fn build_asset_provider(
    fs: &dyn SourceFs,
    doc: &Document,
    project_dir: &Path,
    locked: bool,
) -> Result<BytesAssetProvider, PipelineError> {
    let mut provider = BytesAssetProvider::new();
    register_document_assets(fs, &mut provider, doc, "", project_dir, locked)?;
    Ok(provider)
}

/// [`build_asset_provider`] plus the image and SVG assets of every imported
/// document, read from its own directory and registered as
/// `"<import-id>/<asset-id>"`.
///
/// # Errors
///
/// As [`build_asset_provider`], for host and imported assets alike.
pub fn build_asset_provider_with_imports(
    fs: &dyn SourceFs,
    doc: &Document,
    project_dir: &Path,
    imports: &LoadedImportGraph,
    locked: bool,
) -> Result<BytesAssetProvider, PipelineError> {
    let mut provider = BytesAssetProvider::new();
    register_document_assets(fs, &mut provider, doc, "", project_dir, locked)?;
    for (import_id, imported, dir) in imports.documents_with_dirs() {
        register_document_assets(
            fs,
            &mut provider,
            imported,
            &format!("{import_id}/"),
            dir,
            locked,
        )?;
    }
    Ok(provider)
}

/// The pixel size of every image / SVG asset `assets` holds, by the id an
/// `image` node references (host ids, then `import-id/asset-id`), for
/// auto-layout image hug sizing. Empty when no page, component, master, or
/// imported document holds a layout frame, the only reader of the sizes.
#[must_use]
pub fn image_sizes(
    doc: &Document,
    imports: Option<&LoadedImportGraph>,
    assets: &BytesAssetProvider,
) -> BTreeMap<String, (f64, f64)> {
    if !document_uses_layout(doc, imports) {
        return BTreeMap::new();
    }
    let visual = |kind: &AssetKind| matches!(kind, AssetKind::Image | AssetKind::Svg);
    let mut ids: Vec<String> = doc
        .assets
        .assets
        .iter()
        .filter(|d| visual(&d.kind))
        .map(|d| d.id.clone())
        .collect();
    for (import_id, imported, _) in imports.into_iter().flat_map(|g| g.documents_with_dirs()) {
        ids.extend(
            imported
                .assets
                .assets
                .iter()
                .filter(|d| visual(&d.kind))
                .map(|d| format!("{import_id}/{}", d.id)),
        );
    }
    zenith_render::asset_intrinsic_sizes(assets, ids.iter().map(String::as_str))
}

/// `true` when `doc` or any imported document holds an auto-layout frame in a
/// page, a component, or a master. Components and masters lower when they
/// expand, so a layout frame there needs image sizes on any page.
fn document_uses_layout(doc: &Document, imports: Option<&LoadedImportGraph>) -> bool {
    let uses = |d: &Document| {
        d.body
            .pages
            .iter()
            .any(|p| subtree_uses_layout(&p.children))
            || d.components
                .iter()
                .any(|c| subtree_uses_layout(&c.children))
            || d.masters.iter().any(|m| subtree_uses_layout(&m.children))
    };
    uses(doc) || imports.is_some_and(|g| g.documents_with_dirs().any(|(_, d, _)| uses(d)))
}

/// [`image_sizes`] for a caller that holds no asset provider: reads the
/// image / SVG assets from `project_dir` (no sha256 check). Empty when
/// `project_dir` is `None`, no layout frame exists anywhere, or the document
/// declares no such asset.
#[must_use]
pub fn read_image_sizes(
    fs: &dyn SourceFs,
    doc: &Document,
    project_dir: Option<&Path>,
    imports: &LoadedImportGraph,
) -> BTreeMap<String, (f64, f64)> {
    let Some(dir) = project_dir.filter(|_| document_uses_layout(doc, Some(imports))) else {
        return BTreeMap::new();
    };
    match build_asset_provider_with_imports(fs, doc, dir, imports, false) {
        Ok(assets) => image_sizes(doc, Some(imports), &assets),
        Err(_) => BTreeMap::new(),
    }
}

fn register_document_assets(
    fs: &dyn SourceFs,
    provider: &mut BytesAssetProvider,
    doc: &Document,
    id_prefix: &str,
    project_dir: &Path,
    locked: bool,
) -> Result<(), PipelineError> {
    for decl in &doc.assets.assets {
        if !matches!(decl.kind, AssetKind::Image | AssetKind::Svg) {
            continue;
        }
        let path = project_dir.join(&decl.src);
        let bytes = match fs.read(&path) {
            Ok(bytes) => bytes,
            Err(e) => {
                if locked {
                    return Err(PipelineError::new(
                        "asset.read_failed",
                        format!(
                            "--locked: could not read asset '{}' from '{}': {}",
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
            verify_locked_sha256(&decl.id, "asset", decl.sha256.as_deref(), &bytes)?;
        }

        provider.register(
            &format!("{id_prefix}{}", decl.id),
            decl.kind.clone(),
            bytes.into(),
        );
    }
    Ok(())
}
