//! Scene-command → PDF content-operator translation.
//!
//! [`translate`] walks the scene display list once and emits a single page
//! content stream, accumulating the page resources it references (alpha
//! ExtGStates, axial-gradient shadings, image XObjects) into [`PageResources`]
//! for the document writer to materialize.
//!
//! Every [`SceneCommand`](zenith_scene::SceneCommand) variant is handled explicitly — no wildcard arm
//! silently drops a primitive. The one honest v0 limitation matched explicitly
//! at its arm is color-bitmap (emoji) glyphs (omitted; the print scenarios use
//! none).
//!
//! Complete structural ranges containing effects or group opacity rasterize under
//! the page transform. Non-normal blends rasterize the whole page with its backdrop.
//! Unaffected ranges retain native operators. Structural errors retain every
//! command through the vector emitter. Raster errors use the same fallback.
//!
//! Submodules: `resources` (page-resource accumulator + name builder), `draw`
//! (shared fill/alpha/line-style primitives), `command` (the scene-walk driver
//! and per-command emitters), and `image` (raster placement and SVG dispatch).

mod command;
mod draw;
mod image;
mod resources;

pub(in crate::pdf) use command::{emit_command, translate};
pub(in crate::pdf) use draw::{apply_alpha, push_gradient};
pub(in crate::pdf) use resources::{
    ALPHA_PREFIX, FONT_PREFIX, IMAGE_PREFIX, LinkAnnot, PageResources, SHADING_PREFIX, name,
};
