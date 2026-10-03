//! The page ledger: every compiled node of one page in paint order, with its
//! final box and paint facts, plus the glyph ink of every text node.
//!
//! One page compile builds it: the box recorder supplies each node's final
//! box and the subtree each `instance` expanded to, and one pass over the
//! finished command stream supplies the glyph ink. The node walk visits the
//! master projection, then the page children, in source order. A parent
//! comes before its children, so the entry index is the paint order.

use std::collections::BTreeMap;

use zenith_core::{Node, Span};

use crate::layout::LayoutBox;

use super::super::boxes::{CompiledBox, Expansion, TextInk};
use super::super::chart::{ChartTextRole, parse_chart_source};
use super::geom::{Coverage, bounds, intersect};
use super::paint::{Authored, PaintEnv, hollow, occluder_of, own_effects};

/// Opaque paint of one node: the region its fill or image covers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Occluder {
    pub(super) region: LayoutBox,
    pub(super) shape: Coverage,
}

/// The `shape` facts the label checks read.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ShapeFacts {
    /// `kind`, `process` when absent.
    pub(super) kind: String,
    /// Label inset on every side, in px.
    pub(super) pad: f64,
}

/// The `connector` facts the crossing check reads.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ConnectorFacts {
    /// The `from` / `to` node ids, port suffix (`#port`) removed.
    pub(super) ends: [Option<String>; 2],
    /// `route`, `straight` when absent.
    pub(super) route: String,
}

/// One compiled node. `role="decoration"` / `"background"` (own or
/// inherited) folds into `exempt`; `role="guide"` nodes never compile and
/// have no entry.
#[derive(Clone, Debug)]
pub(super) struct Entry {
    /// Compiled id (expanded content carries its `<owner>/` prefix).
    pub(super) id: String,
    /// Index of the parent entry.
    pub(super) parent: Option<usize>,
    pub(super) kind: &'static str,
    pub(super) span: Option<Span>,
    /// The final box, when the compile recorded one.
    pub(super) compiled: Option<CompiledBox>,
    /// Own opacity times every ancestor's.
    pub(super) opacity: f64,
    /// Opaque solid fill or opaque image at full effective opacity.
    pub(super) occluder: Option<Occluder>,
    pub(super) visible: bool,
    /// Placed by a `row` / `column` / `grid` frame of the authored document.
    pub(super) in_flow: bool,
    /// `role="decoration"` / `"background"` on the node or an ancestor.
    pub(super) exempt: bool,
    /// An ancestor draws under a rotation, scale, or effect the checks do not
    /// model, so the final box is not the drawn geometry.
    pub(super) unmodeled: bool,
    /// The node itself draws through a blend layer, mask, filter, or blur.
    pub(super) effects: bool,
    /// A `rect`, `ellipse`, or `shape` with no fill: an outline only.
    pub(super) hollow: bool,
    /// Expanded from an `instance` or projected from a master.
    pub(super) expanded: bool,
    /// The enclosing `table` or `chart` entry.
    pub(super) scope: Option<usize>,
    /// Intersection of the clipping frames around the node, in page px.
    pub(super) clip: Option<LayoutBox>,
    pub(super) shape: Option<ShapeFacts>,
    pub(super) connector: Option<ConnectorFacts>,
}

/// What drew a [`TextItem`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TextSource {
    /// A text-like node (`text`, `code`, `field`, `toc`).
    Node,
    /// A `shape` / `connector` label.
    Label,
    /// One string of a chart, in the given role.
    Chart(ChartTextRole),
}

/// The glyph ink of one text node, `shape` / `connector` label, or chart
/// string.
#[derive(Clone, Debug)]
pub(super) struct TextItem {
    /// The glyph-run source id (`<owner>/label` for a label,
    /// `<chart>/<role>/<index>` for a chart string).
    pub(super) id: String,
    /// The text entry, or the owner entry of a label or chart string.
    pub(super) entry: usize,
    pub(super) source: TextSource,
    pub(super) ink: TextInk,
    /// Union of the glyph ink boxes.
    pub(super) bounds: LayoutBox,
}

/// Every compiled node of one page in paint order, and the text ink.
#[derive(Debug, Default)]
pub(super) struct PageLedger {
    pub(super) entries: Vec<Entry>,
    pub(super) texts: Vec<TextItem>,
}

/// What the walk carries from a container to its children.
#[derive(Clone, Copy)]
struct Inherit {
    parent: Option<usize>,
    opacity: f64,
    unmodeled: bool,
    exempt: bool,
    expanded: bool,
    imported: bool,
    scope: Option<usize>,
    clip: Option<LayoutBox>,
}

/// The inputs of [`PageLedger::build`].
pub(super) struct LedgerInput<'a> {
    pub(super) master: &'a [Node],
    pub(super) children: &'a [Node],
    pub(super) boxes: &'a BTreeMap<String, CompiledBox>,
    pub(super) expansions: &'a BTreeMap<String, Expansion>,
    pub(super) authored: &'a BTreeMap<String, Authored>,
    pub(super) paint: PaintEnv<'a>,
}

impl PageLedger {
    /// Build the ledger of one compiled page from its node trees and the
    /// glyph ink of its command stream.
    pub(super) fn build(input: &LedgerInput<'_>, inks: BTreeMap<String, TextInk>) -> Self {
        let mut ledger = PageLedger::default();
        let root = Inherit {
            parent: None,
            opacity: 1.0,
            unmodeled: false,
            exempt: false,
            expanded: false,
            imported: false,
            scope: None,
            clip: None,
        };
        ledger.walk(
            input.master,
            Inherit {
                expanded: true,
                ..root
            },
            input,
        );
        ledger.walk(input.children, root, input);
        ledger.attach_inks(inks);
        ledger
    }

    fn walk(&mut self, nodes: &[Node], inherit: Inherit, input: &LedgerInput<'_>) {
        for node in nodes {
            self.visit(node, inherit, input);
        }
    }

    fn visit(&mut self, node: &Node, inherit: Inherit, input: &LedgerInput<'_>) {
        // Guide nodes never compile.
        if node.role() == Some("guide") {
            return;
        }
        let Some(id) = node.id() else {
            return;
        };
        let visible = node.is_visible();
        let opacity = inherit.opacity * node.opacity().unwrap_or(1.0).clamp(0.0, 1.0);
        let exempt = inherit.exempt || node.is_decorative();
        let compiled = input.boxes.get(id).copied();
        let own_effects = own_effects(node);
        let occluder = match compiled {
            Some(b)
                if visible
                    && !exempt
                    && !inherit.unmodeled
                    && !inherit.imported
                    && !own_effects
                    && b.rotate.is_none()
                    && opacity >= 1.0 =>
            {
                occluder_of(node, b.rect, input.paint)
            }
            Some(_) | None => None,
        };
        let shape = match node {
            Node::Shape(s) => Some(ShapeFacts {
                kind: s.kind.clone().unwrap_or_else(|| "process".to_owned()),
                pad: super::super::resolve_property_dimension_px(
                    s.padding.as_ref(),
                    input.paint.resolved,
                    0.0,
                ),
            }),
            Node::Rect(_)
            | Node::Ellipse(_)
            | Node::Line(_)
            | Node::Text(_)
            | Node::Code(_)
            | Node::Frame(_)
            | Node::Group(_)
            | Node::Image(_)
            | Node::Polygon(_)
            | Node::Polyline(_)
            | Node::Path(_)
            | Node::Instance(_)
            | Node::Field(_)
            | Node::Toc(_)
            | Node::Footnote(_)
            | Node::Table(_)
            | Node::Connector(_)
            | Node::Pattern(_)
            | Node::Chart(_)
            | Node::Light(_)
            | Node::Mesh(_)
            | Node::Unknown(_) => None,
        };
        let connector = match node {
            Node::Connector(c) => {
                let end = |e: &Option<String>| {
                    e.as_deref()
                        .map(|e| e.split_once('#').map_or(e, |(id, _)| id).to_owned())
                };
                Some(ConnectorFacts {
                    ends: [end(&c.from), end(&c.to)],
                    route: c.route.clone().unwrap_or_else(|| "straight".to_owned()),
                })
            }
            Node::Rect(_)
            | Node::Ellipse(_)
            | Node::Line(_)
            | Node::Text(_)
            | Node::Code(_)
            | Node::Frame(_)
            | Node::Group(_)
            | Node::Image(_)
            | Node::Polygon(_)
            | Node::Polyline(_)
            | Node::Path(_)
            | Node::Instance(_)
            | Node::Field(_)
            | Node::Toc(_)
            | Node::Footnote(_)
            | Node::Table(_)
            | Node::Shape(_)
            | Node::Pattern(_)
            | Node::Chart(_)
            | Node::Light(_)
            | Node::Mesh(_)
            | Node::Unknown(_) => None,
        };
        let index = self.entries.len();
        self.entries.push(Entry {
            id: id.to_owned(),
            parent: inherit.parent,
            kind: node.kind_str(),
            span: node.source_span(),
            compiled,
            opacity,
            occluder,
            visible,
            in_flow: !inherit.expanded && input.authored.get(id).is_some_and(|a| a.in_flow),
            exempt,
            unmodeled: inherit.unmodeled,
            effects: own_effects,
            hollow: hollow(node, input.paint),
            expanded: inherit.expanded,
            scope: inherit.scope,
            clip: inherit.clip,
            shape,
            connector,
        });
        if !visible {
            return;
        }
        let child = Inherit {
            parent: Some(index),
            opacity,
            unmodeled: inherit.unmodeled || own_effects || node_rotates(compiled),
            exempt,
            ..inherit
        };
        match node {
            Node::Frame(f) => {
                let clip = match compiled {
                    Some(b) if f.clips() && !child.unmodeled => match inherit.clip {
                        Some(c) => Some(intersect(c, b.rect).unwrap_or(LayoutBox {
                            w: 0.0,
                            h: 0.0,
                            ..b.rect
                        })),
                        None => Some(b.rect),
                    },
                    Some(_) | None => inherit.clip,
                };
                self.walk(&f.children, Inherit { clip, ..child }, input);
            }
            Node::Group(g) => self.walk(&g.children, child, input),
            Node::Unknown(u) => self.walk(&u.children, child, input),
            Node::Table(t) => {
                let cell = Inherit {
                    scope: Some(index),
                    ..child
                };
                for row in &t.rows {
                    for c in &row.cells {
                        self.walk(&c.children, cell, input);
                    }
                }
            }
            Node::Instance(_) => {
                if let Some(expansion) = input.expansions.get(id) {
                    let inner = Inherit {
                        expanded: true,
                        unmodeled: child.unmodeled || expansion.scaled(),
                        imported: child.imported || expansion.import.is_some(),
                        ..child
                    };
                    self.walk(&expansion.children, inner, input);
                }
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
            | Node::Shape(_)
            | Node::Connector(_)
            | Node::Pattern(_)
            | Node::Chart(_)
            | Node::Light(_)
            | Node::Mesh(_) => {}
        }
    }

    /// Pair each glyph-run source with its entry: a text-like node by id, a
    /// label by its owner id, a chart string by its chart id. Ink of any
    /// other source is dropped.
    fn attach_inks(&mut self, inks: BTreeMap<String, TextInk>) {
        let by_id: BTreeMap<&str, usize> = self
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| (e.id.as_str(), i))
            .collect();
        let mut texts = Vec::new();
        for (id, ink) in inks {
            let Some(b) = bounds(&ink.glyphs) else {
                continue;
            };
            let direct = || {
                by_id
                    .get(id.as_str())
                    .copied()
                    .filter(|&i| self.is_kind(i, &["text", "code", "field", "toc"]))
                    .map(|i| (i, TextSource::Node))
            };
            let label = || {
                id.strip_suffix("/label")
                    .and_then(|owner| by_id.get(owner).copied())
                    .filter(|&i| self.is_kind(i, &["shape", "connector"]))
                    .map(|i| (i, TextSource::Label))
            };
            let chart = || {
                let (chart, role, _) = parse_chart_source(&id)?;
                by_id
                    .get(chart)
                    .copied()
                    .filter(|&i| self.is_kind(i, &["chart"]))
                    .map(|i| (i, TextSource::Chart(role)))
            };
            let Some((entry, source)) = direct().or_else(label).or_else(chart) else {
                continue;
            };
            texts.push(TextItem {
                id,
                entry,
                source,
                ink,
                bounds: b,
            });
        }
        self.texts = texts;
    }

    /// Whether the entry at `index` is one of `kinds`.
    fn is_kind(&self, index: usize, kinds: &[&str]) -> bool {
        self.entries
            .get(index)
            .is_some_and(|e| kinds.contains(&e.kind))
    }

    /// `true` when the entry at `ancestor` contains the entry at `index`.
    pub(super) fn is_ancestor(&self, ancestor: usize, index: usize) -> bool {
        let mut at = self.entry(index).and_then(|e| e.parent);
        while let Some(i) = at {
            if i == ancestor {
                return true;
            }
            at = self.entry(i).and_then(|e| e.parent);
        }
        false
    }

    /// The entry at `index`.
    pub(super) fn entry(&self, index: usize) -> Option<&Entry> {
        self.entries.get(index)
    }
}

/// `true` when the node's own rotation turns its children.
fn node_rotates(compiled: Option<CompiledBox>) -> bool {
    compiled.is_some_and(|b| b.rotate.is_some())
}
