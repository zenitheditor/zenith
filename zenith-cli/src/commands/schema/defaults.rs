//! The `zenith schema defaults` surface: the per-kind default-style block.

use zenith_core::ast::{DEFAULTS_ENTRY_PROPS, DEFAULTS_UNSUPPORTED_KINDS, DefaultsKind};

use crate::commands::serialize_pretty;

/// One-line summary of the surface.
const SUMMARY: &str = "Per-node-kind default styles — the style a node takes when it sets no \
    `style` of its own.";

/// Where the block may appear.
const PLACEMENT: &[(&str, &str)] = &[
    (
        "document",
        "top-level `defaults { … }` inside `zenith`, written right after `styles`",
    ),
    ("page", "`defaults { … }` at the start of a `page` body"),
];

/// Row shape and attribute meanings.
const ENTRY: &str = "<kind> style=\"<style id>\" [text-style=\"<style id>\"]";
const ENTRY_ATTRS: &[(&str, &str)] = &[
    (
        "style",
        "required; a declared style id, the kind's default `style`",
    ),
    (
        "text-style",
        "optional; a declared style id, the default label style (connector and shape only)",
    ),
];

/// How a default combines with node, page, and document styles.
const CASCADE: &str = "A node's own `style` wins. Without one, the node takes its kind's \
    page `defaults` row, else the document `defaults` row. Node attributes still override every \
    style key. Each kind appears at most once per block.";

/// Diagnostics the block can raise.
const DIAGNOSTICS: &[(&str, &str)] = &[
    ("defaults.unknown_kind", "row names an unknown node kind"),
    (
        "defaults.unsupported_kind",
        "row names a kind that carries no style (instance, light, mesh)",
    ),
    ("defaults.duplicate_kind", "kind repeated in one block"),
    (
        "defaults.unknown_style",
        "style or text-style id is not declared",
    ),
    (
        "defaults.text_style_unsupported",
        "text-style on a kind without a label",
    ),
    (
        "defaults.unknown_property",
        "row attribute other than style / text-style",
    ),
];

const EXAMPLE: &str = concat!(
    "styles {\n",
    "  style id=\"body\" { font-size (token)\"size.body\"; fill (token)\"color.text\" }\n",
    "  style id=\"box\" { fill (token)\"color.surface\"; shadow (token)\"shadow.card\" }\n",
    "  style id=\"box.label\" { align \"center\"; fill (token)\"color.text\" }\n",
    "}\n",
    "defaults {\n",
    "  shape style=\"box\" text-style=\"box.label\"\n",
    "  text style=\"body\"\n",
    "}\n",
);

/// `zenith schema defaults`: placement, entries, cascade, and an example.
///
/// Returns `(stdout, exit_code)`.
pub fn defaults(json: bool) -> (String, u8) {
    let kinds = DefaultsKind::names();
    let text_style_kinds: Vec<&str> = DefaultsKind::ALL
        .iter()
        .filter(|k| k.accepts_text_style())
        .map(|k| k.name())
        .collect();
    if json {
        let out = serde_json::json!({
            "schema": "zenith-schema-v1",
            "surface": "defaults",
            "summary": SUMMARY,
            "placement": PLACEMENT
                .iter()
                .map(|(scope, desc)| serde_json::json!({"scope": scope, "description": desc}))
                .collect::<Vec<_>>(),
            "entry": ENTRY,
            "entry_attributes": DEFAULTS_ENTRY_PROPS,
            "kinds": kinds,
            "text_style_kinds": text_style_kinds,
            "unsupported_kinds": DEFAULTS_UNSUPPORTED_KINDS,
            "cascade": CASCADE,
            "diagnostics": DIAGNOSTICS
                .iter()
                .map(|(code, desc)| serde_json::json!({"code": code, "description": desc}))
                .collect::<Vec<_>>(),
            "example": EXAMPLE,
        });
        return (serialize_pretty(&out), 0);
    }
    let mut text = format!("defaults: {SUMMARY}\n\nPlacement:\n");
    for (scope, desc) in PLACEMENT {
        text.push_str(&format!("  {scope:<9}  {desc}\n"));
    }
    text.push_str(&format!("\nEntry:\n  {ENTRY}\n"));
    for (name, desc) in ENTRY_ATTRS {
        text.push_str(&format!("  {name:<10}  {desc}\n"));
    }
    text.push_str(&format!("\nKinds:\n  {}\n", kinds.join(", ")));
    text.push_str(&format!(
        "  text-style kinds: {}\n  no default: {}\n",
        text_style_kinds.join(", "),
        DEFAULTS_UNSUPPORTED_KINDS.join(", ")
    ));
    text.push_str(&format!("\nCascade:\n  {CASCADE}\n"));
    text.push_str("\nDiagnostics:\n");
    for (code, desc) in DIAGNOSTICS {
        text.push_str(&format!("  {code:<32}  {desc}\n"));
    }
    text.push_str(&format!(
        "\nExample:\n  {}",
        EXAMPLE.trim_end().replace('\n', "\n  ")
    ));
    (text, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_lists_kinds_placement_and_example() {
        let (out, code) = defaults(true);
        assert_eq!(code, 0);
        let v: serde_json::Value = serde_json::from_str(&out).expect("json");
        assert_eq!(v["surface"], "defaults");
        let kinds: Vec<&str> = v["kinds"]
            .as_array()
            .expect("kinds")
            .iter()
            .filter_map(|k| k.as_str())
            .collect();
        assert_eq!(kinds, DefaultsKind::names());
        assert!(!kinds.contains(&"instance"));
        assert_eq!(
            v["text_style_kinds"],
            serde_json::json!(["connector", "shape"])
        );
        assert_eq!(
            v["unsupported_kinds"],
            serde_json::json!(["instance", "light", "mesh"])
        );
        assert_eq!(v["placement"].as_array().map(Vec::len), Some(2));
        assert!(v["cascade"].as_str().is_some_and(|c| c.contains("page")));
        assert!(
            v["example"]
                .as_str()
                .is_some_and(|e| e.contains("defaults {"))
        );
    }

    #[test]
    fn diagnostics_listed_match_catalog() {
        for (code, _) in DIAGNOSTICS {
            assert!(zenith_core::diag_catalog::lookup(code).is_some(), "{code}");
        }
    }

    #[test]
    fn example_parses_and_validates_clean() {
        let src = format!(
            r##"zenith version=1 {{
  tokens format="zenith-token-v1" {{
    token id="size.body" type="dimension" value=(px)16
    token id="color.text" type="color" value="#111111"
    token id="color.surface" type="color" value="#eeeeee"
    token id="shadow.card" type="shadow" {{
      layer dx=(px)0 dy=(px)2 blur=(px)6 color=(token)"color.text"
    }}
  }}
  {EXAMPLE}
  document id="d" {{
    page id="p" w=(px)100 h=(px)100 {{
    }}
  }}
}}
"##
        );
        use zenith_core::KdlSource;
        let doc = zenith_core::KdlAdapter
            .parse(src.as_bytes())
            .expect("example parses");
        assert_eq!(doc.defaults.entries.len(), 2);
        let report = zenith_core::validate(&doc);
        assert!(
            !report
                .diagnostics
                .iter()
                .any(|d| d.code.starts_with("defaults.") || d.code.starts_with("style.")),
            "{:?}",
            report.diagnostics
        );
    }

    #[test]
    fn text_output_names_sections() {
        let (out, code) = defaults(false);
        assert_eq!(code, 0);
        for needle in [
            "Placement:",
            "Entry:",
            "Kinds:",
            "Cascade:",
            "Diagnostics:",
            "Example:",
        ] {
            assert!(out.contains(needle), "{needle}\n{out}");
        }
    }
}
