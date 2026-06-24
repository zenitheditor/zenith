//! Logic for `zenith workspace scratch new`, `list`, and `show`.

use std::path::Path;

use zenith_session::adapter::{OsClock, OsFs};
use zenith_session::{
    CandidateEntry, CandidateMeta, CandidateStatus, NewCandidate, StorePaths, list_scratch,
    put_scratch, resolve_data_dir,
};

use crate::cli::ScratchNewArgs;
use crate::commands::serialize_pretty;
use crate::history::{ensure_doc_id_in, read_doc_id};

// ── status parsing ────────────────────────────────────────────────────────────

/// Parse a status string into [`CandidateStatus`].
///
/// Returns `Err` with a human-readable message on unrecognised input.
pub(crate) fn parse_status(s: &str) -> Result<CandidateStatus, String> {
    match s {
        "draft" => Ok(CandidateStatus::Draft),
        "selected" => Ok(CandidateStatus::Selected),
        "rejected" => Ok(CandidateStatus::Rejected),
        other => Err(format!(
            "unknown status '{other}'; expected one of: draft, selected, rejected"
        )),
    }
}

// ── store helpers ─────────────────────────────────────────────────────────────

pub(crate) fn open_store() -> Result<StorePaths, String> {
    resolve_data_dir()
        .map(StorePaths::new)
        .map_err(|e| e.message)
}

// ── scratch new ───────────────────────────────────────────────────────────────

/// Outcome of `scratch new`: the created candidate id plus any non-fatal
/// `warning` surfaced while transparently attaching a `doc-id` (e.g. the file
/// was stamped but the initial history version could not be recorded).
#[derive(Debug)]
pub struct ScratchNewOutcome {
    pub id: String,
    pub warning: Option<String>,
}

/// Record the document bytes as a new scratch candidate.
///
/// If the document has no `doc-id` yet, one is transparently minted, stamped
/// into the file, and recorded as the initial history version before the
/// candidate is created (see [`ensure_doc_id_in`]). The candidate snapshot is
/// always the caller-supplied `doc_bytes`; identity comes from the file at
/// `doc_path` (in production these are the same content).
pub fn scratch_new(
    doc_bytes: &[u8],
    doc_path: &Path,
    args: &ScratchNewArgs,
) -> Result<ScratchNewOutcome, String> {
    let paths = open_store()?;
    scratch_new_in(&paths, doc_bytes, doc_path, args)
}

/// Testable variant with an explicit store root.
pub fn scratch_new_in(
    paths: &StorePaths,
    doc_bytes: &[u8],
    doc_path: &Path,
    args: &ScratchNewArgs,
) -> Result<ScratchNewOutcome, String> {
    // Attach a doc-id on first use (no-op + no history when one already exists).
    // Identity comes from the file at `doc_path`; the snapshot is always the
    // caller-supplied `doc_bytes` (in production these are the same content).
    let ensured = ensure_doc_id_in(paths, doc_path)?;
    let fs = OsFs;
    let clock = OsClock;
    let status = parse_status(&args.status)?;
    let meta = CandidateMeta {
        workspace_role: args.workspace_role.as_deref(),
        promotion_target: args.promotion_target.as_deref(),
        cleanup_policy: args.cleanup_policy.as_deref(),
        notes: args.notes.as_deref(),
    };
    let entry = put_scratch(
        &fs,
        paths,
        &clock,
        &ensured.doc_id,
        NewCandidate {
            page_id: args.page.as_deref().unwrap_or("*"),
            snapshot: doc_bytes,
            status,
            meta,
        },
    )
    .map_err(|e| e.message)?;
    Ok(ScratchNewOutcome {
        id: entry.id,
        warning: ensured.warning,
    })
}

// ── scratch list ──────────────────────────────────────────────────────────────

/// List all scratch candidates for the document at `doc_path`.
///
/// Returns a human-readable listing or a JSON array depending on `json`.
pub fn scratch_list(doc_path: &Path, json: bool) -> Result<String, String> {
    let paths = open_store()?;
    scratch_list_in(&paths, doc_path, json)
}

/// Testable variant with an explicit store root.
pub fn scratch_list_in(paths: &StorePaths, doc_path: &Path, json: bool) -> Result<String, String> {
    let doc_id = read_doc_id(doc_path)?;
    let fs = OsFs;
    let entries = list_scratch(&fs, paths, &doc_id).map_err(|e| e.message)?;

    if json {
        Ok(serialize_pretty(&entries))
    } else if entries.is_empty() {
        Ok("(no scratch candidates recorded yet)".to_owned())
    } else {
        let mut lines = Vec::with_capacity(entries.len());
        for e in &entries {
            let status = status_label(e.status);
            let notes = e.notes.as_deref().unwrap_or("");
            let notes_part = if notes.is_empty() {
                String::new()
            } else {
                format!("  notes={notes}")
            };
            lines.push(format!(
                "{}  {}  page={}{}",
                e.id, status, e.page_id, notes_part
            ));
        }
        Ok(lines.join("\n"))
    }
}

// ── scratch show ──────────────────────────────────────────────────────────────

/// Show detail for the candidate with `cand_id` in the document at `doc_path`.
pub fn scratch_show(doc_path: &Path, cand_id: &str, json: bool) -> Result<String, String> {
    let paths = open_store()?;
    scratch_show_in(&paths, doc_path, cand_id, json)
}

/// Testable variant with an explicit store root.
pub fn scratch_show_in(
    paths: &StorePaths,
    doc_path: &Path,
    cand_id: &str,
    json: bool,
) -> Result<String, String> {
    let doc_id = read_doc_id(doc_path)?;
    let fs = OsFs;
    let entries = list_scratch(&fs, paths, &doc_id).map_err(|e| e.message)?;
    let entry = entries
        .iter()
        .find(|e| e.id == cand_id)
        .ok_or_else(|| format!("candidate not found: {cand_id}"))?;

    if json {
        Ok(serialize_pretty(entry))
    } else {
        Ok(format_entry_detail(entry))
    }
}

// ── formatting helpers ────────────────────────────────────────────────────────

fn status_label(s: CandidateStatus) -> &'static str {
    match s {
        CandidateStatus::Draft => "draft",
        CandidateStatus::Selected => "selected",
        CandidateStatus::Rejected => "rejected",
    }
}

fn format_entry_detail(e: &CandidateEntry) -> String {
    let mut out = format!(
        "id:      {}\nseq:     {}\npage:    {}\nstatus:  {}\nhash:    {}",
        e.id,
        e.seq,
        e.page_id,
        status_label(e.status),
        e.snapshot_hash,
    );
    if let Some(r) = &e.workspace_role {
        out.push_str(&format!("\nrole:    {r}"));
    }
    if let Some(t) = &e.promotion_target {
        out.push_str(&format!("\ntarget:  {t}"));
    }
    if let Some(p) = &e.cleanup_policy {
        out.push_str(&format!("\npolicy:  {p}"));
    }
    if let Some(n) = &e.notes {
        out.push_str(&format!("\nnotes:   {n}"));
    }
    if let Some(ts) = e.timestamp_ms {
        out.push_str(&format!("\nts_ms:   {ts}"));
    }
    out
}
