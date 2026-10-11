//! `zenith schema write`: everything an agent needs to write a document, in
//! one read. A worked example, the layout rule, and the common attributes of
//! the common kinds. Types and defaults come from the live schema, so the
//! tables cannot drift. A test in `tests/schema_write.rs` validates and
//! renders the example.

use zenith_core::schema as core_schema;

use crate::commands::serialize_pretty;
use crate::json_types::{SchemaAttr, SchemaWriteKind, SchemaWriteOutput};

use super::common::format_attr_table;

/// A complete document that validates clean.
pub const EXAMPLE: &str = r##"zenith version=1 {
  project id="proj.demo" name="Demo"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#f7f8fa"
    token id="color.ink" type="color" value="#101828"
    token id="color.accent" type="color" value="#135bf9"
    token id="font.body" type="fontFamily" value="Noto Sans"
    token id="weight.bold" type="fontWeight" value=700
    token id="size.title" type="dimension" value=(px)64
    token id="size.body" type="dimension" value=(px)28
    token id="space.md" type="dimension" value=(px)24
  }
  styles {
  }
  document id="doc.demo" title="Demo" {
    page id="page.1" w=(px)800 h=(px)600 background=(token)"color.bg" {
      frame id="body" x=(px)48 y=(px)48 w=(px)704 h=(px)504 layout="column" gap=(token)"space.md" {
        rect id="bar" w="fill" h=(px)8 fill=(token)"color.accent"
        text id="title" w="fill" font-family=(token)"font.body" font-size=(token)"size.title" font-weight=(token)"weight.bold" fill=(token)"color.ink" { span "Neighborhood Cleanup" }
        text id="note" w="fill" font-family=(token)"font.body" font-size=(token)"size.body" fill=(token)"color.ink" { span "Saturday, March 14" }
      }
    }
  }
}"##;

/// A kind and the attributes worth knowing first.
struct Entry {
    kind: &'static str,
    attrs: &'static [&'static str],
}

const ENTRIES: &[Entry] = &[
    Entry {
        kind: "page",
        attrs: &["id", "w", "h", "background"],
    },
    Entry {
        kind: "frame",
        attrs: &[
            "id",
            "x",
            "y",
            "w",
            "h",
            "layout",
            "gap",
            "padding",
            "align",
            "justify",
            "wrap",
            "columns",
            "fill",
            "stroke",
            "stroke-width",
            "radius",
            "clip",
            "margin",
        ],
    },
    Entry {
        kind: "text",
        attrs: &[
            "id",
            "x",
            "y",
            "w",
            "h",
            "font-family",
            "font-size",
            "font-weight",
            "fill",
            "align",
            "v-align",
            "bullet",
            "overflow",
            "margin",
            "style",
        ],
    },
    Entry {
        kind: "rect",
        attrs: &[
            "id",
            "x",
            "y",
            "w",
            "h",
            "fill",
            "stroke",
            "stroke-width",
            "radius",
            "margin",
        ],
    },
    Entry {
        kind: "ellipse",
        attrs: &["id", "x", "y", "w", "h", "fill", "stroke", "stroke-width"],
    },
    Entry {
        kind: "line",
        attrs: &["id", "x1", "y1", "x2", "y2", "stroke", "stroke-width"],
    },
    Entry {
        kind: "image",
        attrs: &["id", "x", "y", "w", "h", "asset", "fit"],
    },
];

const RULES: &str = "\
Rules:
  - Every node has a unique id. Text goes in `span \"...\"` children of a text node.
  - Colors, fonts, and sizes come from tokens: write (token)\"color.ink\". A plain size is (px)16.
  - A frame with layout=\"column\", \"row\", or \"grid\" stacks its children. Give those children w/h (px, a token, \"fill\", or \"hug\") and no x/y.
  - Outside a layout frame, a node takes x and y from its parent's top-left corner.
  - Keep text inside its box: a long line needs a wider w or a smaller font-size. Check with `zenith validate` and `zenith render <file> --png out.png`.";

fn attrs_of(entry: &Entry) -> Vec<SchemaAttr> {
    let known: Vec<&str> = if entry.kind == "page" {
        core_schema::page_attributes()
    } else {
        core_schema::node_attributes(entry.kind)
    };
    entry
        .attrs
        .iter()
        .filter(|name| known.contains(name))
        .map(|&name| SchemaAttr {
            name: name.to_owned(),
            ty: core_schema::attribute_type_for_kind(entry.kind, name).to_owned(),
            default: core_schema::attribute_default(entry.kind, name).map(str::to_owned),
        })
        .collect()
}

/// `zenith schema write`.
///
/// Returns `(stdout, exit_code)`.
pub fn write(json: bool) -> (String, u8) {
    let kinds: Vec<SchemaWriteKind> = ENTRIES
        .iter()
        .map(|entry| SchemaWriteKind {
            kind: entry.kind.to_owned(),
            attributes: attrs_of(entry),
        })
        .collect();
    if json {
        let out = SchemaWriteOutput {
            schema: "zenith-schema-v1",
            example: EXAMPLE.to_owned(),
            rules: RULES.to_owned(),
            kinds,
        };
        return (serialize_pretty(&out), 0);
    }
    let mut text = String::from(
        "Write a .zen document. Start with `zenith new <path> --starter <kind>` (see `zenith new --help`) and change it, or write one like this:\n\n---\n",
    );
    text.push_str(EXAMPLE);
    text.push_str("\n---\n\n");
    text.push_str(RULES);
    text.push_str("\n\nCommon attributes (`zenith schema node <kind> --notes` lists all):\n");
    for kind in &kinds {
        text.push_str(&format!("\n{}:\n", kind.kind));
        text.push_str(&format_attr_table(&kind.attributes));
    }
    text.push_str(
        "\nOther kinds: `zenith schema nodes`. Tokens: `zenith tokens <file>` lists a file's tokens. Edit a file with `zenith tx <file> --apply --ops '[...]'` (`zenith schema ops`).",
    );
    (text, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_attribute_exists_in_the_schema() {
        for entry in ENTRIES {
            assert_eq!(
                attrs_of(entry).len(),
                entry.attrs.len(),
                "{} lists an attribute the schema does not know",
                entry.kind
            );
        }
    }

    #[test]
    fn the_example_parses() {
        use zenith_core::{KdlAdapter, KdlSource as _};
        assert!(KdlAdapter.parse(EXAMPLE.as_bytes()).is_ok());
    }
}
