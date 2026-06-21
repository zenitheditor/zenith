//! Revision-spec resolution: map a revspec string to a session record id.
//!
//! Supported forms (resolved over the Tier-1 journal DAG):
//! - `@head` / `HEAD` / `@`            → the current HEAD record
//! - `@head~N` / `HEAD~N` (N >= 0)     → walk N parent links back from HEAD
//! - `@head^` / `HEAD^`                → parent of HEAD (same as `~1`)
//! - `<seq>` (a bare non-negative int) → the record with that `seq`
//! - `@time:<unix_ms>`                 → the most recent record at or before that time
//! - `@latest:<label>`                 → the highest-seq record whose `op_kind` OR `label` equals `<label>`
//! - `<id>` or a unique `<id-prefix>`  → the record whose id equals / uniquely begins with the text
//!
//! Resolution is deterministic and never panics; ambiguity or no-match is an error.

use crate::adapter::Fs;
use crate::error::SessionError;
use crate::layout::StorePaths;
use crate::manifest::{HistoryRecord, read_records};
use crate::session::{find_record, journal_path, load_state};

// ── Private helpers ────────────────────────────────────────────────────────────

/// Walk `n` parent links back from `start`, returning the resulting record id.
/// `n == 0` returns `start` unchanged.
fn walk_parents(
    records: &[HistoryRecord],
    start: &str,
    n: usize,
    spec: &str,
) -> Result<String, SessionError> {
    let mut cur = start.to_owned();
    for _ in 0..n {
        let rec = find_record(records, &cur)
            .ok_or_else(|| SessionError::new(format!("unknown record {cur}")))?;
        cur = rec
            .parent
            .clone()
            .ok_or_else(|| SessionError::new(format!("revspec {spec} goes past the root")))?;
    }
    Ok(cur)
}

/// Among records with `timestamp_ms <= target`, pick the one with the largest
/// `timestamp_ms`; tie-break by largest `seq`. Returns `None` if no candidates.
fn resolve_time(records: &[HistoryRecord], target: u128) -> Option<String> {
    records
        .iter()
        .filter_map(|r| {
            r.timestamp_ms
                .filter(|&ts| ts <= target)
                .map(|ts| (ts, r.seq, &r.id))
        })
        .max_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)))
        .map(|(_, _, id)| id.clone())
}

/// Among records whose `op_kind` or `label` equals `label`, pick the one with
/// the highest `seq`. Returns `None` if no candidates.
fn resolve_latest(records: &[HistoryRecord], label: &str) -> Option<String> {
    records
        .iter()
        .filter(|r| r.op_kind.as_deref() == Some(label) || r.label.as_deref() == Some(label))
        .max_by_key(|r| r.seq)
        .map(|r| r.id.clone())
}

/// Resolve an id or id-prefix: exact match wins; otherwise exactly-one prefix
/// match succeeds; zero or multiple prefix matches are errors.
fn resolve_id_or_prefix(records: &[HistoryRecord], s: &str) -> Result<String, SessionError> {
    // Exact match short-circuits immediately.
    if let Some(rec) = find_record(records, s) {
        return Ok(rec.id.clone());
    }
    // Prefix scan.
    let matches: Vec<&HistoryRecord> = records.iter().filter(|r| r.id.starts_with(s)).collect();
    match matches.len() {
        0 => Err(SessionError::new(format!("no record matching {s}"))),
        1 => Ok(matches[0].id.clone()),
        n => Err(SessionError::new(format!(
            "ambiguous revspec {s} matches {n} records"
        ))),
    }
}

// ── Public pure resolver ───────────────────────────────────────────────────────

/// Resolve `spec` to a record id, given the full record list and the current
/// HEAD (`None` if the session is empty). Returns an error if the spec is
/// malformed, ambiguous, or matches no record.
pub fn resolve_revspec(
    records: &[HistoryRecord],
    head: Option<&str>,
    spec: &str,
) -> Result<String, SessionError> {
    let s = spec.trim();

    // 1. Empty spec.
    if s.is_empty() {
        return Err(SessionError::new("empty revspec"));
    }

    // 2. Bare HEAD aliases.
    if s == "@head" || s == "HEAD" || s == "@" {
        return head
            .map(str::to_owned)
            .ok_or_else(|| SessionError::new("no HEAD in session"));
    }

    // 3. HEAD-relative parent walk.
    if let Some(rest) = s.strip_prefix("@head").or_else(|| s.strip_prefix("HEAD")) {
        // `rest` is the suffix after "@head" or "HEAD".
        let head_id = head.ok_or_else(|| SessionError::new("no HEAD in session"))?;
        if rest == "^" {
            return walk_parents(records, head_id, 1, s);
        }
        if let Some(n_str) = rest.strip_prefix('~') {
            let n = n_str
                .parse::<usize>()
                .map_err(|_| SessionError::new(format!("unrecognized HEAD revspec: {s}")))?;
            return walk_parents(records, head_id, n, s);
        }
        return Err(SessionError::new(format!("unrecognized HEAD revspec: {s}")));
    }

    // 4. @time:<unix_ms>
    if let Some(ts_str) = s.strip_prefix("@time:") {
        let target = ts_str
            .parse::<u128>()
            .map_err(|_| SessionError::new(format!("invalid @time timestamp: {ts_str}")))?;
        return resolve_time(records, target)
            .ok_or_else(|| SessionError::new(format!("no record at or before time {target}")));
    }

    // 5. @latest:<label>
    if let Some(label) = s.strip_prefix("@latest:") {
        return resolve_latest(records, label)
            .ok_or_else(|| SessionError::new(format!("no record matching @latest:{label}")));
    }

    // 6. Bare non-negative integer → seq lookup.
    if let Ok(n) = s.parse::<u64>() {
        return records
            .iter()
            .find(|r| r.seq == n)
            .map(|r| r.id.clone())
            .ok_or_else(|| SessionError::new(format!("no record with seq {n}")));
    }

    // 7. Id or id-prefix.
    resolve_id_or_prefix(records, s)
}

// ── Public fs convenience ──────────────────────────────────────────────────────

/// Load the session for `doc_id` and resolve `spec` to a record id.
pub fn resolve_revspec_for(
    fs: &impl Fs,
    paths: &StorePaths,
    doc_id: &str,
    spec: &str,
) -> Result<String, SessionError> {
    let state = load_state(fs, paths, doc_id)?;
    let records = read_records(fs, &journal_path(paths, doc_id))?;
    resolve_revspec(&records, state.head.as_deref(), spec)
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::{FakeClock, FakeRng, MemFs};
    use crate::layout::StorePaths;
    use crate::session::record_state;

    // ── Test fixture ──────────────────────────────────────────────────────────

    /// Build a 4-record chain:
    /// r0(seq0,parent None, ts 100, op "edit")
    ///   ← r1(seq1,parent r0, ts 200, op "external")
    ///     ← r2(seq2,parent r1, ts 300, op "edit", label Some("v1.0"))
    ///       ← r3(seq3,parent r2, ts 400, op "edit")
    /// head = "r3"
    fn make_chain() -> Vec<HistoryRecord> {
        let mut r0 = HistoryRecord::new("r0", 0, None, "snap0");
        r0.timestamp_ms = Some(100);
        r0.op_kind = Some("edit".to_owned());

        let mut r1 = HistoryRecord::new("r1", 1, Some("r0".to_owned()), "snap1");
        r1.timestamp_ms = Some(200);
        r1.op_kind = Some("external".to_owned());

        let mut r2 = HistoryRecord::new("r2", 2, Some("r1".to_owned()), "snap2");
        r2.timestamp_ms = Some(300);
        r2.op_kind = Some("edit".to_owned());
        r2.label = Some("v1.0".to_owned());

        let mut r3 = HistoryRecord::new("r3", 3, Some("r2".to_owned()), "snap3");
        r3.timestamp_ms = Some(400);
        r3.op_kind = Some("edit".to_owned());

        vec![r0, r1, r2, r3]
    }

    const HEAD: &str = "r3";

    // ── head_forms ────────────────────────────────────────────────────────────

    #[test]
    fn head_forms() {
        let records = make_chain();
        assert_eq!(
            resolve_revspec(&records, Some(HEAD), "@head").unwrap(),
            "r3"
        );
        assert_eq!(resolve_revspec(&records, Some(HEAD), "HEAD").unwrap(), "r3");
        assert_eq!(resolve_revspec(&records, Some(HEAD), "@").unwrap(), "r3");
    }

    // ── head_tilde ────────────────────────────────────────────────────────────

    #[test]
    fn head_tilde() {
        let records = make_chain();
        assert_eq!(
            resolve_revspec(&records, Some(HEAD), "@head~1").unwrap(),
            "r2"
        );
        assert_eq!(
            resolve_revspec(&records, Some(HEAD), "HEAD~2").unwrap(),
            "r1"
        );
        assert_eq!(
            resolve_revspec(&records, Some(HEAD), "@head~0").unwrap(),
            "r3"
        );
        assert_eq!(
            resolve_revspec(&records, Some(HEAD), "@head^").unwrap(),
            "r2"
        );
    }

    // ── head_tilde_past_root_errors ───────────────────────────────────────────

    #[test]
    fn head_tilde_past_root_errors() {
        let records = make_chain();
        assert!(resolve_revspec(&records, Some(HEAD), "@head~99").is_err());
    }

    // ── head_no_session_errors ────────────────────────────────────────────────

    #[test]
    fn head_no_session_errors() {
        let records = make_chain();
        assert!(resolve_revspec(&records, None, "@head").is_err());
    }

    // ── seq_lookup ────────────────────────────────────────────────────────────

    #[test]
    fn seq_lookup() {
        let records = make_chain();
        assert_eq!(resolve_revspec(&records, Some(HEAD), "0").unwrap(), "r0");
        assert_eq!(resolve_revspec(&records, Some(HEAD), "2").unwrap(), "r2");
        assert!(resolve_revspec(&records, Some(HEAD), "9").is_err());
    }

    // ── time_at_or_before ─────────────────────────────────────────────────────

    #[test]
    fn time_at_or_before() {
        let records = make_chain();
        // ts200 is the latest at or before 250
        assert_eq!(
            resolve_revspec(&records, Some(HEAD), "@time:250").unwrap(),
            "r1"
        );
        // exact match at 400
        assert_eq!(
            resolve_revspec(&records, Some(HEAD), "@time:400").unwrap(),
            "r3"
        );
        // nothing at or before 50
        assert!(resolve_revspec(&records, Some(HEAD), "@time:50").is_err());
    }

    // ── latest_label ──────────────────────────────────────────────────────────

    #[test]
    fn latest_label() {
        let records = make_chain();
        // matches op_kind "external" → r1
        assert_eq!(
            resolve_revspec(&records, Some(HEAD), "@latest:external").unwrap(),
            "r1"
        );
        // matches label "v1.0" → r2
        assert_eq!(
            resolve_revspec(&records, Some(HEAD), "@latest:v1.0").unwrap(),
            "r2"
        );
        // no match
        assert!(resolve_revspec(&records, Some(HEAD), "@latest:nope").is_err());
    }

    // ── id_exact_and_prefix ───────────────────────────────────────────────────

    #[test]
    fn id_exact_and_prefix() {
        let records = make_chain();
        // Exact id match.
        assert_eq!(resolve_revspec(&records, Some(HEAD), "r2").unwrap(), "r2");

        // Unique prefix "r0" is an exact match in this chain, but test a prefix
        // that is a prefix of exactly one: none of r1/r2/r3 start with "r0".
        assert_eq!(resolve_revspec(&records, Some(HEAD), "r0").unwrap(), "r0");

        // Build a set where "ra" is ambiguous.
        let mut extra = records.clone();
        let ra1 = HistoryRecord::new("ra1", 10, None, "snapA");
        let ra2 = HistoryRecord::new("ra2", 11, None, "snapB");
        extra.push(ra1);
        extra.push(ra2);

        // Ambiguous prefix → error.
        assert!(resolve_revspec(&extra, Some(HEAD), "ra").is_err());
        // Exact id "ra1" → success even though "ra" is ambiguous.
        assert_eq!(resolve_revspec(&extra, Some(HEAD), "ra1").unwrap(), "ra1");
    }

    // ── malformed ─────────────────────────────────────────────────────────────

    #[test]
    fn malformed() {
        let records = make_chain();
        assert!(resolve_revspec(&records, Some(HEAD), "").is_err());
        assert!(resolve_revspec(&records, Some(HEAD), "@head~x").is_err());
        assert!(resolve_revspec(&records, Some(HEAD), "@bogus").is_err());
    }

    // ── resolve_revspec_for_smoke ─────────────────────────────────────────────

    #[test]
    fn resolve_revspec_for_smoke() {
        let fs = MemFs::new();
        let paths = StorePaths::new("/data");
        let clock = FakeClock(std::time::SystemTime::UNIX_EPOCH);
        let rng = FakeRng(0);

        record_state(&fs, &paths, &clock, &rng, "doc1", b"v1", None).unwrap(); // r0
        record_state(&fs, &paths, &clock, &rng, "doc1", b"v2", None).unwrap(); // r1

        // @head resolves to the current HEAD (r1).
        let head_id = resolve_revspec_for(&fs, &paths, "doc1", "@head").unwrap();
        assert_eq!(head_id, "r1");

        // @head~1 resolves to the parent (r0).
        let parent_id = resolve_revspec_for(&fs, &paths, "doc1", "@head~1").unwrap();
        assert_eq!(parent_id, "r0");
    }
}
