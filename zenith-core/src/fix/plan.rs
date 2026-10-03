//! Plan machine fixes from the structured [`FixHint`] on each diagnostic.
//!
//! Validation decides what is fixable and sets the hint; planning only maps a
//! hint to property sites in the KDL source. Planning reads only;
//! [`super::apply`] writes. Fix kinds:
//!
//! - [`FixHint::RawLiteral`] → reference `exact_match`, else a minted token.
//! - [`FixHint::RenameProperty`] → rename, when the node does not already set
//!   the target.
//! - [`FixHint::ReplaceTokenRef`] → swap the token id.
//! - [`FixHint::ReplaceValue`] → swap the enum value. On a `defaults` row it
//!   swaps the kind (node name) or a style id; on a `style` block it swaps the
//!   value of the property child. Row diagnostics are located by span.

use std::collections::BTreeSet;

use kdl::{KdlDocument, KdlEntry, KdlValue};

use crate::ast::canonicalize_style_key;
use crate::ast::value::{Dimension, Unit};
use crate::diagnostics::{Diagnostic, FixHint};
use crate::suggest::LiteralValue;

use super::locate::{
    NodePath, Site, Slot, annotation, child_value_site, entry_at, find_node, find_node_by_span,
    node_at, property_sites, quote, scalar_text, value_text,
};
use super::mint::{MintedToken, Minter};

/// One fix `zenith fix` applied.
#[derive(Debug, Clone, PartialEq)]
pub struct AppliedFix {
    /// The diagnostic code the fix resolves.
    pub code: String,
    /// The diagnostic subject (a node, page, or asset id; empty for a
    /// document-scope `defaults` row).
    pub subject_id: String,
    /// The property name as the source had it.
    pub property: String,
    /// The old text: a property name for a rename, else the source value.
    pub from: String,
    /// The new text, in the same form as `from`.
    pub to: String,
}

/// The edit for one site.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Change {
    /// Replace the value with `(token)"<id>"`.
    TokenRef(String),
    /// Replace the string value, keeping any annotation.
    Text(String),
    /// Rename the property.
    Rename(String),
    /// Rename the node (a `defaults` row kind).
    NodeName(String),
}

/// A planned edit plus the record it produces.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PlannedFix {
    pub site: Site,
    pub change: Change,
    pub record: AppliedFix,
}

/// Everything one planning pass produces.
pub(super) struct Plan {
    pub fixes: Vec<PlannedFix>,
    pub minted: Vec<MintedToken>,
}

/// Plan fixes for the hinted `diagnostics` against the KDL source `doc`.
///
/// `declared` holds every token id the document declares (resolved or not),
/// so minted ids never collide. Diagnostics are visited in order; a site is
/// fixed at most once.
pub(super) fn plan(
    doc: &KdlDocument,
    diagnostics: &[Diagnostic],
    declared: BTreeSet<String>,
) -> Plan {
    let mut minter = Minter::new(declared);
    let mut fixes: Vec<PlannedFix> = Vec::new();
    let mut taken: BTreeSet<Site> = BTreeSet::new();
    let mut renamed: BTreeSet<(NodePath, String)> = BTreeSet::new();

    for d in diagnostics {
        let Some(hint) = d.fix() else {
            continue;
        };
        let Some(path) = subject_path(doc, d) else {
            continue;
        };
        let planned = match hint {
            FixHint::RawLiteral {
                property,
                literal,
                token_type,
                exact_match,
                nearest: _,
            } => {
                let raw = RawLiteral {
                    property,
                    literal,
                    token_type,
                    exact_match: exact_match.as_deref(),
                };
                plan_raw_literal(doc, d, &path, &raw, &mut minter)
            }
            FixHint::ReplaceTokenRef { property, from, to } => {
                plan_token_ref(doc, d, &path, property, from, to)
            }
            FixHint::RenameProperty { from, to } => {
                plan_rename(doc, d, &path, from, to, &mut renamed)
            }
            FixHint::ReplaceValue { property, from, to } => match d.code.as_str() {
                "defaults.unknown_kind" => plan_node_name(doc, d, &path, from, to),
                "style.invalid_value" => plan_style_value(doc, d, &path, property, from, to),
                _ => plan_value(doc, d, &path, property, from, to),
            },
        };
        for fix in planned {
            if taken.insert(fix.site.clone()) {
                fixes.push(fix);
            }
        }
    }
    Plan {
        fixes,
        minted: minter.minted,
    }
}

/// Codes whose subject is an id-less row: a `defaults` row or a `style`
/// block. Their diagnostics are located by source span, not by subject id.
fn is_row_code(code: &str) -> bool {
    code.starts_with("defaults.") || code == "style.invalid_value"
}

/// The node a diagnostic's fix edits: the row at its span for row codes,
/// else the node whose id is the diagnostic subject.
fn subject_path(doc: &KdlDocument, d: &Diagnostic) -> Option<NodePath> {
    if is_row_code(&d.code) {
        find_node_by_span(doc, d.span?)
    } else {
        find_node(doc, d.subject_id.as_deref()?)
    }
}

fn record(d: &Diagnostic, property: &str, from: String, to: String) -> AppliedFix {
    AppliedFix {
        code: d.code.clone(),
        subject_id: d.subject_id.clone().unwrap_or_default(),
        property: property.to_owned(),
        from,
        to,
    }
}

/// The literal text of a raw entry, in the form a [`FixHint::RawLiteral`]
/// carries it (`#ffffff`, `(px)24`, `700`). `None` for references.
fn entry_literal(entry: &KdlEntry) -> Option<String> {
    match annotation(entry) {
        Some("token" | "data") => None,
        Some(unit) => {
            let value: f64 = match entry.value() {
                KdlValue::Integer(_) | KdlValue::Float(_) => {
                    scalar_text(entry.value())?.parse().ok()?
                }
                KdlValue::String(_) | KdlValue::Bool(_) | KdlValue::Null => return None,
            };
            Some(
                Dimension {
                    value,
                    unit: Unit::from_annotation(unit),
                }
                .to_kdl_string(),
            )
        }
        None => scalar_text(entry.value()),
    }
}

/// The fields of a [`FixHint::RawLiteral`] the planner reads.
struct RawLiteral<'a> {
    property: &'a str,
    literal: &'a str,
    token_type: &'a str,
    exact_match: Option<&'a str>,
}

fn plan_raw_literal(
    doc: &KdlDocument,
    d: &Diagnostic,
    path: &[usize],
    raw: &RawLiteral<'_>,
    minter: &mut Minter,
) -> Vec<PlannedFix> {
    let id = match raw.exact_match {
        Some(id) => id.to_owned(),
        None => {
            let Some(lit) = LiteralValue::from_hint(raw.token_type, raw.literal) else {
                return Vec::new();
            };
            match minter.mint(&lit, raw.property) {
                Some(id) => id,
                None => return Vec::new(),
            }
        }
    };
    property_sites(doc, path, raw.property)
        .into_iter()
        .filter_map(|site| {
            let entry = entry_at(doc, &site)?;
            (entry_literal(entry).as_deref() == Some(raw.literal)).then(|| PlannedFix {
                record: record(
                    d,
                    raw.property,
                    value_text(entry),
                    format!("(token){}", quote(&id)),
                ),
                change: Change::TokenRef(id.clone()),
                site,
            })
        })
        .collect()
}

fn plan_token_ref(
    doc: &KdlDocument,
    d: &Diagnostic,
    path: &[usize],
    property: &str,
    from: &str,
    to: &str,
) -> Vec<PlannedFix> {
    property_sites(doc, path, property)
        .into_iter()
        .filter(|site| {
            entry_at(doc, site).is_some_and(|e| {
                annotation(e) == Some("token") && e.value().as_string() == Some(from)
            })
        })
        .map(|site| PlannedFix {
            site,
            change: Change::TokenRef(to.to_owned()),
            record: record(
                d,
                property,
                format!("(token){}", quote(from)),
                format!("(token){}", quote(to)),
            ),
        })
        .collect()
}

fn plan_rename(
    doc: &KdlDocument,
    d: &Diagnostic,
    path: &[usize],
    from: &str,
    to: &str,
    renamed: &mut BTreeSet<(NodePath, String)>,
) -> Vec<PlannedFix> {
    let Some(node) = node_at(doc, path) else {
        return Vec::new();
    };
    let named = |name: &str| {
        node.entries()
            .iter()
            .position(|e| e.name().map(|n| n.value()) == Some(name))
    };
    let Some(index) = named(from) else {
        return Vec::new();
    };
    if named(to).is_some() || !renamed.insert((path.to_vec(), to.to_owned())) {
        return Vec::new();
    }
    vec![PlannedFix {
        site: Site::entry(path.to_vec(), index),
        change: Change::Rename(to.to_owned()),
        record: record(d, from, from.to_owned(), to.to_owned()),
    }]
}

fn plan_value(
    doc: &KdlDocument,
    d: &Diagnostic,
    path: &[usize],
    property: &str,
    from: &str,
    to: &str,
) -> Vec<PlannedFix> {
    let Some(node) = node_at(doc, path) else {
        return Vec::new();
    };
    // `text-style` on a `defaults` row is also accepted as `text_style`.
    let alias = property.replace('-', "_");
    let Some(index) = node.entries().iter().position(|e| {
        let name = e.name().map(|n| n.value());
        (name == Some(property) || name == Some(alias.as_str()))
            && e.value().as_string() == Some(from)
    }) else {
        return Vec::new();
    };
    vec![PlannedFix {
        site: Site::entry(path.to_vec(), index),
        change: Change::Text(to.to_owned()),
        record: record(d, property, quote(from), quote(to)),
    }]
}

/// Rename a `defaults` row's kind (its node name) from `from` to `to`.
///
/// Skipped when a sibling row already names `to`: the rename would only turn
/// an unknown kind into a duplicate kind.
fn plan_node_name(
    doc: &KdlDocument,
    d: &Diagnostic,
    path: &[usize],
    from: &str,
    to: &str,
) -> Vec<PlannedFix> {
    let Some(node) = node_at(doc, path) else {
        return Vec::new();
    };
    if node.name().value() != from {
        return Vec::new();
    }
    let Some((_, parent)) = path.split_last() else {
        return Vec::new();
    };
    let taken = node_at(doc, parent)
        .and_then(|p| p.children())
        .is_some_and(|c| c.nodes().iter().any(|n| n.name().value() == to));
    if taken {
        return Vec::new();
    }
    vec![PlannedFix {
        site: Site {
            node: path.to_vec(),
            slot: Slot::Name,
        },
        change: Change::NodeName(to.to_owned()),
        record: record(d, "kind", from.to_owned(), to.to_owned()),
    }]
}

/// Swap the value of the `style` block property child `property` (any
/// spelling that canonicalizes to it) from `from` to `to`.
fn plan_style_value(
    doc: &KdlDocument,
    d: &Diagnostic,
    path: &[usize],
    property: &str,
    from: &str,
    to: &str,
) -> Vec<PlannedFix> {
    let is_name = |name: &str| canonicalize_style_key(name) == Some(property);
    let Some(site) = child_value_site(doc, path, is_name, from) else {
        return Vec::new();
    };
    vec![PlannedFix {
        site,
        change: Change::Text(to.to_owned()),
        record: record(d, property, quote(from), quote(to)),
    }]
}
