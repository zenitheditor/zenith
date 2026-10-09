//! Coverage-mask fills on a [`Surface`]. Each has the contract of the
//! tiny-skia `Mask` call it wraps.

use tiny_skia::{FillRule, Mask, Path, Transform};

use super::draw::{FILL_MARGIN, device_path};
use super::scratch::mask_back;
use super::window::{Placement, Surface};

/// Fill `path` under the full-page transform `ts` into the surface-sized
/// `mask`, which must be all zero.
///
/// Same contract as `Mask::fill_path`. A path that crosses the surface edge
/// fills a scratch mask over its whole device box, so its coverage equals the
/// full-page coverage.
pub(in crate::tiny_skia) fn mask_fill_path(
    mask: &mut Mask,
    surface: Surface,
    path: &Path,
    fill_rule: FillRule,
    anti_alias: bool,
    ts: Transform,
) {
    if surface.is_page() {
        mask.fill_path(path, fill_rule, anti_alias, ts);
        return;
    }
    let Some(device) = device_path(path, ts) else {
        return;
    };
    let Some(placement) = surface.place(device.bounds(), FILL_MARGIN, false) else {
        return;
    };
    if let Placement::Scratch(area) = placement
        && let Some(mut scratch) = Mask::new(area.w(), area.h())
        && let Some(local) = device.clone().transform(area.shift())
    {
        scratch.fill_path(&local, fill_rule, anti_alias, Transform::identity());
        mask_back(mask, surface, &scratch, area);
        return;
    }
    if let Some(local) = device.transform(surface.shift()) {
        mask.fill_path(&local, fill_rule, anti_alias, Transform::identity());
    }
}

/// Multiply the surface-sized `mask` by `path` filled under `ts`.
///
/// Same contract as `Mask::intersect_path`, including its rounding.
pub(in crate::tiny_skia) fn mask_intersect_path(
    mask: &mut Mask,
    surface: Surface,
    path: &Path,
    fill_rule: FillRule,
    anti_alias: bool,
    ts: Transform,
) {
    if surface.is_page() {
        mask.intersect_path(path, fill_rule, anti_alias, ts);
        return;
    }
    let Some(mut sub) = Mask::new(surface.w, surface.h) else {
        return;
    };
    mask_fill_path(&mut sub, surface, path, fill_rule, anti_alias, ts);
    for (a, &b) in mask.data_mut().iter_mut().zip(sub.data()) {
        *a = premultiply_u8(*a, b);
    }
}

/// tiny-skia's `premultiply_u8` (private in 0.11), used by
/// `Mask::intersect_path`.
fn premultiply_u8(c: u8, a: u8) -> u8 {
    let prod = u32::from(c) * u32::from(a) + 128;
    ((prod + (prod >> 8)) >> 8) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_skia::PathBuilder;

    #[test]
    fn crossing_mask_matches_the_page_mask() {
        let page = Surface::page(64, 48);
        let path = PathBuilder::from_circle(30.3, 20.7, 18.4).expect("circle");
        let ts = Transform::from_rotate_at(20.0, 30.0, 20.0);
        let mut full = Mask::new(64, 48).expect("alloc");
        mask_fill_path(&mut full, page, &path, FillRule::Winding, true, ts);
        let clip = PathBuilder::from_circle(40.0, 25.0, 15.0).expect("clip");
        mask_intersect_path(&mut full, page, &clip, FillRule::Winding, true, ts);
        let s = Surface {
            x: 13,
            y: 7,
            w: 30,
            h: 29,
            ..page
        };
        let mut part = Mask::new(s.w, s.h).expect("alloc");
        mask_fill_path(&mut part, s, &path, FillRule::Winding, true, ts);
        mask_intersect_path(&mut part, s, &clip, FillRule::Winding, true, ts);
        for row in 0..s.h {
            let f = ((row + s.y) * 64 + s.x) as usize;
            let p = (row * s.w) as usize;
            assert_eq!(
                &part.data()[p..p + s.w as usize],
                &full.data()[f..f + s.w as usize],
                "row {row}"
            );
        }
    }

    #[test]
    fn premultiply_matches_tiny_skia() {
        let mut mask = Mask::new(4, 1).expect("alloc");
        mask.data_mut().copy_from_slice(&[0, 77, 200, 255]);
        let mut ours = mask.clone();
        let rect = tiny_skia::Rect::from_xywh(0.0, 0.0, 2.5, 1.0).expect("rect");
        let path = PathBuilder::from_rect(rect);
        mask.intersect_path(&path, FillRule::Winding, true, Transform::identity());
        let mut sub = Mask::new(4, 1).expect("alloc");
        sub.fill_path(&path, FillRule::Winding, true, Transform::identity());
        for (a, &b) in ours.data_mut().iter_mut().zip(sub.data()) {
            *a = premultiply_u8(*a, b);
        }
        assert_eq!(ours.data(), mask.data());
    }
}
