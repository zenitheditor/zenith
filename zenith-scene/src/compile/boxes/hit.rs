//! [`hit_test`]: the compiled nodes under a page point.

use std::collections::BTreeMap;

use super::compiled::CompiledBox;

/// The ids of every box under the page point `(x, y)`, topmost first.
///
/// Rules:
/// - A box is hit when [`CompiledBox::contains`] holds: the point lies
///   inside every open clip and in the node. A line, polygon, polyline,
///   path, or connector with a `shape` is in when the point lies in a fill
///   (under its fill rule, so holes and concave notches miss) or a stroke
///   band (see [`HitShape`](super::HitShape)); a stroke-only shape misses
///   inside its outline. Any other node is in when the point lies in the
///   drawn box (`local` through `world` and `spin`, edges included). Masks
///   and transparent paint do not cut a node.
/// - Order: `paint_order`, largest first. A child ranks above its parent,
///   and a later sibling (with all its descendants) above an earlier one.
/// - `hidden` boxes (`visible=false`) are never hit. Their whole subtree
///   draws nothing and records no box.
/// - A zero-width or zero-height box is hit only on its edge.
/// - Guide nodes (`role="guide"`) and their subtrees have no box and are
///   never hit.
/// - Ids come back raw: master projections as `<page-id>/<id>`, instance
///   content as `<instance-id>/<id>`, pattern motifs as
///   `<pattern-id>/<index>/<motif-id>`. Map them with
///   [`selectable_id`](super::selectable_id).
/// - A non-finite point hits nothing.
#[must_use]
pub fn hit_test(boxes: &BTreeMap<String, CompiledBox>, x: f64, y: f64) -> Vec<&str> {
    let mut hits: Vec<(&str, usize)> = boxes
        .iter()
        .filter(|(_, b)| !b.hidden && b.contains(x, y))
        .map(|(id, b)| (id.as_str(), b.paint_order))
        .collect();
    hits.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    hits.into_iter().map(|(id, _)| id).collect()
}

/// [`hit_test`] with click slop for thin boxes: the ids of every box under
/// or near the page point `(x, y)`, topmost first.
///
/// A box counts when [`hit_test`] would hit it, or when `tolerance` is
/// finite and `> 0` and it is near the point.
///
/// A box with a `shape` is near when either holds, and the region's point
/// nearest to `(x, y)` lies inside every open clip:
/// - a stroke band lies within `tolerance` page px of the point (the band
///   reach scaled by the square root of the stroke map's area scale);
/// - the box is thin (shorter drawn side at most `2 · tolerance` page px)
///   and a fill edge lies within `tolerance` page px of the point.
///
/// A box without a `shape` is near when all of these hold:
/// - the box is thin: its shorter drawn side (see
///   [`CompiledBox::drawn_size`]) is at most `2 · tolerance` page px, as for
///   a hairline, a rule, or an axis-aligned line;
/// - the page distance from the point to the drawn box is at most
///   `tolerance` (see [`CompiledBox::nearest`]);
/// - the nearest point of the box lies inside every open clip.
///
/// Large boxes get no slop, so a near miss never steals a click from the
/// box under the point, and slop never fills a stroke-only interior. Order
/// and the `hidden` rule match [`hit_test`].
#[must_use]
pub fn hit_test_within(
    boxes: &BTreeMap<String, CompiledBox>,
    x: f64,
    y: f64,
    tolerance: f64,
) -> Vec<&str> {
    let slop = tolerance.is_finite() && tolerance > 0.0;
    let mut hits: Vec<(&str, usize)> = boxes
        .iter()
        .filter(|(_, b)| !b.hidden && (b.contains(x, y) || (slop && near(b, x, y, tolerance))))
        .map(|(id, b)| (id.as_str(), b.paint_order))
        .collect();
    hits.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    hits.into_iter().map(|(id, _)| id).collect()
}

/// `true` when `b` lies within `tolerance` of `(x, y)` at a point every
/// clip keeps: a stroke band of its shape, or a thin box or thin shape's
/// fill edge.
fn near(b: &CompiledBox, x: f64, y: f64, tolerance: f64) -> bool {
    let (w, h) = b.drawn_size();
    let thin = w.min(h) <= 2.0 * tolerance;
    let kept = |found: Option<(f64, (f64, f64))>| {
        found.is_some_and(|(d, (nx, ny))| {
            d <= tolerance && b.clip.iter().all(|c| c.contains(nx, ny))
        })
    };
    match &b.shape {
        Some(shape) => {
            kept(shape.nearest_stroke(x, y)) || (thin && kept(shape.nearest_fill_edge(x, y)))
        }
        None => thin && kept(b.nearest(x, y)),
    }
}

#[cfg(test)]
mod tests {
    use super::super::affine::Affine2;
    use super::super::clip::ClipShape;
    use super::*;
    use crate::layout::LayoutBox;

    fn unit(x: f64, y: f64, w: f64, h: f64, paint_order: usize) -> CompiledBox {
        let rect = LayoutBox { x, y, w, h };
        CompiledBox {
            rect,
            rotate: None,
            visual: rect,
            local: rect,
            spin: Affine2::IDENTITY,
            world: Affine2::IDENTITY,
            clip: Vec::new(),
            paint_order,
            command_index: paint_order,
            hidden: false,
            shape: None,
        }
    }

    #[test]
    fn later_paint_wins_and_hidden_and_clipped_boxes_miss() {
        let mut boxes = BTreeMap::new();
        boxes.insert("a".to_owned(), unit(0.0, 0.0, 10.0, 10.0, 0));
        boxes.insert("b".to_owned(), unit(5.0, 5.0, 10.0, 10.0, 1));
        let mut hidden = unit(0.0, 0.0, 20.0, 20.0, 2);
        hidden.hidden = true;
        boxes.insert("h".to_owned(), hidden);
        let mut clipped = unit(0.0, 0.0, 20.0, 20.0, 3);
        clipped.clip.push(ClipShape {
            world: Affine2::IDENTITY,
            rect: LayoutBox {
                x: 0.0,
                y: 0.0,
                w: 6.0,
                h: 6.0,
            },
            radius: 0.0,
        });
        boxes.insert("c".to_owned(), clipped);
        assert_eq!(hit_test(&boxes, 5.5, 5.5), vec!["c", "b", "a"]);
        assert_eq!(hit_test(&boxes, 8.0, 8.0), vec!["b", "a"]);
        assert!(hit_test(&boxes, f64::NAN, 1.0).is_empty());
    }

    #[test]
    fn slop_reaches_thin_boxes_only() {
        let mut boxes = BTreeMap::new();
        boxes.insert("big".to_owned(), unit(0.0, 0.0, 100.0, 100.0, 0));
        boxes.insert("rule".to_owned(), unit(10.0, 50.0, 80.0, 1.0, 1));
        // 3 px above the rule: inside big, near the rule.
        assert_eq!(
            hit_test_within(&boxes, 20.0, 47.0, 4.0),
            vec!["rule", "big"]
        );
        assert_eq!(hit_test_within(&boxes, 20.0, 47.0, 2.0), vec!["big"]);
        // Just outside big: the big box takes no slop.
        assert!(hit_test_within(&boxes, 101.0, 5.0, 4.0).is_empty());
        // Zero tolerance is the plain hit test.
        assert_eq!(
            hit_test_within(&boxes, 20.0, 50.5, 0.0),
            hit_test(&boxes, 20.0, 50.5)
        );
        assert!(hit_test_within(&boxes, f64::NAN, 1.0, 4.0).is_empty());
    }

    #[test]
    fn slop_follows_rotation_and_clips() {
        let mut boxes = BTreeMap::new();
        let mut rule = unit(0.0, 0.0, 100.0, 1.0, 0);
        rule.world = Affine2::rotate_at(90.0, 0.0, 0.0);
        boxes.insert("v".to_owned(), rule.clone());
        // Rotated a quarter turn: the rule runs down x = 0 .. -1.
        assert_eq!(hit_test_within(&boxes, 2.0, 50.0, 3.0), vec!["v"]);
        assert!(hit_test_within(&boxes, 50.0, 2.0, 3.0).is_empty());
        rule.clip.push(ClipShape {
            world: Affine2::IDENTITY,
            rect: LayoutBox {
                x: -10.0,
                y: 0.0,
                w: 20.0,
                h: 20.0,
            },
            radius: 0.0,
        });
        boxes.insert("v".to_owned(), rule);
        assert!(hit_test_within(&boxes, 2.0, 50.0, 3.0).is_empty());
        assert_eq!(hit_test_within(&boxes, 2.0, 10.0, 3.0), vec!["v"]);
    }

    #[test]
    fn nearest_point_of_a_box() {
        let b = unit(0.0, 0.0, 10.0, 10.0, 0);
        assert_eq!(b.nearest(5.0, 5.0), Some((0.0, (5.0, 5.0))));
        assert_eq!(b.nearest(13.0, 14.0), Some((5.0, (10.0, 10.0))));
        assert_eq!(b.drawn_size(), (10.0, 10.0));
    }

    #[test]
    fn a_zero_width_box_is_hit_only_on_its_edge() {
        let mut boxes = BTreeMap::new();
        boxes.insert("v".to_owned(), unit(10.0, 0.0, 0.0, 10.0, 0));
        assert_eq!(hit_test(&boxes, 10.0, 5.0), vec!["v"]);
        assert!(hit_test(&boxes, 10.5, 5.0).is_empty());
    }
}
