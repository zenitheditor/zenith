//! Final per-node boxes recorded during a page compile. Wiring only.
//!
//! [`compile_node`](super::compile_node) reports every node it compiles to
//! the page's [`BoxRecorder`] when one is set; with none set, nothing is
//! computed. Each record holds three facts, in page-absolute px:
//!
//! - `rect` — the unrotated box the compile used. A box node (rect, text,
//!   frame, image, table, …) takes its resolved `x` / `y` (or its
//!   anchor-derived origin), `w`, and `h`; a text or code without `h` takes
//!   its measured content height; a field or toc takes the box of the text
//!   it resolves to; a footnote takes its slot in the footnote zone. Any
//!   other node (line, polygon, polyline, path, connector, light, instance,
//!   a group without a full box) takes the bounds of the commands it emitted
//!   with its own rotations left out.
//! - `rotate` — the node's own rotation in degrees, when set.
//! - `visual` — the axis-aligned bounds of what the node paints, with every
//!   transform applied: strokes grow by half their width and glyph runs add
//!   their ink.
//!
//! Ancestor transforms (rotation, instance scaling) apply to both boxes.
//! Expanded content records under its expanded id: master projections as
//! `<page-id>/<id>`, instance content as `<instance-id>/<id>`, pattern motif
//! instances as `<pattern-id>/<index>/<motif-id>`. The first record of an id
//! wins.
//!
//! - `bounds` — the affine transform stack and paint extents of commands.
//! - `record` — the recorder and the per-node declared box.

mod bounds;
mod record;

pub use record::CompiledBox;
pub(in crate::compile) use record::{BoxRecorder, Compiled, Placed};
