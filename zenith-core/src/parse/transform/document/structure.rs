//! Leaf-marker structural blocks: masters, sections, libraries, actions, and
//! provenance. Each block holds a list of simple declaration nodes.

use kdl::{KdlNode, KdlValue};

use crate::ast::action::ActionDef;
use crate::ast::document::{MasterDef, SectionDef};
use crate::ast::library::LibraryDef;
use crate::ast::provenance::ProvenanceDef;
use crate::error::{ParseError, ParseErrorCode};
use crate::parse::transform::helpers::{
    collect_unknown_props, node_span, optional_bool_prop, optional_string_prop,
    optional_string_prop_aliased, optional_u32_prop, required_string_prop,
    required_string_prop_aliased,
};

use super::body::transform_children;

// ---------------------------------------------------------------------------
// Masters
// ---------------------------------------------------------------------------

/// Transform the document-level `masters { … }` block into a list of
/// [`MasterDef`]. Each `master id="..." { <child nodes> }` becomes one
/// definition whose children are parsed exactly like page/group children (via
/// [`crate::parse::transform::node::transform_node`]). Non-`master` children
/// inside the block are silently ignored (forward-compat). Mirrors
/// [`transform_components`](super::components::transform_components).
pub(super) fn transform_masters(node: &KdlNode) -> Result<Vec<MasterDef>, ParseError> {
    let mut defs: Vec<MasterDef> = Vec::new();
    if let Some(children) = node.children() {
        for child in children.nodes() {
            if child.name().value() == "master" {
                defs.push(transform_master_def(child)?);
            }
        }
    }
    Ok(defs)
}

fn transform_master_def(node: &KdlNode) -> Result<MasterDef, ParseError> {
    let id = required_string_prop(node, "id")?.to_owned();
    let children = transform_children(node)?;
    Ok(MasterDef {
        id,
        children,
        source_span: node_span(node),
    })
}

// ---------------------------------------------------------------------------
// Sections
// ---------------------------------------------------------------------------

/// Transform the document-level `sections { … }` block into a list of
/// [`SectionDef`]. Each `section id="…" name="…" start-page="…" …` is a leaf
/// marker (it takes no children); non-`section` children inside the block are
/// silently ignored (forward-compat). Mirrors [`transform_masters`].
pub(super) fn transform_sections(node: &KdlNode) -> Result<Vec<SectionDef>, ParseError> {
    let mut defs: Vec<SectionDef> = Vec::new();
    if let Some(children) = node.children() {
        for child in children.nodes() {
            if child.name().value() == "section" {
                defs.push(transform_section_def(child)?);
            }
        }
    }
    Ok(defs)
}

fn transform_section_def(node: &KdlNode) -> Result<SectionDef, ParseError> {
    let id = required_string_prop(node, "id")?.to_owned();
    let name = required_string_prop(node, "name")?.to_owned();
    let start_page = required_string_prop_aliased(node, "start-page", "start_page")?.to_owned();

    // `folio-start` / `folio_start`: optional non-negative integer.
    // `optional_u32_prop` silently drops negative or non-integer values, which
    // is the same forward-compat posture used for other optional integer props
    // (e.g. `tab-width`, `page-parity-start`).
    let folio_start = optional_u32_prop(node, "folio-start")
        .or_else(|| optional_u32_prop(node, "folio_start"))
        .map(|n| n as usize);

    // `folio-style` / `folio_style`: optional string.
    let folio_style =
        optional_string_prop_aliased(node, "folio-style", "folio_style").map(str::to_owned);

    Ok(SectionDef {
        id,
        name,
        folio_start,
        folio_style,
        start_page,
        source_span: node_span(node),
    })
}

// ---------------------------------------------------------------------------
// Libraries
// ---------------------------------------------------------------------------

const LIBRARY_KNOWN_PROPS: &[&str] = &["id", "version", "hash"];

/// Transform the document-level `libraries { … }` block into a list of
/// [`LibraryDef`]. Each `library id="…" version="…" hash="…"` is a leaf marker
/// (it takes no children); non-`library` children inside the block are silently
/// ignored (forward-compat). Mirrors [`transform_sections`].
pub(super) fn transform_libraries(node: &KdlNode) -> Result<Vec<LibraryDef>, ParseError> {
    let mut defs: Vec<LibraryDef> = Vec::new();
    if let Some(children) = node.children() {
        for child in children.nodes() {
            if child.name().value() == "library" {
                defs.push(transform_library_def(child)?);
            }
        }
    }
    Ok(defs)
}

fn transform_library_def(node: &KdlNode) -> Result<LibraryDef, ParseError> {
    let id = required_string_prop(node, "id")?.to_owned();
    let version = optional_string_prop(node, "version").map(str::to_owned);
    let hash = optional_string_prop(node, "hash").map(str::to_owned);
    let unknown_props = collect_unknown_props(node, LIBRARY_KNOWN_PROPS);
    let source_span = node_span(node);

    Ok(LibraryDef {
        id,
        version,
        hash,
        source_span,
        unknown_props,
    })
}

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

const ACTION_KNOWN_PROPS: &[&str] = &["id", "label", "version"];

/// Transform the document-level `actions { … }` block into a list of
/// [`ActionDef`]. Each `action id="…" label="…" version="…" { tx "…" }` is a
/// block node whose `tx` child carries the opaque JSON payload as a positional
/// string argument; non-`action` children inside the block are silently ignored
/// (forward-compat). Mirrors [`transform_libraries`].
pub(super) fn transform_actions(node: &KdlNode) -> Result<Vec<ActionDef>, ParseError> {
    let mut defs: Vec<ActionDef> = Vec::new();
    if let Some(children) = node.children() {
        for child in children.nodes() {
            if child.name().value() == "action" {
                defs.push(transform_action_def(child)?);
            }
        }
    }
    Ok(defs)
}

fn transform_action_def(node: &KdlNode) -> Result<ActionDef, ParseError> {
    let id = required_string_prop(node, "id")?.to_owned();
    let label = optional_string_prop(node, "label").map(str::to_owned);
    let version = optional_string_prop(node, "version").map(str::to_owned);
    let unknown_props = collect_unknown_props(node, ACTION_KNOWN_PROPS);
    let source_span = node_span(node);

    // The `tx_json` payload lives in a `tx` child node whose first positional
    // argument is the decoded string. Exactly like `content` in CodeNode, the
    // value is stored decoded; the writer re-encodes it on format.
    let tx_json = node
        .children()
        .and_then(|doc| {
            doc.nodes().iter().find_map(|child| {
                if child.name().value() != "tx" {
                    return None;
                }
                child.get(0).and_then(|v| match v {
                    KdlValue::String(s) => Some(s.clone()),
                    _ => None,
                })
            })
        })
        .ok_or_else(|| {
            ParseError::spanless(
                ParseErrorCode::InvalidPropertyValue,
                format!("node `action` id=\"{id}\" is missing required `tx` child node"),
            )
        })?;

    Ok(ActionDef {
        id,
        label,
        version,
        tx_json,
        source_span,
        unknown_props,
    })
}

// ---------------------------------------------------------------------------
// Provenance
// ---------------------------------------------------------------------------

const PROVENANCE_KNOWN_PROPS: &[&str] = &["id", "node", "library", "item", "linked"];

/// Transform the document-level `provenance { … }` block into a list of
/// [`ProvenanceDef`]. Each `origin id="…" node="…" library="…" …` is a leaf
/// marker (it takes no children); non-`origin` children inside the block are
/// silently ignored (forward-compat). Mirrors [`transform_libraries`].
pub(super) fn transform_provenance(node: &KdlNode) -> Result<Vec<ProvenanceDef>, ParseError> {
    let mut defs: Vec<ProvenanceDef> = Vec::new();
    if let Some(children) = node.children() {
        for child in children.nodes() {
            if child.name().value() == "origin" {
                defs.push(transform_provenance_def(child)?);
            }
        }
    }
    Ok(defs)
}

fn transform_provenance_def(node: &KdlNode) -> Result<ProvenanceDef, ParseError> {
    let id = required_string_prop(node, "id")?.to_owned();
    let document_node = required_string_prop(node, "node")?.to_owned();
    let library = required_string_prop(node, "library")?.to_owned();
    let item = optional_string_prop(node, "item").map(str::to_owned);
    let linked = optional_bool_prop(node, "linked");
    let unknown_props = collect_unknown_props(node, PROVENANCE_KNOWN_PROPS);
    let source_span = node_span(node);

    Ok(ProvenanceDef {
        id,
        node: document_node,
        library,
        item,
        linked,
        source_span,
        unknown_props,
    })
}
