//! [`Surface`]: the device window a render writes, and [`Placement`]: where
//! one draw runs so its pixels match the full-page render.
//!
//! Why a draw can differ on a window, and how each case is avoided:
//! - tiny-skia clips a path that is not inside its pixmap. Clipping chops the
//!   edges at the pixmap border and splits every curve at its extrema, so the
//!   edges of that path differ from the unclipped full render.
//! - A gradient or image shader maps each pixel center back through the
//!   inverse transform. A shifted pixmap changes that `f32` arithmetic.
//!
//! A draw whose device box lies inside the surface runs directly, shifted by
//! the integer surface origin. Otherwise, in exact mode, it runs on a scratch
//! buffer that holds the whole device box (clamped to the page, where the full
//! render clips too). A shader draw's scratch starts at the page origin. The
//! scratch then sees the same geometry, clip, and pixel centers as the full
//! render.

use tiny_skia::{Rect, Transform};

/// The device-pixel window of the page that a render writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::tiny_skia) struct Surface {
    /// Left edge in full-page device pixels.
    pub(in crate::tiny_skia) x: u32,
    /// Top edge in full-page device pixels.
    pub(in crate::tiny_skia) y: u32,
    /// Width in pixels (at least 1).
    pub(in crate::tiny_skia) w: u32,
    /// Height in pixels (at least 1).
    pub(in crate::tiny_skia) h: u32,
    /// Full-page device width in pixels.
    pub(in crate::tiny_skia) page_w: u32,
    /// Full-page device height in pixels.
    pub(in crate::tiny_skia) page_h: u32,
    /// Exact mode: draws that cross the surface edge run on a scratch buffer
    /// so the window equals the full render byte for byte. Off for pages too
    /// large to render whole, where scratch buffers could reach the page size.
    pub(in crate::tiny_skia) exact: bool,
}

/// A device box `[x0, x1) × [y0, y1)` in full-page device pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::tiny_skia) struct DeviceBox {
    pub(in crate::tiny_skia) x0: u32,
    pub(in crate::tiny_skia) y0: u32,
    pub(in crate::tiny_skia) x1: u32,
    pub(in crate::tiny_skia) y1: u32,
}

impl DeviceBox {
    pub(in crate::tiny_skia) fn w(self) -> u32 {
        self.x1 - self.x0
    }

    pub(in crate::tiny_skia) fn h(self) -> u32 {
        self.y1 - self.y0
    }

    /// The shift from full-page device space to this box's pixels.
    pub(in crate::tiny_skia) fn shift(self) -> Transform {
        Transform::from_translate(-(self.x0 as f32), -(self.y0 as f32))
    }

    /// The overlap with `other`, or `None` when empty.
    pub(in crate::tiny_skia) fn intersect(self, other: DeviceBox) -> Option<DeviceBox> {
        let b = DeviceBox {
            x0: self.x0.max(other.x0),
            y0: self.y0.max(other.y0),
            x1: self.x1.min(other.x1),
            y1: self.y1.min(other.y1),
        };
        (b.x0 < b.x1 && b.y0 < b.y1).then_some(b)
    }

    fn contains(self, other: DeviceBox) -> bool {
        self.x0 <= other.x0 && self.y0 <= other.y0 && self.x1 >= other.x1 && self.y1 >= other.y1
    }
}

/// Where one draw runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::tiny_skia) enum Placement {
    /// On the surface itself, shifted by its origin.
    Direct,
    /// On a scratch buffer covering `area`. The draw's ink stays inside
    /// `ink`, a part of `area`.
    Scratch { area: DeviceBox, ink: DeviceBox },
}

impl Surface {
    /// The whole page: origin `(0, 0)`, size `page_w x page_h`.
    pub(in crate::tiny_skia) fn page(page_w: u32, page_h: u32) -> Surface {
        Surface {
            x: 0,
            y: 0,
            w: page_w,
            h: page_h,
            page_w,
            page_h,
            exact: true,
        }
    }

    /// True for the whole page: draws go to tiny-skia unchanged.
    pub(in crate::tiny_skia) fn is_page(self) -> bool {
        self.x == 0 && self.y == 0 && self.w == self.page_w && self.h == self.page_h
    }

    /// The surface as a device box.
    pub(in crate::tiny_skia) fn device_box(self) -> DeviceBox {
        DeviceBox {
            x0: self.x,
            y0: self.y,
            x1: self.x.saturating_add(self.w),
            y1: self.y.saturating_add(self.h),
        }
    }

    /// The surface bounds `(x0, y0, x1, y1)` in full-page device pixels.
    pub(in crate::tiny_skia) fn bounds(self) -> (f64, f64, f64, f64) {
        let (x, y) = (f64::from(self.x), f64::from(self.y));
        (x, y, x + f64::from(self.w), y + f64::from(self.h))
    }

    /// The page bounds `(0, 0, page_w, page_h)` in device pixels.
    pub(in crate::tiny_skia) fn page_bounds(self) -> (f64, f64, f64, f64) {
        (0.0, 0.0, f64::from(self.page_w), f64::from(self.page_h))
    }

    /// The shift from full-page device space to surface pixels.
    pub(in crate::tiny_skia) fn shift(self) -> Transform {
        self.device_box().shift()
    }

    /// The full-page device column and row of surface pixel `(0, 0)`.
    pub(in crate::tiny_skia) fn origin(self) -> (i64, i64) {
        (i64::from(self.x), i64::from(self.y))
    }

    /// `rect` (full-page device space) moved onto the surface.
    ///
    /// Each edge is shifted on its own. `a - n` is exact in `f32` for an
    /// integer `n <= a < 2^24`, so an edge on or inside the surface keeps its
    /// full-render value. Returns `None` for an empty result.
    pub(in crate::tiny_skia) fn local_rect(self, rect: Rect) -> Option<Rect> {
        if self.x == 0 && self.y == 0 {
            return Some(rect);
        }
        let (ox, oy) = (self.x as f32, self.y as f32);
        Rect::from_ltrb(
            rect.left() - ox,
            rect.top() - oy,
            rect.right() - ox,
            rect.bottom() - oy,
        )
    }

    /// Where a draw with device-space bounds `bounds` runs.
    ///
    /// `margin` (device pixels) covers what the rasterizer adds around the
    /// geometry: anti-aliasing and hairline caps. `pixel_centers` marks a
    /// shader draw, whose scratch must start at the page origin. Returns
    /// `None` when the draw misses the page.
    pub(in crate::tiny_skia) fn place(
        self,
        bounds: Rect,
        margin: f32,
        pixel_centers: bool,
    ) -> Option<Placement> {
        let page = DeviceBox {
            x0: 0,
            y0: 0,
            x1: self.page_w,
            y1: self.page_h,
        };
        let snap = |v: f32, max: u32| -> u32 {
            if v.is_nan() || v <= 0.0 {
                0
            } else if v >= max as f32 {
                max
            } else {
                v as u32
            }
        };
        let near = DeviceBox {
            x0: snap((bounds.left() - margin).floor(), page.x1),
            y0: snap((bounds.top() - margin).floor(), page.y1),
            x1: snap((bounds.right() + margin).ceil(), page.x1),
            y1: snap((bounds.bottom() + margin).ceil(), page.y1),
        };
        let near = near.intersect(page)?;
        // A draw that misses the surface changes no surface pixel.
        near.intersect(self.device_box())?;
        if !self.exact {
            return Some(Placement::Direct);
        }
        let needed = if pixel_centers {
            DeviceBox {
                x0: 0,
                y0: 0,
                ..near
            }
        } else {
            near
        };
        if self.device_box().contains(needed) {
            Some(Placement::Direct)
        } else {
            Some(Placement::Scratch {
                area: needed,
                ink: near,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(x: u32, y: u32, w: u32, h: u32) -> Surface {
        Surface {
            x,
            y,
            w,
            h,
            page_w: 100,
            page_h: 80,
            exact: true,
        }
    }

    const CROSSING: DeviceBox = DeviceBox {
        x0: 9,
        y0: 29,
        x1: 52,
        y1: 41,
    };

    const WIDE: DeviceBox = DeviceBox {
        x0: 0,
        y0: 29,
        x1: 100,
        y1: 41,
    };

    fn rect(l: f32, t: f32, r: f32, b: f32) -> Rect {
        Rect::from_ltrb(l, t, r, b).expect("rect")
    }

    #[test]
    fn inside_runs_direct_and_crossing_runs_on_scratch() {
        let s = window(20, 20, 40, 40);
        assert_eq!(
            s.place(rect(25.5, 30.0, 50.2, 40.0), 1.0, false),
            Some(Placement::Direct)
        );
        assert_eq!(
            s.place(rect(10.5, 30.0, 50.2, 40.0), 1.0, false),
            Some(Placement::Scratch {
                area: CROSSING,
                ink: CROSSING
            })
        );
    }

    #[test]
    fn scratch_clamps_to_the_page_and_shaders_start_at_the_origin() {
        let s = window(20, 20, 40, 40);
        assert_eq!(
            s.place(rect(-30.0, 30.0, 150.0, 40.0), 1.0, false),
            Some(Placement::Scratch {
                area: WIDE,
                ink: WIDE
            })
        );
        assert_eq!(
            s.place(rect(25.0, 30.0, 50.0, 40.0), 1.0, true),
            Some(Placement::Scratch {
                area: DeviceBox {
                    x0: 0,
                    y0: 0,
                    x1: 51,
                    y1: 41
                },
                ink: DeviceBox {
                    x0: 24,
                    y0: 29,
                    x1: 51,
                    y1: 41
                }
            })
        );
        assert_eq!(s.place(rect(120.0, 0.0, 150.0, 10.0), 1.0, false), None);
        assert_eq!(s.place(rect(1.0, 1.0, 5.0, 5.0), 1.0, false), None);
    }

    #[test]
    fn inexact_mode_always_runs_direct() {
        let s = Surface {
            exact: false,
            ..window(20, 20, 40, 40)
        };
        assert_eq!(
            s.place(rect(-30.0, 30.0, 150.0, 40.0), 1.0, true),
            Some(Placement::Direct)
        );
    }
}
