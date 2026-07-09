//! Loading an SVG icon library — from the binary, or from a project directory.
//!
//! An SVG library is just a DIRECTORY of `*.svg` files. Each file is one icon;
//! its id is the file stem. That is the whole format: drop a folder of icons
//! under `<project>/libraries/` and it is a usable pack, with no authoring step.
//! An optional `library.kdl` beside the icons adds identity and search metadata
//! (see [`super::manifest`]).
//!
//! Bundled libraries come from the `build.rs`-generated [`RawSvgLibrary`] table
//! and cost no I/O; project libraries are read from disk. Both produce the same
//! [`SvgLibrary`], so everything downstream is source-blind.

use std::borrow::Cow;
use std::path::Path;

use super::manifest::{Manifest, parse_manifest};
use super::raw::{EMBEDDED_SVG_LIBRARIES, RawSvgLibrary};

/// The color token every synthesized icon component strokes with.
pub const STROKE_TOKEN: &str = "lib.icons.stroke";
/// The dimension token every synthesized icon component strokes at.
pub const STROKE_WIDTH_TOKEN: &str = "lib.icons.stroke_width";
/// How many tokens a synthesized SVG-library pack declares.
pub const SVG_PACK_TOKEN_COUNT: usize = 2;

/// One icon: its id, its SVG source, and its search metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgIcon {
    /// The icon id — the `.svg` file stem.
    pub name: String,
    /// The verbatim SVG source. Borrowed for bundled icons, owned for project ones.
    pub svg: Cow<'static, str>,
    /// Alternate names from the manifest; empty when unlisted.
    pub aliases: Vec<String>,
    /// Related words from the manifest; empty when unlisted.
    pub tags: Vec<String>,
    /// Filterable categories from the manifest; empty when unlisted.
    pub categories: Vec<String>,
}

/// A loaded SVG icon library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgLibrary {
    /// The pack id, from the manifest or derived from the directory name.
    pub id: String,
    /// The pack version, when the manifest declares one.
    pub version: Option<String>,
    /// SPDX-style license expression, when the manifest declares one.
    pub license: Option<String>,
    /// Upstream project URL, when the manifest declares one.
    pub source_url: Option<String>,
    /// Exact upstream revision, when the manifest declares one.
    pub revision: Option<String>,
    /// Every icon, sorted by id.
    pub icons: Vec<SvgIcon>,
}

impl SvgLibrary {
    /// Look up one icon by id.
    pub fn icon(&self, name: &str) -> Option<&SvgIcon> {
        self.icons.iter().find(|i| i.name == name)
    }
}

/// Reject icon file stems that could not round-trip through a pack item spec
/// (`<pkg>#<item>`) or a KDL component id.
///
/// A rejected file is skipped rather than fataled: one oddly-named file must not
/// make a user's whole icon folder unusable.
fn is_usable_icon_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('#')
        && !name.contains('"')
        && !name.chars().any(char::is_whitespace)
}

/// Assemble a library from its id and its `(name, svg)` pairs, attaching the
/// manifest's per-icon metadata. Icons are sorted by id.
fn assemble(
    default_id: &str,
    manifest: Manifest,
    mut icons: Vec<(String, Cow<'static, str>)>,
) -> SvgLibrary {
    icons.sort_by(|a, b| a.0.cmp(&b.0));
    let icons = icons
        .into_iter()
        .map(|(name, svg)| {
            let meta = manifest.icons.get(&name);
            SvgIcon {
                aliases: meta.map(|m| m.aliases.clone()).unwrap_or_default(),
                tags: meta.map(|m| m.tags.clone()).unwrap_or_default(),
                categories: meta.map(|m| m.categories.clone()).unwrap_or_default(),
                name,
                svg,
            }
        })
        .collect();

    SvgLibrary {
        id: manifest.id.unwrap_or_else(|| default_id.to_owned()),
        version: manifest.version,
        license: manifest.license,
        source_url: manifest.source_url,
        revision: manifest.revision,
        icons,
    }
}

/// Parse a directory's manifest text, falling back to an empty manifest when it
/// is absent or malformed. A bad manifest degrades search metadata; it never
/// hides the icons themselves.
fn manifest_or_default(text: Option<&str>, label: &str) -> Manifest {
    match text {
        None => Manifest::default(),
        Some(text) => match parse_manifest(text) {
            Ok(manifest) => manifest,
            Err(e) => {
                eprintln!("note: ignoring '{label}/library.kdl': {e}");
                Manifest::default()
            }
        },
    }
}

/// Build one [`SvgLibrary`] from a bundled [`RawSvgLibrary`].
fn load_embedded(raw: &RawSvgLibrary) -> SvgLibrary {
    let manifest = manifest_or_default(raw.manifest, raw.dir);
    let icons = raw
        .icons
        .iter()
        .filter(|(name, _)| is_usable_icon_name(name))
        .map(|(name, svg)| ((*name).to_owned(), Cow::Borrowed(*svg)))
        .collect();
    assemble(&format!("@zenith/icons-{}", raw.dir), manifest, icons)
}

/// Every SVG icon library bundled into the binary, in generated (sorted) order.
pub fn embedded_svg_libraries() -> Vec<SvgLibrary> {
    EMBEDDED_SVG_LIBRARIES.iter().map(load_embedded).collect()
}

/// Whether `path` is a directory holding at least one `*.svg` file — the sole
/// test for "is this an SVG library?".
pub fn is_svg_dir(path: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(path) else {
        return false;
    };
    entries
        .flatten()
        .any(|e| e.path().extension().and_then(|x| x.to_str()) == Some("svg"))
}

/// Load an SVG library from a project directory.
///
/// Every readable `*.svg` file becomes an icon; `library.kdl`, when present,
/// supplies identity and search metadata. With no manifest the id defaults to
/// `@local/<dirname>`.
///
/// # Errors
///
/// Returns a message when the directory cannot be read, or when it holds no
/// usable `*.svg` file.
pub fn load_svg_dir(path: &Path) -> Result<SvgLibrary, String> {
    let entries =
        std::fs::read_dir(path).map_err(|e| format!("cannot read '{}': {e}", path.display()))?;

    let mut icons: Vec<(String, Cow<'static, str>)> = Vec::new();
    for entry in entries.flatten() {
        let file = entry.path();
        if file.extension().and_then(|e| e.to_str()) != Some("svg") {
            continue;
        }
        let Some(stem) = file.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if !is_usable_icon_name(stem) {
            eprintln!("note: skipping '{}': unusable icon name", file.display());
            continue;
        }
        match std::fs::read_to_string(&file) {
            Ok(svg) => icons.push((stem.to_owned(), Cow::Owned(svg))),
            Err(e) => eprintln!("note: skipping '{}': {e}", file.display()),
        }
    }

    if icons.is_empty() {
        return Err(format!("'{}' holds no usable SVG icon", path.display()));
    }

    let dir_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("icons")
        .to_owned();
    let manifest_text = std::fs::read_to_string(path.join("library.kdl")).ok();
    let manifest = manifest_or_default(manifest_text.as_deref(), &dir_name);

    Ok(assemble(&format!("@local/{dir_name}"), manifest, icons))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_lucide_is_present_and_pinned() {
        let libs = embedded_svg_libraries();
        let lucide = libs
            .iter()
            .find(|l| l.id == "@zenith/icons-lucide")
            .expect("lucide must be bundled");
        assert_eq!(lucide.version.as_deref(), Some("1.23.0"));
        assert_eq!(
            lucide.revision.as_deref(),
            Some("c67c9bdbfb43b0ecd69b52d37aeb4ab2d5386271"),
            "the vendored revision must stay recorded"
        );
        assert_eq!(lucide.license.as_deref(), Some("ISC AND MIT"));
    }

    #[test]
    fn bundled_icons_are_sorted_and_unique() {
        for lib in embedded_svg_libraries() {
            let names: Vec<&str> = lib.icons.iter().map(|i| i.name.as_str()).collect();
            let mut sorted = names.clone();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(names, sorted, "{} icons must be sorted + unique", lib.id);
        }
    }

    #[test]
    fn manifest_metadata_lands_on_icons() {
        let libs = embedded_svg_libraries();
        let lucide = libs
            .iter()
            .find(|l| l.id == "@zenith/icons-lucide")
            .expect("lucide");
        let house = lucide.icon("house").expect("house icon exists");
        assert!(house.aliases.contains(&"home".to_owned()));
        assert!(house.tags.contains(&"living".to_owned()));
        assert!(house.categories.contains(&"buildings".to_owned()));
    }

    /// `sync` was a Zenith-local name for upstream `refresh-cw`. It must not be
    /// an id, and must remain findable as an alias.
    #[test]
    fn legacy_names_survive_as_tags_not_ids() {
        let libs = embedded_svg_libraries();
        let lucide = libs
            .iter()
            .find(|l| l.id == "@zenith/icons-lucide")
            .expect("lucide");
        assert!(
            lucide.icon("sync").is_none(),
            "`sync` is not an upstream id"
        );
        let refresh = lucide.icon("refresh-cw").expect("refresh-cw exists");
        assert!(refresh.aliases.contains(&"sync".to_owned()));

        assert!(lucide.icon("upload-cloud").is_none());
        let upload = lucide.icon("cloud-upload").expect("cloud-upload exists");
        assert!(upload.aliases.contains(&"upload-cloud".to_owned()));
    }

    #[test]
    fn rejects_unusable_icon_names() {
        assert!(is_usable_icon_name("arrow-right"));
        assert!(!is_usable_icon_name(""));
        assert!(!is_usable_icon_name("a#b"));
        assert!(!is_usable_icon_name("my icon"));
        assert!(!is_usable_icon_name("q\"t"));
    }
}
