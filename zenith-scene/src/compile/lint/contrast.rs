//! Text and label contrast over the page as the compile drew it: the master
//! projection and each `instance` replaced by the subtree it expanded to.
//!
//! The pass rebuilds the page content as the compile drew it (master
//! projection first, each `instance` replaced by its expansion group),
//! measures the drawn ink of every `text` node and label, and runs the core
//! contrast walk over it once. Every text is judged where its glyphs land,
//! authored or expanded.
//!
//! An expansion group carries a [`ContentScope`] when it needs one: an
//! imported component draws in its own document's tokens and styles, and an
//! instance with a `w` / `h` fit draws under its scale transform. A
//! diagnostic about a node inside an imported component keeps that
//! component's span and names the import.

use std::collections::BTreeMap;

use zenith_core::{
    ContentScope, ContentScopes, ContrastInks, Diagnostic, GlyphInk, InkLine, Node, ScopeTokens,
    page_contrast_checks,
};

use super::super::boxes::{Expansion, TextInk};
use super::super::container::synthetic_group;
use super::super::imports::ImportScopes;
use super::super::session::{chart_inks, label_inks};
use super::run::LintEnv;

/// The rebuilt page content and what the walks need to judge it.
#[derive(Default)]
struct Rebuilt<'a> {
    /// The import id of every node inside an imported component.
    imported: BTreeMap<String, String>,
    scopes: ContentScopes<'a>,
    /// An instance was replaced by its expansion.
    substituted: bool,
}

/// The glyph ink of every drawn text, in the form the core walk reads.
pub(super) fn text_inks(inks: &BTreeMap<String, TextInk>) -> BTreeMap<String, GlyphInk> {
    inks.iter()
        .map(|(id, ink)| {
            let lines = ink
                .lines
                .iter()
                .map(|line| InkLine {
                    x: line.rect.x,
                    y: line.rect.y,
                    w: line.rect.w,
                    h: line.rect.h,
                    matrix: line.matrix,
                })
                .collect();
            (id.clone(), GlyphInk { lines })
        })
        .collect()
}

/// Contrast diagnostics of every text and label on the page. `texts` is the
/// drawn glyph ink of each text (see [`text_inks`]).
pub(super) fn content_contrast(
    env: &LintEnv<'_>,
    expansions: &BTreeMap<String, Expansion>,
    texts: &BTreeMap<String, GlyphInk>,
) -> Vec<Diagnostic> {
    let labels = label_inks(env.commands, env.bleed, env.shape);
    let charts = chart_inks(env.commands, env.bleed, env.shape);
    let inks = ContrastInks {
        labels: &labels,
        texts,
        charts: &charts,
    };
    let judge = |children: &[Node], scopes: &ContentScopes<'_>| {
        page_contrast_checks(
            env.page,
            children,
            env.paint.resolved,
            env.paint.style_map,
            Some(inks),
            scopes,
        )
    };
    let none = ContentScopes::new();
    // Nothing to rebuild: skip the deep copy of the page content.
    if env.master.is_empty() && expansions.is_empty() {
        return judge(&env.page.children, &none);
    }
    let mut rebuilt = Rebuilt::default();
    let children = substitute(
        &env.page.children,
        expansions,
        env.imports,
        None,
        &mut rebuilt,
    );
    if env.master.is_empty() && !rebuilt.substituted {
        return judge(&env.page.children, &none);
    }
    let mut content: Vec<Node> = env.master.to_vec();
    content.extend(children);
    let mut out = judge(&content, &rebuilt.scopes);
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
                if let Some(import) = inner_import {
                    collect_imports(&e.children, import, rebuilt);
                }
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

/// Map every id in `nodes` and their containers' descendants to `import`.
fn collect_imports(nodes: &[Node], import: &str, rebuilt: &mut Rebuilt<'_>) {
    for node in nodes {
        if let Some(id) = node.id() {
            rebuilt.imported.insert(id.to_owned(), import.to_owned());
        }
        if let Some(children) = node.children() {
            collect_imports(children, import, rebuilt);
        }
    }
}
