//! Synthesizing a `.zen` pack document from an SVG icon library.
//!
//! An SVG library holds no Zenith geometry — only SVG. This module converts it
//! on demand: each requested icon becomes a `component` of native `path` nodes
//! (via [`svg_to_native_paths`]), stroked with the pack's two tokens, so the
//! result is an ordinary pack [`Document`] that `show` / `add` consume without
//! knowing the pack's format.
//!
//! Conversion is SCOPED. Materializing one icon out of a 1745-icon library must
//! convert exactly one icon, so callers pass [`ItemScope::Only`]; whole-library
//! conversion ([`ItemScope::All`]) exists for tests and small libraries.
//!
//! This is why no `.zen` icon pack is committed: the vendored SVGs are the only
//! source of truth, and the pack document is derived from them deterministically.

use zenith_core::{
    AnchorKind, Dimension, KdlAdapter, KdlSource, Node, PathAnchor, PathNode, PathSubpath,
    PropertyValue, format::format_document,
};
use zenith_producers::{SvgNativeOptions, svg_to_native_paths};

use super::load::{STROKE_TOKEN, STROKE_WIDTH_TOKEN, SvgIcon, SvgLibrary};

/// Which icons of a library to convert into components.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemScope<'a> {
    /// Convert every icon. Cost is linear in library size.
    All,
    /// Convert only the icon with this id, if it exists. An id that names no
    /// icon (a token id, say) yields a pack with no components.
    Only(&'a str),
}

impl ItemScope<'_> {
    /// Whether `icon` is inside this scope.
    fn admits(&self, icon: &SvgIcon) -> bool {
        match self {
            ItemScope::All => true,
            ItemScope::Only(id) => icon.name == *id,
        }
    }
}

/// Escape a string for a KDL double-quoted literal.
fn esc(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Generate the canonical `.zen` pack source for `lib`, converting the icons
/// admitted by `scope` into components.
///
/// The result declares the library self-entry (so [`super::super::parse_pack`]
/// recovers the same identity), the two icon tokens, the components, and an
/// empty preview document.
///
/// # Errors
///
/// Returns a message when an icon fails to convert, converts to nothing, or
/// yields a non-path node; or when the generated source fails to parse/format.
pub fn synthesize_pack_source(lib: &SvgLibrary, scope: ItemScope<'_>) -> Result<String, String> {
    let id = esc(&lib.id);

    let mut out = String::new();
    out.push_str("zenith version=1 {\n");
    out.push_str(&format!("  project id=\"{id}\" name=\"{id}\"\n"));
    out.push_str("  libraries {\n");
    match &lib.version {
        Some(v) => out.push_str(&format!("    library id=\"{id}\" version=\"{}\"\n", esc(v))),
        None => out.push_str(&format!("    library id=\"{id}\"\n")),
    }
    out.push_str("  }\n");
    out.push_str("  tokens format=\"zenith-token-v1\" {\n");
    out.push_str(&format!(
        "    token id=\"{STROKE_TOKEN}\" type=\"color\" value=\"#111827\"\n"
    ));
    out.push_str(&format!(
        "    token id=\"{STROKE_WIDTH_TOKEN}\" type=\"dimension\" value=(px)2\n"
    ));
    out.push_str("  }\n");
    out.push_str("  components {\n");
    for icon in lib.icons.iter().filter(|i| scope.admits(i)) {
        write_icon_component(&mut out, icon)?;
    }
    out.push_str("  }\n");
    out.push_str("  document id=\"pack.preview\" title=\"Icon pack preview\" {\n");
    out.push_str("    page id=\"pack.pg\" name=\"Preview\" w=(px)100 h=(px)100 {\n");
    out.push_str("    }\n");
    out.push_str("  }\n");
    out.push_str("}\n");

    canonicalize_pack(&out)
}

fn write_icon_component(out: &mut String, icon: &SvgIcon) -> Result<(), String> {
    let options = SvgNativeOptions {
        id_prefix: "icon".to_owned(),
        stroke: Some(PropertyValue::TokenRef(STROKE_TOKEN.to_owned())),
        fill: None,
        stroke_width: Some(PropertyValue::TokenRef(STROKE_WIDTH_TOKEN.to_owned())),
    };
    let nodes = svg_to_native_paths(icon.svg.as_bytes(), &options)
        .map_err(|err| format!("failed to convert SVG icon '{}': {err}", icon.name))?;
    if nodes.is_empty() {
        return Err(format!(
            "SVG icon '{}' converted to no path nodes",
            icon.name
        ));
    }

    out.push_str("    component id=\"");
    out.push_str(&esc(&icon.name));
    out.push_str("\" {\n");
    for node in nodes {
        let Node::Path(path) = node else {
            return Err(format!("SVG icon '{}' produced a non-path node", icon.name));
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
        .map_err(|err| format!("generated icon pack failed to parse: {err}"))?;
    let formatted = format_document(&doc)
        .map_err(|err| format!("generated icon pack failed to format: {err}"))?;
    String::from_utf8(formatted).map_err(|err| format!("formatted icon pack was not UTF-8: {err}"))
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
    use super::super::load::embedded_svg_libraries;
    use super::*;

    fn lucide() -> SvgLibrary {
        embedded_svg_libraries()
            .into_iter()
            .find(|l| l.id == "@zenith/icons-lucide")
            .expect("lucide is bundled")
    }

    #[test]
    fn scoped_synthesis_emits_exactly_one_component() {
        let lib = lucide();
        let source = synthesize_pack_source(&lib, ItemScope::Only("house")).expect("synthesizes");
        assert_eq!(source.matches("component id=").count(), 1);
        assert!(source.contains("component id=\"house\""));
    }

    #[test]
    fn scope_of_a_non_icon_id_yields_no_components() {
        let lib = lucide();
        let source =
            synthesize_pack_source(&lib, ItemScope::Only(STROKE_TOKEN)).expect("synthesizes");
        assert_eq!(source.matches("component id=").count(), 0);
        // The tokens still ship, so a token item is materializable.
        assert!(source.contains(STROKE_TOKEN));
    }

    #[test]
    fn synthesized_pack_declares_its_identity_and_version() {
        let lib = lucide();
        let source = synthesize_pack_source(&lib, ItemScope::Only("box")).expect("synthesizes");
        assert!(source.contains("library id=\"@zenith/icons-lucide\" version=\"1.23.0\""));
    }

    #[test]
    fn synthesis_is_deterministic() {
        let lib = lucide();
        let a = synthesize_pack_source(&lib, ItemScope::Only("cpu")).expect("a");
        let b = synthesize_pack_source(&lib, ItemScope::Only("cpu")).expect("b");
        assert_eq!(a, b);
    }

    #[test]
    fn every_bundled_icon_converts() {
        // The whole point of dropping the committed `.zen` pack: conversion must
        // succeed for every vendored icon, or the library is not usable.
        for lib in embedded_svg_libraries() {
            for icon in &lib.icons {
                let mut out = String::new();
                write_icon_component(&mut out, icon)
                    .unwrap_or_else(|e| panic!("{}#{} must convert: {e}", lib.id, icon.name));
            }
        }
    }
}
