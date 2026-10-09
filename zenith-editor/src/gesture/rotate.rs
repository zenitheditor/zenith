//! Rotate gestures: `set_geometry rotate` about the node's own pivot.

use zenith_core::Node;
use zenith_scene::Affine2;
use zenith_tx::Op;

use super::kind::rotate_blocked;
use crate::error::EditorError;
use crate::geom::det;

/// The ops that turn `node` by `angle` degrees (clockwise on screen).
///
/// The new angle is the authored `rotate` plus `angle` (negated under a
/// mirroring ancestor transform), snapped to multiples of `snap` when set,
/// then normalized to `[-180, 180)`. The scene turns a node about its own
/// centre (box centre, or the bounds centre of its points or anchors), so
/// one `set_geometry rotate` turns it about that pivot. A result of 0
/// removes the attribute. No op when the angle does not change.
///
/// # Errors
///
/// The code [`rotate_blocked`] gives, as `editor.rejected` input.
pub(crate) fn plan_rotate(
    node: &Node,
    id: &str,
    world: Affine2,
    angle: f64,
    snap: Option<f64>,
) -> Result<Vec<Op>, EditorError> {
    if let Some((code, why)) = rotate_blocked(node) {
        return Err(EditorError::new(
            code,
            format!("'{id}' cannot rotate on the canvas: {why}"),
        ));
    }
    let current = node.rotate().map_or(0.0, |d| d.value);
    let sign = if det(world) < 0.0 { -1.0 } else { 1.0 };
    let mut next = current + sign * angle;
    if let Some(step) = snap.filter(|s| s.is_finite() && *s > 0.0) {
        next = (next / step).round() * step;
    }
    next = (next + 180.0).rem_euclid(360.0) - 180.0;
    if next == current {
        return Ok(Vec::new());
    }
    Ok(vec![Op::SetGeometry {
        node: id.to_owned(),
        x: None,
        y: None,
        w: None,
        h: None,
        rotate: Some((next != 0.0).then_some(next)),
    }])
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::{KdlAdapter, KdlSource};

    fn rect(rotate: &str) -> Node {
        let src = format!(
            r#"zenith version=1 {{ document id="d" {{ page id="p" w=(px)10 h=(px)10 {{
              rect id="r" x=(px)0 y=(px)0 w=(px)5 h=(px)5 {rotate}
            }} }} }}"#
        );
        let doc = KdlAdapter.parse(src.as_bytes()).expect("parse");
        doc.body
            .pages
            .into_iter()
            .next()
            .and_then(|p| p.children.into_iter().next())
            .expect("rect")
    }

    fn angle_of(ops: &[Op]) -> Option<Option<f64>> {
        if let Some(Op::SetGeometry { rotate, .. }) = ops.first() {
            *rotate
        } else {
            None
        }
    }

    #[test]
    fn adds_snaps_and_normalizes() {
        let r = rect("rotate=(deg)170");
        let ops = plan_rotate(&r, "r", Affine2::IDENTITY, 20.0, None).expect("ops");
        assert_eq!(angle_of(&ops), Some(Some(-170.0)));
        let snapped = plan_rotate(&r, "r", Affine2::IDENTITY, 12.0, Some(15.0)).expect("ops");
        assert_eq!(angle_of(&snapped), Some(Some(-180.0)));
        let plain = rect("");
        let back = plan_rotate(&plain, "r", Affine2::IDENTITY, 0.0, None).expect("ops");
        assert!(back.is_empty());
        let mirrored = Affine2 {
            a: -1.0,
            ..Affine2::IDENTITY
        };
        let m = plan_rotate(&plain, "r", mirrored, 30.0, None).expect("ops");
        assert_eq!(angle_of(&m), Some(Some(-30.0)));
        let zero =
            plan_rotate(&rect("rotate=(deg)30"), "r", Affine2::IDENTITY, -30.0, None).expect("ops");
        assert_eq!(angle_of(&zero), Some(None));
    }
}
