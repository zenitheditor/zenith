//! Container chains: the translating containers between a page and a child
//! space, and the px shift between two child spaces.

use std::collections::BTreeMap;

use zenith_core::{Document, FrameNode, Node, ResolvedToken, resolve_geometry_px, resolve_tokens};

use super::offset::{Scope, own_offset, placed_anchor_origin};

/// One container between a page (or master) and a child space.
#[derive(Debug, Clone, PartialEq)]
pub(in crate::engine) struct Link {
    /// The container id (a node kind name for an unknown node without one).
    pub id: String,
    /// The px offset the container adds to its children, or `None` when it
    /// does not resolve from authored values.
    pub offset: Option<(f64, f64)>,
}

/// The containers from a page (or master) down to a child space, outermost
/// first. A page or master child list gives an empty chain.
pub(in crate::engine) type Chain = Vec<Link>;

/// The resolved token table of `doc`, for px geometry reads.
pub(in crate::engine) fn resolved_tokens(doc: &Document) -> BTreeMap<String, ResolvedToken> {
    resolve_tokens(&doc.tokens).resolved
}

/// The chain of the child space of `container_id`: a page, a master, a
/// `frame`, or a `group`. It ends with the container itself. `None` when no
/// such container exists.
pub(in crate::engine) fn container_chain(
    doc: &Document,
    container_id: &str,
    resolved: &BTreeMap<String, ResolvedToken>,
) -> Option<Chain> {
    walk_roots(
        doc,
        Query::Container(container_id),
        resolved,
        &mut |_, _| {},
    )
}

/// The chain of the child space that holds the `x` / `y` of node `node_id`.
/// `None` when no node has that id.
pub(in crate::engine) fn parent_chain(
    doc: &Document,
    node_id: &str,
    resolved: &BTreeMap<String, ResolvedToken>,
) -> Option<Chain> {
    walk_roots(doc, Query::Parent(node_id), resolved, &mut |_, _| {})
}

/// The `(x, y)` the anchor of node `node_id` gives it, in the space that
/// holds its `x` / `y`, as the scene derives it. `None` when no node has
/// that id, a layout frame places it in flow, it has no anchor, or a box
/// the anchor needs does not resolve from authored values.
pub(in crate::engine) fn anchor_origin_of(
    doc: &Document,
    node_id: &str,
    resolved: &BTreeMap<String, ResolvedToken>,
) -> Option<(f64, f64)> {
    let mut origin = None;
    walk_roots(doc, Query::Parent(node_id), resolved, &mut |node, scope| {
        origin = placed_anchor_origin(node, scope);
    });
    origin
}

/// The number of leading links every chain in `chains` shares.
pub(in crate::engine) fn common_prefix(chains: &[&[Link]]) -> usize {
    let Some((first, rest)) = chains.split_first() else {
        return 0;
    };
    first
        .iter()
        .enumerate()
        .take_while(|(i, link)| {
            rest.iter()
                .all(|c| c.get(*i).map(|l| &l.id) == Some(&link.id))
        })
        .count()
}

/// The px origin of a chain's child space, counted from the space after its
/// first `skip` links. `Err` names the first link whose offset does not
/// resolve.
pub(in crate::engine) fn chain_origin(chain: &[Link], skip: usize) -> Result<(f64, f64), String> {
    let mut origin = (0.0, 0.0);
    for link in chain.iter().skip(skip) {
        let (dx, dy) = link.offset.ok_or_else(|| link.id.clone())?;
        origin = (origin.0 + dx, origin.1 + dy);
    }
    Ok(origin)
}

/// The px shift that moves coordinates from the child space of `from` into
/// the child space of `to`. Only links below the shared ancestor count, so
/// two spaces inside one unresolved container still convert. `Err` names
/// the first link that does not resolve.
pub(in crate::engine) fn shift_between(from: &[Link], to: &[Link]) -> Result<(f64, f64), String> {
    let shared = common_prefix(&[from, to]);
    let (fx, fy) = chain_origin(from, shared)?;
    let (tx, ty) = chain_origin(to, shared)?;
    Ok((fx - tx, fy - ty))
}

/// The frame that directly holds node `id`, or `None` when its parent is a
/// page, a master, or a non-frame container.
pub(in crate::engine) fn parent_frame<'d>(doc: &'d Document, id: &str) -> Option<&'d FrameNode> {
    let roots = doc
        .body
        .pages
        .iter()
        .map(|p| p.children.as_slice())
        .chain(doc.masters.iter().map(|m| m.children.as_slice()));
    for children in roots {
        if let Some(found) = find_parent_frame(children, None, id) {
            return found;
        }
    }
    None
}

/// `Some(parent)` once node `id` is found below `nodes`, where `parent` is
/// the frame that holds `nodes`.
fn find_parent_frame<'d>(
    nodes: &'d [Node],
    parent: Option<&'d FrameNode>,
    id: &str,
) -> Option<Option<&'d FrameNode>> {
    for node in nodes {
        if node.id() == Some(id) {
            return Some(parent);
        }
        let found = match node {
            Node::Frame(f) => find_parent_frame(&f.children, Some(f), id),
            Node::Group(g) => find_parent_frame(&g.children, None, id),
            Node::Table(t) => t
                .rows
                .iter()
                .flat_map(|r| r.cells.iter())
                .find_map(|c| find_parent_frame(&c.children, None, id)),
            Node::Unknown(u) => find_parent_frame(&u.children, None, id),
            Node::Rect(_)
            | Node::Ellipse(_)
            | Node::Line(_)
            | Node::Text(_)
            | Node::Code(_)
            | Node::Image(_)
            | Node::Polygon(_)
            | Node::Polyline(_)
            | Node::Path(_)
            | Node::Instance(_)
            | Node::Field(_)
            | Node::Footnote(_)
            | Node::Toc(_)
            | Node::Shape(_)
            | Node::Connector(_)
            | Node::Pattern(_)
            | Node::Chart(_)
            | Node::Light(_)
            | Node::Mesh(_) => None,
        };
        if found.is_some() {
            return found;
        }
    }
    None
}

/// What a walk looks for.
#[derive(Clone, Copy)]
enum Query<'q> {
    /// The child space of the container with this id.
    Container(&'q str),
    /// The space that holds the node with this id.
    Parent(&'q str),
}

/// Called once with the queried node and its scope when a
/// [`Query::Parent`] walk finds it.
type Hit<'h> = dyn FnMut(&Node, &Scope<'_>) + 'h;

fn walk_roots(
    doc: &Document,
    query: Query<'_>,
    resolved: &BTreeMap<String, ResolvedToken>,
    hit: &mut Hit<'_>,
) -> Option<Chain> {
    // Master chrome projects onto pages of any size: no page for anchors.
    let roots = doc
        .body
        .pages
        .iter()
        .map(|p| (p.id.as_str(), p.children.as_slice(), Some(p)))
        .chain(
            doc.masters
                .iter()
                .map(|m| (m.id.as_str(), m.children.as_slice(), None)),
        );
    for (root_id, children, page) in roots {
        if let Query::Container(id) = query
            && id == root_id
        {
            return Some(Vec::new());
        }
        let mut chain = Vec::new();
        let scope = Scope {
            siblings: children,
            parent: None,
            parent_size: None,
            page,
            acc: Some((0.0, 0.0)),
            resolved,
        };
        if walk(&scope, &mut chain, query, hit) {
            return Some(chain);
        }
    }
    None
}

/// Search the child list of `scope`, whose space `chain` describes. `true`
/// once the query is answered: `chain` then holds the answer.
fn walk(scope: &Scope<'_>, chain: &mut Chain, query: Query<'_>, hit: &mut Hit<'_>) -> bool {
    let resolved = scope.resolved;
    for node in scope.siblings {
        if let Query::Parent(id) = query
            && node.id() == Some(id)
        {
            hit(node, scope);
            return true;
        }
        let is_target = |cid: &str| matches!(query, Query::Container(id) if id == cid);
        let found = match node {
            Node::Frame(f) => {
                let offset = own_offset(node, scope);
                chain.push(Link {
                    id: f.id.clone(),
                    offset,
                });
                // The anchor-parent box of a plain frame is its authored box.
                let px = |v: Option<&zenith_core::PropertyValue>| resolve_geometry_px(v, resolved);
                let plain = f.layout.as_ref().is_none_or(|k| !k.positions_children());
                let parent_size =
                    (plain && px(f.x.as_ref()).is_some() && px(f.y.as_ref()).is_some())
                        .then(|| px(f.w.as_ref()).zip(px(f.h.as_ref())))
                        .flatten();
                let inner = scope.enter(&f.children, Some(f), parent_size, offset);
                is_target(&f.id) || walk(&inner, chain, query, hit)
            }
            Node::Group(g) => {
                let offset = own_offset(node, scope);
                chain.push(Link {
                    id: g.id.clone(),
                    offset,
                });
                let px = |v: Option<&zenith_core::PropertyValue>| resolve_geometry_px(v, resolved);
                let parent_size = px(g.w.as_ref()).zip(px(g.h.as_ref()));
                let inner = scope.enter(&g.children, None, parent_size, offset);
                is_target(&g.id) || walk(&inner, chain, query, hit)
            }
            Node::Table(t) => {
                // The table places its cells: cell content has no authored origin.
                chain.push(Link {
                    id: t.id.clone(),
                    offset: None,
                });
                t.rows.iter().flat_map(|r| r.cells.iter()).any(|c| {
                    walk(
                        &scope.enter(&c.children, None, None, None),
                        chain,
                        query,
                        hit,
                    )
                })
            }
            Node::Unknown(u) => {
                chain.push(Link {
                    id: node.id_or_kind().to_owned(),
                    offset: None,
                });
                walk(
                    &scope.enter(&u.children, None, None, None),
                    chain,
                    query,
                    hit,
                )
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
            | Node::Instance(_)
            | Node::Field(_)
            | Node::Footnote(_)
            | Node::Toc(_)
            | Node::Shape(_)
            | Node::Connector(_)
            | Node::Pattern(_)
            | Node::Chart(_)
            | Node::Light(_)
            | Node::Mesh(_) => continue,
        };
        if found {
            return true;
        }
        chain.pop();
    }
    false
}

#[cfg(test)]
mod tests {
    use zenith_core::{KdlAdapter, KdlSource};

    use super::*;

    fn doc(body: &str) -> Document {
        let src = format!(
            r##"zenith version=1 {{
  project id="p" name="P"
  tokens format="zenith-token-v1" {{
    token id="dim.x" type="dimension" value=(px)7
  }}
  styles {{}}
  document id="d" title="D" {{
    page id="pg" w=(px)400 h=(px)300 {{
      {body}
    }}
  }}
}}"##
        );
        KdlAdapter.parse(src.as_bytes()).expect("parse")
    }

    const NESTED: &str = r#"frame id="f" x=(px)10 y=(px)20 w=(px)200 h=(px)200 {
        group id="g" x=(token)"dim.x" y=(px)3 {
          rect id="r" x=(px)1 y=(px)1 w=(px)5 h=(px)5
        }
      }
      frame id="col" x=(px)0 y=(px)0 w=(px)100 h=(px)100 layout="column" {
        frame id="slot" w=(px)50 h=(px)50 {
          group id="g1" x=(px)4 y=(px)0 {
            rect id="deep" x=(px)1 y=(px)1 w=(px)5 h=(px)5
          }
          group id="g2" x=(px)0 y=(px)9 { }
        }
      }
      group id="spun" x=(px)5 y=(px)5 rotate=(deg)30 { }"#;

    fn origin(chain: Option<Chain>) -> Result<(f64, f64), String> {
        chain_origin(&chain.expect("found"), 0)
    }

    #[test]
    fn origins_sum_every_translating_ancestor() {
        let d = doc(NESTED);
        let r = resolved_tokens(&d);
        assert_eq!(container_chain(&d, "pg", &r), Some(Vec::new()));
        assert_eq!(origin(container_chain(&d, "f", &r)), Ok((10.0, 20.0)));
        assert_eq!(origin(container_chain(&d, "g", &r)), Ok((17.0, 23.0)));
        assert_eq!(origin(parent_chain(&d, "r", &r)), Ok((17.0, 23.0)));
        assert_eq!(origin(parent_chain(&d, "g", &r)), Ok((10.0, 20.0)));
        assert_eq!(container_chain(&d, "r", &r), None);
        assert_eq!(container_chain(&d, "missing", &r), None);
    }

    #[test]
    fn in_flow_and_rotated_containers_do_not_resolve() {
        let d = doc(NESTED);
        let r = resolved_tokens(&d);
        assert_eq!(origin(container_chain(&d, "col", &r)), Ok((0.0, 0.0)));
        assert_eq!(
            origin(container_chain(&d, "slot", &r)),
            Err("slot".to_owned())
        );
        assert_eq!(origin(parent_chain(&d, "deep", &r)), Err("slot".to_owned()));
        assert_eq!(
            origin(container_chain(&d, "spun", &r)),
            Err("spun".to_owned())
        );
    }

    #[test]
    fn shift_below_a_shared_unresolved_container_resolves() {
        let d = doc(NESTED);
        let r = resolved_tokens(&d);
        let from = parent_chain(&d, "deep", &r).expect("deep");
        let to = container_chain(&d, "g2", &r).expect("g2");
        assert_eq!(shift_between(&from, &to), Ok((4.0, -9.0)));
        let page = container_chain(&d, "pg", &r).expect("pg");
        assert_eq!(shift_between(&from, &page), Err("slot".to_owned()));
    }
}
