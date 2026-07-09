//! Generated native Lucide pack source.

use zenith_core::{
    AnchorKind, Dimension, KdlAdapter, KdlSource, Node, PathAnchor, PathNode, PathSubpath,
    PropertyValue, format::format_document,
};
use zenith_producers::{SvgNativeOptions, svg_to_native_paths};

const PACKAGE_ID: &str = "@zenith/icons-lucide";
const PACKAGE_VERSION: &str = "0.1.0";
const STROKE_TOKEN: &str = "lib.icons.stroke";
const STROKE_WIDTH_TOKEN: &str = "lib.icons.stroke_width";

struct LucideIcon {
    id: &'static str,
    bytes: &'static [u8],
}

const LUCIDE_ICONS: &[LucideIcon] = &[
    LucideIcon {
        id: "monitor",
        bytes: include_bytes!("../../assets/libraries/icons/lucide/monitor.svg"),
    },
    LucideIcon {
        id: "smartphone",
        bytes: include_bytes!("../../assets/libraries/icons/lucide/smartphone.svg"),
    },
    LucideIcon {
        id: "tablet",
        bytes: include_bytes!("../../assets/libraries/icons/lucide/tablet.svg"),
    },
    LucideIcon {
        id: "server",
        bytes: include_bytes!("../../assets/libraries/icons/lucide/server.svg"),
    },
    LucideIcon {
        id: "database",
        bytes: include_bytes!("../../assets/libraries/icons/lucide/database.svg"),
    },
    LucideIcon {
        id: "cloud",
        bytes: include_bytes!("../../assets/libraries/icons/lucide/cloud.svg"),
    },
    LucideIcon {
        id: "hard-drive",
        bytes: include_bytes!("../../assets/libraries/icons/lucide/hard-drive.svg"),
    },
    LucideIcon {
        id: "cpu",
        bytes: include_bytes!("../../assets/libraries/icons/lucide/cpu.svg"),
    },
    LucideIcon {
        id: "network",
        bytes: include_bytes!("../../assets/libraries/icons/lucide/network.svg"),
    },
    LucideIcon {
        id: "wifi",
        bytes: include_bytes!("../../assets/libraries/icons/lucide/wifi.svg"),
    },
    LucideIcon {
        id: "globe",
        bytes: include_bytes!("../../assets/libraries/icons/lucide/globe.svg"),
    },
    LucideIcon {
        id: "box",
        bytes: include_bytes!("../../assets/libraries/icons/lucide/box.svg"),
    },
    LucideIcon {
        id: "file",
        bytes: include_bytes!("../../assets/libraries/icons/lucide/file.svg"),
    },
    LucideIcon {
        id: "folder",
        bytes: include_bytes!("../../assets/libraries/icons/lucide/folder.svg"),
    },
    LucideIcon {
        id: "lock",
        bytes: include_bytes!("../../assets/libraries/icons/lucide/lock.svg"),
    },
    LucideIcon {
        id: "key",
        bytes: include_bytes!("../../assets/libraries/icons/lucide/key.svg"),
    },
    LucideIcon {
        id: "search",
        bytes: include_bytes!("../../assets/libraries/icons/lucide/search.svg"),
    },
    LucideIcon {
        id: "settings",
        bytes: include_bytes!("../../assets/libraries/icons/lucide/settings.svg"),
    },
    LucideIcon {
        id: "arrow-right-left",
        bytes: include_bytes!("../../assets/libraries/icons/lucide/arrow-right-left.svg"),
    },
    LucideIcon {
        id: "sync",
        bytes: include_bytes!("../../assets/libraries/icons/lucide/sync.svg"),
    },
    LucideIcon {
        id: "upload-cloud",
        bytes: include_bytes!("../../assets/libraries/icons/lucide/upload-cloud.svg"),
    },
    LucideIcon {
        id: "download-cloud",
        bytes: include_bytes!("../../assets/libraries/icons/lucide/download-cloud.svg"),
    },
];

pub(super) fn generate_pack_source() -> Result<String, String> {
    let mut out = String::new();
    out.push_str("zenith version=1 {\n");
    out.push_str("  project id=\"@zenith/icons-lucide\" name=\"Zenith Lucide Icons\"\n");
    out.push_str("  libraries {\n");
    out.push_str("    library id=\"");
    out.push_str(PACKAGE_ID);
    out.push_str("\" version=\"");
    out.push_str(PACKAGE_VERSION);
    out.push_str("\"\n");
    out.push_str("  }\n");
    out.push_str("  tokens format=\"zenith-token-v1\" {\n");
    out.push_str("    token id=\"lib.icons.stroke\" type=\"color\" value=\"#111827\"\n");
    out.push_str("    token id=\"lib.icons.stroke_width\" type=\"dimension\" value=(px)2\n");
    out.push_str("  }\n");
    out.push_str("  components {\n");
    for icon in LUCIDE_ICONS {
        write_icon_component(&mut out, icon)?;
    }
    out.push_str("  }\n");
    out.push_str("  document id=\"pack.preview\" title=\"Lucide icon pack preview\" {\n");
    out.push_str("    page id=\"pack.pg\" name=\"Preview\" w=(px)100 h=(px)100 {\n");
    out.push_str("    }\n");
    out.push_str("  }\n");
    out.push_str("}\n");

    canonicalize_pack(&out)
}

fn write_icon_component(out: &mut String, icon: &LucideIcon) -> Result<(), String> {
    let options = SvgNativeOptions {
        id_prefix: "icon".to_owned(),
        stroke: Some(PropertyValue::TokenRef(STROKE_TOKEN.to_owned())),
        fill: None,
        stroke_width: Some(PropertyValue::TokenRef(STROKE_WIDTH_TOKEN.to_owned())),
    };
    let nodes = svg_to_native_paths(icon.bytes, &options)
        .map_err(|err| format!("failed to convert native Lucide icon '{}': {err}", icon.id))?;
    if nodes.is_empty() {
        return Err(format!(
            "native Lucide icon '{}' converted to no path nodes",
            icon.id
        ));
    }

    out.push_str("    component id=\"");
    out.push_str(icon.id);
    out.push_str("\" {\n");
    for node in nodes {
        let Node::Path(path) = node else {
            return Err(format!(
                "native Lucide icon '{}' produced a non-path node",
                icon.id
            ));
        };
        write_path(out, &path, 6);
    }
    out.push_str("    }\n");
    Ok(())
}

fn canonicalize_pack(source: &str) -> Result<String, String> {
    let adapter = KdlAdapter;
    let doc = adapter
        .parse(source.as_bytes())
        .map_err(|err| format!("generated Lucide pack failed to parse: {err}"))?;
    let formatted = format_document(&doc)
        .map_err(|err| format!("generated Lucide pack failed to format: {err}"))?;
    String::from_utf8(formatted)
        .map_err(|err| format!("formatted Lucide pack was not UTF-8: {err}"))
}

fn write_path(out: &mut String, path: &PathNode, depth: usize) {
    indent(out, depth);
    out.push_str("path id=\"");
    out.push_str(&path.id);
    out.push('"');
    if let Some(role) = &path.role {
        write_str_prop(out, "role", role);
    }
    write_property_value(out, "fill", path.fill.as_ref());
    write_property_value(out, "stroke", path.stroke.as_ref());
    write_property_value(out, "stroke-width", path.stroke_width.as_ref());
    if let Some(stroke_linejoin) = &path.stroke_linejoin {
        write_str_prop(out, "stroke-linejoin", stroke_linejoin);
    }
    if let Some(stroke_linecap) = &path.stroke_linecap {
        write_str_prop(out, "stroke-linecap", stroke_linecap);
    }
    if let Some(fill_rule) = &path.fill_rule {
        write_str_prop(out, "fill-rule", fill_rule);
    }
    out.push_str(" {\n");
    write_anchors(out, &path.anchors, depth + 2);
    for subpath in &path.subpaths {
        write_subpath(out, subpath, depth + 2);
    }
    indent(out, depth);
    out.push_str("}\n");
}

fn write_subpath(out: &mut String, subpath: &PathSubpath, depth: usize) {
    indent(out, depth);
    out.push_str("subpath");
    if let Some(closed) = subpath.closed {
        out.push_str(" closed=#");
        out.push_str(if closed { "true" } else { "false" });
    }
    out.push_str(" {\n");
    write_anchors(out, &subpath.anchors, depth + 2);
    indent(out, depth);
    out.push_str("}\n");
}

fn write_anchors(out: &mut String, anchors: &[PathAnchor], depth: usize) {
    for anchor in anchors {
        indent(out, depth);
        out.push_str("anchor");
        write_dimension(out, "x", anchor.x.as_ref());
        write_dimension(out, "y", anchor.y.as_ref());
        if let Some(kind) = &anchor.kind {
            write_anchor_kind(out, kind);
        }
        write_dimension(out, "in-x", anchor.in_x.as_ref());
        write_dimension(out, "in-y", anchor.in_y.as_ref());
        write_dimension(out, "out-x", anchor.out_x.as_ref());
        write_dimension(out, "out-y", anchor.out_y.as_ref());
        out.push('\n');
    }
}

fn write_anchor_kind(out: &mut String, kind: &AnchorKind) {
    out.push_str(" kind=\"");
    out.push_str(kind.kind_str());
    out.push('"');
}

fn write_property_value(out: &mut String, key: &str, value: Option<&PropertyValue>) {
    let Some(value) = value else {
        return;
    };
    out.push(' ');
    out.push_str(key);
    out.push('=');
    match value {
        PropertyValue::TokenRef(id) => {
            out.push_str("(token)\"");
            out.push_str(id);
            out.push('"');
        }
        PropertyValue::Literal(value) => {
            out.push('"');
            out.push_str(value);
            out.push('"');
        }
        PropertyValue::Dimension(dim) => push_dimension_value(out, dim),
        PropertyValue::DataRef(path) => {
            out.push_str("(data)\"");
            out.push_str(path);
            out.push('"');
        }
    }
}

fn write_str_prop(out: &mut String, key: &str, value: &str) {
    out.push(' ');
    out.push_str(key);
    out.push_str("=\"");
    out.push_str(value);
    out.push('"');
}

fn write_dimension(out: &mut String, key: &str, value: Option<&Dimension>) {
    let Some(value) = value else {
        return;
    };
    out.push(' ');
    out.push_str(key);
    out.push('=');
    push_dimension_value(out, value);
}

fn push_dimension_value(out: &mut String, value: &Dimension) {
    out.push_str(&value.to_kdl_string());
}

fn indent(out: &mut String, depth: usize) {
    out.push_str(&" ".repeat(depth));
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::validate;

    const COMMITTED_PACK_SOURCE: &str =
        include_str!("../../assets/libraries/zenith-icons-lucide.zen");

    const PROVENANCE_JSON: &str =
        include_str!("../../assets/libraries/icons/lucide/provenance.json");

    use serde::Deserialize;
    use sha2::{Digest, Sha256};
    use std::collections::BTreeSet;

    #[derive(Deserialize)]
    struct Provenance {
        source_project: String,
        license: String,
        license_file: String,
        icons: Vec<ProvenanceIcon>,
    }

    #[derive(Deserialize)]
    struct ProvenanceIcon {
        name: String,
        upstream_name: String,
        sha256: String,
    }

    fn parse_provenance() -> Provenance {
        serde_json::from_str(PROVENANCE_JSON).expect("provenance.json parses")
    }

    #[test]
    fn provenance_records_expected_metadata() {
        let provenance = parse_provenance();
        assert_eq!(provenance.source_project, "Lucide");
        assert_eq!(provenance.license, "ISC");
        assert_eq!(provenance.license_file, "NOTICE");
    }

    #[test]
    fn provenance_covers_every_vendored_svg() {
        let provenance = parse_provenance();

        let embedded: BTreeSet<&str> = LUCIDE_ICONS.iter().map(|icon| icon.id).collect();
        let recorded: BTreeSet<&str> = provenance
            .icons
            .iter()
            .map(|icon| icon.name.as_str())
            .collect();
        assert_eq!(
            recorded, embedded,
            "provenance icon names must match the embedded Lucide SVG set exactly"
        );

        // Guard against files added to / removed from the directory on disk without
        // a matching provenance entry (drift the embedded set alone cannot catch).
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/libraries/icons/lucide");
        let mut on_disk = BTreeSet::new();
        for entry in std::fs::read_dir(&dir).expect("read lucide icon dir") {
            let path = entry.expect("dir entry").path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("svg") {
                let stem = path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .expect("svg file stem")
                    .to_owned();
                on_disk.insert(stem);
            }
        }
        let recorded_owned: BTreeSet<String> = provenance
            .icons
            .iter()
            .map(|icon| icon.name.clone())
            .collect();
        assert_eq!(
            recorded_owned, on_disk,
            "provenance entries must match the .svg files on disk one-to-one"
        );
    }

    #[test]
    fn provenance_sha256_matches_svg_bytes() {
        let provenance = parse_provenance();
        for icon in &provenance.icons {
            let embedded = LUCIDE_ICONS
                .iter()
                .find(|candidate| candidate.id == icon.name)
                .unwrap_or_else(|| panic!("embedded bytes for '{}'", icon.name));
            let actual = format!("{:x}", Sha256::digest(embedded.bytes));
            assert_eq!(
                actual, icon.sha256,
                "sha256 drift for Lucide icon '{}'",
                icon.name
            );
        }
    }

    #[test]
    fn provenance_alias_mappings_are_recorded() {
        let provenance = parse_provenance();
        let upstream_of = |name: &str| {
            provenance
                .icons
                .iter()
                .find(|icon| icon.name == name)
                .map(|icon| icon.upstream_name.as_str())
                .unwrap_or_else(|| panic!("provenance entry for '{name}'"))
        };
        assert_eq!(upstream_of("sync"), "refresh-cw");
        assert_eq!(upstream_of("upload-cloud"), "cloud-upload");
        assert_eq!(upstream_of("download-cloud"), "cloud-download");

        // Every non-alias icon maps to its own upstream name.
        for icon in &provenance.icons {
            let is_alias = matches!(
                icon.name.as_str(),
                "sync" | "upload-cloud" | "download-cloud"
            );
            if !is_alias {
                assert_eq!(
                    icon.name, icon.upstream_name,
                    "non-alias icon '{}' must map to itself upstream",
                    icon.name
                );
            }
        }
    }

    #[test]
    fn provenance_matches_generated_pack_components() {
        let provenance = parse_provenance();
        let generated = generate_pack_source().expect("generate native Lucide pack");
        let doc = KdlAdapter
            .parse(generated.as_bytes())
            .expect("generated pack parses");

        let components: BTreeSet<&str> = doc
            .components
            .iter()
            .map(|component| component.id.as_str())
            .collect();
        let recorded: BTreeSet<&str> = provenance
            .icons
            .iter()
            .map(|icon| icon.name.as_str())
            .collect();
        assert_eq!(
            recorded, components,
            "provenance must cover exactly the components shipped in the pack"
        );
    }

    #[test]
    fn generated_lucide_pack_matches_committed_source() {
        let generated = generate_pack_source().expect("generate native Lucide pack");
        if std::env::var_os("ZENITH_UPDATE_LUCIDE_PACK").is_some() {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("assets/libraries/zenith-icons-lucide.zen");
            std::fs::write(path, generated).expect("write generated Lucide pack");
            return;
        }
        assert_eq!(generated, COMMITTED_PACK_SOURCE);
    }

    #[test]
    fn generated_lucide_pack_uses_native_paths() {
        let generated = generate_pack_source().expect("generate native Lucide pack");
        let doc = KdlAdapter
            .parse(generated.as_bytes())
            .expect("generated pack parses");
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
            "native icon components must contain editable path nodes"
        );
        assert!(monitor.children.len() >= 2);

        let report = validate(&doc);
        let errors: Vec<_> = report
            .diagnostics
            .into_iter()
            .filter(|diagnostic| diagnostic.severity == zenith_core::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "generated pack errors: {errors:?}");
    }
}
