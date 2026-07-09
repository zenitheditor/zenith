//! Pack parsing, embedded/project loading, and `resolve_packs` tests.

use super::support::{FILTERS_SRC, hard_errors};
use crate::library::{
    EMBEDDED_PACKS, ItemKind, ItemScope, PackFormat, PackItem, PackSource, embedded_preset_asset,
    embedded_preset_assets_for_document, load_embedded_packs, load_project_packs, pack_document,
    parse_pack, resolve_packs,
};
use zenith_core::{KdlAdapter, KdlSource, Node};

/// A `PackItem` with no search metadata — the shape every `.zen` pack item takes.
fn item(id: &str, kind: ItemKind) -> PackItem {
    PackItem {
        id: id.to_owned(),
        kind,
        aliases: Vec::new(),
        tags: Vec::new(),
        categories: Vec::new(),
    }
}

const FLOWCHART_SRC: &str = include_str!("../../../assets/libraries/zenith-flowchart.zen");

#[test]
fn parse_embedded_flowchart_identity_and_items() {
    let pack = parse_pack(FLOWCHART_SRC, PackSource::Preset).expect("flowchart pack parses");
    assert_eq!(pack.id, "@zenith/flowchart");
    assert_eq!(pack.version.as_deref(), Some("1.0.0"));
    assert_eq!(pack.source, PackSource::Preset);
    assert_eq!(
        pack.items,
        vec![
            item("process", ItemKind::Component),
            item("decision", ItemKind::Component),
            item("terminator", ItemKind::Component),
        ]
    );
}

#[test]
fn parse_pack_with_actions_lists_action_items() {
    const ACTION_PACK_SRC: &str = r#"zenith version=1 {
  project id="@test/actions" name="Test Actions"
  libraries { library id="@test/actions" version="1.0.0" }
  actions {
    action id="apply-brand-kit" {
      tx "{\"ops\":[]}"
    }
  }
  document id="d" title="x" {
    page id="pg" w=(px)100 h=(px)100 {
    }
  }
}
"#;
    let pack = parse_pack(ACTION_PACK_SRC, PackSource::Preset).expect("action pack parses");
    assert_eq!(pack.id, "@test/actions");
    assert!(
        pack.items
            .contains(&item("apply-brand-kit", ItemKind::Action)),
        "action item must be present; items: {:?}",
        pack.items
    );
}

#[test]
fn embedded_masks_pack_lists_mask_token_items() {
    let packs = resolve_packs(None);
    let masks = packs
        .iter()
        .find(|p| p.id == "@zenith/masks")
        .expect("@zenith/masks embedded");
    // Mask tokens are exported as token items.
    assert!(
        masks
            .items
            .iter()
            .any(|it| it.id == "vignette" && it.kind == ItemKind::Token),
        "vignette listed as a token item"
    );
    assert!(masks.items.iter().any(|it| it.id == "spotlight"));
}

#[test]
fn embedded_brand_kit_pack_lists_action_items() {
    let packs = resolve_packs(None);
    let brand = packs
        .iter()
        .find(|p| p.id == "@zenith/brand-kit")
        .expect("@zenith/brand-kit embedded");
    // Actions are exported as action items.
    assert!(
        brand
            .items
            .iter()
            .any(|it| it.id == "apply-2026" && it.kind == ItemKind::Action),
        "apply-2026 listed as an action item"
    );
    assert!(brand.items.iter().any(|it| it.id == "apply-mono"));
}

#[test]
fn bundled_lucide_is_an_svg_library_of_native_icon_components() {
    let packs = load_embedded_packs();
    let pack = packs
        .iter()
        .find(|p| p.id == "@zenith/icons-lucide")
        .expect("lucide is bundled");

    assert_eq!(pack.format, PackFormat::SvgDir);
    assert_eq!(pack.source, PackSource::Preset);
    assert_eq!(pack.version.as_deref(), Some("1.23.0"));
    // The whole upstream set, not a hand-picked subset.
    assert!(pack.items.len() > 1700, "got {} items", pack.items.len());

    for id in ["monitor", "cloud", "server", "house", "refresh-cw"] {
        assert!(
            pack.items
                .iter()
                .any(|it| it.id == id && it.kind == ItemKind::Component),
            "{id} component listed"
        );
    }

    // Search metadata rides on the item, sourced from `library.kdl`.
    let refresh = pack
        .items
        .iter()
        .find(|it| it.id == "refresh-cw")
        .expect("refresh-cw");
    assert!(refresh.aliases.contains(&"sync".to_owned()));
    assert!(refresh.categories.contains(&"arrows".to_owned()));
}

#[test]
fn bundled_lucide_synthesizes_only_the_requested_icon() {
    let packs = load_embedded_packs();
    let pack = packs
        .iter()
        .find(|p| p.id == "@zenith/icons-lucide")
        .expect("lucide is bundled");

    let doc = pack_document(pack, ItemScope::Only("monitor")).expect("monitor synthesizes");

    // Scoped conversion: one component, not 1745.
    assert_eq!(doc.components.len(), 1);
    let monitor = doc
        .components
        .iter()
        .find(|component| component.id == "monitor")
        .expect("monitor component exists");
    assert!(
        monitor
            .children
            .iter()
            .all(|node| matches!(node, Node::Path(_))),
        "an SVG icon component should contain editable paths"
    );
    assert!(monitor.children.len() >= 2);

    // A synthesized icon pack declares no SVG assets: geometry is native.
    assert!(
        embedded_preset_assets_for_document(&doc).is_empty(),
        "native icon components should not require SVG asset write intents"
    );

    let errors = hard_errors(&doc);
    assert!(
        errors.is_empty(),
        "synthesized lucide pack must validate with no errors; got: {:?}",
        errors
    );
}

#[test]
fn every_bundled_icon_is_addressable_as_an_embedded_asset() {
    let asset = embedded_preset_asset("assets/zenith/icons/lucide/house.svg")
        .expect("bundled icons are addressable as assets");
    assert!(asset.bytes.starts_with(b"<svg"));

    // Paths that name no bundled icon, or escape the icon root, do not resolve.
    assert!(embedded_preset_asset("assets/zenith/icons/lucide/no-such-icon.svg").is_none());
    assert!(embedded_preset_asset("assets/zenith/icons/lucide/../../etc/passwd.svg").is_none());
    assert!(embedded_preset_asset("assets/zenith/icons/nope/house.svg").is_none());
    assert!(embedded_preset_asset("elsewhere/house.svg").is_none());
}

#[test]
fn parse_embedded_filters_lists_filter_token_items() {
    let pack = parse_pack(FILTERS_SRC, PackSource::Preset).expect("filters pack parses");
    assert_eq!(pack.id, "@zenith/filters");
    assert_eq!(pack.version.as_deref(), Some("1.0.0"));

    // Filter tokens are items; color dep tokens are NOT.
    assert!(pack.items.contains(&item("noir", ItemKind::Token)));
    assert!(pack.items.contains(&item("duotone-gold", ItemKind::Token)));
    // Color dep tokens are dependencies, not exported items.
    assert!(
        !pack
            .items
            .iter()
            .any(|i| i.id == "lib.filters.duo.gold.shadow"),
        "color dep tokens must not be items"
    );
    // The filters pack ships no components, so every item is a token.
    assert!(pack.items.iter().all(|i| i.kind == ItemKind::Token));
}

#[test]
fn embedded_flowchart_parses_and_validates_clean() {
    let doc = KdlAdapter
        .parse(FLOWCHART_SRC.as_bytes())
        .expect("embedded pack must parse");
    let errors = hard_errors(&doc);
    assert!(
        errors.is_empty(),
        "embedded pack must validate with no errors; got: {:?}",
        errors
    );
}

#[test]
fn load_embedded_packs_includes_flowchart() {
    let packs = load_embedded_packs();
    assert!(
        packs.iter().any(|p| p.id == "@zenith/flowchart"),
        "embedded packs must include @zenith/flowchart"
    );
}

#[test]
fn pack_without_self_entry_errors() {
    let src = r#"zenith version=1 {
  project id="proj.x" name="No Library"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="d" title="x" {
    page id="pg" w=(px)10 h=(px)10 {}
  }
}
"#;
    let err = parse_pack(src, PackSource::Preset).expect_err("must require a self-entry");
    assert!(err.message.contains("library self-entry"));
}

#[test]
fn load_project_packs_finds_libraries_dir_pack() {
    let dir = tempfile::tempdir().expect("tempdir");
    let lib_dir = dir.path().join("libraries");
    std::fs::create_dir_all(&lib_dir).expect("create libraries dir");
    std::fs::write(lib_dir.join("foo.zen"), FLOWCHART_SRC).expect("write pack");

    let packs = load_project_packs(dir.path());
    assert_eq!(packs.len(), 1);
    assert_eq!(packs[0].id, "@zenith/flowchart");
    assert!(matches!(packs[0].source, PackSource::Project(_)));
}

#[test]
fn load_project_packs_missing_dir_is_empty() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(load_project_packs(dir.path()).is_empty());
}

#[test]
fn resolve_packs_includes_embedded_when_no_project_dir() {
    let packs = resolve_packs(None);
    assert!(packs.iter().any(|p| p.id == "@zenith/flowchart"));
}

#[test]
fn resolve_packs_is_sorted_by_id() {
    let packs = resolve_packs(None);
    let mut sorted = packs.clone();
    sorted.sort_by(|a, b| a.id.cmp(&b.id));
    let ids: Vec<_> = packs.iter().map(|p| &p.id).collect();
    let sorted_ids: Vec<_> = sorted.iter().map(|p| &p.id).collect();
    assert_eq!(ids, sorted_ids);
}

#[test]
fn embedded_pack_ids_are_unique() {
    let ids: Vec<&str> = EMBEDDED_PACKS.iter().map(|(id, _)| *id).collect();
    let total = ids.len();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        total,
        "EMBEDDED_PACKS ids must be pairwise unique; got: {:?}",
        ids
    );
}
