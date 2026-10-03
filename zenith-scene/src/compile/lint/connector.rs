//! `connector.crosses_node`: a connector's drawn route runs through the box
//! of a node it does not connect.
//!
//! The route is the stroked polyline the connector compiled to, in page px.
//! An obstacle is a visible, unrotated `rect`, `ellipse`, `shape`, `image`,
//! `frame`, `text`, `code`, `field`, `toc`, `table`, or `chart` that is not
//! `role="decoration"` / `"background"` and draws under no unmodeled
//! ancestor. These are never obstacles: the endpoints, their ancestors and
//! descendants (an endpoint `instance` includes its expanded content), the
//! connector's ancestors, and any node whose box holds another node's box (a
//! panel, card, or lane behind other content). A route crosses an obstacle
//! when one of its segments enters the box inset by 2px on every side.

use std::collections::{BTreeMap, BTreeSet};

use zenith_core::{Diagnostic, FixHint};

use crate::layout::LayoutBox;

use super::arrange::num;
use super::ledger::{Entry, PageLedger};

/// Inset of the obstacle box before the crossing test, in px.
const INSET: f64 = 2.0;
/// Slack when one box holds another, in px.
const HOLD_SLACK: f64 = 0.5;
/// Most obstacles a message names.
const MAX_NAMED: usize = 3;

/// One obstacle a route runs through.
struct Crossing {
    /// The obstacle entry.
    node: usize,
    /// Where the route enters and leaves the inset box.
    enter: (f64, f64),
    exit: (f64, f64),
}

/// The node kinds that block a route.
fn blocks(kind: &str) -> bool {
    matches!(
        kind,
        "rect"
            | "ellipse"
            | "shape"
            | "image"
            | "frame"
            | "text"
            | "code"
            | "field"
            | "toc"
            | "table"
            | "chart"
    )
}

/// `true` when `outer` holds `inner` (within the slack).
fn holds(outer: LayoutBox, inner: LayoutBox) -> bool {
    inner.x >= outer.x - HOLD_SLACK
        && inner.y >= outer.y - HOLD_SLACK
        && inner.x + inner.w <= outer.x + outer.w + HOLD_SLACK
        && inner.y + inner.h <= outer.y + outer.h + HOLD_SLACK
}

/// A candidate obstacle: drawn, solid, and not a backdrop of other nodes.
fn obstacle(entry: &Entry) -> Option<LayoutBox> {
    let b = entry.compiled?;
    let drawn = entry.visible && !entry.exempt && !entry.unmodeled && b.rotate.is_none();
    (drawn && blocks(entry.kind)).then_some(b.rect)
}

/// Every `connector.crosses_node` of the page.
pub(super) fn crosses_node(
    ledger: &PageLedger,
    routes: &BTreeMap<String, Vec<(f64, f64)>>,
) -> Vec<Diagnostic> {
    let connectors: Vec<(usize, &Entry)> = ledger
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.connector.is_some() && e.visible && !e.exempt)
        .filter(|(_, e)| routes.contains_key(&e.id))
        .collect();
    if connectors.is_empty() {
        return Vec::new();
    }
    let obstacles = backdrop_free_obstacles(ledger);
    let by_id: BTreeMap<&str, usize> = ledger
        .entries
        .iter()
        .enumerate()
        .map(|(i, e)| (e.id.as_str(), i))
        .collect();
    let mut out = Vec::new();
    for (index, entry) in connectors {
        let (Some(facts), Some(route)) = (&entry.connector, routes.get(&entry.id)) else {
            continue;
        };
        let ends: Vec<&str> = facts.ends.iter().flatten().map(String::as_str).collect();
        if ends.len() < 2 || ends.first() == ends.get(1) {
            continue;
        }
        let end_index: Vec<usize> = ends.iter().filter_map(|e| by_id.get(e).copied()).collect();
        let mut crossed: Vec<Crossing> = Vec::new();
        for &(o, rect) in &obstacles {
            let related = o == index
                || ledger.is_ancestor(o, index)
                || end_index
                    .iter()
                    .any(|&e| e == o || ledger.is_ancestor(o, e) || ledger.is_ancestor(e, o))
                || ledger.entry(o).is_some_and(|ob| {
                    ends.iter()
                        .any(|e| ob.id.strip_prefix(e).is_some_and(|r| r.starts_with('/')))
                });
            if related {
                continue;
            }
            if let Some((a, b)) = route_hit(route, rect) {
                crossed.push(Crossing {
                    node: o,
                    enter: a,
                    exit: b,
                });
            }
        }
        if !crossed.is_empty() {
            out.push(diagnostic(ledger, entry, &facts.route, &crossed));
        }
    }
    out
}

/// The obstacles of the page that hold no other node's box.
fn backdrop_free_obstacles(ledger: &PageLedger) -> Vec<(usize, LayoutBox)> {
    let boxes: Vec<(usize, LayoutBox)> = ledger
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.visible && e.kind != "connector")
        .filter_map(|(i, e)| e.compiled.map(|b| (i, b.rect)))
        .collect();
    let holders: BTreeSet<usize> = boxes
        .iter()
        .filter(|(i, outer)| {
            boxes.iter().any(|(j, inner)| {
                j != i && (inner.w > 0.0 || inner.h > 0.0) && holds(*outer, *inner)
            })
        })
        .map(|(i, _)| *i)
        .collect();
    ledger
        .entries
        .iter()
        .enumerate()
        .filter(|(i, _)| !holders.contains(i))
        .filter_map(|(i, e)| obstacle(e).map(|r| (i, r)))
        .collect()
}

/// The first part of `route` inside `rect` inset by [`INSET`].
fn route_hit(route: &[(f64, f64)], rect: LayoutBox) -> Option<((f64, f64), (f64, f64))> {
    let inner = LayoutBox {
        x: rect.x + INSET,
        y: rect.y + INSET,
        w: rect.w - 2.0 * INSET,
        h: rect.h - 2.0 * INSET,
    };
    if inner.w <= 0.0 || inner.h <= 0.0 {
        return None;
    }
    route.windows(2).find_map(|pair| match pair {
        [a, b] => clip(*a, *b, inner),
        _ => None,
    })
}

/// The part of segment `a`–`b` inside `r` with positive length
/// (Liang–Barsky).
fn clip(a: (f64, f64), b: (f64, f64), r: LayoutBox) -> Option<((f64, f64), (f64, f64))> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let mut t0 = 0.0_f64;
    let mut t1 = 1.0_f64;
    for (p, q) in [
        (-dx, a.0 - r.x),
        (dx, r.x + r.w - a.0),
        (-dy, a.1 - r.y),
        (dy, r.y + r.h - a.1),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
            continue;
        }
        let t = q / p;
        if p < 0.0 {
            t0 = t0.max(t);
        } else {
            t1 = t1.min(t);
        }
        if t0 >= t1 {
            return None;
        }
    }
    let at = |t: f64| (a.0 + t * dx, a.1 + t * dy);
    Some((at(t0), at(t1)))
}

fn diagnostic(
    ledger: &PageLedger,
    connector: &Entry,
    route: &str,
    crossed: &[Crossing],
) -> Diagnostic {
    let named: Vec<String> = crossed
        .iter()
        .take(MAX_NAMED)
        .filter_map(|c| {
            let (a, b) = (c.enter, c.exit);
            let e = ledger.entry(c.node)?;
            Some(format!(
                "{} '{}' from ({},{}) to ({},{})",
                e.kind,
                e.id,
                num(a.0),
                num(a.1),
                num(b.0),
                num(b.1)
            ))
        })
        .collect();
    let more = crossed.len().saturating_sub(MAX_NAMED);
    let tail = if more > 0 {
        format!(" and {more} more")
    } else {
        String::new()
    };
    let head = format!(
        "connector '{}' (route=\"{route}\") runs through {}{tail}",
        connector.id,
        named.join(", ")
    );
    let first = crossed
        .first()
        .and_then(|c| ledger.entry(c.node))
        .map_or("", |e| e.id.as_str());
    let (message, fix) = if route == "avoid" {
        (
            format!(
                "{head} — the avoid router found no clear path; move '{first}' off the line \
                 between the connector's endpoints"
            ),
            None,
        )
    } else {
        (
            format!("{head} — set route=\"avoid\" to route around it"),
            Some(FixHint::SetProperty {
                property: "route".to_owned(),
                to: "avoid".to_owned(),
            }),
        )
    };
    Diagnostic::advisory(
        "connector.crosses_node",
        message,
        connector.span,
        Some(connector.id.clone()),
    )
    .with_fix(fix)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(x: f64, y: f64, w: f64, h: f64) -> LayoutBox {
        LayoutBox { x, y, w, h }
    }

    #[test]
    fn clip_keeps_the_inside_part() {
        let hit = clip((0.0, 50.0), (200.0, 50.0), b(50.0, 0.0, 100.0, 100.0));
        assert_eq!(hit, Some(((50.0, 50.0), (150.0, 50.0))));
        assert_eq!(
            clip((0.0, 150.0), (200.0, 150.0), b(50.0, 0.0, 100.0, 100.0)),
            None
        );
    }

    #[test]
    fn a_route_grazing_the_edge_misses_the_inset_box() {
        let route = [(0.0, 1.0), (200.0, 1.0)];
        assert_eq!(route_hit(&route, b(50.0, 0.0, 100.0, 100.0)), None);
        let route = [(0.0, 10.0), (200.0, 10.0)];
        assert!(route_hit(&route, b(50.0, 0.0, 100.0, 100.0)).is_some());
    }
}
