use std::path::Path;

use zenith_core::{Dimension, Document, KdlAdapter, KdlSource, Node, Unit, validate};
use zenith_tx::{Op, Permissions, Position, Transaction, TxStatus, run_transaction};

use crate::library::{
    EmbeddedPresetAsset, ItemKind, collect_node_ids, embedded_preset_assets_for_document,
    parse_spec, resolve_packs,
};

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
    /// Embedded preset asset files the dispatcher should materialize beside the
    /// target document for non-dry-run adds.
    pub embedded_assets: Vec<EmbeddedPresetAsset>,
    /// The container the instance was placed into (`--parent`), when given.
    pub parent: Option<String>,
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
/// `parent` moves the new instance into a frame or group on `page` with a
/// `Reparent` transaction. The instance keeps `at` as written, so the engine's
/// coordinate rules for that container apply. Layout frames place it in flow.
/// COMPONENT items only.
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
    parent: Option<&str>,
) -> Result<AddResult, AddCmdErr> {
    let (pkg_id, item) = parse_spec(spec).map_err(|e| AddCmdErr::new(e.message, 2))?;

    let mut target = KdlAdapter
        .parse(target_src.as_bytes())
        .map_err(|e| AddCmdErr::new(format!("error[parse.error]: {}", e.message), 2))?;

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

    if parent.is_some() {
        match item_kind {
            Some(ItemKind::Component) | None => {}
            Some(ItemKind::Token) | Some(ItemKind::Action) => {
                return Err(AddCmdErr::new(
                    "--parent applies to component items only; token and action items have no placement",
                    2,
                ));
            }
        }
    }

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
            let embedded_assets = embedded_preset_assets_for_document(&result_doc);

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
            return Ok(AddResult {
                formatted,
                summary,
                embedded_assets,
                parent: None,
            });
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
            let mut placement = format!("on page '{page}'");
            if let Some(parent) = parent {
                target = move_into_parent(&target, page, &outcome.instance_id, parent, at)?;
                placement = format!("in '{parent}' on page '{page}'");
            }
            let mut summary = String::new();
            summary.push_str(&format!(
                "added {}#{} as instance '{}' {}\n",
                outcome.pkg_id, outcome.item, outcome.instance_id, placement
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
    let embedded_assets = embedded_preset_assets_for_document(&target);
    Ok(AddResult {
        formatted,
        summary,
        embedded_assets,
        parent: parent.map(str::to_owned),
    })
}

/// Move the instance `instance_id` (placed on `page`) into the frame or group
/// `parent` with a `Reparent` transaction. `parent` must sit on `page`.
///
/// `Reparent` converts x / y to keep the page position. `--at` counts in the
/// parent's space, so the instance gets `at` back after the move.
fn move_into_parent(
    target: &Document,
    page: &str,
    instance_id: &str,
    parent: &str,
    at: (f64, f64),
) -> Result<Document, AddCmdErr> {
    let on_page = target
        .body
        .pages
        .iter()
        .find(|p| p.id == page)
        .is_some_and(|p| {
            let mut ids = std::collections::BTreeSet::new();
            collect_node_ids(&p.children, &mut ids);
            ids.contains(parent)
        });
    if !on_page {
        return Err(AddCmdErr::new(
            format!(
                "parent '{parent}' not found on page '{page}'; pass the id of a frame or group on that page"
            ),
            2,
        ));
    }
    let tx = Transaction {
        ops: vec![Op::Reparent {
            node: instance_id.to_owned(),
            new_parent: parent.to_owned(),
            position: Position::default(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(target, &tx)
        .map_err(|e| AddCmdErr::new(format!("error: could not move into parent: {e}"), 2))?;
    match result.status {
        TxStatus::Rejected => {
            let lines: Vec<String> = result
                .diagnostics
                .iter()
                .map(crate::commands::format_diagnostic_line)
                .collect();
            Err(AddCmdErr::new(
                format!("cannot place into parent '{parent}':\n{}", lines.join("\n")),
                1,
            ))
        }
        TxStatus::Accepted | TxStatus::AcceptedWithWarnings => {
            let mut moved = KdlAdapter
                .parse(result.source_after.as_bytes())
                .map_err(|e| {
                    AddCmdErr::new(
                        format!(
                            "internal error: could not re-parse moved document: {}",
                            e.message
                        ),
                        2,
                    )
                })?;
            for p in &mut moved.body.pages {
                if set_instance_origin(&mut p.children, instance_id, at) {
                    break;
                }
            }
            Ok(moved)
        }
    }
}

/// Write `at` as the px x / y of instance `id` below `nodes`. `true` once
/// the instance is found.
fn set_instance_origin(nodes: &mut [Node], id: &str, at: (f64, f64)) -> bool {
    let px = |v: f64| Dimension {
        value: v,
        unit: Unit::Px,
    };
    for node in nodes {
        if let Node::Instance(inst) = node
            && inst.id == id
        {
            inst.x = Some(px(at.0));
            inst.y = Some(px(at.1));
            return true;
        }
        if let Some(children) = node.children_mut()
            && set_instance_origin(children, id, at)
        {
            return true;
        }
    }
    false
}

/// Validate the mutated `target` (hard errors abort with no write) then format it
/// to canonical bytes. Shared by the component and token `add` branches.
fn validate_and_format(target: &zenith_core::Document) -> Result<Vec<u8>, AddCmdErr> {
    let report = validate(target);
    let errors: Vec<String> = report
        .diagnostics
        .iter()
        .filter(|d| d.is_error())
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

#[cfg(test)]
mod tests {
    use super::*;

    /// `add` without `--parent`: the 6-argument form most tests exercise.
    fn add(
        target_src: &str,
        spec: &str,
        project_dir: Option<&Path>,
        page: Option<&str>,
        at: (f64, f64),
        id_override: Option<&str>,
    ) -> Result<AddResult, AddCmdErr> {
        super::add(target_src, spec, project_dir, page, at, id_override, None)
    }

    // ── `add` command tests ────────────────────────────────────────────────────

    const PARENT_SRC: &str = r#"zenith version=1 {
  project id="proj.x" name="Target"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="d" title="x" {
    page id="pg" w=(px)800 h=(px)600 {
      frame id="plain" x=(px)100 y=(px)50 w=(px)300 h=(px)200 {}
      frame id="flow" x=(px)10 y=(px)20 w=(px)400 h=(px)60 layout="row" gap=(px)8 align="start" {}
      group id="grp" {}
    }
    page id="pg2" w=(px)800 h=(px)600 {
      frame id="other" x=(px)0 y=(px)0 w=(px)100 h=(px)100 {}
    }
  }
}
"#;

    /// Find the node `id` and return its direct children's ids.
    fn child_ids(doc: &zenith_core::Document, id: &str) -> Vec<String> {
        fn walk(nodes: &[zenith_core::Node], id: &str) -> Option<Vec<String>> {
            for n in nodes {
                let (nid, kids): (&str, &[zenith_core::Node]) = match n {
                    zenith_core::Node::Frame(f) => (&f.id, &f.children),
                    zenith_core::Node::Group(g) => (&g.id, &g.children),
                    _ => continue,
                };
                if nid == id {
                    return Some(
                        kids.iter()
                            .filter_map(|k| match k {
                                zenith_core::Node::Instance(i) => Some(i.id.clone()),
                                _ => None,
                            })
                            .collect(),
                    );
                }
                if let Some(found) = walk(kids, id) {
                    return Some(found);
                }
            }
            None
        }
        doc.body
            .pages
            .iter()
            .find_map(|p| walk(&p.children, id))
            .unwrap_or_default()
    }

    fn add_into(parent: &str, page: &str) -> Result<AddResult, AddCmdErr> {
        super::add(
            PARENT_SRC,
            "@zenith/flowchart#decision",
            None,
            Some(page),
            (12.0, 34.0),
            None,
            Some(parent),
        )
    }

    #[test]
    fn add_with_parent_places_instance_inside_frame_and_group() {
        for parent in ["plain", "flow", "grp"] {
            let result = add_into(parent, "pg").expect("add into parent");
            assert_eq!(result.parent.as_deref(), Some(parent));
            assert!(
                result
                    .summary
                    .contains(&format!("in '{parent}' on page 'pg'")),
                "summary: {}",
                result.summary
            );
            let src = String::from_utf8(result.formatted).expect("utf8");
            let doc = KdlAdapter.parse(src.as_bytes()).expect("reparse");
            assert_eq!(
                child_ids(&doc, parent),
                vec!["decision".to_owned()],
                "{src}"
            );
            // The instance keeps `--at` as written; the engine reads it in the
            // parent's coordinate space.
            assert!(src.contains("x=(px)12"), "{src}");
            assert!(src.contains("y=(px)34"), "{src}");
            // The page itself holds no stray instance.
            let page = doc.body.pages.iter().find(|p| p.id == "pg").expect("page");
            assert!(
                !page
                    .children
                    .iter()
                    .any(|n| matches!(n, zenith_core::Node::Instance(_))),
                "{src}"
            );
        }
    }

    #[test]
    fn add_with_parent_on_another_page_errors() {
        let err = add_into("other", "pg").expect_err("parent on another page");
        assert_eq!(err.exit_code, 2);
        assert!(
            err.message
                .contains("parent 'other' not found on page 'pg'"),
            "{}",
            err.message
        );
    }

    #[test]
    fn add_with_unknown_parent_errors() {
        let err = add_into("nope", "pg").expect_err("unknown parent");
        assert_eq!(err.exit_code, 2);
        assert!(
            err.message.contains("parent 'nope' not found"),
            "{}",
            err.message
        );
    }

    #[test]
    fn add_with_parent_rejects_non_component_items() {
        let err = super::add(
            PARENT_SRC,
            "@zenith/filters#noir",
            None,
            None,
            (0.0, 0.0),
            None,
            Some("plain"),
        )
        .expect_err("token item with parent");
        assert_eq!(err.exit_code, 2);
        assert!(err.message.contains("--parent"), "{}", err.message);
    }

    #[test]
    fn add_without_parent_reports_no_parent() {
        let result = add(
            PARENT_SRC,
            "@zenith/flowchart#decision",
            None,
            Some("pg"),
            (0.0, 0.0),
            None,
        )
        .expect("add ok");
        assert_eq!(result.parent, None);
        assert!(
            result.summary.contains("on page 'pg'"),
            "{}",
            result.summary
        );
    }

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
            .filter(|d| d.is_error())
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
        let artifact = crate::commands::render::to_scene_json(
            &src,
            None,
            1,
            &crate::config::CliPolicyFlags::default(),
            None,
        )
        .expect("compile ok");
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
            .filter(|d| d.is_error())
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
            .filter(|d| d.is_error())
            .collect();
        assert!(
            applied_errors.is_empty(),
            "applied errors: {:?}",
            applied_errors
        );
        let artifact = crate::commands::render::to_scene_json(
            &applied_src,
            None,
            1,
            &crate::config::CliPolicyFlags::default(),
            None,
        )
        .expect("compile ok");
        let scene: serde_json::Value =
            serde_json::from_str(&artifact.json).expect("scene json parses");
        let commands = scene["commands"].as_array().expect("commands array");
        assert!(!commands.is_empty(), "applied filter compiles to commands");
    }

    #[test]
    fn add_lucide_icon_materializes_native_paths_and_renders_locked() {
        let result = add(
            TARGET_SRC,
            "@zenith/icons-lucide#monitor",
            None,
            Some("pg"),
            (100.0, 100.0),
            None,
        )
        .expect("add lucide icon ok");

        let src = String::from_utf8(result.formatted).expect("utf8");
        let doc = KdlAdapter.parse(src.as_bytes()).expect("reparse");
        let errors: Vec<_> = validate(&doc)
            .diagnostics
            .into_iter()
            .filter(|d| d.is_error())
            .collect();
        assert!(errors.is_empty(), "errors: {:?}", errors);
        assert!(
            src.contains("library id=\"@zenith/icons-lucide\""),
            "source: {}",
            src
        );
        assert!(
            src.contains("component=\"lib.zenith.icons-lucide.monitor\""),
            "source: {}",
            src
        );
        assert!(
            result.embedded_assets.is_empty(),
            "native Lucide icon should not require SVG asset write intents"
        );
        assert!(src.contains("path id=\""), "source: {src}");
        assert!(
            !src.contains("image id=\"icon\""),
            "native Lucide icon should not materialize an SVG image wrapper: {src}"
        );

        let dir = tempfile::tempdir().expect("tempdir");

        let artifact = crate::commands::render::to_png_with_dir(
            &src,
            Some(dir.path()),
            1,
            true,
            &crate::config::CliPolicyFlags::default(),
            None,
        )
        .expect("locked render ok with native Lucide icon");
        assert!(
            !artifact.png.is_empty(),
            "render must produce PNG bytes for Lucide icon"
        );
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
