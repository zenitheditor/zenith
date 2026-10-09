//! The in-memory project every method runs against: the request's files
//! decoded into a [`MemFs`], the document's place in it, and the policy
//! flags.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use zenith_core::FontMissLog;
use zenith_pipeline::{ExtraFont, FsConfig, Host, MemFs, PolicyFlags};

use super::font_supply::decode_fonts;

use crate::protocol::ErrorBody;

/// The document path when a request names none.
pub(crate) const DEFAULT_PATH: &str = "document.zen";

/// The project fields shared by `diagnose` and `render` params.
pub(crate) struct ProjectFields<'a> {
    /// Where the document sits in `files`. Its parent is the project
    /// directory that imports, assets, fonts, text sources, data files, and
    /// the local `.zenith.kdl` resolve against.
    pub(crate) path: Option<&'a str>,
    /// Project files as path → base64 bytes.
    pub(crate) files: &'a BTreeMap<String, String>,
    /// Bundled font files the module does not embed: file name → base64.
    pub(crate) fonts: &'a BTreeMap<String, String>,
    /// The path in `files` of the global config, when any.
    pub(crate) global_config: Option<&'a str>,
    /// `--allow` codes.
    pub(crate) allow: &'a [String],
    /// `--warn` codes.
    pub(crate) warn: &'a [String],
    /// `--deny` codes.
    pub(crate) deny: &'a [String],
}

/// A decoded request project.
pub(crate) struct Project {
    pub(crate) fs: MemFs,
    pub(crate) dir: PathBuf,
    pub(crate) global_config: Option<PathBuf>,
    pub(crate) flags: PolicyFlags,
    /// Bundled faces the request supplied in `fonts`.
    pub(crate) extra_fonts: Vec<ExtraFont>,
    /// Every face request the provider could not match exactly.
    pub(crate) font_log: FontMissLog,
}

impl Project {
    /// Decode `fields`.
    ///
    /// # Errors
    ///
    /// `request.invalid_params` naming the file or font whose bytes are not
    /// base64, or a `fonts` key that is not a bundled font file.
    pub(crate) fn decode(fields: &ProjectFields<'_>) -> Result<Self, ErrorBody> {
        let mut fs = MemFs::new();
        for (path, b64) in fields.files {
            let bytes = STANDARD.decode(b64).map_err(|e| {
                ErrorBody::new(
                    "request.invalid_params",
                    format!(
                        "files['{path}'] is not valid base64: {e}; send each file as standard base64"
                    ),
                )
            })?;
            fs.insert(path, bytes);
        }
        let path = Path::new(fields.path.unwrap_or(DEFAULT_PATH));
        let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
        Ok(Self {
            fs,
            dir,
            extra_fonts: decode_fonts(fields.fonts)?,
            font_log: FontMissLog::new(),
            global_config: fields.global_config.map(PathBuf::from),
            flags: PolicyFlags {
                allow: fields.allow.to_vec(),
                warn: fields.warn.to_vec(),
                deny: fields.deny.to_vec(),
            },
        })
    }

    /// The config source over this project's files.
    pub(crate) fn config(&self) -> FsConfig<'_> {
        FsConfig::new(&self.fs, self.global_config.clone())
    }

    /// The host over this project's files and `config`: no local fonts,
    /// sequential pages, ignored warnings. It adds the request's `fonts` and
    /// records the faces the document asks for.
    pub(crate) fn host<'a>(&'a self, config: &'a FsConfig<'a>) -> Host<'a> {
        Host::new(&self.fs, config)
            .with_extra_fonts(&self.extra_fonts)
            .with_font_log(&self.font_log)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_pipeline::SourceFs;

    fn fields<'a>(path: Option<&'a str>, files: &'a BTreeMap<String, String>) -> ProjectFields<'a> {
        static NO_FONTS: BTreeMap<String, String> = BTreeMap::new();
        ProjectFields {
            path,
            files,
            fonts: &NO_FONTS,
            global_config: None,
            allow: &[],
            warn: &[],
            deny: &[],
        }
    }

    #[test]
    fn decodes_files_and_places_the_document() {
        let mut files = BTreeMap::new();
        files.insert("site/a.txt".to_owned(), STANDARD.encode(b"hi"));
        let project = Project::decode(&fields(Some("site/page.zen"), &files)).expect("decode");
        assert_eq!(project.dir, PathBuf::from("site"));
        assert_eq!(
            project.fs.read(Path::new("site/a.txt")).expect("read"),
            b"hi"
        );
        let default = Project::decode(&fields(None, &files)).expect("decode");
        assert_eq!(default.dir, PathBuf::new());
    }

    #[test]
    fn bad_base64_names_the_file() {
        let mut files = BTreeMap::new();
        files.insert("x.png".to_owned(), "***".to_owned());
        let err = Project::decode(&fields(None, &files))
            .err()
            .expect("bad base64");
        assert_eq!(err.code, "request.invalid_params");
        assert!(err.message.contains("files['x.png']"), "{}", err.message);
    }
}
