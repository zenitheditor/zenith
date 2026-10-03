//! Text and label contrast over the content validation cannot see: the
//! master projection and the subtree each `instance` expanded to.
//!
//! The pass rebuilds the page content as the compile drew it (master
//! projection first, each local `instance` replaced by its expansion group)
//! and runs the core contrast walks over it. Text diagnostics are kept for
//! expanded ids only, because validation already judges authored text.
//! Label contrast covers every label, authored or expanded. An instance
//! drawn under a fit scale or expanded from an import (its own token scope)
//! stays unexpanded, so the walks skip it.

use std::collections::{BTreeMap, BTreeSet};

use zenith_core::{Diagnostic, Node, expanded_text_contrast_checks};

use super::super::boxes::Expansion;
use super::super::container::synthetic_group;
use super::super::session::label_contrast;
use super::run::LintEnv;

/// Contrast diagnostics of every label, and of the text in expanded content.
pub(super) fn content_contrast(
    env: &LintEnv<'_>,
    expansions: &BTreeMap<String, Expansion>,
) -> Vec<Diagnostic> {
    // Nothing to rebuild: skip the deep copy of the page content.
    if env.master.is_empty() && !expansions.values().any(|e| !e.scaled && !e.imported) {
        return label_contrast(
            env.commands,
            env.page,
            env.bleed,
            env.paint.resolved,
            env.paint.style_map,
            env.shape,
        );
    }
    let mut expanded: BTreeSet<String> = BTreeSet::new();
    collect_ids(env.master, &mut expanded);
    let mut substituted = false;
    let children = substitute(
        &env.page.children,
        expansions,
        &mut expanded,
        &mut substituted,
    );
    if env.master.is_empty() && !substituted {
        return label_contrast(
            env.commands,
            env.page,
            env.bleed,
            env.paint.resolved,
            env.paint.style_map,
            env.shape,
        );
    }
    let mut content: Vec<Node> = env.master.to_vec();
    content.extend(children);
    let mut out: Vec<Diagnostic> =
        expanded_text_contrast_checks(env.page, &content, env.paint.resolved, env.paint.style_map)
            .into_iter()
            .filter(|d| {
                d.subject_id
                    .as_ref()
                    .is_some_and(|id| expanded.contains(id))
            })
            .collect();
    let mut page = env.page.clone();
    page.children = content;
    out.extend(label_contrast(
        env.commands,
        &page,
        env.bleed,
        env.paint.resolved,
        env.paint.style_map,
        env.shape,
    ));
    out
}

/// `nodes` with each drawable local `instance` replaced by its expansion
/// group. Every id inside an expansion joins `expanded`; `substituted`
/// turns true on the first replacement.
fn substitute(
    nodes: &[Node],
    expansions: &BTreeMap<String, Expansion>,
    expanded: &mut BTreeSet<String>,
    substituted: &mut bool,
) -> Vec<Node> {
    nodes
        .iter()
        .map(|node| match node {
            Node::Instance(instance) => match expansions.get(&instance.id) {
                Some(e) if !e.scaled && !e.imported => {
                    *substituted = true;
                    collect_ids(&e.children, expanded);
                    let inner = substitute(&e.children, expansions, expanded, substituted);
                    Node::Group(synthetic_group(instance, inner))
                }
                Some(_) | None => node.clone(),
            },
            Node::Frame(f) => {
                let mut f = f.clone();
                f.children = substitute(&f.children, expansions, expanded, substituted);
                Node::Frame(f)
            }
            Node::Group(g) => {
                let mut g = g.clone();
                g.children = substitute(&g.children, expansions, expanded, substituted);
                Node::Group(g)
            }
            Node::Rect(_)
            | Node::Ellipse(_)
            | Node::Line(_)
            | Node::Text(_)
            | Node::Code(_)
            | Node::Image(_)
            | Node::Polygon(_)
            | Node::Polyline(_)
            | Node::Path(_)
            | Node::Field(_)
            | Node::Toc(_)
            | Node::Footnote(_)
            | Node::Table(_)
            | Node::Shape(_)
            | Node::Connector(_)
            | Node::Pattern(_)
            | Node::Chart(_)
            | Node::Light(_)
            | Node::Mesh(_)
            | Node::Unknown(_) => node.clone(),
        })
        .collect()
}

/// Every id in `nodes` and their containers' descendants.
fn collect_ids(nodes: &[Node], out: &mut BTreeSet<String>) {
    for node in nodes {
        if let Some(id) = node.id() {
            out.insert(id.to_owned());
        }
        if let Some(children) = node.children() {
            collect_ids(children, out);
        }
    }
}
