//! Machine-local fonts: the [`LocalFontSource`] trait and [`NoLocalFonts`].
//!
//! Bundled fonts come from `zenith-core`. Project font assets are project
//! files and come through [`SourceFs`](super::SourceFs). Only fonts installed
//! on the host machine come through this trait.

use std::collections::BTreeSet;
use std::path::Path;

use zenith_core::LocalFontEntry;

/// Fonts installed on the host machine, used as a last-resort source for
/// families that bundled and project fonts do not supply.
pub trait LocalFontSource {
    /// Every local face whose family is in `wanted` (case-insensitive), in
    /// the host's scan order. The first face of a `(family, weight, style)`
    /// slot wins.
    fn faces(&self, wanted: &BTreeSet<String>) -> Vec<LocalFontEntry>;

    /// The bytes of the font file at `path`, a path from [`Self::faces`].
    /// `None` skips every face of that file.
    fn read(&self, path: &Path) -> Option<Vec<u8>>;
}

/// A host with no local fonts (a browser, a sandbox, or a test).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoLocalFonts;

impl LocalFontSource for NoLocalFonts {
    fn faces(&self, _wanted: &BTreeSet<String>) -> Vec<LocalFontEntry> {
        Vec::new()
    }

    fn read(&self, _path: &Path) -> Option<Vec<u8>> {
        None
    }
}
