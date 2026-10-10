//! [`CompiledBox`]: the final geometry of one compiled node.

use crate::layout::LayoutBox;

use super::affine::Affine2;
use super::clip::{ClipShape, in_rounded_rect};
use super::region::{clip_convex, convex_overlap, point_in_convex, usable};
use super::shape::HitShape;

/// The final geometry of one compiled node, in page-absolute px.
///
/// `rect`, `rotate`, and `visual` are axis-aligned summaries. `world`,
/// `spin`, and `local` give the exact drawn box: [`CompiledBox::corners`]
/// maps the corners of `local` through [`CompiledBox::transform`].
#[derive(Clone, Debug, PartialEq)]
pub struct CompiledBox {
    /// The unrotated box the compile used. Under a rotated or scaled
    /// ancestor it is the axis-aligned bounds of that box.
    pub rect: LayoutBox,
    /// The node's own rotation in degrees, when set and non-zero.
    pub rotate: Option<f64>,
    /// The axis-aligned bounds of what the node paints, every transform
    /// applied; `rect` (rotated by `rotate`) when nothing paints.
    pub visual: LayoutBox,
    /// The box before any transform, in the node's own space. A box node
    /// takes its resolved box. Any other node takes the bounds of what it
    /// paints with its own rotation left out (and its descendants'
    /// rotations kept).
    pub local: LayoutBox,
    /// The node's own rotation as the compile drew it, in the space of
    /// `local`. Identity when the compile drew none (no `rotate`, or a
    /// rotation the node kind skips, such as `text` without `h`).
    pub spin: Affine2,
    /// The transform open where the node starts: every ancestor rotation,
    /// instance fit scale, and offset, from the node's space to page px.
    pub world: Affine2,
    /// The clips open where the node starts, in page px. A point is visible
    /// only inside all of them. Empty when nothing clips (a page always has
    /// its media clip).
    pub clip: Vec<ClipShape>,
    /// The node's rank in paint order: nodes are numbered as the compile
    /// starts them, a parent before its children and an earlier sibling
    /// before a later one. A larger rank paints on top.
    pub paint_order: usize,
    /// Index of the node's first command in the page's scene commands.
    /// A node that draws nothing takes the index of the next command.
    pub command_index: usize,
    /// The node has `visible=false`: the compile drew nothing for it.
    pub hidden: bool,
    /// The exact painted region, for a `line`, `polygon`, `polyline`,
    /// `path`, or `connector` that paints only lines, polygons, polylines,
    /// and paths. `None` for every other node: the drawn box stands in.
    pub shape: Option<HitShape>,
}

impl CompiledBox {
    /// The map from the space of `local` to page px: `world ∘ spin`.
    #[must_use]
    pub fn transform(&self) -> Affine2 {
        self.world.then(self.spin)
    }

    /// The corners of the drawn box in page px: top-left, top-right,
    /// bottom-right, bottom-left of `local`, through
    /// [`CompiledBox::transform`]. Selection handles sit here.
    #[must_use]
    pub fn corners(&self) -> [(f64, f64); 4] {
        let m = self.transform();
        let LayoutBox { x, y, w, h } = self.local;
        [
            m.apply(x, y),
            m.apply(x + w, y),
            m.apply(x + w, y + h),
            m.apply(x, y + h),
        ]
    }

    /// `true` when the page point `(x, y)` lies inside the node and inside
    /// every open clip. With a `shape`, inside means in a fill or a stroke
    /// band (see [`HitShape`]). Without one, inside means in the drawn box,
    /// edges included: a box with a zero width or height holds only the
    /// points on its edge, and a singular transform (a zero scale) holds no
    /// point.
    #[must_use]
    pub fn contains(&self, x: f64, y: f64) -> bool {
        if let Some(shape) = &self.shape {
            return shape.contains(x, y) && self.clip.iter().all(|c| c.contains(x, y));
        }
        let Some(inverse) = self.transform().inverse() else {
            return false;
        };
        let (lx, ly) = inverse.apply(x, y);
        in_rounded_rect(self.local, 0.0, lx, ly) && self.clip.iter().all(|c| c.contains(x, y))
    }

    /// `true` when the node meets the convex polygon `region` (page px,
    /// either winding) inside every open clip.
    ///
    /// With a `shape`, the node meets the region where a fill or a stroke
    /// band does (see [`HitShape::touches`]); without one, where its drawn
    /// box does (see [`CompiledBox::corners`]). Clips count as their square
    /// rectangles: a rounded clip corner keeps a little more than it draws.
    /// A region with fewer than three points or no area meets nothing.
    #[must_use]
    pub fn touches_region(&self, region: &[(f64, f64)]) -> bool {
        if !usable(region) {
            return false;
        }
        let mut visible: Vec<(f64, f64)> = region.to_vec();
        for clip in &self.clip {
            let LayoutBox { x, y, w, h } = clip.rect;
            if w < 0.0 || h < 0.0 {
                return false;
            }
            let m = clip.world;
            let square = [
                m.apply(x, y),
                m.apply(x + w, y),
                m.apply(x + w, y + h),
                m.apply(x, y + h),
            ];
            visible = clip_convex(&visible, &square);
            if visible.is_empty() {
                return false;
            }
        }
        match &self.shape {
            Some(shape) => shape.touches(&visible),
            None => convex_overlap(&self.corners(), &visible),
        }
    }

    /// `true` when the whole drawn box (see [`CompiledBox::corners`]) lies
    /// inside the convex polygon `region` (page px, either winding), edges
    /// included. Clips are not applied.
    #[must_use]
    pub fn within_region(&self, region: &[(f64, f64)]) -> bool {
        usable(region) && self.corners().iter().all(|c| point_in_convex(*c, region))
    }

    /// `true` when every point of `region` (page px) lies inside the drawn
    /// box (see [`CompiledBox::corners`]), edges included: the box holds
    /// the whole region. Clips are not applied. `false` for an empty region
    /// or a box with no area.
    #[must_use]
    pub fn holds_region(&self, region: &[(f64, f64)]) -> bool {
        let corners = self.corners();
        !region.is_empty() && region.iter().all(|p| point_in_convex(*p, &corners))
    }

    /// The lengths in page px of the drawn box's top and left sides: its
    /// drawn width and height, every transform applied.
    #[must_use]
    pub fn drawn_size(&self) -> (f64, f64) {
        let [tl, tr, _, bl] = self.corners();
        (distance(tl, tr), distance(tl, bl))
    }

    /// The page px distance from `(x, y)` to the drawn box (its corners, see
    /// [`CompiledBox::corners`]), and the nearest point of the box. Distance
    /// 0 and the point itself when the point lies inside. Clips are not
    /// applied. `None` for a non-finite point or a singular transform.
    #[must_use]
    pub fn nearest(&self, x: f64, y: f64) -> Option<(f64, (f64, f64))> {
        if !(x.is_finite() && y.is_finite()) {
            return None;
        }
        let inverse = self.transform().inverse()?;
        let (lx, ly) = inverse.apply(x, y);
        if in_rounded_rect(self.local, 0.0, lx, ly) {
            return Some((0.0, (x, y)));
        }
        let c = self.corners();
        let mut best: Option<(f64, (f64, f64))> = None;
        for (i, &a) in c.iter().enumerate() {
            let b = c.get((i + 1) % c.len()).copied().unwrap_or(a);
            let p = nearest_on_segment((x, y), a, b);
            let d = distance((x, y), p);
            if best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, p));
            }
        }
        best
    }
}

fn distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    zenith_geometry::math::hypot(a.0 - b.0, a.1 - b.1)
}

/// The point of segment `a`–`b` nearest to `p`.
fn nearest_on_segment(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    let (vx, vy) = (b.0 - a.0, b.1 - a.1);
    let len2 = vx * vx + vy * vy;
    if len2 == 0.0 || !len2.is_finite() {
        return a;
    }
    let t = (((p.0 - a.0) * vx + (p.1 - a.1) * vy) / len2).clamp(0.0, 1.0);
    (a.0 + t * vx, a.1 + t * vy)
}
