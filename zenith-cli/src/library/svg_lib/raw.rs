//! The build-script-generated table of SVG icon libraries bundled in the binary.
//!
//! `build.rs` walks `assets/libraries/icons/<name>/` and emits one
//! [`RawSvgLibrary`] per directory, holding the directory's optional
//! `library.kdl` manifest text and every `*.svg` file keyed by stem. Both the
//! directory order and the icon order are sorted, so the table — and everything
//! derived from it — is deterministic.
//!
//! This is the RAW, unparsed form. [`super::load`] turns it into a
//! [`super::SvgLibrary`].

/// One bundled SVG icon library, as emitted by `build.rs`.
pub struct RawSvgLibrary {
    /// The directory name under `assets/libraries/icons/`, e.g. `"lucide"`.
    pub dir: &'static str,
    /// The verbatim `library.kdl` manifest text, when the directory has one.
    pub manifest: Option<&'static str>,
    /// Every icon in the directory, as `(file stem, SVG text)`, sorted by stem.
    pub icons: &'static [(&'static str, &'static str)],
}

include!(concat!(env!("OUT_DIR"), "/svg_libraries.rs"));

/// Look up the verbatim SVG text of one bundled icon.
///
/// Used to expose bundled icons as embedded preset ASSETS (addressable as
/// `assets/zenith/icons/<dir>/<name>.svg`), where a `&'static str` is required.
pub fn raw_embedded_icon(dir: &str, name: &str) -> Option<&'static str> {
    let library = EMBEDDED_SVG_LIBRARIES.iter().find(|l| l.dir == dir)?;
    library
        .icons
        .iter()
        .find(|(stem, _)| *stem == name)
        .map(|(_, svg)| *svg)
}
