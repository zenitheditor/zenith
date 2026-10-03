//! Non-axis-aligned clips: a `PushClip` under a rotating or skewing transform
//! clips to the transformed rectangle (a quad), not to its bounding box.
//!
//! The clip stack keeps two parts per entry:
//! - `rect` — the device-space bounding box of every active clip, intersected.
//!   It drives culling and the exact AA-off rect mask.
//! - `shape` — `None` while every active clip is axis-aligned. Otherwise a
//!   page-sized coverage mask: the AA quad of each non-axis-aligned clip,
//!   multiplied together.
//!
//! A rounded clip (`PushClipRoundedRect`) always adds its AA rounded path to
//! `shape`, whatever the transform.
//!
//! A draw clips to `rect`, then to `shape` when present. With no rotated clip
//! active `shape` is `None` and every draw is byte-identical to the rect-only
//! clip path.

use std::rc::Rc;

use tiny_skia::{FillRule, Mask, Path, PathBuilder, Rect, Transform};

use super::paths::{build_rounded_rect_path, clip_mask};

/// True when `ts` maps rectangles to axis-aligned rectangles (scale and
/// translate only, no rotation or skew).
pub(super) fn is_axis_aligned(ts: Transform) -> bool {
    ts.kx == 0.0 && ts.ky == 0.0
}

/// The clip shape after pushing the user-space rect `(x, y, w, h)` under `ts`.
///
/// An axis-aligned `ts` keeps the `parent` shape (its bounds go into the rect
/// part). Otherwise the rect is filled under `ts` (anti-aliased) into a
/// `width × height` mask and multiplied with `parent`. Returns an all-zero
/// mask for a degenerate rect, so the clip hides everything.
pub(super) fn push_clip_shape(
    parent: Option<&Rc<Mask>>,
    ts: Transform,
    (x, y, w, h): (f64, f64, f64, f64),
    width: u32,
    height: u32,
) -> Option<Rc<Mask>> {
    if is_axis_aligned(ts) {
        return parent.cloned();
    }
    let path = Rect::from_xywh(x as f32, y as f32, w as f32, h as f32).map(PathBuilder::from_rect);
    shape_mask(parent, path.as_ref(), ts, width, height)
}

/// The clip shape after pushing the user-space rounded rect `(x, y, w, h)`
/// with corner `radius` under `ts`.
///
/// Always builds a mask, since the corners are not rectangular under any
/// transform. Same contract as [`push_clip_shape`] otherwise.
pub(super) fn push_rounded_clip_shape(
    parent: Option<&Rc<Mask>>,
    ts: Transform,
    (x, y, w, h): (f64, f64, f64, f64),
    radius: f64,
    width: u32,
    height: u32,
) -> Option<Rc<Mask>> {
    let r = radius as f32;
    let path = build_rounded_rect_path(x as f32, y as f32, w as f32, h as f32, [r; 4]);
    shape_mask(parent, path.as_ref(), ts, width, height)
}

/// Fill `path` under `ts` (anti-aliased) into a page-sized mask and multiply
/// it with `parent`. A missing path yields an all-zero mask.
fn shape_mask(
    parent: Option<&Rc<Mask>>,
    path: Option<&Path>,
    ts: Transform,
    width: u32,
    height: u32,
) -> Option<Rc<Mask>> {
    let mut mask = Mask::new(width, height)?;
    if let Some(path) = path {
        mask.fill_path(path, FillRule::Winding, true, ts);
    }
    if let Some(parent) = parent {
        intersect_masks(&mut mask, parent);
    }
    Some(Rc::new(mask))
}

/// Multiply `dst` by `other` per pixel: `(a × b + 127) / 255`.
///
/// Both masks are page-sized; a size mismatch leaves `dst` unchanged.
pub(super) fn intersect_masks(dst: &mut Mask, other: &Mask) {
    if dst.width() != other.width() || dst.height() != other.height() {
        return;
    }
    for (d, &o) in dst.data_mut().iter_mut().zip(other.data()) {
        *d = ((u32::from(*d) * u32::from(o) + 127) / 255) as u8;
    }
}

/// The draw mask for the clip `rect` plus the optional `shape`.
///
/// Same contract as [`clip_mask`]: `None` skips the draw, `Some(None)` draws
/// unmasked, `Some(Some(m))` draws through `m`. With `shape = None` this is
/// exactly [`clip_mask`].
pub(super) fn draw_clip_mask(
    rect: (f64, f64, f64, f64),
    width: u32,
    height: u32,
    shape: Option<&Mask>,
) -> Option<Option<Mask>> {
    let base = clip_mask(rect, width, height)?;
    let Some(shape) = shape else {
        return Some(base);
    };
    Some(Some(match base {
        Some(mut m) => {
            intersect_masks(&mut m, shape);
            m
        }
        None => shape.clone(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_aligned_push_keeps_parent() {
        let ts = Transform::from_scale(0.5, 2.0).post_translate(3.0, 4.0);
        assert!(is_axis_aligned(ts));
        assert!(push_clip_shape(None, ts, (0.0, 0.0, 10.0, 10.0), 20, 20).is_none());
    }

    #[test]
    fn rotated_push_builds_quad() {
        let ts = Transform::from_rotate_at(45.0, 10.0, 10.0);
        let shape = push_clip_shape(None, ts, (5.0, 5.0, 10.0, 10.0), 20, 20).expect("quad mask");
        let at = |x: u32, y: u32| shape.data()[(y * 20 + x) as usize];
        assert_eq!(at(10, 10), 255, "center inside the diamond");
        assert_eq!(at(5, 5), 0, "bbox corner outside the diamond");
    }

    #[test]
    fn draw_mask_without_shape_is_rect_mask() {
        let a = draw_clip_mask((0.0, 0.0, 5.0, 5.0), 10, 10, None);
        let b = clip_mask((0.0, 0.0, 5.0, 5.0), 10, 10);
        assert_eq!(
            a.map(|m| m.map(|m| m.data().to_vec())),
            b.map(|m| m.map(|m| m.data().to_vec()))
        );
    }
}
