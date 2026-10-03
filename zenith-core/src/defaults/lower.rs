//! The lowering entry point and the structural walk.

use std::collections::BTreeMap;

use crate::ast::defaults::{DefaultsBlock, DefaultsKind};
use crate::ast::document::{ComponentDef, Document, MasterDef};
use crate::ast::node::{Node, TableNode};
use crate::ast::style::Style;
use crate::ast::value::PropertyValue;
use crate::tokens::ResolvedToken;

use super::alias::{CopyIds, IdAliases};
use super::apply;
use super::label;
use super::pair::Pairing;
use super::scope::{Scope, Synth};

/// Lower the `defaults` blocks of `doc` into explicit node values.
///
/// Returns `None` when neither the document nor any page carries a
/// `defaults` block: the caller keeps `doc` unchanged and nothing is cloned.
///
/// Otherwise returns a clone in which, per node and per style key the node
/// kind consumes, a key that neither the node attribute nor the node's own
/// `style` carries takes the page default style's value, else the document
/// default style's value, written as an explicit attribute. The resulting
/// precedence is: node attribute > node `style` > page default > document
/// default > engine default.
///
/// - `shape` / `connector` labels: the label style cascades the same way
///   (node `text-style` > page default > document default).
/// - Table header cells: the table `header_style` stays the node style of a
///   header text that sets no `style`.
/// - Content pairing runs when the document or the page has a block: a text
///   fill that neither the node nor its style sets takes the content token the
///   ambient structural fill implies (rules in the `pair` module) before the default
///   style's fill.
/// - Content drawn on a page lowers with that page's defaults: each local
///   component an `instance` on the page places, and the page's master, lower
///   into a per-page copy that starts from the ambient pair at the
///   placement. The instance or page points at the copy by an internal id
///   that [`Lowered::aliases`] maps back to the authored id. The authored
///   component and master lower with the document defaults only. An
///   imported document lowers with its own defaults.
///
/// A key that no node attribute holds (a label style key, or `font-weight`
/// on a `field`) lowers into a merged style named `defaults:…`, appended to
/// the clone's `styles`. Source spans stay on the authored nodes.
pub fn lower(doc: &Document, resolved: &BTreeMap<String, ResolvedToken>) -> Option<Lowered> {
    let doc_block = present(&doc.defaults);
    if !doc_block && !doc.body.pages.iter().any(|p| present(&p.defaults)) {
        return None;
    }
    let styles: BTreeMap<&str, &Style> = doc
        .styles
        .styles
        .iter()
        .map(|s| (s.id.as_str(), s))
        .collect();
    let pairing = Pairing::new(resolved);
    let mut components: BTreeMap<&str, &ComponentDef> = BTreeMap::new();
    for component in &doc.components {
        components.entry(component.id.as_str()).or_insert(component);
    }
    let mut masters: BTreeMap<&str, &MasterDef> = BTreeMap::new();
    for master in &doc.masters {
        masters.entry(master.id.as_str()).or_insert(master);
    }
    let mut cx = Cx {
        pairing: &pairing,
        synth: Synth::default(),
        components,
        copies: BTreeMap::new(),
        building: Vec::new(),
        ids: CopyIds::new(
            doc.components
                .iter()
                .map(|c| c.id.clone())
                .chain(doc.masters.iter().map(|m| m.id.clone()))
                .collect(),
        ),
    };
    let base = Scope {
        styles: &styles,
        page: None,
        page_id: None,
        doc: &doc.defaults,
        pairing: doc_block,
    };

    let mut out = doc.clone();
    for component in &mut out.components {
        walk(&mut component.children, &mut cx, base, None);
    }
    for master in &mut out.masters {
        walk(&mut master.children, &mut cx, base, None);
    }
    let mut master_copies: Vec<MasterDef> = Vec::new();
    for (page, source) in out.body.pages.iter_mut().zip(&doc.body.pages) {
        let scope = Scope {
            page: Some(&source.defaults),
            page_id: Some(source.id.as_str()),
            pairing: doc_block || present(&source.defaults),
            ..base
        };
        let pair = if scope.pairing {
            pairing.child_pair(source.background.as_ref(), None)
        } else {
            None
        };
        walk(&mut page.children, &mut cx, scope, pair.as_ref());
        if let Some(master) = source.master.as_deref().and_then(|id| masters.get(id)) {
            let mut copy = (*master).clone();
            copy.id = cx.ids.id(&master.id, &source.id);
            walk(&mut copy.children, &mut cx, scope, pair.as_ref());
            page.master = Some(copy.id.clone());
            master_copies.push(copy);
        }
    }
    out.masters.extend(master_copies);
    out.components.extend(cx.copies.into_values());
    let aliases = cx.ids.aliases;
    out.styles.styles.extend(cx.synth.into_styles());
    Some(Lowered {
        document: out,
        aliases,
    })
}

/// A defaults-lowered document and the internal ids its lowering made.
#[derive(Debug, Clone)]
pub struct Lowered {
    /// The lowered clone.
    pub document: Document,
    /// Copy id → authored id of each per-page component and master copy.
    /// Map every id shown to a user through it.
    pub aliases: IdAliases,
}

/// Whether a `defaults` block is written in the source (even when empty).
fn present(block: &DefaultsBlock) -> bool {
    block.source_span.is_some() || !block.is_empty()
}

/// Walk state shared across one document.
pub(super) struct Cx<'p> {
    pub(super) pairing: &'p Pairing<'p>,
    pub(super) synth: Synth,
    /// The authored local components, by id (first declaration wins).
    components: BTreeMap<&'p str, &'p ComponentDef>,
    /// Per-page component copies, by copy id.
    copies: BTreeMap<String, ComponentDef>,
    /// Copy ids under construction, so a recursive component terminates.
    building: Vec<String>,
    /// Copy id allocation and the copy → authored map.
    ids: CopyIds,
}

impl Cx<'_> {
    /// The id of the copy of local component `id` lowered for the page of
    /// `scope` with ambient `pair`, built on first use. `None` when `id`
    /// names no local component.
    fn component_copy(
        &mut self,
        id: &str,
        scope: Scope<'_>,
        page_id: &str,
        pair: Option<&PropertyValue>,
    ) -> Option<String> {
        let source = *self.components.get(id)?;
        let pair_id = match pair {
            Some(PropertyValue::TokenRef(token)) => token.as_str(),
            Some(PropertyValue::Literal(_))
            | Some(PropertyValue::Dimension(_))
            | Some(PropertyValue::DataRef(_))
            | None => "-",
        };
        let copy_id = self.ids.id(id, &format!("{page_id}:{pair_id}"));
        if self.copies.contains_key(&copy_id) || self.building.contains(&copy_id) {
            return Some(copy_id);
        }
        self.building.push(copy_id.clone());
        let mut copy = source.clone();
        copy.id = copy_id.clone();
        walk(&mut copy.children, self, scope, pair);
        self.building.pop();
        self.copies.insert(copy_id.clone(), copy);
        Some(copy_id)
    }

    /// The pair the children of a node with ambient fill `fill` see.
    pub(super) fn child_pair(
        &self,
        scope: Scope<'_>,
        fill: Option<&PropertyValue>,
        parent: Option<&PropertyValue>,
    ) -> Option<PropertyValue> {
        if scope.pairing {
            self.pairing.child_pair(fill, parent)
        } else {
            None
        }
    }
}

/// Lower `nodes` in place. `pair` is the ambient content pair.
fn walk(nodes: &mut [Node], cx: &mut Cx<'_>, scope: Scope<'_>, pair: Option<&PropertyValue>) {
    for node in nodes {
        lower_node(node, cx, scope, pair, None);
    }
}

/// Lower one node. `header_style` is the table `header_style` of a header
/// cell (the node style of a text that sets none).
fn lower_node(
    node: &mut Node,
    cx: &mut Cx<'_>,
    scope: Scope<'_>,
    pair: Option<&PropertyValue>,
    header_style: Option<&str>,
) {
    match node {
        Node::Rect(n) => apply::rect(n, &scope.cascade(DefaultsKind::Rect, n.style.as_deref())),
        Node::Ellipse(n) => {
            apply::ellipse(n, &scope.cascade(DefaultsKind::Ellipse, n.style.as_deref()))
        }
        Node::Line(n) => apply::line(n, &scope.cascade(DefaultsKind::Line, n.style.as_deref())),
        Node::Text(n) => {
            let own = n.style.as_deref().or(header_style);
            apply::text(n, &scope.cascade(DefaultsKind::Text, own), pair);
        }
        Node::Code(n) => apply::code(n, &scope.cascade(DefaultsKind::Code, n.style.as_deref())),
        Node::Frame(n) => {
            let cascade = scope.cascade(DefaultsKind::Frame, n.style.as_deref());
            apply::frame(n, &cascade);
            let fill = n.fill.as_ref().or_else(|| cascade.node_value("fill"));
            let child = cx.child_pair(scope, fill, pair);
            walk(&mut n.children, cx, scope, child.as_ref());
        }
        Node::Group(n) => {
            apply::group(n, &scope.cascade(DefaultsKind::Group, n.style.as_deref()));
            walk(&mut n.children, cx, scope, pair);
        }
        Node::Image(n) => apply::image(n, &scope.cascade(DefaultsKind::Image, n.style.as_deref())),
        Node::Polygon(n) => {
            apply::polygon(n, &scope.cascade(DefaultsKind::Polygon, n.style.as_deref()))
        }
        Node::Polyline(n) => apply::polyline(
            n,
            &scope.cascade(DefaultsKind::Polyline, n.style.as_deref()),
        ),
        Node::Path(n) => apply::path(n, &scope.cascade(DefaultsKind::Path, n.style.as_deref())),
        Node::Field(n) => {
            let cascade = scope.cascade(DefaultsKind::Field, n.style.as_deref());
            apply::field(n, &cascade, pair, &mut cx.synth);
        }
        Node::Footnote(n) => {
            let cascade = scope.cascade(DefaultsKind::Footnote, n.style.as_deref());
            apply::footnote(n, &cascade, pair, &mut cx.synth);
        }
        Node::Toc(n) => {
            let cascade = scope.cascade(DefaultsKind::Toc, n.style.as_deref());
            apply::toc(n, &cascade, pair, &mut cx.synth);
        }
        Node::Table(n) => table(n, cx, scope, pair),
        Node::Shape(n) => label::shape(n, cx, scope, pair),
        Node::Connector(n) => label::connector(n, cx, scope, pair),
        Node::Pattern(n) => {
            apply::pattern(n, &scope.cascade(DefaultsKind::Pattern, n.style.as_deref()))
        }
        Node::Chart(n) => apply::chart(n, &scope.cascade(DefaultsKind::Chart, n.style.as_deref())),
        // A local instance on a page points at the component copy lowered
        // for that page and ambient. An imported instance (`source=`) keeps
        // its own document's defaults. Effect and unknown nodes take no
        // defaults.
        Node::Instance(n) => {
            if n.source.is_none()
                && let Some(page_id) = scope.page_id
                && let Some(component) = n.component.as_deref()
                && let Some(copy) = cx.component_copy(component, scope, page_id, pair)
            {
                n.component = Some(copy);
            }
        }
        Node::Light(_) | Node::Mesh(_) | Node::Unknown(_) => {}
    }
}

/// Lower a table and its cells. A cell's ambient fill is its own fill, else
/// the header fill (header rows), else the table fill.
fn table(table: &mut TableNode, cx: &mut Cx<'_>, scope: Scope<'_>, pair: Option<&PropertyValue>) {
    apply::table(
        table,
        &scope.cascade(DefaultsKind::Table, table.style.as_deref()),
    );
    let body_pair = cx.child_pair(scope, table.fill.as_ref(), pair);
    let header_pair = cx.child_pair(scope, table.header_fill.as_ref(), body_pair.as_ref());
    let header_rows = table.header_rows.unwrap_or(0) as usize;
    let header_style = table.header_style.as_deref();
    for (index, row) in table.rows.iter_mut().enumerate() {
        let is_header = index < header_rows;
        let row_pair = if is_header { &header_pair } else { &body_pair };
        let row_style = header_style.filter(|_| is_header);
        for cell in &mut row.cells {
            let cell_pair = cx.child_pair(scope, cell.fill.as_ref(), row_pair.as_ref());
            for child in &mut cell.children {
                lower_node(child, cx, scope, cell_pair.as_ref(), row_style);
            }
        }
    }
}
