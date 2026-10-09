//! Gesture params, the confirmation flags, and handle ids.

use serde::Deserialize;
use zenith_tx::OpPathHandle;

use crate::commands::render::ViewportParams;
use crate::error::EditorError;
use crate::geom::Grip;

/// The params of `gesture.preview` and `gesture.commit`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GestureParams {
    /// The node; default the one selected node.
    #[serde(default)]
    pub(crate) node: Option<String>,
    /// Several nodes moved, resized, or turned as one selection.
    #[serde(default)]
    pub(crate) nodes: Option<Vec<String>>,
    /// Snap moves and resizes to other nodes and the page within this many
    /// page px (the page sends its screen threshold / zoom). Absent or 0:
    /// no snapping.
    #[serde(default)]
    pub(crate) snap_distance: Option<f64>,
    /// The handle id from `node.handles`; absent for a move.
    #[serde(default)]
    pub(crate) handle: Option<String>,
    /// Pointer delta in page px.
    #[serde(default)]
    pub(crate) dx: f64,
    /// Pointer delta in page px.
    #[serde(default)]
    pub(crate) dy: f64,
    /// Rotation delta in degrees, clockwise on screen.
    #[serde(default)]
    pub(crate) angle: f64,
    /// Snap the resulting rotation to multiples of this many degrees.
    #[serde(default)]
    pub(crate) snap: Option<f64>,
    /// Move: keep to the dominant axis. Resize: keep the aspect ratio.
    /// Endpoint / vertex: keep the segment at a multiple of 45°.
    #[serde(default)]
    pub(crate) constrain: bool,
    /// Resize about the centre.
    #[serde(default)]
    pub(crate) from_center: bool,
    #[serde(default)]
    pub(crate) detach: bool,
    #[serde(default)]
    pub(crate) detach_anchor: bool,
    #[serde(default)]
    pub(crate) confirm_size: bool,
    #[serde(default)]
    pub(crate) replace: bool,
    #[serde(default)]
    pub(crate) absolute: bool,
    #[serde(default)]
    pub(crate) reorder: bool,
    /// Preview raster scale; default the viewport zoom.
    #[serde(default)]
    pub(crate) scale: Option<f64>,
    /// Preview: rasterize the page. Default `true`.
    #[serde(default = "yes")]
    pub(crate) render: bool,
    /// Preview: rasterize only this page-px window, as `doc.render` does.
    #[serde(default)]
    pub(crate) viewport: Option<ViewportParams>,
}

fn yes() -> bool {
    true
}

/// The confirmations a gesture carries after a rejection offered them.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Flags {
    /// Replace a token-bound axis with px.
    pub(crate) detach: bool,
    /// Remove the anchor and place the node by x / y.
    pub(crate) detach_anchor: bool,
    /// Write the measured size plus the delta for a computed size.
    pub(crate) confirm_size: bool,
    /// Replace a value with no px conversion by px.
    pub(crate) replace: bool,
    /// Take an in-flow child out of the layout.
    pub(crate) absolute: bool,
    /// Move an in-flow child to another flow slot.
    pub(crate) reorder: bool,
}

impl GestureParams {
    /// The confirmation flags.
    pub(crate) fn flags(&self) -> Flags {
        Flags {
            detach: self.detach,
            detach_anchor: self.detach_anchor,
            confirm_size: self.confirm_size,
            replace: self.replace,
            absolute: self.absolute,
            reorder: self.reorder,
        }
    }
}

/// What a handle id names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HandleRef {
    /// No handle: move the node.
    Move,
    /// A resize grip.
    Grip(Grip),
    /// The rotate handle.
    Rotate,
    /// A line's start point (`start`).
    LineStart,
    /// A line's end point (`end`).
    LineEnd,
    /// A polygon / polyline vertex (`p<i>`).
    Vertex(usize),
    /// A path anchor (`a<s>.<i>`).
    Anchor { subpath: usize, index: usize },
    /// A path anchor's handle (`a<s>.<i>.in` / `.out`).
    Control {
        subpath: usize,
        index: usize,
        which: OpPathHandle,
    },
}

impl HandleRef {
    /// The handle id.
    pub(crate) fn id(self) -> String {
        match self {
            HandleRef::Move => String::new(),
            HandleRef::Grip(g) => g.id().to_owned(),
            HandleRef::Rotate => "rotate".to_owned(),
            HandleRef::LineStart => "start".to_owned(),
            HandleRef::LineEnd => "end".to_owned(),
            HandleRef::Vertex(i) => format!("p{i}"),
            HandleRef::Anchor { subpath, index } => format!("a{subpath}.{index}"),
            HandleRef::Control {
                subpath,
                index,
                which,
            } => {
                let side = match which {
                    OpPathHandle::In => "in",
                    OpPathHandle::Out => "out",
                };
                format!("a{subpath}.{index}.{side}")
            }
        }
    }

    /// Parse a handle id; `None` is a move.
    pub(crate) fn parse(id: Option<&str>) -> Result<HandleRef, EditorError> {
        let Some(id) = id else {
            return Ok(HandleRef::Move);
        };
        let bad = || {
            EditorError::new(
                "editor.unknown_handle",
                format!(
                    "unknown handle '{id}'; use an id node.handles returns (nw, n, ne, e, se, s, \
                     sw, w, rotate, start, end, p<i>, a<s>.<i>, a<s>.<i>.in, a<s>.<i>.out)"
                ),
            )
        };
        if let Some(g) = Grip::parse(id) {
            return Ok(HandleRef::Grip(g));
        }
        match id {
            "rotate" => return Ok(HandleRef::Rotate),
            "start" => return Ok(HandleRef::LineStart),
            "end" => return Ok(HandleRef::LineEnd),
            _ => {}
        }
        if let Some(i) = id.strip_prefix('p') {
            return i.parse().map(HandleRef::Vertex).map_err(|_| bad());
        }
        let rest = id.strip_prefix('a').ok_or_else(bad)?;
        let mut parts = rest.split('.');
        let subpath = parts.next().and_then(|s| s.parse().ok()).ok_or_else(bad)?;
        let index = parts.next().and_then(|s| s.parse().ok()).ok_or_else(bad)?;
        let which = match parts.next() {
            None => return Ok(HandleRef::Anchor { subpath, index }),
            Some("in") => OpPathHandle::In,
            Some("out") => OpPathHandle::Out,
            Some(_) => return Err(bad()),
        };
        if parts.next().is_some() {
            return Err(bad());
        }
        Ok(HandleRef::Control {
            subpath,
            index,
            which,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_ids_round_trip() {
        for id in [
            "nw", "e", "rotate", "start", "end", "p3", "a0.2", "a1.0.in", "a0.4.out",
        ] {
            let h = HandleRef::parse(Some(id)).expect(id);
            assert_eq!(h.id(), id);
        }
        assert_eq!(HandleRef::parse(None).expect("move"), HandleRef::Move);
        for bad in ["px", "a", "a0", "a0.1.up", "a0.1.in.x", "zz"] {
            assert_eq!(
                HandleRef::parse(Some(bad)).expect_err(bad).code,
                "editor.unknown_handle"
            );
        }
    }
}
