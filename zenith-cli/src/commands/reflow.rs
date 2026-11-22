//! Automatic overflow pagination (`zenith reflow`): spawn continuation pages
//! for overflowing `chain="..."` text flows until all chained copy fits.
//!
//! Pure over in-memory source (reads external `src=` text files and project
//! fonts read-only through `zenith-pipeline`'s native host; never writes disk).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::Serialize;
use zenith_core::fix::unified_diff_with_context;
use zenith_core::{Document, KdlAdapter, KdlSource, Node};
use zenith_pipeline::assets::{build_font_provider, resolve_text_sources};
use zenith_scene::{DocumentPrep, PageCompiler};
use zenith_tx::{Op, Transaction, TxStatus, run_transaction};

use crate::native;

/// Hard upper bound on spawned pages per reflow invocation.
pub const MAX_REFLOW_PAGES_CAP: usize = 1000;

/// Result of running [`run_reflow`].
#[derive(Debug, Clone)]
pub struct ReflowReport {
    /// Number of pages in the input document.
    pub pages_before: usize,
    /// Number of pages in the output document.
    pub pages_after: usize,
    /// Ids of continuation pages spawned in order.
    pub spawned_pages: Vec<String>,
    /// True if at least one chain still overflows after reaching `max_pages` or
    /// encountering a box that cannot accept additional text.
    pub still_overflowing: bool,
    /// Canonical `.zen` source after reflow (equal to input when no pages spawned).
    pub source_after: String,
    /// Unified source diff between input and output.
    pub diff: String,
}

/// JSON envelope for `zenith reflow --json`.
#[derive(Serialize)]
pub struct ReflowJson {
    pub schema: &'static str,
    pub path: String,
    pub applied: bool,
    pub pages_before: usize,
    pub pages_after: usize,
    pub pages_added: usize,
    pub spawned_pages: Vec<String>,
    pub still_overflowing: bool,
    pub diff: String,
}

/// Error returned when `run_reflow` cannot parse or compile the input document.
#[derive(Debug)]
pub struct ReflowError {
    pub code: &'static str,
    pub message: String,
}

impl ReflowError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// Spawn continuation pages for every overflowing `chain="..."` text flow in
/// `doc_src` until all chains fit or `max_pages` new pages have been added.
///
/// Each continuation page duplicates the page owning the chain's last member
/// (preserving `master`, page size, bleed, margins, and layout structure) and
/// clears `spans` and `src` on the duplicated chain text node so it receives
/// only the overflow tail.
pub fn run_reflow(
    doc_src: &str,
    project_dir: Option<&Path>,
    max_pages: usize,
) -> Result<ReflowReport, ReflowError> {
    if max_pages == 0 || max_pages > MAX_REFLOW_PAGES_CAP {
        return Err(ReflowError::new(
            "reflow.invalid_max_pages",
            format!("--max-pages must be in 1..={MAX_REFLOW_PAGES_CAP}, got {max_pages}"),
        ));
    }

    let mut authored = KdlAdapter.parse(doc_src.as_bytes()).map_err(|e| {
        ReflowError::new("reflow.parse", crate::report::parse_error_line(doc_src, &e))
    })?;
    let pages_before = authored.body.pages.len();

    let fonts = build_font_provider(native::host(), &authored, project_dir, false)
        .map_err(|e| ReflowError::new("reflow.fonts", e.message))?;

    let mut spawned_pages = Vec::new();
    let mut still_overflowing = false;

    for _ in 0..=max_pages {
        let tails = chain_tails(&authored);
        if tails.is_empty() {
            break;
        }

        // Resolve external `src=` text files on a compile-only clone so the
        // authored document preserves its `src="..."` references.
        let mut compile_doc = authored.clone();
        let mut resolve_diags = Vec::new();
        resolve_text_sources(
            native::host().fs,
            &mut compile_doc,
            project_dir,
            &mut resolve_diags,
        );
        let prep = DocumentPrep::new(&compile_doc, None, None);
        let compiler = PageCompiler::new(&prep, &fonts);

        // Compile only the pages owning chain tails and find the first overflowing chain.
        let mut overflowing: Option<(String, usize, String)> = None;
        let tail_pages: BTreeSet<usize> = tails.values().map(|(pi, _)| *pi).collect();
        let mut overflow_subjects: BTreeSet<String> = BTreeSet::new();
        for page_idx in tail_pages {
            let compiled = compiler.compile_page_local(page_idx);
            for diag in compiled.diagnostics {
                if matches!(diag.code.as_str(), "text.overflow" | "text.fit_failed")
                    && let Some(subj) = diag.subject_id
                {
                    overflow_subjects.insert(subj);
                }
            }
        }
        for (chain_id, (page_idx, node_id)) in &tails {
            if overflow_subjects.contains(node_id) {
                overflowing = Some((chain_id.clone(), *page_idx, node_id.clone()));
                break;
            }
        }

        let Some((chain_id, last_page_idx, _last_node_id)) = overflowing else {
            still_overflowing = false;
            break;
        };

        if spawned_pages.len() >= max_pages {
            still_overflowing = true;
            break;
        }

        let Some(source_page_id) = authored.body.pages.get(last_page_idx).map(|p| p.id.clone())
        else {
            still_overflowing = true;
            break;
        };

        let (new_page_id, suffix) = next_page_id_and_suffix(&authored, &source_page_id);
        let tx = Transaction {
            permissions: zenith_tx::Permissions::default(),
            ops: vec![Op::DuplicatePage {
                page: source_page_id,
                new_id: new_page_id.clone(),
                id_suffix: suffix,
            }],
        };
        let tx_res = run_transaction(&authored, &tx)
            .map_err(|e| ReflowError::new("reflow.tx_failed", e.message))?;
        if tx_res.status != TxStatus::Accepted {
            let msg = tx_res
                .diagnostics
                .iter()
                .map(|d| d.message.as_str())
                .collect::<Vec<_>>()
                .join("; ");
            return Err(ReflowError::new(
                "reflow.tx_rejected",
                format!("could not spawn continuation page '{new_page_id}': {msg}"),
            ));
        }

        authored = KdlAdapter
            .parse(tx_res.source_after.as_bytes())
            .map_err(|e| ReflowError::new("reflow.reparse", format!("reparse failed: {e}")))?;

        // Clear spans and src on the duplicated chain text node so the new page
        // acts purely as a continuation slot receiving the overflow tail.
        if let Some(new_page) = authored.body.pages.iter_mut().find(|p| p.id == new_page_id) {
            clear_chain_source(&mut new_page.children, &chain_id);
        }
        spawned_pages.push(new_page_id);
    }

    let pages_after = authored.body.pages.len();
    let source_after = if spawned_pages.is_empty() {
        doc_src.to_owned()
    } else {
        let formatted = KdlAdapter
            .format(&authored)
            .map_err(|e| ReflowError::new("reflow.format", format!("format failed: {e}")))?;
        String::from_utf8(formatted)
            .map_err(|e| ReflowError::new("reflow.utf8", format!("invalid utf8: {e}")))?
    };
    let diff = if source_after == doc_src {
        String::new()
    } else {
        unified_diff_with_context("document", doc_src, &source_after, 3)
    };

    Ok(ReflowReport {
        pages_before,
        pages_after,
        spawned_pages,
        still_overflowing,
        source_after,
        diff,
    })
}

/// Map each `chain` id to the `(page_index, node_id)` of its LAST member in
/// document order (page order, then depth-first child order).
fn chain_tails(doc: &Document) -> BTreeMap<String, (usize, String)> {
    let mut tails = BTreeMap::new();
    for (page_idx, page) in doc.body.pages.iter().enumerate() {
        collect_chain_tails(&page.children, page_idx, &mut tails);
    }
    tails
}

fn collect_chain_tails(
    nodes: &[Node],
    page_idx: usize,
    tails: &mut BTreeMap<String, (usize, String)>,
) {
    for node in nodes {
        match node {
            Node::Text(t) => {
                if let Some(chain) = &t.chain {
                    tails.insert(chain.clone(), (page_idx, t.id.clone()));
                }
            }
            Node::Frame(f) => collect_chain_tails(&f.children, page_idx, tails),
            Node::Group(g) => collect_chain_tails(&g.children, page_idx, tails),
            Node::Unknown(u) => collect_chain_tails(&u.children, page_idx, tails),
            Node::Table(t) => {
                for cell in t.rows.iter().flat_map(|r| r.cells.iter()) {
                    collect_chain_tails(&cell.children, page_idx, tails);
                }
            }
            Node::Rect(_)
            | Node::Ellipse(_)
            | Node::Line(_)
            | Node::Code(_)
            | Node::Image(_)
            | Node::Polygon(_)
            | Node::Polyline(_)
            | Node::Path(_)
            | Node::Instance(_)
            | Node::Field(_)
            | Node::Footnote(_)
            | Node::Toc(_)
            | Node::Shape(_)
            | Node::Connector(_)
            | Node::Pattern(_)
            | Node::Chart(_)
            | Node::Light(_)
            | Node::Mesh(_) => {}
        }
    }
}

/// Clear `spans` and `src` on every `Node::Text` belonging to `chain_id` inside
/// a newly duplicated continuation page.
fn clear_chain_source(nodes: &mut [Node], chain_id: &str) {
    for node in nodes {
        match node {
            Node::Text(t) => {
                if t.chain.as_deref() == Some(chain_id) {
                    t.spans.clear();
                    t.src = None;
                }
            }
            Node::Frame(f) => clear_chain_source(&mut f.children, chain_id),
            Node::Group(g) => clear_chain_source(&mut g.children, chain_id),
            Node::Unknown(u) => clear_chain_source(&mut u.children, chain_id),
            Node::Table(t) => {
                for cell in t.rows.iter_mut().flat_map(|r| r.cells.iter_mut()) {
                    clear_chain_source(&mut cell.children, chain_id);
                }
            }
            Node::Rect(_)
            | Node::Ellipse(_)
            | Node::Line(_)
            | Node::Code(_)
            | Node::Image(_)
            | Node::Polygon(_)
            | Node::Polyline(_)
            | Node::Path(_)
            | Node::Instance(_)
            | Node::Field(_)
            | Node::Footnote(_)
            | Node::Toc(_)
            | Node::Shape(_)
            | Node::Connector(_)
            | Node::Pattern(_)
            | Node::Chart(_)
            | Node::Light(_)
            | Node::Mesh(_) => {}
        }
    }
}

/// Choose a deterministic, collision-free continuation page id and node suffix.
///
/// For `page.1` -> `("page.2", ".2")`, then `("page.3", ".3")`, etc.
fn next_page_id_and_suffix(doc: &Document, source_page_id: &str) -> (String, String) {
    let existing_pages: BTreeSet<&str> = doc.body.pages.iter().map(|p| p.id.as_str()).collect();
    let base = match source_page_id.rsplit_once('.') {
        Some((prefix, tail)) if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) => {
            prefix
        }
        _ => source_page_id,
    };
    let mut n: usize = 2;
    loop {
        let candidate_page = format!("{base}.{n}");
        if !existing_pages.contains(candidate_page.as_str()) {
            return (candidate_page, format!(".{n}"));
        }
        n += 1;
    }
}
