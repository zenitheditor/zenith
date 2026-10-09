//! Read-only placement facts that editors need before they build ops.
//!
//! These answer with the same structural rules the ops apply, so a caller
//! that checks first sees the result the op would give.

use zenith_core::Document;

use super::layout::flow_frame;
use super::space::{chain_origin, parent_chain, resolved_tokens};

/// A row, column, or grid frame that places a node in flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowPlacement {
    /// The id of the layout frame.
    pub frame: String,
    /// The layout mode: `row`, `column`, or `grid`.
    pub mode: &'static str,
}

/// The layout frame that places node `id` in flow, or `None` when no
/// layout frame manages its position.
///
/// A node is in flow when its direct parent is a `row`, `column`, or `grid`
/// frame, it is a box kind, its `position` is not `absolute`, it is visible,
/// and its role is not `guide`. Ops that place such a node by hand fail with
/// `tx.layout_managed`.
#[must_use]
pub fn layout_flow(doc: &Document, id: &str) -> Option<FlowPlacement> {
    flow_frame(doc, id).map(|(frame, mode)| FlowPlacement {
        frame: frame.to_owned(),
        mode,
    })
}

/// The px origin of the space that holds the `x` / `y` of node `id`,
/// relative to its page (or master): the sum of the translations of every
/// `group` and `frame` above it, as the scene applies them.
///
/// Rotations are not part of the origin. A node's coordinates in the scene
/// are its authored coordinates plus this origin, before any rotation.
///
/// `None` when no node has that id. `Some(Err(id))` names the first
/// container whose offset does not resolve to px.
#[must_use]
pub fn parent_space_origin(doc: &Document, id: &str) -> Option<Result<(f64, f64), String>> {
    let resolved = resolved_tokens(doc);
    parent_chain(doc, id, &resolved).map(|chain| chain_origin(&chain, 0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::{KdlAdapter, KdlSource};

    const SRC: &str = r#"
zenith version=1 {
  document id="d" {
    page id="pg" w=(px)400 h=(px)400 {
      group id="g" x=(px)10 y=(px)20 {
        frame id="f" x=(px)5 y=(px)7 w=(px)100 h=(px)100 {
          rect id="r" x=(px)1 y=(px)1 w=(px)5 h=(px)5
        }
      }
      frame id="row" x=(px)0 y=(px)200 w=(px)200 h=(px)50 layout="row" {
        rect id="a" w=(px)10 h=(px)10
        rect id="b" w=(px)10 h=(px)10 position="absolute" x=(px)3 y=(px)3
      }
    }
  }
}
"#;

    fn doc() -> Document {
        KdlAdapter.parse(SRC.as_bytes()).expect("parse")
    }

    #[test]
    fn origin_sums_container_offsets() {
        let d = doc();
        assert_eq!(parent_space_origin(&d, "r"), Some(Ok((15.0, 27.0))));
        assert_eq!(parent_space_origin(&d, "g"), Some(Ok((0.0, 0.0))));
        assert_eq!(parent_space_origin(&d, "nope"), None);
    }

    #[test]
    fn flow_reports_the_managing_frame() {
        let d = doc();
        assert_eq!(
            layout_flow(&d, "a"),
            Some(FlowPlacement {
                frame: "row".to_owned(),
                mode: "row"
            })
        );
        assert_eq!(layout_flow(&d, "b"), None);
        assert_eq!(layout_flow(&d, "r"), None);
    }
}
