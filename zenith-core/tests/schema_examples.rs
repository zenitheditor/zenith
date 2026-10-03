//! Every example the `zenith schema` surfaces print must parse with the real
//! parser. A schema example that fails to parse teaches agents broken syntax.

use zenith_core::schema::{
    node_content, node_example, node_kinds, ports_descriptor, variant_descriptor,
};
use zenith_core::{Document, KdlAdapter, KdlSource};

/// Parse `source`, panicking with the example text when the parse fails.
fn parse_example(label: &str, example: &str, source: &str) -> Document {
    match KdlAdapter.parse(source.as_bytes()) {
        Ok(doc) => doc,
        Err(e) => panic!(
            "{label} example failed to parse: {e}\nexample:\n{example}\nwrapped doc:\n{source}"
        ),
    }
}

/// Wrap page children in a minimal document.
fn page_doc(children: &str) -> String {
    format!(
        "zenith version=1 {{\n  document id=\"doc\" {{\n    page id=\"pg\" w=(px)800 h=(px)600 {{\n{children}\n    }}\n  }}\n}}\n"
    )
}

#[test]
fn node_content_examples_parse() {
    for kind in node_kinds() {
        let Some(content) = node_content(kind) else {
            continue;
        };
        // Provide every required property any kind needs. Extra properties on a
        // kind that ignores them are collected as unknown props, not errors.
        let source = page_doc(&format!(
            "{kind} id=\"k\" kind=\"bar\" type=\"text\" asset=\"a\" {{\n{}\n}}",
            content.example
        ));
        let doc = parse_example(&format!("node_content({kind})"), content.example, &source);
        assert!(
            doc.unsupported_children.is_empty(),
            "node_content({kind}) example has children the parser drops: {:?}\nexample:\n{}",
            doc.unsupported_children,
            content.example,
        );
    }
}

#[test]
fn node_full_examples_parse() {
    for kind in node_kinds() {
        let Some(example) = node_example(kind) else {
            continue;
        };
        let source = page_doc(example);
        let doc = parse_example(&format!("node_example({kind})"), example, &source);
        assert!(
            doc.unsupported_children.is_empty(),
            "node_example({kind}) has children the parser drops: {:?}",
            doc.unsupported_children,
        );
    }
}

#[test]
fn variants_example_parses() {
    let example = variant_descriptor().example;
    let source = format!(
        "zenith version=1 {{\n{example}\n  document id=\"doc\" {{\n    page id=\"pg\" w=(px)800 h=(px)600 {{}}\n  }}\n}}\n"
    );
    parse_example("variant_descriptor", example, &source);
}

#[test]
fn ports_example_parses() {
    let example = ports_descriptor().example;
    let source = format!("zenith version=1 {{\n  document id=\"doc\" {{\n{example}\n  }}\n}}\n");
    parse_example("ports_descriptor", example, &source);
}

/// The table example must carry an `id` on every cell node, since a bare
/// `text` is a parse error.
#[test]
fn table_example_cell_text_has_ids() {
    let Some(content) = node_content("table") else {
        panic!("table has no content descriptor");
    };
    assert!(
        !content.example.contains("text {"),
        "table example shows an id-less `text`:\n{}",
        content.example
    );
}
