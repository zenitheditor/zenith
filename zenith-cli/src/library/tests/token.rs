//! `materialize_token` + `collect_filter_dep_ids` tests.

use super::support::{FILTERS_SRC, hard_errors, parse_target};
use crate::library::token::collect_filter_dep_ids;
use crate::library::{
    ItemScope, PackSource, load_pack_document, materialize_token, parse_pack, resolve_packs,
};

#[test]
fn collect_filter_dep_ids_duotone_and_simple() {
    let pack = load_pack_document(
        &parse_pack(FILTERS_SRC, PackSource::Preset).expect("pack"),
        ItemScope::All,
    )
    .expect("pack doc");

    let gold = pack
        .tokens
        .tokens
        .iter()
        .find(|t| t.id == "duotone-gold")
        .expect("duotone-gold present");
    let deps = collect_filter_dep_ids(gold, &pack.tokens.tokens);
    let deps: Vec<String> = deps.into_iter().collect();
    assert_eq!(
        deps,
        vec![
            "lib.filters.duo.gold.highlight".to_owned(),
            "lib.filters.duo.gold.shadow".to_owned(),
        ]
    );

    let noir = pack
        .tokens
        .tokens
        .iter()
        .find(|t| t.id == "noir")
        .expect("noir present");
    assert!(
        collect_filter_dep_ids(noir, &pack.tokens.tokens).is_empty(),
        "non-duotone filters have no token deps"
    );
}

#[test]
fn materialize_token_copies_filter_and_deps_records_provenance() {
    let mut target = parse_target();
    let packs = resolve_packs(None);
    let outcome = materialize_token(
        &mut target,
        &packs,
        "@zenith/filters",
        "duotone-gold",
        "duotone-gold",
    )
    .expect("materialize_token ok");

    // Filter token + its two color deps copied.
    assert!(target.tokens.tokens.iter().any(|t| t.id == "duotone-gold"));
    assert!(
        target
            .tokens
            .tokens
            .iter()
            .any(|t| t.id == "lib.filters.duo.gold.shadow")
    );
    assert!(
        target
            .tokens
            .tokens
            .iter()
            .any(|t| t.id == "lib.filters.duo.gold.highlight")
    );
    assert_eq!(
        outcome.dep_token_ids,
        vec![
            "lib.filters.duo.gold.highlight".to_owned(),
            "lib.filters.duo.gold.shadow".to_owned(),
        ]
    );
    assert_eq!(outcome.token_id, "duotone-gold");

    // Library + provenance recorded; provenance.node is the TOKEN id.
    assert!(target.libraries.iter().any(|l| l.id == "@zenith/filters"));
    let prov = target
        .provenance
        .iter()
        .find(|p| p.node == "duotone-gold")
        .expect("provenance recorded");
    assert_eq!(prov.library, "@zenith/filters");
    assert_eq!(prov.item.as_deref(), Some("duotone-gold"));
    assert_eq!(outcome.provenance_id, prov.id);

    assert!(
        hard_errors(&target).is_empty(),
        "errors: {:?}",
        hard_errors(&target)
    );
}

#[test]
fn materialize_token_mask_applies_via_mask_property_no_deps() {
    let mut target = parse_target();
    let packs = resolve_packs(None);
    let outcome = materialize_token(&mut target, &packs, "@zenith/masks", "vignette", "vignette")
        .expect("materialize_token ok");
    // The mask token is copied; masks are self-contained (no deps).
    assert!(target.tokens.tokens.iter().any(|t| t.id == "vignette"));
    assert!(outcome.dep_token_ids.is_empty());
    // It applies through the `mask` property (not `filter`).
    assert_eq!(outcome.apply_property, "mask");
    // Provenance recorded against the token id.
    assert!(target.provenance.iter().any(|p| p.node == "vignette"));
    assert!(hard_errors(&target).is_empty());
}

#[test]
fn materialize_token_simple_filter_no_deps() {
    let mut target = parse_target();
    let packs = resolve_packs(None);
    let outcome = materialize_token(&mut target, &packs, "@zenith/filters", "noir", "noir")
        .expect("materialize_token ok");
    assert!(target.tokens.tokens.iter().any(|t| t.id == "noir"));
    assert!(outcome.dep_token_ids.is_empty());
    assert!(hard_errors(&target).is_empty());
}

#[test]
fn materialize_token_double_add_dedups_token_and_provenance() {
    let mut target = parse_target();
    let packs = resolve_packs(None);
    let o1 =
        materialize_token(&mut target, &packs, "@zenith/filters", "noir", "noir").expect("first");
    let o2 =
        materialize_token(&mut target, &packs, "@zenith/filters", "noir", "noir").expect("second");

    // Token copied exactly once.
    assert_eq!(
        target
            .tokens
            .tokens
            .iter()
            .filter(|t| t.id == "noir")
            .count(),
        1
    );
    // Identical provenance is not duplicated.
    assert_eq!(target.provenance.len(), 1);
    assert_eq!(o1.provenance_id, o2.provenance_id);
    // One library entry only.
    assert_eq!(
        target
            .libraries
            .iter()
            .filter(|l| l.id == "@zenith/filters")
            .count(),
        1
    );
    assert!(hard_errors(&target).is_empty());
}

#[test]
fn materialize_token_unknown_item_errors_with_available() {
    let mut target = parse_target();
    let packs = resolve_packs(None);
    let err = materialize_token(&mut target, &packs, "@zenith/filters", "nope", "nope")
        .expect_err("unknown filter token errors");
    assert!(
        err.message.contains("noir"),
        "lists available: {}",
        err.message
    );
}
