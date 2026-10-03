//! Shared parse/validate, page-resolution, and hash-verification helpers.

use std::path::Path;

use sha2::{Digest, Sha256};

use zenith_core::{
    Diagnostic, DiagnosticPolicy, Document, KdlAdapter, KdlSource, apply_policy,
    merge_brand_contract, validate_with_policy,
};

use crate::config::{CliPolicyFlags, load_global_and_local, merge_policy};
use crate::report::{ImportFiles, attributed_loader_diagnostics};

use crate::commands::composition_imports::{LoadedImportGraph, load_import_graph};

use super::assets::disk_diagnostics_with_imports;
use super::entry::RenderCmdErr;

/// Verify that `bytes` match the `sha256` field declared on an asset.
///
/// `id` is the asset identifier (for error messages); `kind` is a short noun
/// used in error messages (`"asset"` or `"font asset"`).
///
/// Returns `Err` (exit code 2) when:
/// - `sha256` is `None` (no hash declared).
/// - The computed SHA-256 hex digest does not match `sha256` (case-insensitive,
///   trimmed).
pub(super) fn verify_locked_sha256(
    id: &str,
    kind: &str,
    sha256: Option<&str>,
    bytes: &[u8],
) -> Result<(), RenderCmdErr> {
    let declared = sha256.ok_or_else(|| {
        RenderCmdErr::new(
            "asset.sha256_missing",
            format!(
                "--locked: {kind} '{id}' has no declared sha256; add sha256=\"<hex>\" to it or \
                 drop --locked"
            ),
            2,
        )
    })?;
    let hex = format!("{:x}", Sha256::digest(bytes));
    if declared.trim().to_lowercase() != hex {
        return Err(RenderCmdErr::new(
            "asset.sha256_mismatch",
            format!("--locked: {kind} '{id}' sha256 mismatch (declared {declared}, actual {hex})"),
            2,
        ));
    }
    Ok(())
}

/// Parse → validate with the merged diagnostic policy and brand contract,
/// returning the parsed [`Document`], the merged [`DiagnosticPolicy`], the
/// import graph, and the validation diagnostics (warnings and advisories).
///
/// The effective policy is `merge_policy(global, local, in_file, flags)`,
/// mirroring the `validate` command exactly:
/// - Global config is always consulted.
/// - Local config is walked up from `start_dir` when `Some`.
/// - In-file policy comes from the parsed document.
/// - CLI flags layer on top.
///
/// The effective brand contract is `merge_brand_contract(global, local)` then
/// overridden by `doc.brand_contract` (per-category: in-file > local > global).
///
/// The merged policy is returned so the render entry points can apply the SAME
/// policy to the compile-stage diagnostics emitted by `zenith-scene` (which run
/// after validation) — see [`govern_compile_diagnostics`].
///
/// A config-load error returns exit code 2. Parse errors return exit code 2.
/// Validation errors (at least one Error-severity diagnostic after policy
/// application) return exit code 1. With no config files and no flags the
/// merged policy and brand are both empty, so the result is byte-identical to
/// the old behaviour.
pub(super) fn parse_validate(
    src: &str,
    start_dir: Option<&Path>,
    flags: &CliPolicyFlags,
) -> Result<Validated, RenderCmdErr> {
    // Resolve config policy and brand contract ───────────────────────────────
    let (global, local, global_brand, local_brand) = load_global_and_local(start_dir)
        .map_err(|msg| RenderCmdErr::new("config.error", msg, 2))?;

    // Parse ─────────────────────────────────────────────────────────────────
    let doc = KdlAdapter.parse(src.as_bytes()).map_err(|e| {
        RenderCmdErr::blocked(
            vec![Diagnostic::error("parse.error", e.message, e.span, None)],
            2,
        )
    })?;

    // Validate ───────────────────────────────────────────────────────────────
    let merged = merge_policy(&global, &local, &doc.diagnostic_policy, flags);
    let effective_brand = merge_brand_contract(
        &merge_brand_contract(&global_brand, &local_brand),
        &doc.brand_contract,
    );
    let report = validate_with_policy(&doc, &merged, &effective_brand);
    let imports = load_import_graph(&doc, start_dir);
    if Diagnostic::has_errors(&report.diagnostics) {
        // Report the cheap disk and import diagnostics with the validation
        // errors, so one round shows every known problem.
        let mut diagnostics = report.diagnostics;
        diagnostics.extend(attributed_loader_diagnostics(&imports));
        diagnostics.extend(disk_diagnostics_with_imports(&doc, start_dir, &imports));
        return Err(RenderCmdErr::blocked(diagnostics, 1)
            .with_import_files(ImportFiles::from_graph(&imports)));
    }

    Ok(Validated {
        doc,
        policy: merged,
        imports,
        diagnostics: report.diagnostics,
    })
}

/// A parsed, validated document with the state the render entry points share.
pub(super) struct Validated {
    pub(super) doc: Document,
    /// The merged diagnostic policy, applied to compile-stage diagnostics too.
    pub(super) policy: DiagnosticPolicy,
    pub(super) imports: LoadedImportGraph,
    /// Policy-governed validation diagnostics. None is an error. Entry points
    /// put them first in their own diagnostic list.
    pub(super) diagnostics: Vec<Diagnostic>,
}

/// Apply the merged diagnostic `policy` to a list of compile-stage diagnostics
/// and return the governed list for attachment to the artifact.
///
/// Compile diagnostics (e.g. `font.unresolved`, `font.glyph_missing`) are
/// emitted by `zenith-scene` AFTER validation, so they never pass through the
/// validation choke point. This is the single place the render path runs them
/// through [`zenith_core::apply_policy`] — so `deny`/`allow`/`warn` entries for
/// those codes take effect on render exactly as they do for validation
/// diagnostics.
///
/// This function is **infallible**: it applies the policy and returns the
/// governed `Vec<Diagnostic>` directly. The caller attaches it to the artifact;
/// the dispatch layer (`Diagnostic::has_errors`) decides the exit code when any
/// of those diagnostics are [`Severity::Error`](zenith_core::Severity::Error).
///
/// With an empty policy this is an exact identity pass: the diagnostics are
/// returned unchanged, so artifacts and exit codes stay byte-identical to the
/// no-policy case. The policy only ever filters/relabels the diagnostic LIST —
/// it never touches the already-rendered scene/PNG/PDF bytes, which are
/// produced independently.
pub(super) fn govern_compile_diagnostics(
    diagnostics: Vec<Diagnostic>,
    policy: &DiagnosticPolicy,
) -> Vec<Diagnostic> {
    apply_policy(diagnostics, policy)
}

/// Resolve a 1-based `page` number to a 0-based page index within `doc`.
///
/// Returns `Err` (exit code 2) when the document has no pages or when `page`
/// is outside `1..=pages.len()`.
pub(super) fn resolve_page_index(doc: &Document, page: usize) -> Result<usize, RenderCmdErr> {
    let n = doc.body.pages.len();
    if doc.body.pages.is_empty() || page < 1 || page > n {
        return Err(RenderCmdErr::new(
            "render.page_out_of_range",
            format!("page {page} out of range; document has {n} page(s); pass --page 1 to {n}"),
            2,
        ));
    }
    Ok(page - 1)
}
