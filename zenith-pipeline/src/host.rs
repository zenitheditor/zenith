//! [`Host`]: the bundle of I/O a pipeline call runs against.

use std::sync::Arc;

use zenith_core::{FontMissLog, FontSource, FontStyle};

use crate::io::{
    ConfigSource, IgnoreWarnings, LocalFontSource, NoLocalFonts, PageRunner, Sequential, SourceFs,
    WarningSink,
};

/// A font face a host supplies beyond the embedded set.
///
/// `Debug` shows the face, not the font bytes.
///
/// The provider registers it after the bundled faces and before the
/// document's own font assets. A face registered with
/// [`FontSource::Bundled`] under a bundled family, weight, and style equals
/// the embedded face, so a build that drops that face renders as the full
/// build.
#[derive(Clone)]
pub struct ExtraFont {
    /// The family the face registers under.
    pub family: String,
    /// The face weight.
    pub weight: u16,
    /// The face style.
    pub style: FontStyle,
    /// The font file bytes.
    pub bytes: Arc<[u8]>,
    /// The face index within a collection (0 for a single face).
    pub index: u32,
    /// The provenance recorded on the face.
    pub source: FontSource,
}

impl std::fmt::Debug for ExtraFont {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtraFont")
            .field("family", &self.family)
            .field("weight", &self.weight)
            .field("style", &self.style)
            .field("bytes", &self.bytes.len())
            .field("index", &self.index)
            .field("source", &self.source)
            .finish()
    }
}

/// The I/O of one pipeline call. Cheap to copy.
#[derive(Clone, Copy)]
pub struct Host<'a> {
    /// Project files.
    pub fs: &'a dyn SourceFs,
    /// Global and local config files.
    pub config: &'a dyn ConfigSource,
    /// Fonts installed on the host machine.
    pub local_fonts: &'a dyn LocalFontSource,
    /// Per-page job scheduling.
    pub runner: &'a dyn PageRunner,
    /// Non-diagnostic warnings.
    pub warnings: &'a dyn WarningSink,
    /// Faces registered after the bundled set. Empty for a native host.
    pub extra_fonts: &'a [ExtraFont],
    /// When set, the font provider records every face request it cannot
    /// match exactly.
    pub font_log: Option<&'a FontMissLog>,
}

impl<'a> Host<'a> {
    /// A host over `fs` and `config` with no local fonts, sequential pages,
    /// and ignored warnings.
    #[must_use]
    pub fn new(fs: &'a dyn SourceFs, config: &'a dyn ConfigSource) -> Self {
        Self {
            fs,
            config,
            local_fonts: &NoLocalFonts,
            runner: &Sequential,
            warnings: &IgnoreWarnings,
            extra_fonts: &[],
            font_log: None,
        }
    }

    /// This host with `local_fonts` as the machine font source.
    #[must_use]
    pub fn with_local_fonts(mut self, local_fonts: &'a dyn LocalFontSource) -> Self {
        self.local_fonts = local_fonts;
        self
    }

    /// This host with `runner` scheduling per-page jobs.
    #[must_use]
    pub fn with_runner(mut self, runner: &'a dyn PageRunner) -> Self {
        self.runner = runner;
        self
    }

    /// This host with `warnings` receiving non-diagnostic warnings.
    #[must_use]
    pub fn with_warnings(mut self, warnings: &'a dyn WarningSink) -> Self {
        self.warnings = warnings;
        self
    }

    /// This host with `extra_fonts` registered after the bundled faces.
    #[must_use]
    pub fn with_extra_fonts(mut self, extra_fonts: &'a [ExtraFont]) -> Self {
        self.extra_fonts = extra_fonts;
        self
    }

    /// This host with `log` recording the face requests that miss.
    #[must_use]
    pub fn with_font_log(mut self, log: &'a FontMissLog) -> Self {
        self.font_log = Some(log);
        self
    }
}
