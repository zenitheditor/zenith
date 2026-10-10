//! Document-level inspect logic for `zenith inspect`.
//!
//! The public entry point [`run`] takes the source text and the document's
//! directory; the directory locates project fonts, text sources, imports,
//! and image assets for the resolved boxes, as on render.
//!
//! The tree-building pass is decoupled from printing so it can be tested
//! directly: [`build_doc_tree`] / [`find_node_tree`] return [`PageEntry`] /
//! [`NodeEntry`] values that serialise to JSON and render to human-readable
//! format.

use std::collections::BTreeMap;
use std::path::Path;

use zenith_core::{Document, KdlAdapter, KdlSource, ResolvedToken, resolve_tokens};

use crate::commands::serialize_pretty;
use crate::json_types::RecipeInspectJson;

use super::boxes::{BoxInfo, NodeBox, attach_boxes, resolved_boxes};
use super::recipes;
use super::tree::{build_doc_tree, find_node_tree};

// ── Error type ────────────────────────────────────────────────────────────────

/// Error produced by the inspect command.
#[derive(Debug)]
pub struct InspectCmdErr {
    /// Human-readable message.
    pub message: String,
    /// Recommended exit code.
    pub exit_code: u8,
}

impl InspectCmdErr {
    fn new(msg: impl Into<String>, exit_code: u8) -> Self {
        Self {
            message: msg.into(),
            exit_code,
        }
    }
}

// ── Tree representation ───────────────────────────────────────────────────────

/// The geometry summary emitted per node.  Missing fields are `None` when the
/// node kind does not carry that property (e.g. `polygon` has no bbox).
#[derive(Debug, Clone, serde::Serialize)]
pub struct NodeGeometry {
    /// Left edge (px) for bbox nodes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    /// Top edge (px) for bbox nodes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
    /// Width (px) for bbox nodes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub w: Option<f64>,
    /// Height (px) for bbox nodes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h: Option<f64>,
    /// First endpoint x (px) for `line`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x1: Option<f64>,
    /// First endpoint y (px) for `line`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y1: Option<f64>,
    /// Second endpoint x (px) for `line`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x2: Option<f64>,
    /// Second endpoint y (px) for `line`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y2: Option<f64>,
    /// Point count for `polygon`/`polyline`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub point_count: Option<usize>,
}

/// A single node in the inspect tree.
#[derive(Debug, Clone, serde::Serialize)]
pub struct NodeEntry {
    pub id: String,
    pub kind: String,
    /// The node's `role` attribute, when authored. Surfacing it lets consumers
    /// group same-role nodes (e.g. every `role="heading"`) and reason about
    /// cross-page consistency without re-parsing the source.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// The authored geometry, resolved to px.
    pub geometry: Option<NodeGeometry>,
    /// The final geometry the page compile used: `box` (unrotated; layout
    /// and anchors resolved, text height measured, line / path / connector
    /// boxes from their stroked bounds), `rotate`, and `bounds` (what the
    /// node paints, when it differs from `box`). Absent for nodes the
    /// compile skips (guides, nodes that fail to compile).
    #[serde(flatten)]
    pub resolved: Option<BoxInfo>,
    pub visible: Option<bool>,
    pub locked: Option<bool>,
    pub children: Vec<NodeEntry>,
}

/// The resolved token table used to turn `(token)"id"` dimension refs into px
/// values. Built once per `inspect` run from the document's `tokens` block.
pub(super) type Resolved = BTreeMap<String, ResolvedToken>;

/// A page in the inspect tree.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PageEntry {
    pub id: String,
    pub name: Option<String>,
    pub width: f64,
    pub height: f64,
    pub children: Vec<NodeEntry>,
    /// Final boxes of compiled nodes that are not in `children`: master
    /// projections (`<page-id>/<id>`) and instance content
    /// (`<instance-id>/<id>`), by expanded id.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub expanded: BTreeMap<String, BoxInfo>,
}

/// The top-level JSON envelope for `inspect`.
#[derive(Debug, serde::Serialize)]
pub struct InspectOutput {
    pub schema: &'static str,
    pub pages: Vec<PageEntry>,
    /// Empty when the document has no `recipes` block.
    pub recipes: Vec<RecipeInspectJson>,
}

/// The subtree rooted at a single found node (used for `--node <ID>`).
#[derive(Debug, serde::Serialize)]
pub struct InspectNodeOutput {
    pub schema: &'static str,
    pub node: NodeEntry,
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Run `zenith inspect`.
///
/// - `src`      — raw `.zen` source text.
/// - `node_id`  — when `Some`, restrict output to the subtree rooted at that id.
/// - `json`     — emit JSON instead of the human-readable tree.
/// - `project_dir` — the document's directory, or `None` for bundled fonts
///   and no project assets.
///
/// Returns a formatted string on success, or an [`InspectCmdErr`] on parse
/// error, not-found error, etc.
pub fn run(
    src: &str,
    node_id: Option<&str>,
    json: bool,
    project_dir: Option<&Path>,
) -> Result<String, InspectCmdErr> {
    // Parse ─────────────────────────────────────────────────────────────────
    let doc = parse(src)?;
    let resolved = resolve_tokens(&doc.tokens).resolved;

    if let Some(id) = node_id {
        // --node <ID>: find the subtree rooted at that node.
        let entry = node_tree(&doc, id, &resolved, project_dir)?;

        let out = if json {
            let output = InspectNodeOutput {
                schema: "zenith-inspect-v1",
                node: entry,
            };
            serialize_pretty(&output)
        } else {
            render_node_human(&entry, 0).trim_end().to_owned()
        };
        Ok(out)
    } else {
        // Whole document.
        let pages = page_trees(&doc, &resolved, project_dir);

        let out = if json {
            let recipe_entries = recipes::build_recipe_entries(&doc.recipes);
            let output = InspectOutput {
                schema: "zenith-inspect-v1",
                pages,
                recipes: recipe_entries,
            };
            serialize_pretty(&output)
        } else {
            let mut text = render_pages_human(&pages);
            let recipe_section = recipes::render_recipes_human(&doc.recipes);
            if !recipe_section.is_empty() {
                text.push('\n');
                text.push('\n');
                text.push_str(&recipe_section);
            }
            text
        };
        Ok(out)
    }
}

// ── Token-efficient summary (MCP) ───────────────────────────────────────────────

/// Build a token-minimal structured summary of a document's node tree.
///
/// This is the shape the MCP `zenith_inspect` tool returns: instead of the full
/// recursive tree with geometry on every node, it returns a *shallow* view.
///
/// - `node`   — when `Some`, summarise only the subtree rooted at that id.
/// - `depth`  — how many node levels below each page (or below `node`) to expand.
///   Deeper children collapse to a `childCount`. `0` shows only the top level.
/// - `detail` — when `true`, re-include `geometry`/`box`/`visible`/`locked`
///   per node.
/// - `project_dir` — the document's directory (see [`run`]).
///
/// Returns a [`serde_json::Value`] ready to embed as the tool's structured
/// result; the caller decides inline-vs-offload by serialized size.
pub fn summary(
    src: &str,
    node: Option<&str>,
    depth: usize,
    detail: bool,
    project_dir: Option<&Path>,
) -> Result<serde_json::Value, InspectCmdErr> {
    let doc = parse(src)?;
    let resolved = resolve_tokens(&doc.tokens).resolved;

    if let Some(id) = node {
        let entry = node_tree(&doc, id, &resolved, project_dir)?;
        Ok(serde_json::json!({
            "schema": "zenith-inspect-summary-v1",
            "node": trim_node(&entry, depth, detail),
        }))
    } else {
        let pages = page_trees(&doc, &resolved, project_dir);
        let page_values: Vec<serde_json::Value> =
            pages.iter().map(|p| trim_page(p, depth, detail)).collect();
        Ok(serde_json::json!({
            "schema": "zenith-inspect-summary-v1",
            "pages": page_values,
            "recipe_count": doc.recipes.len(),
        }))
    }
}

/// Parse `src`, mapping a parse error to `parse.error` (exit code 2).
fn parse(src: &str) -> Result<Document, InspectCmdErr> {
    KdlAdapter
        .parse(src.as_bytes())
        .map_err(|e| InspectCmdErr::new(crate::report::parse_error_line(src, &e), 2))
}

/// Every page tree with resolved boxes attached.
fn page_trees(doc: &Document, resolved: &Resolved, project_dir: Option<&Path>) -> Vec<PageEntry> {
    let mut pages = build_doc_tree(&doc.body.pages, resolved);
    for (page, boxes) in pages.iter_mut().zip(resolved_boxes(doc, project_dir)) {
        let mut unused = boxes.clone();
        for entry in &mut page.children {
            attach_boxes(entry, &boxes, &mut unused);
        }
        page.expanded = unused;
    }
    pages
}

/// The subtree rooted at node `id` with resolved boxes attached.
fn node_tree(
    doc: &Document,
    id: &str,
    resolved: &Resolved,
    project_dir: Option<&Path>,
) -> Result<NodeEntry, InspectCmdErr> {
    let mut entry =
        find_node_tree(&doc.body.pages, id, resolved).ok_or_else(|| node_not_found(id))?;
    let mut boxes: BTreeMap<String, BoxInfo> = BTreeMap::new();
    for page in resolved_boxes(doc, project_dir) {
        for (id, b) in page {
            boxes.entry(id).or_insert(b);
        }
    }
    attach_boxes(&mut entry, &boxes, &mut BTreeMap::new());
    Ok(entry)
}

/// Trim a [`PageEntry`] to the shallow summary shape.
fn trim_page(p: &PageEntry, depth: usize, detail: bool) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    obj.insert("id".into(), p.id.clone().into());
    if let Some(name) = &p.name {
        obj.insert("name".into(), name.clone().into());
    }
    obj.insert("width".into(), p.width.into());
    obj.insert("height".into(), p.height.into());
    insert_children(&mut obj, &p.children, depth, detail);
    if detail && !p.expanded.is_empty() {
        obj.insert(
            "expanded".into(),
            serde_json::to_value(&p.expanded).unwrap_or(serde_json::Value::Null),
        );
    }
    serde_json::Value::Object(obj)
}

/// Trim a [`NodeEntry`] to the shallow summary shape, recursing `depth` levels.
fn trim_node(n: &NodeEntry, depth: usize, detail: bool) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    obj.insert("id".into(), n.id.clone().into());
    obj.insert("kind".into(), n.kind.clone().into());
    if detail {
        if let Some(role) = &n.role {
            obj.insert("role".into(), role.clone().into());
        }
        if let Some(g) = &n.geometry {
            obj.insert(
                "geometry".into(),
                serde_json::to_value(g).unwrap_or(serde_json::Value::Null),
            );
        }
        if let Some(serde_json::Value::Object(fields)) = n
            .resolved
            .as_ref()
            .map(serde_json::to_value)
            .and_then(Result::ok)
        {
            obj.extend(fields);
        }
        if let Some(v) = n.visible {
            obj.insert("visible".into(), v.into());
        }
        if let Some(l) = n.locked {
            obj.insert("locked".into(), l.into());
        }
    }
    insert_children(&mut obj, &n.children, depth, detail);
    serde_json::Value::Object(obj)
}

/// Insert either an expanded `children` array (when `depth > 0`) or a collapsed
/// `child_count` (when `depth == 0`), omitting both when there are no children.
fn insert_children(
    obj: &mut serde_json::Map<String, serde_json::Value>,
    children: &[NodeEntry],
    depth: usize,
    detail: bool,
) {
    if children.is_empty() {
        return;
    }
    if depth == 0 {
        obj.insert("child_count".into(), children.len().into());
    } else {
        let kids: Vec<serde_json::Value> = children
            .iter()
            .map(|c| trim_node(c, depth - 1, detail))
            .collect();
        obj.insert("children".into(), serde_json::Value::Array(kids));
    }
}

// ── Human rendering ───────────────────────────────────────────────────────────

fn render_pages_human(pages: &[PageEntry]) -> String {
    let mut out = String::new();
    for page in pages {
        let name_part = page
            .name
            .as_deref()
            .map(|n| format!(" \"{}\"", n))
            .unwrap_or_default();
        out.push_str(&format!(
            "page {}{} ({}x{})\n",
            page.id, name_part, page.width, page.height
        ));
        for child in &page.children {
            out.push_str(&render_node_human(child, 1));
        }
    }
    out.trim_end().to_owned()
}

/// Render a single node (and its subtree) at the given indent depth.
/// Called by both the whole-document path and the `--node` subtree path.
fn render_node_human(node: &NodeEntry, depth: usize) -> String {
    let indent = "  ".repeat(depth);
    let geom = render_geom_summary(node);
    let resolved = render_box_summary(node);
    let flags = render_flags(node);
    let suffix = [geom, resolved, flags]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let suffix_part = if suffix.is_empty() {
        String::new()
    } else {
        format!("  {}", suffix)
    };

    let mut out = format!("{}{} {}{}\n", indent, node.kind, node.id, suffix_part);
    for child in &node.children {
        out.push_str(&render_node_human(child, depth + 1));
    }
    out
}

fn render_geom_summary(node: &NodeEntry) -> String {
    let Some(ref g) = node.geometry else {
        return String::new();
    };

    // bbox summary: x,y WxH
    if g.x.is_some() || g.y.is_some() || g.w.is_some() || g.h.is_some() {
        let x = g.x.unwrap_or(0.0);
        let y = g.y.unwrap_or(0.0);
        let w = g.w.unwrap_or(0.0);
        let h = g.h.unwrap_or(0.0);
        return format!(
            "{},{} {}x{}",
            fmt_f64(x),
            fmt_f64(y),
            fmt_f64(w),
            fmt_f64(h)
        );
    }

    // line endpoint summary
    if g.x1.is_some() || g.y1.is_some() || g.x2.is_some() || g.y2.is_some() {
        let x1 = g.x1.unwrap_or(0.0);
        let y1 = g.y1.unwrap_or(0.0);
        let x2 = g.x2.unwrap_or(0.0);
        let y2 = g.y2.unwrap_or(0.0);
        return format!(
            "({},{})→({},{})",
            fmt_f64(x1),
            fmt_f64(y1),
            fmt_f64(x2),
            fmt_f64(y2)
        );
    }

    // poly point count
    if let Some(count) = g.point_count {
        return format!("{} pts", count);
    }

    String::new()
}

/// `box=x,y WxH` when the resolved box differs from the authored x/y/w/h
/// (a laid-out or group-translated node); empty otherwise.
fn render_box_summary(node: &NodeEntry) -> String {
    let Some(info) = node.resolved else {
        return String::new();
    };
    let rect = |label: &str, b: NodeBox| {
        format!(
            "{label}={},{} {}x{}",
            fmt_f64(b.x),
            fmt_f64(b.y),
            fmt_f64(b.w),
            fmt_f64(b.h)
        )
    };
    let b = info.rect;
    let authored = node.geometry.as_ref().map(|g| (g.x, g.y, g.w, g.h));
    let mut parts: Vec<String> = Vec::new();
    if authored != Some((Some(b.x), Some(b.y), Some(b.w), Some(b.h))) {
        parts.push(rect("box", b));
    }
    if let Some(deg) = info.rotate {
        parts.push(format!("rot={}", fmt_f64(deg)));
    }
    if let Some(v) = info.bounds {
        parts.push(rect("bounds", v));
    }
    parts.join(" ")
}

fn render_flags(node: &NodeEntry) -> String {
    let mut flags = Vec::new();
    if node.visible == Some(false) {
        flags.push("[hidden]");
    }
    if node.locked == Some(true) {
        flags.push("[locked]");
    }
    flags.join(" ")
}

/// Format an `f64` without a trailing `.0` when the value is whole.
fn fmt_f64(v: f64) -> String {
    zenith_core::format_number(v)
}

/// An `inspect.node_not_found` error for `id` (exit code 2).
fn node_not_found(id: &str) -> InspectCmdErr {
    InspectCmdErr::new(
        format!(
            "error[inspect.node_not_found]: node '{id}' not found; run `zenith inspect <FILE>` \
             to list node ids"
        ),
        2,
    )
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "document_tests.rs"]
mod tests;
