//! Font/asset provider construction and disk-based diagnostics for `render`.

use std::path::Path;
use std::sync::Arc;

use std::collections::BTreeSet;

use zenith_core::{
    AssetKind, BytesAssetProvider, BytesFontProvider, Diagnostic, Document, FontProvider,
    FontSource, FontStyle, ImageNode, Node, TokenLiteral, TokenType, TokenValue, default_provider,
    dim_to_px,
};

use crate::commands::fonts::os_font_dirs;

use crate::commands::composition_imports::LoadedImportGraph;

use super::entry::RenderCmdErr;
use super::pipeline::verify_locked_sha256;

/// Build a [`BytesFontProvider`] preloaded with bundled fonts and any
/// `font`-kind assets declared in the document.
///
/// When `project_dir` is `None`, returns the default bundled-only provider
/// immediately (no filesystem access is attempted). When `Some`, each
/// `font`-kind [`AssetDecl`] in the document is read from disk and its
/// family/weight/style metadata is extracted via
/// [`zenith_layout::face_metadata`]. Successfully read faces are registered
/// under their real family name so that a `font.family` token whose value
/// matches that family resolves to the actual face instead of falling back
/// to Noto.
///
/// Non-locked failures (unreadable file, unparseable font) silently skip the
/// asset; a missing file is instead reported as a hard `asset.missing` Error
/// diagnostic by [`collect_missing_asset_diagnostics`]. When `locked` is `true`,
/// the same conditions
/// are hard errors (exit code 2), and every font asset's bytes are verified
/// against its declared `sha256` exactly like image and SVG assets.
pub(crate) fn build_font_provider(
    doc: &Document,
    project_dir: Option<&Path>,
    locked: bool,
) -> Result<BytesFontProvider, RenderCmdErr> {
    let mut provider = default_provider();
    if let Some(dir) = project_dir {
        register_project_fonts(&mut provider, doc, dir, locked)?;
    }
    register_local_fonts(&mut provider, doc);
    Ok(provider)
}

pub(crate) fn build_font_provider_with_imports(
    doc: &Document,
    project_dir: Option<&Path>,
    imports: &LoadedImportGraph,
    locked: bool,
) -> Result<BytesFontProvider, RenderCmdErr> {
    let mut provider = default_provider();
    if let Some(dir) = project_dir {
        register_project_fonts(&mut provider, doc, dir, locked)?;
        for (_, imported, import_dir) in imports.documents_with_dirs() {
            register_project_fonts(&mut provider, imported, import_dir, locked)?;
        }
    }
    register_local_fonts(&mut provider, doc);
    Ok(provider)
}

/// Register every `font`-kind project asset declared in `doc` into `provider`
/// with [`FontSource::Project`]. Extracted from [`build_font_provider`] so the
/// project pass and the local-system pass are clearly separated.
fn register_project_fonts(
    provider: &mut BytesFontProvider,
    doc: &Document,
    dir: &Path,
    locked: bool,
) -> Result<(), RenderCmdErr> {
    for decl in &doc.assets.assets {
        if decl.kind != AssetKind::Font {
            continue;
        }
        let path = dir.join(&decl.src);
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                if locked {
                    return Err(RenderCmdErr::new(
                        format!(
                            "--locked: could not read font asset '{}' from '{}': {}",
                            decl.id,
                            path.display(),
                            e
                        ),
                        2,
                    ));
                }
                // Missing/unreadable file is surfaced as a hard `asset.missing`
                // diagnostic by `collect_missing_asset_diagnostics`; skip here.
                continue;
            }
        };

        if locked {
            verify_locked_sha256(&decl.id, "font asset", decl.sha256.as_deref(), &bytes)?;
        }

        let arc: Arc<[u8]> = Arc::from(bytes.as_slice());
        match zenith_layout::face_metadata(&arc, 0) {
            Ok(m) => {
                provider.register(&m.family, m.weight, m.style, arc, 0, FontSource::Project);
            }
            Err(e) => {
                if locked {
                    return Err(RenderCmdErr::new(
                        format!(
                            "--locked: font asset '{}' could not be parsed: {}",
                            decl.id, e
                        ),
                        2,
                    ));
                }
                eprintln!(
                    "warning: font asset '{}' could not be parsed: {} — skipping",
                    decl.id, e
                );
            }
        }
    }
    Ok(())
}

/// Register machine-local/system fonts as a LAST-RESORT resolution source.
///
/// Scans the OS font directories ([`os_font_dirs`]) and registers each face with
/// [`FontSource::Local`] — but ONLY when the provider does not already resolve
/// that `(family, weight, style)`. Because bundled and project faces are
/// registered first, this `is_none()` guard guarantees local fonts NEVER shadow
/// a bundled or project face: a document that uses only bundled fonts resolves
/// to exactly the same bytes as before this pass existed (the byte-identical
/// invariant). A face that resolves from here later trips a `font.local`
/// advisory at compile time.
///
/// Read failures are skipped silently (no panic, no hard error): a local font is
/// a best-effort convenience, not a required asset.
fn register_local_fonts(provider: &mut BytesFontProvider, doc: &Document) {
    // Scanning the OS font directories reads and parses every installed font, so
    // do it ONLY when the document actually needs a family that bundled/project
    // fonts cannot satisfy. In a valid document every `font-family` reference
    // resolves through a `fontFamily` token, so those token values are the
    // complete set of families the document can request. A document using only
    // bundled families never touches the filesystem here — keeping render fast
    // and byte-identical.
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

    for entry in zenith_core::scan_font_dirs(&os_font_dirs()) {
        // Bundled/project ALWAYS win: only register a local face for a slot the
        // provider cannot already satisfy. This preserves byte-identical output
        // for documents whose families are covered by bundled/project fonts.
        if provider
            .resolve(
                std::slice::from_ref(&entry.family),
                entry.weight,
                entry.style,
            )
            .is_some()
        {
            continue;
        }
        let bytes = match std::fs::read(&entry.path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let arc: Arc<[u8]> = Arc::from(bytes.as_slice());
        provider.register(
            &entry.family,
            entry.weight,
            entry.style,
            arc,
            entry.index,
            FontSource::Local,
        );
    }
}

/// Build a [`BytesAssetProvider`] from a parsed document and the project
/// directory (the `.zen` file's parent).
///
/// `image`- and `svg`-kind assets are loaded; `font`-kind assets are handled
/// separately by [`build_font_provider`].
///
/// When `locked` is `false` (the default), a read failure silently skips the
/// asset and no hash is checked (a missing file is surfaced separately as a hard
/// `asset.missing` diagnostic). When `locked` is `true`, every image or
/// SVG asset must read successfully and its bytes must match its declared
/// `sha256` (compared case-insensitively, trimmed); a read failure, a missing
/// hash, or a mismatch is a hard error (exit code 2).
pub(crate) fn build_asset_provider(
    doc: &Document,
    project_dir: &Path,
    locked: bool,
) -> Result<BytesAssetProvider, RenderCmdErr> {
    let mut provider = BytesAssetProvider::new();
    register_document_assets(&mut provider, doc, "", project_dir, locked)?;
    Ok(provider)
}

pub(crate) fn build_asset_provider_with_imports(
    doc: &Document,
    project_dir: &Path,
    imports: &LoadedImportGraph,
    locked: bool,
) -> Result<BytesAssetProvider, RenderCmdErr> {
    let mut provider = BytesAssetProvider::new();
    register_document_assets(&mut provider, doc, "", project_dir, locked)?;
    for (import_id, imported, dir) in imports.documents_with_dirs() {
        register_document_assets(
            &mut provider,
            imported,
            &format!("{import_id}/"),
            dir,
            locked,
        )?;
    }
    Ok(provider)
}

fn register_document_assets(
    provider: &mut BytesAssetProvider,
    doc: &Document,
    id_prefix: &str,
    project_dir: &Path,
    locked: bool,
) -> Result<(), RenderCmdErr> {
    for decl in &doc.assets.assets {
        if !matches!(decl.kind, AssetKind::Image | AssetKind::Svg) {
            continue;
        }
        let path = project_dir.join(&decl.src);
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) => {
                if locked {
                    return Err(RenderCmdErr::new(
                        format!(
                            "--locked: could not read asset '{}' from '{}': {}",
                            decl.id,
                            path.display(),
                            e
                        ),
                        2,
                    ));
                }
                // Missing/unreadable file is surfaced as a hard `asset.missing`
                // diagnostic by `collect_missing_asset_diagnostics`; skip here.
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

/// Collect a hard `asset.missing` diagnostic for every declared asset whose
/// file does not exist on disk under `project_dir`.
///
/// All asset kinds are checked (image, svg, font). Declarations are iterated in
/// declaration order, so the resulting diagnostics are deterministic. The
/// returned diagnostics are `Severity::Error`, so once prepended to a render
/// artifact's diagnostics they trip the render gate and block output.
pub(crate) fn collect_missing_asset_diagnostics(
    doc: &Document,
    project_dir: &Path,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for decl in &doc.assets.assets {
        let path = project_dir.join(&decl.src);
        if !path.exists() {
            diagnostics.push(Diagnostic::error(
                "asset.missing",
                format!("asset '{}' file not found: '{}'", decl.id, path.display()),
                decl.source_span,
                Some(decl.id.clone()),
            ));
        }
    }
    diagnostics
}

/// Collect a hard `import.asset_missing` diagnostic for every declared asset of
/// an IMPORTED document whose file does not exist on disk under that import's
/// resolved directory.
///
/// The host document's own assets are covered by
/// [`collect_missing_asset_diagnostics`]; this is the imported-document parallel,
/// carrying the import origin (the import id and the resolved path) so the
/// failure is attributable to the specific import. Imports and their assets are
/// iterated in deterministic (`BTreeMap`) order. Returned diagnostics are
/// `Severity::Error`, so they trip the render gate exactly like `asset.missing`.
pub(crate) fn collect_missing_import_asset_diagnostics(
    imports: &LoadedImportGraph,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for (import_id, doc, dir) in imports.documents_with_dirs() {
        for decl in &doc.assets.assets {
            let path = dir.join(&decl.src);
            if !path.exists() {
                diagnostics.push(Diagnostic::error(
                    "import.asset_missing",
                    format!(
                        "import '{}' asset '{}' file not found: '{}'",
                        import_id,
                        decl.id,
                        path.display()
                    ),
                    decl.source_span,
                    Some(decl.id.clone()),
                ));
            }
        }
    }
    diagnostics
}

/// Collect `image.overflow` and `image.upscale` advisories for all image nodes
/// in the document.
///
/// - **`image.overflow`** (fit="none" only): the image's intrinsic pixel
///   dimensions exceed the declared box, so the image clips unexpectedly.
/// - **`image.upscale`**: the image will be rendered LARGER than its intrinsic
///   pixels (raster will appear pixelated), computed per the active fit mode.
///
/// SVG assets are exempt (vector, scales cleanly). Image nodes whose box uses
/// `(pct)` or other non-absolute units are skipped (not false positives).
/// Nodes referencing unknown or missing assets are skipped (covered elsewhere).
/// Both diagnostics are `Severity::Advisory` and do NOT block rendering.
pub fn collect_image_dimension_diagnostics(doc: &Document, project_dir: &Path) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for page in &doc.body.pages {
        walk_images(&page.children, doc, project_dir, &mut out);
    }
    out
}

/// Collect all disk-based diagnostics (`asset.missing` + `image.overflow` /
/// `image.upscale`) for a document and its project directory.
///
/// When `project_dir` is `None`, no filesystem access is attempted and an
/// empty `Vec` is returned. When `Some`, both
/// [`collect_missing_asset_diagnostics`] and
/// [`collect_image_dimension_diagnostics`] are run and their results merged.
/// This is the single call-site replacement for the repeated inline block:
/// ```text
/// match project_dir {
///     Some(dir) => { let mut d = collect_missing...; d.extend(collect_image...); d }
///     None => Vec::new(),
/// }
/// ```
pub(super) fn disk_diagnostics(doc: &Document, project_dir: Option<&Path>) -> Vec<Diagnostic> {
    match project_dir {
        Some(dir) => {
            let mut d = collect_missing_asset_diagnostics(doc, dir);
            d.extend(collect_image_dimension_diagnostics(doc, dir));
            d
        }
        None => Vec::new(),
    }
}

/// [`disk_diagnostics`] for the host document plus `import.asset_missing`
/// diagnostics for every imported document's missing assets.
///
/// This is the single call-site replacement used by every render entry point so
/// that imported assets are gate-checked alongside the host's own assets.
pub(super) fn disk_diagnostics_with_imports(
    doc: &Document,
    project_dir: Option<&Path>,
    imports: &LoadedImportGraph,
) -> Vec<Diagnostic> {
    let mut d = disk_diagnostics(doc, project_dir);
    d.extend(collect_missing_import_asset_diagnostics(imports));
    d
}

/// Recursively walk `nodes`, collecting image dimension diagnostics.
///
/// Containers (`Frame`, `Group`) are recursed into. All other node variants
/// are listed explicitly and treated as no-ops (exhaustive match guards against
/// silently missing a future container type).
fn walk_images(nodes: &[Node], doc: &Document, project_dir: &Path, out: &mut Vec<Diagnostic>) {
    for node in nodes {
        match node {
            Node::Image(img) => {
                check_image(img, doc, project_dir, out);
            }
            Node::Frame(f) => {
                walk_images(&f.children, doc, project_dir, out);
            }
            Node::Group(g) => {
                walk_images(&g.children, doc, project_dir, out);
            }
            Node::Table(t) => {
                for row in &t.rows {
                    for cell in &row.cells {
                        walk_images(&cell.children, doc, project_dir, out);
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

/// Check a single image node and push any `image.overflow` / `image.upscale`
/// advisories into `out`.
fn check_image(img: &ImageNode, doc: &Document, project_dir: &Path, out: &mut Vec<Diagnostic>) {
    // Resolve box dimensions to pixels — skip if either axis uses a non-pixel
    // unit (pct, deg, unknown) to avoid false positives.
    // Geometry is now `(px)N` literal OR `(token)"id"` ref; this render-layer
    // advisory has no token table, so it only checks raw-dimension boxes and
    // skips token-ref geometry (same skip as a non-pixel unit).
    let w_dim = match img.w.as_ref() {
        Some(zenith_core::PropertyValue::Dimension(d)) => d,
        _ => return,
    };
    let h_dim = match img.h.as_ref() {
        Some(zenith_core::PropertyValue::Dimension(d)) => d,
        _ => return,
    };
    let w = match dim_to_px(w_dim.value, &w_dim.unit) {
        Some(px) => px,
        None => return,
    };
    let h = match dim_to_px(h_dim.value, &h_dim.unit) {
        Some(px) => px,
        None => return,
    };

    // Look up the asset declaration — skip if unknown (unknown_reference handles it).
    let decl = match doc.assets.assets.iter().find(|d| d.id == img.asset) {
        Some(d) => d,
        None => return,
    };

    // SVG assets are vector — they scale without quality loss; skip.
    if decl.kind != AssetKind::Image {
        return;
    }

    // Read only the image header (cheap — no full decode).
    let path = project_dir.join(&decl.src);
    let isz = match imagesize::size(&path) {
        Ok(s) => s,
        Err(_) => return, // missing/unreadable — asset.missing covers it
    };
    let iw = isz.width as f64;
    let ih = isz.height as f64;

    let fit = img.fit.as_deref();

    // ── image.overflow ───────────────────────────────────────────────────────
    // Only emitted for fit="none": the image is placed at intrinsic size with
    // no scaling, so if intrinsic > box the image clips.
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

    // ── image.upscale ────────────────────────────────────────────────────────
    // Emitted when the rendered size is larger than the intrinsic pixel count,
    // per fit mode. fit="none" never upscales (image is placed at intrinsic
    // size). Unknown fit strings are skipped (validate already warns).
    let upscales = match fit {
        Some("none") => false,
        Some("stretch") | None => w > iw || h > ih,
        Some("contain") => {
            // Scale factor = min of both axes; upscale when that factor > 1.
            let s = (w / iw).min(h / ih);
            s > 1.0
        }
        Some("cover") => {
            // Scale factor = max of both axes; upscale when that factor > 1.
            let s = (w / iw).max(h / ih);
            s > 1.0
        }
        Some(_) => false, // unknown fit string — skip
    };

    if upscales {
        out.push(Diagnostic::advisory(
            "image.upscale",
            format!(
                "image '{}': rendered larger than its intrinsic {}x{} px; raster will appear pixelated",
                img.id,
                iw as u32,
                ih as u32,
            ),
            img.source_span,
            Some(img.id.clone()),
        ));
    }
}

#[cfg(test)]
mod tests {
    use zenith_core::{AssetProvider, FontProvider, FontSource, KdlAdapter, KdlSource};

    use crate::commands::composition_imports::load_import_graph;

    use super::*;

    fn parse(src: &str) -> Document {
        KdlAdapter
            .parse(src.as_bytes())
            .expect("test document must parse")
    }

    #[test]
    fn build_asset_provider_registers_imported_assets_from_import_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join("brand")).expect("create brand dir");
        std::fs::write(dir.path().join("brand/logo.bin"), b"imported-logo")
            .expect("write imported asset");
        std::fs::write(
            dir.path().join("brand/brand.zen"),
            r#"zenith version=1 {
  project id="proj.brand" name="Brand"
  assets {
    asset id="logo" kind="image" src="logo.bin"
  }
  document id="doc.brand" title="Brand" {
    page id="page.brand" w=(px)10 h=(px)10
  }
}
"#,
        )
        .expect("write imported document");
        let root = parse(
            r#"zenith version=1 {
  project id="proj.host" name="Host"
  imports {
    import id="brand" kind="zen" src="brand/brand.zen"
  }
  document id="doc.host" title="Host" {
    page id="page.host" w=(px)10 h=(px)10
  }
}
"#,
        );
        let imports = load_import_graph(&root, Some(dir.path()));

        let provider = build_asset_provider_with_imports(&root, dir.path(), &imports, false)
            .expect("provider");

        let asset = provider
            .by_id("brand/logo")
            .expect("imported asset must be registered under import namespace");
        assert_eq!(&asset.bytes[..], b"imported-logo");
    }

    #[test]
    fn collect_missing_import_asset_diagnostics_reports_missing_imported_asset() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join("brand")).expect("create brand dir");
        // The imported document declares an asset whose file is NOT written.
        std::fs::write(
            dir.path().join("brand/brand.zen"),
            r#"zenith version=1 {
  project id="proj.brand" name="Brand"
  assets {
    asset id="logo" kind="image" src="missing.png"
  }
  document id="doc.brand" title="Brand" {
    page id="page.brand" w=(px)10 h=(px)10
  }
}
"#,
        )
        .expect("write imported document");
        let root = parse(
            r#"zenith version=1 {
  project id="proj.host" name="Host"
  imports {
    import id="brand" kind="zen" src="brand/brand.zen"
  }
  document id="doc.host" title="Host" {
    page id="page.host" w=(px)10 h=(px)10
  }
}
"#,
        );
        let imports = load_import_graph(&root, Some(dir.path()));

        let diagnostics = collect_missing_import_asset_diagnostics(&imports);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "import.asset_missing");
        assert_eq!(diagnostics[0].subject_id.as_deref(), Some("logo"));
    }

    #[test]
    fn build_font_provider_registers_imported_font_assets_from_import_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join("brand")).expect("create brand dir");
        let font_src = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../zenith-core/assets/fonts/NotoSans-Regular.ttf");
        std::fs::copy(&font_src, dir.path().join("brand/brand.ttf"))
            .expect("copy imported font asset");
        std::fs::write(
            dir.path().join("brand/brand.zen"),
            r#"zenith version=1 {
  project id="proj.brand" name="Brand"
  assets {
    asset id="font.brand" kind="font" src="brand.ttf"
  }
  tokens format="zenith-token-v1" {
    token id="font.brand" type="fontFamily" value="Noto Sans"
  }
  document id="doc.brand" title="Brand" {
    page id="page.brand" w=(px)10 h=(px)10
  }
}
"#,
        )
        .expect("write imported document");
        let root = parse(
            r#"zenith version=1 {
  project id="proj.host" name="Host"
  imports {
    import id="brand" kind="zen" src="brand/brand.zen"
  }
  document id="doc.host" title="Host" {
    page id="page.host" w=(px)10 h=(px)10
  }
}
"#,
        );
        let imports = load_import_graph(&root, Some(dir.path()));

        let provider = build_font_provider_with_imports(&root, Some(dir.path()), &imports, false)
            .expect("provider");
        let family = "Noto Sans".to_owned();
        let resolved = provider
            .resolve(&[family], 400, zenith_core::FontStyle::Normal)
            .expect("imported font asset must resolve");

        assert_eq!(resolved.source, FontSource::Project);
    }
}
