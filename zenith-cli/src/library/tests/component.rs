//! `materialize` (component) tests.

use super::support::{first_page_instance_ids, hard_errors, parse_target};
use crate::library::add::px;
use crate::library::{materialize, resolve_packs};
use zenith_core::{KdlAdapter, KdlSource, Node};

#[test]
fn materialize_adds_component_tokens_style_instance_provenance() {
    let mut target = parse_target();
    let packs = resolve_packs(None);
    let outcome = materialize(
        &mut target,
        &packs,
        "@zenith/flowchart",
        "decision",
        "pg",
        "decision",
        (10.0, 20.0),
    )
    .expect("materialize ok");

    // Component copied under namespaced id.
    assert_eq!(outcome.target_component_id, "lib.zenith.flowchart.decision");
    assert!(
        target
            .components
            .iter()
            .any(|c| c.id == "lib.zenith.flowchart.decision"),
        "component copied"
    );
    // Child ids are NOT rewritten (still local `shape`).
    let comp = target
        .components
        .iter()
        .find(|c| c.id == "lib.zenith.flowchart.decision")
        .unwrap();
    assert!(matches!(comp.children.first(), Some(Node::Shape(s)) if s.id == "shape"));

    // Dep tokens + style copied.
    assert!(target.tokens.tokens.iter().any(|t| t.id == "lib.flow.fill"));
    assert!(
        target
            .tokens
            .tokens
            .iter()
            .any(|t| t.id == "lib.flow.dec.fill")
    );
    assert!(
        target
            .styles
            .styles
            .iter()
            .any(|s| s.id == "lib.flow.label")
    );
    assert_eq!(target.tokens.format, "zenith-token-v1");

    // Instance inserted on the page referencing the component.
    let inst = target.body.pages[0]
        .children
        .iter()
        .find_map(|n| match n {
            Node::Instance(i) => Some(i),
            _ => None,
        })
        .expect("instance inserted");
    assert_eq!(inst.id, "decision");
    assert_eq!(
        inst.component.as_deref(),
        Some("lib.zenith.flowchart.decision")
    );
    assert_eq!(inst.x, Some(px(10.0)));
    assert_eq!(inst.y, Some(px(20.0)));

    // Library + provenance recorded.
    assert!(target.libraries.iter().any(|l| l.id == "@zenith/flowchart"));
    let prov = target
        .provenance
        .iter()
        .find(|p| p.node == "decision")
        .expect("provenance recorded");
    assert_eq!(prov.library, "@zenith/flowchart");
    assert_eq!(prov.item.as_deref(), Some("decision"));
    assert_eq!(prov.linked, Some(true));
    assert_eq!(outcome.provenance_id, prov.id);
    assert!(outcome.warnings.is_empty());

    // Validates clean.
    assert!(
        hard_errors(&target).is_empty(),
        "errors: {:?}",
        hard_errors(&target)
    );
}

#[test]
fn materialize_round_trips_format_parse() {
    let mut target = parse_target();
    let packs = resolve_packs(None);
    materialize(
        &mut target,
        &packs,
        "@zenith/flowchart",
        "decision",
        "pg",
        "decision",
        (0.0, 0.0),
    )
    .expect("materialize ok");
    let bytes = KdlAdapter.format(&target).expect("format");
    let reparsed = KdlAdapter.parse(&bytes).expect("reparse");
    let bytes2 = KdlAdapter.format(&reparsed).expect("format2");
    assert_eq!(bytes, bytes2, "format→parse→format is stable");
}

#[test]
fn double_add_dedups_component_unique_instance_two_provenance() {
    let mut target = parse_target();
    let packs = resolve_packs(None);
    let o1 = materialize(
        &mut target,
        &packs,
        "@zenith/flowchart",
        "decision",
        "pg",
        "decision",
        (0.0, 0.0),
    )
    .expect("first add");
    let o2 = materialize(
        &mut target,
        &packs,
        "@zenith/flowchart",
        "decision",
        "pg",
        "decision",
        (0.0, 0.0),
    )
    .expect("second add");

    // Component copied exactly once.
    assert_eq!(
        target
            .components
            .iter()
            .filter(|c| c.id == "lib.zenith.flowchart.decision")
            .count(),
        1
    );
    // Tokens not duplicated.
    assert_eq!(
        target
            .tokens
            .tokens
            .iter()
            .filter(|t| t.id == "lib.flow.fill")
            .count(),
        1
    );
    // Unique instance ids.
    assert_eq!(o1.instance_id, "decision");
    assert_eq!(o2.instance_id, "decision.1");
    assert_eq!(
        first_page_instance_ids(&target),
        vec!["decision", "decision.1"]
    );
    // Two provenance records.
    assert_eq!(target.provenance.len(), 2);
    assert_ne!(o1.provenance_id, o2.provenance_id);
    // One library entry only.
    assert_eq!(
        target
            .libraries
            .iter()
            .filter(|l| l.id == "@zenith/flowchart")
            .count(),
        1
    );
    assert!(hard_errors(&target).is_empty());
}

#[test]
fn materialize_unknown_page_errors_and_does_not_mutate() {
    let mut target = parse_target();
    let before = target.clone();
    let packs = resolve_packs(None);
    let err = materialize(
        &mut target,
        &packs,
        "@zenith/flowchart",
        "decision",
        "nope",
        "decision",
        (0.0, 0.0),
    )
    .expect_err("unknown page errors");
    assert!(
        err.message.contains("page 'nope' not found"),
        "msg: {}",
        err.message
    );
    assert_eq!(target, before, "target untouched on page error");
}

#[test]
fn materialize_unknown_pkg_errors_with_available() {
    let mut target = parse_target();
    let packs = resolve_packs(None);
    let err = materialize(
        &mut target,
        &packs,
        "@no/such",
        "decision",
        "pg",
        "decision",
        (0.0, 0.0),
    )
    .expect_err("unknown pkg errors");
    assert!(
        err.message.contains("@zenith/flowchart"),
        "lists available: {}",
        err.message
    );
}

#[test]
fn materialize_unknown_item_errors_with_available() {
    let mut target = parse_target();
    let packs = resolve_packs(None);
    let err = materialize(
        &mut target,
        &packs,
        "@zenith/flowchart",
        "nope",
        "pg",
        "decision",
        (0.0, 0.0),
    )
    .expect_err("unknown item errors");
    assert!(
        err.message.contains("process"),
        "lists available items: {}",
        err.message
    );
}

#[test]
fn materialize_unknown_tokens_item_suggests_theme_apply() {
    // theme.cobalt carries a full token set but exports no items (no
    // components, no filter/mask tokens), so `tokens` is always an unknown
    // item for it — the natural place to point users at `theme apply` instead.
    let mut target = parse_target();
    let packs = resolve_packs(None);
    let err = materialize(
        &mut target,
        &packs,
        "@zenith/theme.cobalt",
        "tokens",
        "pg",
        "tokens",
        (0.0, 0.0),
    )
    .expect_err("unknown item errors");
    assert!(
        err.message
            .contains("zenith theme apply @zenith/theme.cobalt <doc>"),
        "should suggest theme apply: {}",
        err.message
    );
}

#[test]
fn materialize_id_override_used_as_base() {
    let mut target = parse_target();
    let packs = resolve_packs(None);
    let o = materialize(
        &mut target,
        &packs,
        "@zenith/flowchart",
        "decision",
        "pg",
        "my.node",
        (0.0, 0.0),
    )
    .expect("ok");
    assert_eq!(o.instance_id, "my.node");
}
