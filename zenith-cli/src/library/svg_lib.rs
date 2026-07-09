//! SVG icon libraries: the second library pack format.
//!
//! A `.zen` pack is the FEATURE-RICH format — tokens, components, actions,
//! filters, variants. An SVG library is the PLUG-AND-INSTALL format: a directory
//! of `*.svg` files, one icon per file, id = file stem. Nothing to author, and
//! an icon set can be extended by dropping a file into it.
//!
//! The two formats meet at [`super::LibraryPack`]: an SVG library reports its
//! items from filenames, and synthesizes an equivalent pack [`Document`] on
//! demand (converting only the icons actually asked for), so `list` / `search` /
//! `show` / `add` never branch on format.
//!
//! [`Document`]: zenith_core::Document

mod load;
mod manifest;
mod raw;
mod synth;

pub use load::{
    SVG_PACK_TOKEN_COUNT, SvgIcon, SvgLibrary, embedded_svg_libraries, is_svg_dir, load_svg_dir,
};
pub use raw::raw_embedded_icon;
pub use synth::{ItemScope, synthesize_pack_source};
