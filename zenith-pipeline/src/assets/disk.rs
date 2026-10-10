//! Project-file diagnostics: missing assets (host and imported) and image
//! size advisories.

use std::path::Path;

use zenith_core::{AssetKind, Diagnostic, Document, ImageNode, Node, dim_to_px};

use crate::imports::LoadedImportGraph;
use crate::io::SourceFs;

/// A hard `asset.missing` diagnostic for every declared asset of `doc` whose
/// file does not exist under `project_dir`.
///
/// Every asset kind is checked (image, svg, font), in declaration order. The
/// diagnostics are `Severity::Error`, so they block render output.
#[must_use]
pub fn collect_missing_asset_diagnostics(
    fs: &dyn SourceFs,
    doc: &Document,
    project_dir: &Path,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for decl in &doc.assets.assets {
        let path = project_dir.join(&decl.src);
        if !fs.exists(&path) {
            diagnostics.push(Diagnostic::error(
                "asset.missing",
                format!("asset '{}' file not found: '{}'", decl.id, path.display()),
                decl.source_span,
                Some(decl.id.clone()),
            ));
        } else if let Some(e) = fs.refusal(&path) {
            diagnostics.push(Diagnostic::error(
                "asset.read_failed",
                format!("asset '{}' cannot be read: {e}", decl.id),
                decl.source_span,
                Some(decl.id.clone()),
            ));
        }
    }
    diagnostics
}

/// A hard `import.asset_missing` diagnostic for every declared asset of an
/// imported document whose file does not exist under that import's
/// directory.
///
/// The import id rides on each diagnostic, so the failure is attributable to
/// its import. Imports and their assets are visited in deterministic order.
#[must_use]
pub fn collect_missing_import_asset_diagnostics(
    fs: &dyn SourceFs,
    imports: &LoadedImportGraph,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for (import_id, doc, dir) in imports.documents_with_dirs() {
        for decl in &doc.assets.assets {
            let path = dir.join(&decl.src);
            if !fs.exists(&path) {
                diagnostics.push(
                    Diagnostic::error(
                        "import.asset_missing",
                        format!(
                            "import '{}' asset '{}' file not found: '{}'",
                            import_id,
                            decl.id,
                            path.display()
                        ),
                        decl.source_span,
                        Some(decl.id.clone()),
                    )
                    .with_import(import_id),
                );
            } else if let Some(e) = fs.refusal(&path) {
                diagnostics.push(
                    Diagnostic::error(
                        "asset.read_failed",
                        format!(
                            "import '{import_id}' asset '{}' cannot be read: {e}",
                            decl.id
                        ),
                        decl.source_span,
                        Some(decl.id.clone()),
                    )
                    .with_import(import_id),
                );
            }
        }
    }
    diagnostics
}

/// `image.overflow` and `image.upscale` advisories for every image node on
/// the pages of `doc`.
///
/// - **`image.overflow`** (`fit="none"` only): the intrinsic pixels exceed
///   the declared box, so the image clips.
/// - **`image.upscale`**: the image renders larger than its intrinsic pixels
///   under its fit mode, so the raster looks pixelated.
///
/// SVG assets are exempt. Boxes in non-pixel units or token refs, and nodes
/// whose asset is unknown or unreadable, are skipped. Both are advisories.
#[must_use]
pub fn collect_image_dimension_diagnostics(
    fs: &dyn SourceFs,
    doc: &Document,
    project_dir: &Path,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for page in &doc.body.pages {
        walk_images(fs, &page.children, doc, project_dir, &mut out);
    }
    out
}

/// [`collect_missing_asset_diagnostics`] then
/// [`collect_image_dimension_diagnostics`]. Empty, with no file access, when
/// `project_dir` is `None`.
#[must_use]
pub fn disk_diagnostics(
    fs: &dyn SourceFs,
    doc: &Document,
    project_dir: Option<&Path>,
) -> Vec<Diagnostic> {
    match project_dir {
        Some(dir) => {
            let mut d = collect_missing_asset_diagnostics(fs, doc, dir);
            d.extend(collect_image_dimension_diagnostics(fs, doc, dir));
            d
        }
        None => Vec::new(),
    }
}

/// [`disk_diagnostics`] for the host document, then
/// [`collect_missing_import_asset_diagnostics`].
#[must_use]
pub fn disk_diagnostics_with_imports(
    fs: &dyn SourceFs,
    doc: &Document,
    project_dir: Option<&Path>,
    imports: &LoadedImportGraph,
) -> Vec<Diagnostic> {
    let mut d = disk_diagnostics(fs, doc, project_dir);
    d.extend(collect_missing_import_asset_diagnostics(fs, imports));
    d
}

/// Walk `nodes` recursively, collecting image dimension diagnostics.
///
/// Every node variant is listed (exhaustive match), so a future container
/// kind forces a decision here.
fn walk_images(
    fs: &dyn SourceFs,
    nodes: &[Node],
    doc: &Document,
    project_dir: &Path,
    out: &mut Vec<Diagnostic>,
) {
    for node in nodes {
        match node {
            Node::Image(img) => {
                check_image(fs, img, doc, project_dir, out);
            }
            Node::Frame(f) => {
                walk_images(fs, &f.children, doc, project_dir, out);
            }
            Node::Group(g) => {
                walk_images(fs, &g.children, doc, project_dir, out);
            }
            Node::Table(t) => {
                for row in &t.rows {
                    for cell in &row.cells {
                        walk_images(fs, &cell.children, doc, project_dir, out);
                    }
                }
            }
            // Leaf nodes that cannot contain children — explicit for exhaustiveness:
            Node::Rect(_)
            | Node::Ellipse(_)
            | Node::Line(_)
            | Node::Text(_)
            | Node::Code(_)
            | Node::Polygon(_)
            | Node::Polyline(_)
            | Node::Path(_)
            | Node::Instance(_)
            | Node::Field(_)
            | Node::Toc(_)
            | Node::Footnote(_)
            | Node::Shape(_)
            | Node::Connector(_)
            | Node::Pattern(_)
            | Node::Chart(_)
            | Node::Light(_)
            | Node::Mesh(_)
            | Node::Unknown(_) => {}
        }
    }
}

/// Check one image node and push any `image.overflow` / `image.upscale`
/// advisories into `out`.
fn check_image(
    fs: &dyn SourceFs,
    img: &ImageNode,
    doc: &Document,
    project_dir: &Path,
    out: &mut Vec<Diagnostic>,
) {
    // Only raw pixel boxes are checked; a token ref or a non-pixel unit is
    // skipped to avoid false positives (this layer has no token table).
    let w_dim = match img.w.as_ref() {
        Some(zenith_core::PropertyValue::Dimension(d)) => d,
        Some(
            zenith_core::PropertyValue::TokenRef(_)
            | zenith_core::PropertyValue::Literal(_)
            | zenith_core::PropertyValue::DataRef(_),
        )
        | None => return,
    };
    let h_dim = match img.h.as_ref() {
        Some(zenith_core::PropertyValue::Dimension(d)) => d,
        Some(
            zenith_core::PropertyValue::TokenRef(_)
            | zenith_core::PropertyValue::Literal(_)
            | zenith_core::PropertyValue::DataRef(_),
        )
        | None => return,
    };
    let w = match dim_to_px(w_dim.value, &w_dim.unit) {
        Some(px) => px,
        None => return,
    };
    let h = match dim_to_px(h_dim.value, &h_dim.unit) {
        Some(px) => px,
        None => return,
    };

    // An unknown asset id is `unknown_reference`'s job.
    let decl = match doc.assets.assets.iter().find(|d| d.id == img.asset) {
        Some(d) => d,
        None => return,
    };

    // SVG assets are vector: they scale without quality loss.
    if decl.kind != AssetKind::Image {
        return;
    }

    // A missing or unreadable file is `asset.missing`'s job.
    let Ok(bytes) = fs.read(&project_dir.join(&decl.src)) else {
        return;
    };
    let isz = match imagesize::blob_size(&bytes) {
        Ok(s) => s,
        Err(_) => return,
    };
    let iw = isz.width as f64;
    let ih = isz.height as f64;

    let fit = img.fit.as_deref();

    // fit="none" places the image at intrinsic size, so intrinsic > box clips.
    if fit == Some("none") && (iw > w || ih > h) {
        out.push(Diagnostic::advisory(
            "image.overflow",
            format!(
                "image '{}': intrinsic size {}x{} exceeds its box {}x{} (fit=\"none\")",
                img.id, iw as u32, ih as u32, w as u32, h as u32,
            ),
            img.source_span,
            Some(img.id.clone()),
        ));
    }

    // Upscale per fit mode. fit="none" never upscales. An unknown fit string
    // is skipped (validate already warns).
    let upscales = match fit {
        Some("none") => false,
        Some("stretch") | None => w > iw || h > ih,
        Some("contain") => (w / iw).min(h / ih) > 1.0,
        Some("cover") => (w / iw).max(h / ih) > 1.0,
        Some(_) => false,
    };

    if upscales {
        out.push(Diagnostic::advisory(
            "image.upscale",
            format!(
                "image '{}': rendered larger than its intrinsic {}x{} px; raster will appear pixelated",
                img.id, iw as u32, ih as u32,
            ),
            img.source_span,
            Some(img.id.clone()),
        ));
    }
}
