//! Final per-node boxes recorded during a page compile. Wiring only.
//!
//! [`compile_node`](super::compile_node) reports every node it compiles to
//! the page's [`BoxRecorder`] when one is set; with none set, nothing is
//! computed. Each record holds three axis-aligned facts, in page-absolute
//! px:
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
//! Ancestor transforms (rotation, instance scaling) apply to both boxes, so
//! under a turned ancestor they are bounds, not the drawn box. The exact
//! drawn box is `local` (the box in the node's own space) through `spin`
//! (the node's own drawn rotation) and `world` (the transform open at the
//! node). Each record also keeps the clips open at the node, its paint rank,
//! its first command index, its `visible=false` flag, and, for a line,
//! polygon, polyline, path, or connector, the exact region it paints (see
//! [`HitShape`]). [`hit_test`] and [`selectable_id`] read them.
//!
//! Expanded content records under its expanded id: master projections as
//! `<page-id>/<id>`, instance content as `<instance-id>/<id>`, pattern motif
//! instances as `<pattern-id>/<index>/<motif-id>`. The first record of an id
//! wins.
//!
//! With a recorder set, each `instance` also records the lowered subtree it
//! expanded to (see [`Expansion`]), and each stroked `connector` its drawn
//! route, for the page lint pass.
//!
//! - `affine` — the public 2-D affine map.
//! - `bounds` — the transform stack and paint extents of commands.
//! - `clip` — the transform and clips open at a point of a stream.
//! - `compiled` — the public per-node record.
//! - `glyphs` — per-glyph ink of attributed glyph runs.
//! - `hit` — the boxes under a page point.
//! - `record` — the recorder and the per-node declared box.
//! - `region` — the boxes that meet a convex page region (area selection).
//! - `select` — the authored node a compiled id selects.
//! - `shape` — the painted region of a vector node, for exact hits.

mod affine;
mod bounds;
mod clip;
mod compiled;
mod glyphs;
mod hit;
mod record;
mod region;
mod select;
mod shape;

pub use affine::Affine2;
pub use clip::ClipShape;
pub use compiled::CompiledBox;
pub(in crate::compile) use glyphs::{TextInk, glyph_inks};
pub use hit::{hit_test, hit_test_within};
pub(in crate::compile) use record::{BoxRecorder, Compiled, Expansion, Placed, Recorded};
pub use region::hit_region;
pub use select::selectable_id;
pub use shape::HitShape;
