//! Transaction envelope: [`Transaction`] and the [`Op`] enum.
//!
//! Deserializes from JSON like:
//! ```json
//! {"ops":[
//!   {"op":"set_text_align","node":"label","align":"center"},
//!   {"op":"set_fill","node":"box","fill":"color.accent"},
//!   {"op":"set_fill_rule","node":"path.logo","fill_rule":"evenodd"},
//!   {"op":"set_stroke","node":"box","stroke":"color.rule"},
//!   {"op":"set_stroke_width","node":"box","stroke_width":"size.stroke"},
//!   {"op":"set_visible","node":"box","visible":false},
//!   {"op":"set_locked","node":"box","locked":true},
//!   {"op":"set_geometry","node":"r","x":10,"w":200},
//!   {"op":"set_points","node":"poly","points":[{"x":0,"y":0},{"x":100,"y":0},{"x":50,"y":80}]}
//! ]}
//! ```
//!
//! Submodules: `types` (supporting value types), `layout` (auto-layout
//! payloads), `anchor` (the anchor edit payload), `ops` (the [`Op`] enum
//! itself), `subjects` (the ids an op keeps at their page position),
//! `transaction` (the [`Transaction`] envelope).

mod anchor;
mod layout;
mod ops;
mod subjects;
mod transaction;
mod types;

pub use anchor::AnchorEdit;
pub use layout::{LayoutDim, LayoutEdit, SizeInput};
pub use ops::Op;
pub use transaction::Transaction;
pub use types::{
    AddAssetMetadata, FilterOpInput, GradientStopInput, OpPathAnchor, OpPathBooleanOperation,
    OpPathHandle, OpPathSubpath, OpPathTransform, OpPoint, OpSpan, Permissions, Position,
    ShadowLayerInput,
};
