//! Resize math: the eight grips of a box and the new box that keeps the
//! opposite grip fixed on the page, for a box turned about its centre.

use zenith_scene::Affine2;

use super::vector::{Pt, linear};

/// One of the eight resize grips of a box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Grip {
    Nw,
    N,
    Ne,
    E,
    Se,
    S,
    Sw,
    W,
}

impl Grip {
    /// Every grip, clockwise from the top-left corner.
    pub(crate) const ALL: [Grip; 8] = [
        Grip::Nw,
        Grip::N,
        Grip::Ne,
        Grip::E,
        Grip::Se,
        Grip::S,
        Grip::Sw,
        Grip::W,
    ];

    /// The grip a handle id names.
    pub(crate) fn parse(id: &str) -> Option<Grip> {
        Grip::ALL.into_iter().find(|g| g.id() == id)
    }

    /// The handle id: `nw`, `n`, `ne`, `e`, `se`, `s`, `sw`, or `w`.
    pub(crate) fn id(self) -> &'static str {
        match self {
            Grip::Nw => "nw",
            Grip::N => "n",
            Grip::Ne => "ne",
            Grip::E => "e",
            Grip::Se => "se",
            Grip::S => "s",
            Grip::Sw => "sw",
            Grip::W => "w",
        }
    }

    /// Where the grip sits on the box, as fractions of width and height
    /// from the top-left corner.
    pub(crate) fn fractions(self) -> Pt {
        match self {
            Grip::Nw => (0.0, 0.0),
            Grip::N => (0.5, 0.0),
            Grip::Ne => (1.0, 0.0),
            Grip::E => (1.0, 0.5),
            Grip::Se => (1.0, 1.0),
            Grip::S => (0.5, 1.0),
            Grip::Sw => (0.0, 1.0),
            Grip::W => (0.0, 0.5),
        }
    }

    /// The grip across the box.
    pub(crate) fn opposite(self) -> Grip {
        match self {
            Grip::Nw => Grip::Se,
            Grip::N => Grip::S,
            Grip::Ne => Grip::Sw,
            Grip::E => Grip::W,
            Grip::Se => Grip::Nw,
            Grip::S => Grip::N,
            Grip::Sw => Grip::Ne,
            Grip::W => Grip::E,
        }
    }

    /// `true` for a grip that changes the width.
    pub(crate) fn moves_x(self) -> bool {
        self.fractions().0 != 0.5
    }

    /// `true` for a grip that changes the height.
    pub(crate) fn moves_y(self) -> bool {
        self.fractions().1 != 0.5
    }
}

/// An axis-aligned box in a node's own (unrotated) space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Rect {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) w: f64,
    pub(crate) h: f64,
}

impl Rect {
    /// The point at fractions `f` of the box.
    pub(crate) fn at(self, f: Pt) -> Pt {
        (self.x + f.0 * self.w, self.y + f.1 * self.h)
    }
}

/// One resize drag.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Drag {
    /// The grip being dragged.
    pub(crate) grip: Grip,
    /// The pointer delta in the box's own unrotated frame.
    pub(crate) delta: Pt,
    /// Keep the width : height ratio.
    pub(crate) constrain: bool,
    /// Grow about the centre: both sides move.
    pub(crate) from_center: bool,
}

/// The change to a box: deltas of its corner and size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Resized {
    pub(crate) dx: f64,
    pub(crate) dy: f64,
    pub(crate) dw: f64,
    pub(crate) dh: f64,
}

impl Resized {
    /// The box after the change.
    pub(crate) fn apply(self, r: Rect) -> Rect {
        Rect {
            x: r.x + self.dx,
            y: r.y + self.dy,
            w: r.w + self.dw,
            h: r.h + self.dh,
        }
    }
}

/// The change that resizes `rect` per `drag`, for a box drawn turned by
/// `spin` about its own centre (the linear part of `spin` counts).
///
/// The fixed point is the grip opposite the dragged one (the centre with
/// `from_center`); the result keeps it at the same page position, since
/// the pivot moves with the centre. Sizes stop at 0: a drag past the
/// opposite side does not flip the box. Unrotated boxes get exact
/// zeros for the axes the grip does not move.
pub(crate) fn resize(rect: Rect, spin: Affine2, drag: Drag) -> Resized {
    let (hx, hy) = drag.grip.fractions();
    let sign = |f: f64| {
        if f == 1.0 {
            1.0
        } else if f == 0.0 {
            -1.0
        } else {
            0.0
        }
    };
    let (sx, sy) = (sign(hx), sign(hy));
    let mut dw = sx * drag.delta.0;
    let mut dh = sy * drag.delta.1;
    let (fx, fy) = if drag.from_center {
        dw *= 2.0;
        dh *= 2.0;
        (0.5, 0.5)
    } else {
        (1.0 - hx, 1.0 - hy)
    };
    let mut w2 = (rect.w + dw).max(0.0);
    let mut h2 = (rect.h + dh).max(0.0);
    if drag.constrain && rect.w > 0.0 && rect.h > 0.0 {
        let (rw, rh) = (w2 / rect.w, h2 / rect.h);
        let f = match (sx != 0.0, sy != 0.0) {
            (true, true) if (rw - 1.0).abs() >= (rh - 1.0).abs() => rw,
            (true, true) | (false, true) => rh,
            (true, false) => rw,
            (false, false) => 1.0,
        };
        w2 = rect.w * f;
        h2 = rect.h * f;
    }
    // The fixed point v (from the centre) keeps its page place when the new
    // centre is c + R(v - v2), where v2 is the fixed point of the new box.
    let (gw, gh) = (rect.w - w2, rect.h - h2);
    let r = linear(spin, ((fx - 0.5) * gw, (fy - 0.5) * gh));
    Resized {
        dx: gw / 2.0 + r.0,
        dy: gh / 2.0 + r.1,
        dw: w2 - rect.w,
        dh: h2 - rect.h,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::vector::add;

    fn page_of(rect: Rect, rot: f64, f: Pt) -> Pt {
        let c = rect.at((0.5, 0.5));
        Affine2::rotate_at(rot, c.0, c.1).apply(rect.at(f).0, rect.at(f).1)
    }

    #[test]
    fn opposite_grip_stays_for_every_grip_and_angle() {
        let rect = Rect {
            x: 10.0,
            y: 20.0,
            w: 80.0,
            h: 40.0,
        };
        for rot in [0.0, 17.0, 90.0, -135.0] {
            let spin = Affine2::rotate_at(rot, 0.0, 0.0);
            for grip in Grip::ALL {
                for (constrain, from_center) in [(false, false), (true, false), (false, true)] {
                    let drag = Drag {
                        grip,
                        delta: (7.0, -5.0),
                        constrain,
                        from_center,
                    };
                    let after = resize(rect, spin, drag).apply(rect);
                    let fixed = if from_center {
                        (0.5, 0.5)
                    } else {
                        grip.opposite().fractions()
                    };
                    let before = page_of(rect, rot, fixed);
                    let now = page_of(after, rot, fixed);
                    assert!(
                        (before.0 - now.0).abs() < 1e-9 && (before.1 - now.1).abs() < 1e-9,
                        "{grip:?} rot {rot}: {before:?} vs {now:?}"
                    );
                    if constrain {
                        assert!((after.w / after.h - 2.0).abs() < 1e-9, "{grip:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn unrotated_east_grip_leaves_x_exactly() {
        let rect = Rect {
            x: 10.1,
            y: 20.3,
            w: 80.7,
            h: 40.9,
        };
        let r = resize(
            rect,
            Affine2::IDENTITY,
            Drag {
                grip: Grip::E,
                delta: (3.3, 9.0),
                constrain: false,
                from_center: false,
            },
        );
        assert_eq!((r.dx, r.dy, r.dh), (0.0, 0.0, 0.0));
        assert!((r.dw - 3.3).abs() < 1e-12);
        let west = resize(
            rect,
            Affine2::IDENTITY,
            Drag {
                grip: Grip::W,
                delta: (3.0, 0.0),
                constrain: false,
                from_center: false,
            },
        );
        assert_eq!((west.dx, west.dw), (3.0, -3.0));
        let collapsed = resize(
            rect,
            Affine2::IDENTITY,
            Drag {
                grip: Grip::E,
                delta: (-500.0, 0.0),
                constrain: false,
                from_center: false,
            },
        );
        assert_eq!(collapsed.apply(rect).w, 0.0);
        assert_eq!(Grip::parse("se"), Some(Grip::Se));
        assert_eq!(Grip::parse("x"), None);
        assert_eq!(add(rect.at((0.0, 0.0)), (1.0, 1.0)), (11.1, 21.3));
    }
}
