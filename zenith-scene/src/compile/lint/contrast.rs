//! Text and label contrast over the content validation cannot see: the
//! master projection and the subtree each `instance` expanded to.
//!
//! The pass rebuilds the page content as the compile drew it (master
//! projection first, each `instance` replaced by its expansion group) and
//! runs the core contrast walks over it. Text diagnostics are kept for
//! expanded ids only, because validation already judges authored text.
//! Label contrast covers every label, authored or expanded.
//!
//! An expansion group carries a [`ContentScope`] when it needs one: an
//! imported component draws in its own document's tokens and styles, and an
//! instance with a `w` / `h` fit draws under its scale transform. A
//! diagnostic about a node inside an imported component keeps that
//! component's span and names the import.

use std::collections::{BTreeMap, BTreeSet};

use zenith_core::{
    ContentScope, ContentScopes, Diagnostic, Node, Page, ScopeTokens, expanded_text_contrast_checks,
};

use super::super::boxes::Expansion;
use super::super::container::synthetic_group;
use super::super::imports::ImportScopes;
use super::super::session::label_contrast;
use super::run::LintEnv;

/// The rebuilt page content and what the walks need to judge it.
#[derive(Default)]
struct Rebuilt<'a> {
    /// Every id inside the master projection or an expansion.
    expanded: BTreeSet<String>,
    /// The import id of every node inside an imported component.
    imported: BTreeMap<String, String>,
    scopes: ContentScopes<'a>,
    /// An instance was replaced by its expansion.
    substituted: bool,
}

/// Contrast diagnostics of every label, and of the text in expanded content.
pub(super) fn content_contrast(
    env: &LintEnv<'_>,
    expansions: &BTreeMap<String, Expansion>,
) -> Vec<Diagnostic> {
    let none = ContentScopes::new();
    // Nothing to rebuild: skip the deep copy of the page content.
    if env.master.is_empty() && expansions.is_empty() {
        return labels(env, env.page, &none);
    }
    let mut rebuilt = Rebuilt::default();
    collect_ids(env.master, None, &mut rebuilt);
    let children = substitute(
        &env.page.children,
        expansions,
        env.imports,
        None,
        &mut rebuilt,
    );
    if env.master.is_empty() && !rebuilt.substituted {
        return labels(env, env.page, &none);
    }
    let mut content: Vec<Node> = env.master.to_vec();
    content.extend(children);
    let mut out: Vec<Diagnostic> = expanded_text_contrast_checks(
        env.page,
        &content,
        env.paint.resolved,
        env.paint.style_map,
        &rebuilt.scopes,
    )
    .into_iter()
    .filter(|d| {
        d.subject_id
            .as_ref()
            .is_some_and(|id| rebuilt.expanded.contains(id))
    })
    .collect();
    let mut page = env.page.clone();
    page.children = content;
    out.extend(labels(env, &page, &rebuilt.scopes));
    for d in &mut out {
        let import = d
            .subject_id
            .as_ref()
            .and_then(|id| rebuilt.imported.get(id));
        if let Some(import) = import {
            d.set_import(import.clone());
        }
    }
    out
}

/// Label contrast of `page` as the page compile drew it.
fn labels(env: &LintEnv<'_>, page: &Page, scopes: &ContentScopes<'_>) -> Vec<Diagnostic> {
    label_contrast(
        env.commands,
        page,
        env.bleed,
        (env.paint.resolved, env.paint.style_map),
        scopes,
        env.shape,
    )
}

/// `nodes` with each drawn `instance` replaced by its expansion group. An
/// imported expansion whose import is not in `imports` stays an instance,
/// which the walks skip. `import` is the import id of the enclosing
/// imported component.
fn substitute<'a>(
    nodes: &[Node],
    expansions: &BTreeMap<String, Expansion>,
    imports: &'a ImportScopes<'a>,
    import: Option<&str>,
    rebuilt: &mut Rebuilt<'a>,
) -> Vec<Node> {
    nodes
        .iter()
        .map(|node| match node {
            Node::Instance(instance) => {
                let Some(e) = expansions.get(&instance.id) else {
                    return node.clone();
                };
                let tokens = match e.import.as_deref() {
                    Some(id) => match imports.get(id) {
                        Some(scope) => Some(ScopeTokens {
                            resolved: &scope.resolved,
                            styles: &scope.style_map,
                        }),
                        None => return node.clone(),
                    },
                    None => None,
                };
                let inner_import = e.import.as_deref().or(import);
                rebuilt.substituted = true;
                collect_ids(&e.children, inner_import, rebuilt);
                let inner = substitute(&e.children, expansions, imports, inner_import, rebuilt);
                let mut group = synthetic_group(instance, inner);
                if e.fit.is_some() {
                    // The fit transform carries all positioning.
                    group.x = None;
                    group.y = None;
                }
                if tokens.is_some() || e.fit.is_some() {
                    rebuilt
                        .scopes
                        .insert(instance.id.clone(), ContentScope { tokens, fit: e.fit });
                }
                Node::Group(group)
            }
            Node::Frame(f) => {
                let mut f = f.clone();
                f.children = substitute(&f.children, expansions, imports, import, rebuilt);
                Node::Frame(f)
            }
            Node::Group(g) => {
                let mut g = g.clone();
                g.children = substitute(&g.children, expansions, imports, import, rebuilt);
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

/// Every id in `nodes` and their containers' descendants joins `expanded`;
/// with `import` set, each also maps to that import.
fn collect_ids(nodes: &[Node], import: Option<&str>, rebuilt: &mut Rebuilt<'_>) {
    for node in nodes {
        if let Some(id) = node.id() {
            rebuilt.expanded.insert(id.to_owned());
            if let Some(import) = import {
                rebuilt.imported.insert(id.to_owned(), import.to_owned());
            }
        }
        if let Some(children) = node.children() {
            collect_ids(children, import, rebuilt);
        }
    }
}
