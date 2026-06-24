//! Pure logic for `zenith library list`, `zenith library show`, and `zenith library add`.
//!
//! The registry/resolver lives in [`crate::library`]; this module turns a
//! resolved set of packs into stdout text ([`list`]), inspects individual items
//! ([`show`]), and materializes library items into target documents ([`add`]).
//! None of these functions touch the filesystem — the dispatcher reads/writes
//! files and calls [`crate::library::resolve_packs`].

use std::path::Path;

use zenith_core::{KdlAdapter, KdlSource, Severity, TokenLiteral, TokenType, TokenValue, validate};
use zenith_tx::{Transaction, TxStatus};

use crate::commands::serialize_pretty;
use crate::library::{ItemKind, LibraryPack, load_pack_document, parse_spec, resolve_packs};

/// JSON shape for `library list --json`.
#[derive(Debug, serde::Serialize)]
struct LibraryListOutput<'a> {
    schema: &'static str,
    packs: Vec<PackJson<'a>>,
}

/// A single pack entry in the `--json` output.
#[derive(Debug, serde::Serialize)]
struct PackJson<'a> {
    id: &'a str,
    version: Option<&'a str>,
    source: &'static str,
    items: Vec<PackItemJson<'a>>,
}

/// A single exported item in the `--json` output: its id and kind.
#[derive(Debug, serde::Serialize)]
struct PackItemJson<'a> {
    id: &'a str,
    kind: &'static str,
}

/// Render the resolved `packs` for `library list`.
///
/// Packs are expected pre-sorted by id (see [`crate::library::resolve_packs`]);
/// item order is preserved from the pack's component order.
///
/// - Human (default): one header line per pack
///   (`<id>  <version>  [preset|project]`) followed by indented `#<item>` lines.
/// - `--json`: a `{"schema":"zenith-library-v1","packs":[…]}` document.
pub fn list(packs: &[LibraryPack], json: bool) -> String {
    if json {
        let out = LibraryListOutput {
            schema: "zenith-library-v1",
            packs: packs
                .iter()
                .map(|p| PackJson {
                    id: &p.id,
                    version: p.version.as_deref(),
                    source: p.source.label(),
                    items: p
                        .items
                        .iter()
                        .map(|it| PackItemJson {
                            id: it.id.as_str(),
                            kind: it.kind.label(),
                        })
                        .collect(),
                })
                .collect(),
        };
        serialize_pretty(&out)
    } else {
        format_human(packs)
    }
}

/// Human-readable listing.
fn format_human(packs: &[LibraryPack]) -> String {
    if packs.is_empty() {
        return "no libraries found".to_owned();
    }
    let mut lines = Vec::new();
    for pack in packs {
        let version = pack.version.as_deref().unwrap_or("-");
        lines.push(format!(
            "{}  {}  [{}]",
            pack.id,
            version,
            pack.source.label()
        ));
        for item in &pack.items {
            lines.push(format!("  #{} ({})", item.id, item.kind.label()));
        }
    }
    lines.push(String::new());
    lines.push(
        "Run `zenith library show <package>#<item>` to inspect any item in detail.".to_owned(),
    );
    lines.join("\n")
}

// ── `library show` ────────────────────────────────────────────────────────────

/// JSON shape for `library show --json`.
#[derive(Debug, serde::Serialize)]
struct LibraryShowOutput {
    schema: &'static str,
    package: String,
    item: String,
    kind: &'static str,
    detail: ShowDetail,
    to_use: String,
}

/// Kind-specific content for `library show`.
#[derive(Debug, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ShowDetail {
    Token {
        token_type: String,
        /// Human-readable ops / value summary derived from the token literal.
        summary: String,
    },
    Component {
        /// The root node kind (kind of the first child), or "empty" when none.
        root_node_kind: String,
        /// Total direct child count of the component.
        child_count: usize,
        /// Short breakdown of the child node kinds present (sorted, e.g.
        /// `"shape(1)"`).
        node_kinds: String,
    },
    Action {
        /// The op names extracted from the tx JSON (snake_case), in source order.
        ops: Vec<String>,
        /// Human-readable label from the action def, if present.
        label: Option<String>,
    },
}

/// Error produced by the `library show` command.
#[derive(Debug)]
pub struct ShowCmdErr {
    /// Human-readable message.
    pub message: String,
    /// Recommended exit code.
    pub exit_code: u8,
}

impl ShowCmdErr {
    fn new(message: impl Into<String>, exit_code: u8) -> Self {
        Self {
            message: message.into(),
            exit_code,
        }
    }
}

/// Inspect the library item named by `spec` (`<package>#<item>`) and return
/// `(stdout_text, exit_code)`.
///
/// Resolves packs from `project_dir` (embedded presets + project packs), finds
/// the named item, loads the pack's full document to derive content detail, and
/// formats human or JSON output.
///
/// # Errors
///
/// Returns [`ShowCmdErr`] on a malformed spec, unknown package, or unknown item.
pub fn show(spec: &str, project_dir: Option<&Path>, json: bool) -> Result<String, ShowCmdErr> {
    let (pkg_id, item_id) = parse_spec(spec).map_err(|e| ShowCmdErr::new(e.message, 2))?;

    let packs = resolve_packs(project_dir);

    // Find the pack.
    let pack = packs.iter().find(|p| p.id == pkg_id).ok_or_else(|| {
        let mut available: Vec<&str> = packs.iter().map(|p| p.id.as_str()).collect();
        available.sort_unstable();
        available.dedup();
        ShowCmdErr::new(
            format!(
                "unknown library package '{}' (available: {})",
                pkg_id,
                if available.is_empty() {
                    "none".to_owned()
                } else {
                    available.join(", ")
                }
            ),
            2,
        )
    })?;

    // Find the item in the pack's metadata.
    let pack_item = pack
        .items
        .iter()
        .find(|it| it.id == item_id)
        .ok_or_else(|| {
            let available: Vec<&str> = pack.items.iter().map(|it| it.id.as_str()).collect();
            ShowCmdErr::new(
                format!(
                    "unknown item '{}' in package '{}' (available: {})",
                    item_id,
                    pkg_id,
                    if available.is_empty() {
                        "none".to_owned()
                    } else {
                        available.join(", ")
                    }
                ),
                2,
            )
        })?;

    let kind = pack_item.kind;

    // Load the pack's full document to derive content detail.
    let pack_doc = load_pack_document(pack).map_err(|e| ShowCmdErr::new(e.message, 2))?;

    let detail = match kind {
        ItemKind::Token => {
            // Find the token in the pack document.
            let token = pack_doc
                .tokens
                .tokens
                .iter()
                .find(|t| t.id == item_id)
                .ok_or_else(|| {
                    ShowCmdErr::new(
                        format!(
                            "internal error: item '{}' not found in pack document",
                            item_id
                        ),
                        2,
                    )
                })?;

            let token_type = match &token.token_type {
                TokenType::Filter => "filter".to_owned(),
                TokenType::Mask => "mask".to_owned(),
                TokenType::Color => "color".to_owned(),
                TokenType::Dimension => "dimension".to_owned(),
                TokenType::Number => "number".to_owned(),
                TokenType::FontFamily => "fontFamily".to_owned(),
                TokenType::FontWeight => "fontWeight".to_owned(),
                TokenType::Gradient => "gradient".to_owned(),
                TokenType::Shadow => "shadow".to_owned(),
                TokenType::Unknown(s) => s.clone(),
            };

            let summary = match &token.value {
                TokenValue::Reference { token_id } => {
                    format!("alias to {}", token_id)
                }
                TokenValue::Literal(lit) => match lit {
                    TokenLiteral::Filter(lit) => {
                        let ops: Vec<String> = lit
                            .ops
                            .iter()
                            .map(|op| op.kind.as_op_name().to_owned())
                            .collect();
                        format!("ops: {}", ops.join(", "))
                    }
                    TokenLiteral::Mask(lit) => {
                        let parts: Vec<String> = {
                            let mut v = vec![lit.shape.as_shape_name().to_owned()];
                            if lit.feather > 0.0 {
                                v.push(format!("feather={}", lit.feather));
                            }
                            if lit.invert {
                                v.push("invert=true".to_owned());
                            }
                            v
                        };
                        format!("shape: {}", parts.join(", "))
                    }
                    TokenLiteral::String(s) => s.clone(),
                    TokenLiteral::Dimension(d) => {
                        format!("({}){}", d.unit.as_annotation(), d.value)
                    }
                    TokenLiteral::Number(n) => n.to_string(),
                    TokenLiteral::Gradient(g) => {
                        format!("gradient with {} stop(s)", g.stops.len())
                    }
                    TokenLiteral::Shadow(s) => {
                        format!("shadow with {} layer(s)", s.layers.len())
                    }
                },
            };

            ShowDetail::Token {
                token_type,
                summary,
            }
        }

        ItemKind::Component => {
            let comp = pack_doc
                .components
                .iter()
                .find(|c| c.id == item_id)
                .ok_or_else(|| {
                    ShowCmdErr::new(
                        format!(
                            "internal error: component '{}' not found in pack document",
                            item_id
                        ),
                        2,
                    )
                })?;

            let child_count = comp.children.len();
            let root_node_kind = comp
                .children
                .first()
                .map(|n| node_kind_name(n).to_owned())
                .unwrap_or_else(|| "empty".to_owned());

            // Count each node kind among direct children.
            let mut kind_counts: std::collections::BTreeMap<&'static str, usize> =
                std::collections::BTreeMap::new();
            for child in &comp.children {
                *kind_counts.entry(node_kind_name(child)).or_insert(0) += 1;
            }
            let node_kinds: Vec<String> = kind_counts
                .iter()
                .map(|(k, n)| format!("{}({})", k, n))
                .collect();
            let node_kinds = if node_kinds.is_empty() {
                "none".to_owned()
            } else {
                node_kinds.join(", ")
            };

            ShowDetail::Component {
                root_node_kind,
                child_count,
                node_kinds,
            }
        }

        ItemKind::Action => {
            let action_def = pack_doc
                .actions
                .iter()
                .find(|a| a.id == item_id)
                .ok_or_else(|| {
                    ShowCmdErr::new(
                        format!(
                            "internal error: action '{}' not found in pack document",
                            item_id
                        ),
                        2,
                    )
                })?;

            let label = action_def.label.clone();

            // Parse the tx JSON with Transaction::from_json to extract op names.
            let tx = Transaction::from_json(&action_def.tx_json).map_err(|e| {
                ShowCmdErr::new(
                    format!("malformed tx-script in action '{}': {}", item_id, e.message),
                    2,
                )
            })?;

            let ops: Vec<String> = tx.ops.iter().map(op_name).collect();

            ShowDetail::Action { ops, label }
        }
    };

    let to_use = match kind {
        ItemKind::Component => format!(
            "zenith library add {}#{} --into <doc.zen> --page <page-id>",
            pkg_id, item_id
        ),
        ItemKind::Token | ItemKind::Action => {
            format!("zenith library add {}#{} --into <doc.zen>", pkg_id, item_id)
        }
    };

    if json {
        let out = LibraryShowOutput {
            schema: "zenith-library-show-v1",
            package: pkg_id,
            item: item_id,
            kind: kind.label(),
            detail,
            to_use,
        };
        Ok(serialize_pretty(&out))
    } else {
        Ok(format_show_human(&pkg_id, &item_id, kind, &detail, &to_use))
    }
}

/// Format the human-readable output for `library show`.
fn format_show_human(
    pkg_id: &str,
    item_id: &str,
    kind: ItemKind,
    detail: &ShowDetail,
    to_use: &str,
) -> String {
    let mut lines = Vec::new();
    lines.push(format!("package : {}", pkg_id));
    lines.push(format!("item    : {}", item_id));
    lines.push(format!("kind    : {}", kind.label()));
    lines.push(String::new());

    match detail {
        ShowDetail::Token {
            token_type,
            summary,
        } => {
            lines.push(format!("type    : {}", token_type));
            lines.push(format!("content : {}", summary));
        }
        ShowDetail::Component {
            root_node_kind,
            child_count,
            node_kinds,
        } => {
            lines.push(format!("children: {} node(s)", child_count));
            lines.push(format!("root    : {}", root_node_kind));
            lines.push(format!("nodes   : {}", node_kinds));
        }
        ShowDetail::Action { ops, label } => {
            if let Some(lbl) = label {
                lines.push(format!("label   : {}", lbl));
            }
            lines.push(format!(
                "ops     : {}",
                if ops.is_empty() {
                    "(none)".to_owned()
                } else {
                    ops.join(", ")
                }
            ));
        }
    }

    lines.push(String::new());
    lines.push(format!("To use  : {}", to_use));
    lines.join("\n")
}

/// Return the short, stable node-kind name for a [`zenith_core::Node`] variant.
fn node_kind_name(node: &zenith_core::Node) -> &'static str {
    match node {
        zenith_core::Node::Rect(_) => "rect",
        zenith_core::Node::Ellipse(_) => "ellipse",
        zenith_core::Node::Line(_) => "line",
        zenith_core::Node::Text(_) => "text",
        zenith_core::Node::Code(_) => "code",
        zenith_core::Node::Image(_) => "image",
        zenith_core::Node::Polygon(_) => "polygon",
        zenith_core::Node::Polyline(_) => "polyline",
        zenith_core::Node::Frame(_) => "frame",
        zenith_core::Node::Group(_) => "group",
        zenith_core::Node::Instance(_) => "instance",
        zenith_core::Node::Field(_) => "field",
        zenith_core::Node::Toc(_) => "toc",
        zenith_core::Node::Footnote(_) => "footnote",
        zenith_core::Node::Table(_) => "table",
        zenith_core::Node::Shape(_) => "shape",
        zenith_core::Node::Connector(_) => "connector",
        zenith_core::Node::Pattern(_) => "pattern",
        zenith_core::Node::Unknown(_) => "unknown",
    }
}

/// Return the snake_case op name for a [`zenith_tx::Op`] variant.
fn op_name(op: &zenith_tx::Op) -> String {
    match op {
        zenith_tx::Op::SetTextAlign { .. } => "set_text_align",
        zenith_tx::Op::MoveForward { .. } => "move_forward",
        zenith_tx::Op::MoveBackward { .. } => "move_backward",
        zenith_tx::Op::MoveToFront { .. } => "move_to_front",
        zenith_tx::Op::MoveToBack { .. } => "move_to_back",
        zenith_tx::Op::SetVisible { .. } => "set_visible",
        zenith_tx::Op::SetLocked { .. } => "set_locked",
        zenith_tx::Op::SetFill { .. } => "set_fill",
        zenith_tx::Op::SetStroke { .. } => "set_stroke",
        zenith_tx::Op::SetStrokeWidth { .. } => "set_stroke_width",
        zenith_tx::Op::SetOpacity { .. } => "set_opacity",
        zenith_tx::Op::SetGeometry { .. } => "set_geometry",
        zenith_tx::Op::SetPoints { .. } => "set_points",
        zenith_tx::Op::ReplaceText { .. } => "replace_text",
        zenith_tx::Op::DuplicateNode { .. } => "duplicate_node",
        zenith_tx::Op::DuplicatePage { .. } => "duplicate_page",
        zenith_tx::Op::AddNode { .. } => "add_node",
        zenith_tx::Op::RemoveNode { .. } => "remove_node",
        zenith_tx::Op::Group { .. } => "group",
        zenith_tx::Op::Ungroup { .. } => "ungroup",
        zenith_tx::Op::Reparent { .. } => "reparent",
        zenith_tx::Op::AlignNodes { .. } => "align_nodes",
        zenith_tx::Op::SetTextOverflow { .. } => "set_text_overflow",
        zenith_tx::Op::AddPage { .. } => "add_page",
        zenith_tx::Op::DeletePage { .. } => "delete_page",
        zenith_tx::Op::ReorderPages { .. } => "reorder_pages",
        zenith_tx::Op::AddAsset { .. } => "add_asset",
        zenith_tx::Op::SetAsset { .. } => "set_asset",
        zenith_tx::Op::DistributeNodes { .. } => "distribute_nodes",
        zenith_tx::Op::UpdateTokenValue { .. } => "update_token_value",
        zenith_tx::Op::SetStyleProperty { .. } => "set_style_property",
        zenith_tx::Op::SetTextDirection { .. } => "set_text_direction",
        zenith_tx::Op::FindReplaceText { .. } => "find_replace_text",
        zenith_tx::Op::SetPageSize { .. } => "set_page_size",
        zenith_tx::Op::AlignToEdge { .. } => "align_to_edge",
        zenith_tx::Op::CreateToken { .. } => "create_token",
        zenith_tx::Op::CreateRecipe { .. } => "create_recipe",
        zenith_tx::Op::UpdateRecipe { .. } => "update_recipe",
        zenith_tx::Op::DeleteRecipe { .. } => "delete_recipe",
        zenith_tx::Op::DetachPattern { .. } => "detach_pattern",
    }
    .to_owned()
}

// ── `library add` ─────────────────────────────────────────────────────────────

/// Error produced by the `library add` command.
#[derive(Debug)]
pub struct AddCmdErr {
    /// Human-readable message.
    pub message: String,
    /// Recommended exit code.
    pub exit_code: u8,
}

impl AddCmdErr {
    fn new(message: impl Into<String>, exit_code: u8) -> Self {
        Self {
            message: message.into(),
            exit_code,
        }
    }
}

/// The successful outcome of `library add`: the canonical formatted source to
/// write back (or print on `--dry-run`) plus a human-readable summary.
#[derive(Debug)]
pub struct AddResult {
    /// The canonical formatted bytes of the mutated document.
    pub formatted: Vec<u8>,
    /// A multi-line human-readable summary of what was added.
    pub summary: String,
}

/// Materialize the library item named by `spec` into the document `target_src`,
/// returning the formatted result + a summary.
///
/// `project_dir` is the directory whose `libraries/*.zen` packs are resolved
/// alongside the embedded presets (the `--into` file's parent). `at` is the
/// instance origin in pixels; `id_base` overrides the generated instance id base.
///
/// This is pure: it parses, mutates an in-memory [`zenith_core::Document`],
/// VALIDATES the result (hard errors abort with no write), and formats — it never
/// touches the filesystem itself (the dispatcher reads/writes files). Steps mirror
/// [`crate::library::materialize`]: resolve pack → copy component (dedup) → copy
/// dep tokens/styles/assets (dedup) → unique instance id → insert instance →
/// record libraries + provenance → validate → format.
///
/// `page` is required only for COMPONENT items (which materialize as an instance
/// on a page); TOKEN items (filter tokens) ignore it.
///
/// # Errors
///
/// Returns [`AddCmdErr`] on a malformed spec, parse/format failure, unknown
/// package/item, a missing page (for a component item), or a post-mutation
/// validation that has hard errors.
pub fn add(
    target_src: &str,
    spec: &str,
    project_dir: Option<&Path>,
    page: Option<&str>,
    at: (f64, f64),
    id_override: Option<&str>,
) -> Result<AddResult, AddCmdErr> {
    let (pkg_id, item) = parse_spec(spec).map_err(|e| AddCmdErr::new(e.message, 2))?;

    let mut target = KdlAdapter
        .parse(target_src.as_bytes())
        .map_err(|e| AddCmdErr::new(format!("parse error: {}", e.message), 2))?;

    let packs = resolve_packs(project_dir);
    let id_base = id_override.unwrap_or(item.as_str());

    // Determine the item kind from the resolved pack's exported items. An unknown
    // pkg/item falls through to a `materialize*` call, which yields a precise
    // "unknown package/item" diagnostic.
    let item_kind = packs
        .iter()
        .find(|p| p.id == pkg_id)
        .and_then(|p| p.items.iter().find(|it| it.id == item))
        .map(|it| it.kind);

    let summary = match item_kind {
        Some(ItemKind::Action) => {
            let outcome = crate::library::materialize_action(target_src, &packs, &pkg_id, &item)
                .map_err(|e| AddCmdErr::new(e.message, 2))?;

            // Rejected → early-return with the rejection diagnostics; the two
            // accepted variants yield the status label used in the summary.
            let status_label = match outcome.tx_result.status {
                TxStatus::Rejected => {
                    let diag_lines: Vec<String> = outcome
                        .tx_result
                        .diagnostics
                        .iter()
                        .map(crate::commands::format_diagnostic_line)
                        .collect();
                    return Err(AddCmdErr::new(
                        format!(
                            "action '{}#{}' was rejected:\n{}",
                            pkg_id,
                            item,
                            diag_lines.join("\n")
                        ),
                        1,
                    ));
                }
                TxStatus::Accepted => "accepted",
                TxStatus::AcceptedWithWarnings => "accepted-with-warnings",
            };

            let final_source = outcome.final_source.ok_or_else(|| {
                AddCmdErr::new("internal error: accepted action produced no source", 2)
            })?;

            let result_doc = KdlAdapter.parse(final_source.as_bytes()).map_err(|e| {
                AddCmdErr::new(
                    format!(
                        "internal error: could not re-parse action result: {}",
                        e.message
                    ),
                    2,
                )
            })?;

            let formatted = validate_and_format(&result_doc)?;

            let affected = if outcome.tx_result.affected_node_ids.is_empty() {
                "none".to_owned()
            } else {
                outcome.tx_result.affected_node_ids.join(", ")
            };
            let provenance_id = outcome.provenance_id.unwrap_or_default();
            let mut summary = String::new();
            summary.push_str(&format!(
                "applied {}#{} ({})\n",
                outcome.pkg_id, outcome.item, status_label
            ));
            summary.push_str(&format!("  affected: {}\n", affected));
            summary.push_str(&format!("  provenance: {}", provenance_id));
            for w in &outcome.warnings {
                summary.push_str(&format!("\n  warning: {}", w));
            }
            return Ok(AddResult { formatted, summary });
        }
        Some(ItemKind::Token) => {
            // TOKEN item: copy the filter token + color deps; no instance, no page.
            let outcome =
                crate::library::materialize_token(&mut target, &packs, &pkg_id, &item, id_base)
                    .map_err(|e| AddCmdErr::new(e.message, 2))?;
            let deps = if outcome.dep_token_ids.is_empty() {
                "none".to_owned()
            } else {
                outcome.dep_token_ids.join(", ")
            };
            let mut summary = String::new();
            summary.push_str(&format!(
                "added {}#{} as {} token '{}'\n",
                outcome.pkg_id, outcome.item, outcome.apply_property, outcome.token_id
            ));
            summary.push_str(&format!(
                "  apply with: {}=(token)\"{}\"\n",
                outcome.apply_property, outcome.token_id
            ));
            summary.push_str(&format!("  dependencies: {}\n", deps));
            summary.push_str(&format!("  provenance: {}", outcome.provenance_id));
            for w in &outcome.warnings {
                summary.push_str(&format!("\n  warning: {}", w));
            }
            summary
        }
        // COMPONENT item (or unknown). A real component requires `--page`. For an
        // unknown pkg/item (`None`), skip the page requirement and let
        // `materialize` emit the precise "unknown package/item" diagnostic — it
        // checks pkg/item BEFORE page, so an empty page never masks that error.
        Some(ItemKind::Component) | None => {
            let page = match item_kind {
                Some(ItemKind::Component) => page.ok_or_else(|| {
                    AddCmdErr::new(
                        "page is required to add a component item (use --page <id>)",
                        2,
                    )
                })?,
                Some(ItemKind::Token) | Some(ItemKind::Action) | None => page.unwrap_or(""),
            };
            let outcome =
                crate::library::materialize(&mut target, &packs, &pkg_id, &item, page, id_base, at)
                    .map_err(|e| AddCmdErr::new(e.message, 2))?;
            let mut summary = String::new();
            summary.push_str(&format!(
                "added {}#{} as instance '{}' on page '{}'\n",
                outcome.pkg_id, outcome.item, outcome.instance_id, page
            ));
            summary.push_str(&format!("  component: {}\n", outcome.target_component_id));
            summary.push_str(&format!("  provenance: {}", outcome.provenance_id));
            for w in &outcome.warnings {
                summary.push_str(&format!("\n  warning: {}", w));
            }
            summary
        }
    };

    let formatted = validate_and_format(&target)?;
    Ok(AddResult { formatted, summary })
}

/// Validate the mutated `target` (hard errors abort with no write) then format it
/// to canonical bytes. Shared by the component and token `add` branches.
fn validate_and_format(target: &zenith_core::Document) -> Result<Vec<u8>, AddCmdErr> {
    let report = validate(target);
    let errors: Vec<String> = report
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(crate::commands::format_diagnostic_line)
        .collect();
    if !errors.is_empty() {
        return Err(AddCmdErr::new(
            format!(
                "materialized document has {} validation error(s):\n{}",
                errors.len(),
                errors.join("\n")
            ),
            1,
        ));
    }
    KdlAdapter
        .format(target)
        .map_err(|e| AddCmdErr::new(format!("format error: {}", e.message), 2))
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::PackSource;

    // ── `library show` tests ───────────────────────────────────────────────────

    #[test]
    fn show_filter_token_human() {
        let out = show("@zenith/filters#sepia", None, false).expect("show ok");
        assert!(out.contains("package : @zenith/filters"), "pkg: {}", out);
        assert!(out.contains("item    : sepia"), "item: {}", out);
        assert!(out.contains("kind    : token"), "kind: {}", out);
        assert!(out.contains("type    : filter"), "type: {}", out);
        assert!(out.contains("ops: sepia"), "ops: {}", out);
        assert!(out.contains("To use"), "to_use: {}", out);
        assert!(
            out.contains("--into <doc.zen>"),
            "to_use invocation: {}",
            out
        );
    }

    #[test]
    fn show_mask_token_human() {
        let out = show("@zenith/masks#vignette", None, false).expect("show ok");
        assert!(out.contains("kind    : token"), "kind: {}", out);
        assert!(out.contains("type    : mask"), "type: {}", out);
        assert!(out.contains("shape: rounded"), "shape: {}", out);
        assert!(out.contains("invert=true"), "invert: {}", out);
    }

    #[test]
    fn show_component_human() {
        let out = show("@zenith/flowchart#decision", None, false).expect("show ok");
        assert!(out.contains("package : @zenith/flowchart"), "pkg: {}", out);
        assert!(out.contains("item    : decision"), "item: {}", out);
        assert!(out.contains("kind    : component"), "kind: {}", out);
        assert!(out.contains("children:"), "children: {}", out);
        assert!(out.contains("root    : shape"), "root: {}", out);
        // to_use for a component must include --page
        assert!(out.contains("--page <page-id>"), "to_use: {}", out);
    }

    #[test]
    fn show_action_human() {
        let out = show("@zenith/brand-kit#apply-2026", None, false).expect("show ok");
        assert!(out.contains("package : @zenith/brand-kit"), "pkg: {}", out);
        assert!(out.contains("item    : apply-2026"), "item: {}", out);
        assert!(out.contains("kind    : action"), "kind: {}", out);
        assert!(out.contains("update_token_value"), "ops: {}", out);
        assert!(out.contains("To use"), "to_use: {}", out);
        assert!(
            out.contains("--into <doc.zen>"),
            "to_use invocation: {}",
            out
        );
    }

    #[test]
    fn show_filter_token_json() {
        let out = show("@zenith/filters#sepia", None, true).expect("show ok");
        let v: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
        assert_eq!(v["schema"], "zenith-library-show-v1");
        assert_eq!(v["package"], "@zenith/filters");
        assert_eq!(v["item"], "sepia");
        assert_eq!(v["kind"], "token");
        assert_eq!(v["detail"]["token_type"], "filter");
        assert!(
            v["detail"]["summary"]
                .as_str()
                .unwrap_or("")
                .contains("sepia"),
            "filter summary: {}",
            v["detail"]["summary"]
        );
        assert!(
            v["to_use"].as_str().unwrap_or("").contains("--into"),
            "to_use: {}",
            v["to_use"]
        );
    }

    #[test]
    fn show_component_json() {
        let out = show("@zenith/flowchart#decision", None, true).expect("show ok");
        let v: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
        assert_eq!(v["schema"], "zenith-library-show-v1");
        assert_eq!(v["kind"], "component");
        assert_eq!(v["detail"]["root_node_kind"], "shape");
        assert!(
            v["detail"]["child_count"].as_u64().unwrap_or(0) >= 1,
            "child_count"
        );
        assert!(
            v["to_use"].as_str().unwrap_or("").contains("--page"),
            "component to_use needs --page: {}",
            v["to_use"]
        );
    }

    #[test]
    fn show_action_json() {
        let out = show("@zenith/brand-kit#apply-2026", None, true).expect("show ok");
        let v: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
        assert_eq!(v["schema"], "zenith-library-show-v1");
        assert_eq!(v["kind"], "action");
        let ops = v["detail"]["ops"].as_array().expect("ops array");
        assert!(!ops.is_empty(), "ops must not be empty");
        assert!(
            ops.iter().any(|o| o == "update_token_value"),
            "must contain update_token_value; ops: {:?}",
            ops
        );
    }

    #[test]
    fn show_unknown_package_errors() {
        let err = show("@no/such#item", None, false).expect_err("unknown pkg errors");
        assert_eq!(err.exit_code, 2);
        assert!(
            err.message.contains("unknown library package"),
            "{}",
            err.message
        );
        assert!(err.message.contains("@zenith/"), "{}", err.message);
    }

    #[test]
    fn show_unknown_item_errors() {
        let err = show("@zenith/filters#nope", None, false).expect_err("unknown item errors");
        assert_eq!(err.exit_code, 2);
        assert!(err.message.contains("unknown item"), "{}", err.message);
        assert!(err.message.contains("sepia"), "{}", err.message);
    }

    #[test]
    fn show_malformed_spec_errors() {
        let err = show("no-hash", None, false).expect_err("malformed spec errors");
        assert_eq!(err.exit_code, 2);
    }

    // ── `library list` hint ────────────────────────────────────────────────────

    #[test]
    fn list_human_includes_show_hint() {
        let packs = resolve_packs(None);
        let out = list(&packs, false);
        assert!(
            out.contains("zenith library show"),
            "list output must mention show: {}",
            out
        );
    }

    // ── `library list` tests ───────────────────────────────────────────────────

    #[test]
    fn human_lists_flowchart_with_items() {
        let packs = resolve_packs(None);
        let out = list(&packs, false);
        assert!(out.contains("@zenith/flowchart"), "got: {}", out);
        assert!(out.contains("[preset]"), "got: {}", out);
        assert!(out.contains("#process (component)"), "got: {}", out);
        assert!(out.contains("#decision (component)"), "got: {}", out);
        assert!(out.contains("#terminator (component)"), "got: {}", out);
    }

    #[test]
    fn human_lists_filters_with_token_items() {
        let packs = resolve_packs(None);
        let out = list(&packs, false);
        assert!(out.contains("@zenith/filters"), "got: {}", out);
        assert!(out.contains("#noir (token)"), "got: {}", out);
    }

    #[test]
    fn json_is_parseable_and_contains_flowchart() {
        let packs = resolve_packs(None);
        let out = list(&packs, true);
        let value: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
        assert_eq!(value["schema"], "zenith-library-v1");
        let packs_json = value["packs"].as_array().expect("packs array");
        let flow = packs_json
            .iter()
            .find(|p| p["id"] == "@zenith/flowchart")
            .expect("flowchart pack present");
        assert_eq!(flow["version"], "1.0.0");
        assert_eq!(flow["source"], "preset");
        let items = flow["items"].as_array().expect("items array");
        let ids: Vec<&str> = items.iter().filter_map(|v| v["id"].as_str()).collect();
        assert_eq!(ids, vec!["process", "decision", "terminator"]);
        assert!(
            items.iter().all(|v| v["kind"] == "component"),
            "all flowchart items are components"
        );
    }

    #[test]
    fn empty_packs_human_message() {
        let out = list(&[], false);
        assert_eq!(out, "no libraries found");
    }

    #[test]
    fn version_falls_back_to_dash() {
        let pack = LibraryPack {
            id: "@x/y".to_owned(),
            version: None,
            source: PackSource::Preset,
            items: vec![],
        };
        let out = list(std::slice::from_ref(&pack), false);
        assert!(out.contains("@x/y  -  [preset]"), "got: {}", out);
    }

    // ── `add` command tests ────────────────────────────────────────────────────

    const TARGET_SRC: &str = r#"zenith version=1 {
  project id="proj.x" name="Target"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="d" title="x" {
    page id="pg" w=(px)800 h=(px)600 {}
  }
}
"#;

    #[test]
    fn add_produces_formatted_doc_that_round_trips_and_compiles() {
        let result = add(
            TARGET_SRC,
            "@zenith/flowchart#decision",
            None,
            Some("pg"),
            (120.0, 80.0),
            None,
        )
        .expect("add ok");

        // Result is valid UTF-8 KDL that reparses + validates clean.
        let src = String::from_utf8(result.formatted).expect("utf8");
        let doc = KdlAdapter.parse(src.as_bytes()).expect("reparse");
        let errors: Vec<_> = validate(&doc)
            .diagnostics
            .into_iter()
            .filter(|d| d.severity == Severity::Error)
            .collect();
        assert!(errors.is_empty(), "errors: {:?}", errors);

        // Summary mentions the instance + component + provenance.
        assert!(
            result.summary.contains("decision"),
            "summary: {}",
            result.summary
        );
        assert!(
            result.summary.contains("lib.zenith.flowchart.decision"),
            "summary: {}",
            result.summary
        );

        // Smoke: the document compiles to a non-empty scene (instance expands to
        // the shape) when rendered to a scene JSON.
        let artifact = crate::commands::render::to_scene_json(&src, None, 1).expect("compile ok");
        let scene: serde_json::Value =
            serde_json::from_str(&artifact.json).expect("scene json parses");
        let commands = scene["commands"].as_array().expect("commands array");
        assert!(
            !commands.is_empty(),
            "instance must expand to at least one scene command"
        );
    }

    #[test]
    fn add_malformed_spec_errors() {
        let err = add(TARGET_SRC, "no-hash", None, Some("pg"), (0.0, 0.0), None)
            .expect_err("malformed spec errors");
        assert_eq!(err.exit_code, 2);
    }

    #[test]
    fn add_unknown_page_errors() {
        let err = add(
            TARGET_SRC,
            "@zenith/flowchart#decision",
            None,
            Some("nope"),
            (0.0, 0.0),
            None,
        )
        .expect_err("unknown page errors");
        assert!(
            err.message.contains("page 'nope' not found"),
            "msg: {}",
            err.message
        );
    }

    #[test]
    fn add_unknown_pkg_and_item_error() {
        let e1 = add(
            TARGET_SRC,
            "@no/such#decision",
            None,
            Some("pg"),
            (0.0, 0.0),
            None,
        )
        .expect_err("unknown pkg");
        assert!(e1.message.contains("@zenith/flowchart"), "{}", e1.message);
        let e2 = add(
            TARGET_SRC,
            "@zenith/flowchart#nope",
            None,
            Some("pg"),
            (0.0, 0.0),
            None,
        )
        .expect_err("unknown item");
        assert!(e2.message.contains("process"), "{}", e2.message);
    }

    #[test]
    fn add_is_pure_on_input_string() {
        // `add` never mutates its input; writing happens only in the dispatcher.
        // Two calls on the same input yield byte-identical output (deterministic).
        let a = add(
            TARGET_SRC,
            "@zenith/flowchart#process",
            None,
            Some("pg"),
            (0.0, 0.0),
            None,
        )
        .expect("a");
        let b = add(
            TARGET_SRC,
            "@zenith/flowchart#process",
            None,
            Some("pg"),
            (0.0, 0.0),
            None,
        )
        .expect("b");
        assert_eq!(a.formatted, b.formatted, "add is deterministic + pure");
    }

    #[test]
    fn add_filter_token_then_apply_compiles() {
        let result = add(
            TARGET_SRC,
            "@zenith/filters#noir",
            None,
            None,
            (0.0, 0.0),
            None,
        )
        .expect("add filter token ok");

        // Result reparses + validates clean.
        let src = String::from_utf8(result.formatted).expect("utf8");
        let doc = KdlAdapter.parse(src.as_bytes()).expect("reparse");
        let errors: Vec<_> = validate(&doc)
            .diagnostics
            .into_iter()
            .filter(|d| d.severity == Severity::Error)
            .collect();
        assert!(errors.is_empty(), "errors: {:?}", errors);

        // Summary mentions how to apply the token.
        assert!(
            result.summary.contains("filter=(token)\"noir\""),
            "summary: {}",
            result.summary
        );

        // The added token can be applied to a rect: add it into a target that
        // already carries a rect referencing `filter=(token)"noir"`, then assert
        // the result validates clean and compiles to scene commands.
        const TARGET_WITH_RECT: &str = r#"zenith version=1 {
  project id="proj.x" name="Target"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="d" title="x" {
    page id="pg" w=(px)800 h=(px)600 {
      rect id="r" x=(px)10 y=(px)10 w=(px)100 h=(px)100 filter=(token)"noir"
    }
  }
}
"#;
        let applied = add(
            TARGET_WITH_RECT,
            "@zenith/filters#noir",
            None,
            None,
            (0.0, 0.0),
            None,
        )
        .expect("add into rect target ok");
        let applied_src = String::from_utf8(applied.formatted).expect("utf8");
        let applied_doc = KdlAdapter
            .parse(applied_src.as_bytes())
            .expect("reparse applied");
        let applied_errors: Vec<_> = validate(&applied_doc)
            .diagnostics
            .into_iter()
            .filter(|d| d.severity == Severity::Error)
            .collect();
        assert!(
            applied_errors.is_empty(),
            "applied errors: {:?}",
            applied_errors
        );
        let artifact =
            crate::commands::render::to_scene_json(&applied_src, None, 1).expect("compile ok");
        let scene: serde_json::Value =
            serde_json::from_str(&artifact.json).expect("scene json parses");
        let commands = scene["commands"].as_array().expect("commands array");
        assert!(!commands.is_empty(), "applied filter compiles to commands");
    }

    #[test]
    fn add_action_accepted_applies_tx_and_writes_provenance() {
        // Pack source with an action that updates token color.brand to #e11d48.
        // Raw string uses r##"..."## to avoid early termination on "#e11d48".
        const ACTION_PACK_SRC: &str = r##"zenith version=1 {
  project id="@test/actions" name="Test Actions"
  libraries { library id="@test/actions" version="1.0.0" }
  actions {
    action id="apply-brand-kit" {
      tx "{\"ops\":[{\"op\":\"update_token_value\",\"id\":\"color.brand\",\"value\":\"#e11d48\"}]}"
    }
  }
  document id="d" title="x" {
    page id="pg" w=(px)100 h=(px)100 {}
  }
}
"##;
        // Target document that declares the token the action will update.
        const TARGET_WITH_TOKEN: &str = r##"zenith version=1 {
  project id="proj.x" name="Target"
  tokens format="zenith-token-v1" {
    token id="color.brand" type="color" value="#111111"
  }
  styles {}
  document id="d" title="x" {
    page id="pg" w=(px)800 h=(px)600 {}
  }
}
"##;

        let dir = tempfile::tempdir().expect("tempdir");
        let lib_dir = dir.path().join("libraries");
        std::fs::create_dir_all(&lib_dir).expect("create libraries dir");
        std::fs::write(lib_dir.join("actions.zen"), ACTION_PACK_SRC).expect("write pack");

        let result = add(
            TARGET_WITH_TOKEN,
            "@test/actions#apply-brand-kit",
            Some(dir.path()),
            None,
            (0.0, 0.0),
            None,
        )
        .expect("action add ok");

        let src = String::from_utf8(result.formatted).expect("utf8");
        assert!(src.contains("#e11d48"), "updated value in output: {}", src);
        assert!(
            result.summary.contains("apply-brand-kit"),
            "summary mentions action id: {}",
            result.summary
        );
        assert!(
            result.summary.contains("provenance"),
            "summary mentions provenance: {}",
            result.summary
        );
    }

    #[test]
    fn add_action_rejected_returns_error_exit_1() {
        // Action targets a non-existent token — tx will be rejected.
        const ACTION_PACK_SRC: &str = r##"zenith version=1 {
  project id="@test/actions" name="Test Actions"
  libraries { library id="@test/actions" version="1.0.0" }
  actions {
    action id="bad-action" {
      tx "{\"ops\":[{\"op\":\"update_token_value\",\"id\":\"no.such.token\",\"value\":\"#fff\"}]}"
    }
  }
  document id="d" title="x" {
    page id="pg" w=(px)100 h=(px)100 {}
  }
}
"##;

        let dir = tempfile::tempdir().expect("tempdir");
        let lib_dir = dir.path().join("libraries");
        std::fs::create_dir_all(&lib_dir).expect("create libraries dir");
        std::fs::write(lib_dir.join("actions.zen"), ACTION_PACK_SRC).expect("write pack");

        let err = add(
            TARGET_SRC,
            "@test/actions#bad-action",
            Some(dir.path()),
            None,
            (0.0, 0.0),
            None,
        )
        .expect_err("rejected action must return an error");

        assert_eq!(err.exit_code, 1, "exit_code must be 1 for rejected tx");
        assert!(
            err.message.contains("rejected"),
            "msg must mention rejected: {}",
            err.message
        );
    }

    #[test]
    fn add_component_without_page_errors() {
        let err = add(
            TARGET_SRC,
            "@zenith/flowchart#decision",
            None,
            None,
            (0.0, 0.0),
            None,
        )
        .expect_err("component without page errors");
        assert_eq!(err.exit_code, 2);
        assert!(
            err.message.contains("--page"),
            "msg should ask for --page: {}",
            err.message
        );
    }
}
