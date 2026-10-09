//! Property-style round trip of the source patcher over every example.
//!
//! For each example and each id-bearing node the test runs `remove_node`,
//! `duplicate_node`, `move_forward`, and `move_backward`, and
//! `duplicate_page` for each page. Every accepted op must patch to text that
//! parses to the after document. An op that falls back must name an expected
//! layout reason. The patched and fallback counts print with `--nocapture`.

use std::collections::BTreeMap;

use zenith_core::{
    Document, KdlAdapter, KdlSource, Node, PatchErrorCode, Severity, patch_source, strip_spans,
    try_patch_source,
};
use zenith_tx::{Transaction, TxStatus, run_transaction};

fn examples() -> Vec<(String, String)> {
    let dir = format!("{}/../examples", env!("CARGO_MANIFEST_DIR"));
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("examples dir") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("zen") {
            continue;
        }
        let src = std::fs::read_to_string(&path).expect("read");
        let name = path
            .file_name()
            .expect("name")
            .to_string_lossy()
            .into_owned();
        out.push((name, src));
    }
    out.sort();
    out
}

fn node_ids(nodes: &[Node], out: &mut Vec<String>) {
    for node in nodes {
        if let Some(id) = node.id() {
            out.push(id.to_owned());
        }
        if let Some(children) = node.children() {
            node_ids(children, out);
        }
        if let Node::Table(table) = node {
            for cell in table.rows.iter().flat_map(|row| &row.cells) {
                node_ids(&cell.children, out);
            }
        }
    }
}

fn ops_for(doc: &Document) -> Vec<String> {
    let mut ids = Vec::new();
    for page in &doc.body.pages {
        node_ids(&page.children, &mut ids);
    }
    let mut ops = Vec::new();
    for id in &ids {
        ops.push(format!(r#"{{"op":"remove_node","node":"{id}"}}"#));
        ops.push(format!(
            r#"{{"op":"duplicate_node","node":"{id}","new_id":"{id}.dup"}}"#
        ));
        ops.push(format!(r#"{{"op":"move_forward","node":"{id}"}}"#));
        ops.push(format!(r#"{{"op":"move_backward","node":"{id}"}}"#));
    }
    for page in &doc.body.pages {
        let id = &page.id;
        ops.push(format!(
            r#"{{"op":"duplicate_page","page":"{id}","new_id":"{id}.dup","id_suffix":".dup"}}"#
        ));
    }
    ops
}

#[derive(Default)]
struct Tally {
    patched: usize,
    fallback: BTreeMap<&'static str, Vec<String>>,
    /// Ops the engine rejects or that change nothing, by op and reason.
    skipped: BTreeMap<String, usize>,
}

#[test]
fn structural_ops_round_trip_on_every_example() {
    let mut tally = Tally::default();
    for (file, src) in examples() {
        let Ok(before) = KdlAdapter.parse(src.as_bytes()) else {
            continue;
        };
        for op in ops_for(&before) {
            let op_name = op.split('"').nth(3).unwrap_or("?").to_owned();
            let tx = Transaction::from_json(&format!(r#"{{"ops":[{op}]}}"#)).expect("tx");
            let Ok(result) = run_transaction(&before, &tx) else {
                *tally
                    .skipped
                    .entry(format!("{op_name}: run error"))
                    .or_default() += 1;
                continue;
            };
            if result.status == TxStatus::Rejected || result.source_before == result.source_after {
                let why = result
                    .diagnostics
                    .iter()
                    .find(|d| d.severity == Severity::Error)
                    .map_or("no change", |d| d.code.as_str());
                *tally
                    .skipped
                    .entry(format!("{op_name}: {why}"))
                    .or_default() += 1;
                continue;
            }
            let after = &result.document_after;
            let out = patch_source(&src, &before, after).expect("patch_source");
            let reparsed = KdlAdapter
                .parse(out.text.as_bytes())
                .unwrap_or_else(|e| panic!("{file}: {op}: {}\n{}", e.message, out.text));
            assert_eq!(
                strip_spans(reparsed),
                strip_spans(after.clone()),
                "{file}: {op}: the patch must parse to the after document"
            );
            match try_patch_source(&src, &before, after) {
                Ok(_) => {
                    assert!(!out.reformatted, "{file}: {op}");
                    tally.patched += 1;
                }
                Err(e) => {
                    assert!(out.reformatted, "{file}: {op}");
                    assert!(
                        matches!(
                            e.code,
                            PatchErrorCode::UnsupportedLayout | PatchErrorCode::UnalignedSource
                        ),
                        "{file}: {op}: unexpected fallback {e}"
                    );
                    tally
                        .fallback
                        .entry(e.code.as_str())
                        .or_default()
                        .push(format!("{file}: {op}: {}", e.message));
                }
            }
        }
    }
    let fallbacks: usize = tally.fallback.values().map(Vec::len).sum();
    let skipped: usize = tally.skipped.values().sum();
    eprintln!(
        "patch round trip: {} patched in place, {fallbacks} fell back, {skipped} skipped (rejected or no-op)",
        tally.patched
    );
    for (why, n) in &tally.skipped {
        eprintln!("  skipped {why}: {n}");
    }
    for (code, cases) in &tally.fallback {
        eprintln!("  {code}: {}", cases.len());
        for case in cases {
            eprintln!("    {case}");
        }
    }
    assert!(tally.patched > 0);
}
