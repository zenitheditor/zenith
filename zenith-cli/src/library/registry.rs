//! Pack formats, embedded preset table, and the project/preset resolver.
//!
//! A library "pack" comes in one of two [`PackFormat`]s:
//!
//! - [`PackFormat::Zen`] — a `.zen` file whose IDENTITY is declared by a single
//!   `library` SELF-entry in its own `libraries` block. The feature-rich format:
//!   components, tokens, actions.
//! - [`PackFormat::SvgDir`] — a DIRECTORY of `*.svg` files, one icon per file.
//!   The plug-and-install format: nothing to author, and an icon set is extended
//!   by dropping in a file. See [`super::svg_lib`].
//!
//! This module owns the pack METADATA model ([`LibraryPack`], [`PackItem`],
//! [`PackSource`], [`ItemKind`]), the [`EMBEDDED_PACKS`] preset table, parsing a
//! pack's identity/items ([`parse_pack`]), and resolving project packs against
//! embedded presets ([`resolve_packs`]). Both formats produce the same
//! [`LibraryPack`], so callers never branch on format.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use zenith_core::{Document, KdlAdapter, KdlSource, TokenType};

use super::svg_lib::{
    ItemScope, SVG_PACK_TOKEN_COUNT, SvgLibrary, embedded_svg_libraries, is_svg_dir, load_svg_dir,
    raw_embedded_icon, synthesize_pack_source,
};

/// Embedded preset `.zen` packs, as `(pack_id, pack_source)` pairs.
///
/// Each `pack_source` is the verbatim `.zen` text of a shipped preset library,
/// bundled into the binary via [`include_str!`] (mirroring how the default
/// fonts are bundled in `zenith-core`). The `pack_id` is the expected package
/// id and is used only for diagnostics/lookup convenience; the authoritative id
/// is parsed from the pack's own `library` self-entry.
///
/// Bundled ICON packs are not listed here: they are SVG directories under
/// `assets/libraries/icons/`, surfaced by [`embedded_svg_libraries`].
pub const EMBEDDED_PACKS: &[(&str, &str)] = &[
    (
        "@zenith/flowchart",
        include_str!("../../assets/libraries/zenith-flowchart.zen"),
    ),
    (
        "@zenith/filters",
        include_str!("../../assets/libraries/zenith-filters.zen"),
    ),
    (
        "@zenith/masks",
        include_str!("../../assets/libraries/zenith-masks.zen"),
    ),
    (
        "@zenith/brand-kit",
        include_str!("../../assets/libraries/zenith-brand-kit.zen"),
    ),
    (
        "@zenith/theme.cobalt",
        include_str!("../../assets/skill/themes/cobalt.zen"),
    ),
    (
        "@zenith/theme.ember",
        include_str!("../../assets/skill/themes/ember.zen"),
    ),
    (
        "@zenith/theme.harbor",
        include_str!("../../assets/skill/themes/harbor.zen"),
    ),
    (
        "@zenith/theme.lagoon",
        include_str!("../../assets/skill/themes/lagoon.zen"),
    ),
    (
        "@zenith/theme.pine",
        include_str!("../../assets/skill/themes/pine.zen"),
    ),
    (
        "@zenith/theme.poppy",
        include_str!("../../assets/skill/themes/poppy.zen"),
    ),
    (
        "@zenith/theme.prism",
        include_str!("../../assets/skill/themes/prism.zen"),
    ),
    (
        "@zenith/theme.sorbet",
        include_str!("../../assets/skill/themes/sorbet.zen"),
    ),
    (
        "@zenith/theme.sunset",
        include_str!("../../assets/skill/themes/sunset.zen"),
    ),
    (
        "@zenith/theme.volt",
        include_str!("../../assets/skill/themes/volt.zen"),
    ),
];

/// The document-relative directory under which every bundled SVG icon is
/// addressable as an asset: `assets/zenith/icons/<library>/<icon>.svg`.
const EMBEDDED_ICON_ASSET_ROOT: &str = "assets/zenith/icons/";

/// A preset asset embedded in the binary and materialized beside target `.zen`
/// documents when a document declares it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedPresetAsset {
    /// Safe document-relative asset path used in a document's `asset src`.
    pub src: String,
    /// Bundled asset bytes to write at [`Self::src`].
    pub bytes: &'static [u8],
}

/// Find an embedded preset asset by the document-relative `asset src` path.
///
/// Every icon of every bundled SVG library is addressable this way, so
/// `asset src="assets/zenith/icons/lucide/house.svg"` resolves without the icon
/// having to be enumerated anywhere. Paths that escape the icon root, name no
/// bundled library, or name no icon in it, resolve to `None`.
pub fn embedded_preset_asset(src: &str) -> Option<EmbeddedPresetAsset> {
    let rest = src.strip_prefix(EMBEDDED_ICON_ASSET_ROOT)?;
    if rest.contains("..") {
        return None;
    }
    let (dir, file) = rest.split_once('/')?;
    let name = file.strip_suffix(".svg")?;
    if dir.is_empty() || name.is_empty() || name.contains('/') {
        return None;
    }
    let svg = raw_embedded_icon(dir, name)?;
    Some(EmbeddedPresetAsset {
        src: src.to_owned(),
        bytes: svg.as_bytes(),
    })
}

/// Return embedded preset assets declared by `doc`, de-duplicated by `src` and
/// ordered by document asset declaration order.
pub fn embedded_preset_assets_for_document(doc: &Document) -> Vec<EmbeddedPresetAsset> {
    let mut seen = BTreeSet::new();
    let mut assets = Vec::new();
    for decl in &doc.assets.assets {
        if !seen.insert(decl.src.as_str()) {
            continue;
        }
        if let Some(asset) = embedded_preset_asset(&decl.src) {
            assets.push(asset);
        }
    }
    assets
}

/// Where a [`LibraryPack`] was loaded from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackSource {
    /// A preset pack embedded in the binary.
    Preset,
    /// A project pack read from the given path: a `.zen` file for
    /// [`PackFormat::Zen`], a directory for [`PackFormat::SvgDir`].
    Project(PathBuf),
}

impl PackSource {
    /// A short, stable label for human/JSON output: `"preset"` or `"project"`.
    pub fn label(&self) -> &'static str {
        match self {
            PackSource::Preset => "preset",
            PackSource::Project(_) => "project",
        }
    }
}

/// How a [`LibraryPack`]'s content is stored.
///
/// The format decides only how the pack's [`Document`] is OBTAINED — parsed, or
/// synthesized from SVG. Everything downstream of that is identical.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackFormat {
    /// A `.zen` pack: components, tokens, and actions, authored directly.
    Zen,
    /// A directory of `*.svg` files: one icon component per file, converted to
    /// native paths on demand.
    SvgDir,
}

impl PackFormat {
    /// A short, stable label for human/JSON output: `"zen"` or `"svg"`.
    pub fn label(&self) -> &'static str {
        match self {
            PackFormat::Zen => "zen",
            PackFormat::SvgDir => "svg",
        }
    }
}

/// What kind of thing a pack item is.
///
/// A pack exports COMPONENT items (materialized as an instance on a page),
/// TOKEN items (filter tokens, copied into the target's tokens block with their
/// color-token dependencies — no instance, no page required), and ACTION items
/// (addressed as `<pkg>#<action-id>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    /// A component item, addressed `<pkg>#<component-id>`.
    Component,
    /// A filter-token item, addressed `<pkg>#<token-id>`.
    Token,
    /// An action item, addressed `<pkg>#<action-id>`.
    Action,
}

impl ItemKind {
    /// A short, stable label for human/JSON output: `"component"`, `"token"`, or `"action"`.
    pub fn label(&self) -> &'static str {
        match self {
            ItemKind::Component => "component",
            ItemKind::Token => "token",
            ItemKind::Action => "action",
        }
    }
}

/// A single exported item of a [`LibraryPack`]: its id, its kind, and the
/// metadata `library search` ranks and filters on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackItem {
    /// The item id (a component id, a filter-token id, or an action id).
    pub id: String,
    /// Whether the item is a component, a filter token, or an action.
    pub kind: ItemKind,
    /// Alternate NAMES for the item, ranked with near-id authority. Supplied by
    /// an SVG library's `library.kdl`; empty for `.zen` packs.
    pub aliases: Vec<String>,
    /// Related words, ranked below names. Supplied by an SVG library's
    /// `library.kdl`; empty for `.zen` packs, whose items are searched by id.
    pub tags: Vec<String>,
    /// Closed-vocabulary categories used to FILTER (`--category`), never to
    /// rank: a category applies to hundreds of items and would swamp scoring.
    pub categories: Vec<String>,
}

impl PackItem {
    /// A pack item with no search metadata — the shape every `.zen` pack item takes.
    fn bare(id: String, kind: ItemKind) -> Self {
        Self {
            id,
            kind,
            aliases: Vec::new(),
            tags: Vec::new(),
            categories: Vec::new(),
        }
    }
}

/// A loaded library pack: its identity plus the items it provides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryPack {
    /// The package id, parsed from the pack's `library` self-entry.
    pub id: String,
    /// The pack version, parsed from the pack's `library` self-entry.
    pub version: Option<String>,
    /// Where the pack came from.
    pub source: PackSource,
    /// How the pack's content is stored, and therefore how its [`Document`] is
    /// obtained.
    pub format: PackFormat,
    /// SPDX-style license expression the pack declares, when it declares one.
    /// SVG libraries carry it in `library.kdl`; `.zen` packs do not declare one.
    pub license: Option<String>,
    /// The items the pack provides: component ids first (in source order),
    /// then exportable token ids (in source order), then action ids (in source
    /// order).
    pub items: Vec<PackItem>,
    /// The pack's WHOLE token count (every token in its `tokens` block,
    /// unfiltered — not just the exportable filter/mask tokens counted in
    /// [`Self::items`]). This is the size of the token set `zenith theme apply`
    /// would merge into a document, and drives the `(tokens: N)` indicator in
    /// `library list`.
    pub token_count: usize,
}

/// Whether a token type is an EXPORTABLE library item (addressable as
/// `<pkg>#<token-id>` and copied by `materialize_token`).
///
/// Effect tokens — `filter` and `mask` — are self-contained, applyable units
/// that other documents reference by id, so they are exported items. Color /
/// dimension / gradient / shadow tokens are dependencies pulled in transitively
/// when an exported token (or component) needs them, not standalone items.
pub(super) fn is_exportable_token(ty: &TokenType) -> bool {
    matches!(ty, TokenType::Filter | TokenType::Mask)
}

/// An error produced while parsing a pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackError {
    /// Human-readable message describing the failure.
    pub message: String,
}

impl PackError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for PackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for PackError {}

/// Parse a `.zen` pack `source` into a [`LibraryPack`] tagged with `source_kind`.
///
/// Pack identity is derived from the document's `libraries` block: the library
/// entry whose `id` matches the document's `project` id is the SELF-entry; if no
/// entry matches the project id but there is exactly one library entry, that
/// sole entry is used. A pack with no identifying library self-entry is an error
/// (a pack MUST declare its identity).
///
/// Items are the document's component ids in source order, followed by its
/// FILTER token ids in source order, followed by its action ids in source
/// order. (Only filter tokens are exported items; color/dimension tokens are
/// dependencies, not items.) [`LibraryPack::token_count`]
/// separately captures the SIZE OF THE WHOLE `tokens` block, unfiltered — the
/// set `zenith theme apply` would merge wholesale, as opposed to the individually
/// addressable filter/mask token items.
///
/// # Errors
///
/// Returns [`PackError`] when the source fails to parse, or when no library
/// self-entry can be determined.
pub fn parse_pack(source: &str, source_kind: PackSource) -> Result<LibraryPack, PackError> {
    let doc = KdlAdapter
        .parse(source.as_bytes())
        .map_err(|e| PackError::new(format!("parse error: {}", e)))?;

    let project_id = doc.project.as_ref().map(|p| p.id.as_str());

    // Prefer the library entry whose id matches the project id; otherwise fall
    // back to the sole library entry when there is exactly one.
    let self_entry = project_id
        .and_then(|pid| doc.libraries.iter().find(|lib| lib.id == pid))
        .or(match doc.libraries.as_slice() {
            [only] => Some(only),
            _ => None,
        });

    let self_entry = self_entry.ok_or_else(|| {
        PackError::new(
            "pack has no identifying library self-entry (declare \
             `libraries { library id=\"…\" version=\"…\" }`)",
        )
    })?;

    // Component items first (source order), then filter-token items (source
    // order). A token is an exported item only when it is a filter token.
    let mut items: Vec<PackItem> = doc
        .components
        .iter()
        .map(|c| PackItem::bare(c.id.clone(), ItemKind::Component))
        .collect();
    items.extend(
        doc.tokens
            .tokens
            .iter()
            .filter(|t| is_exportable_token(&t.token_type))
            .map(|t| PackItem::bare(t.id.clone(), ItemKind::Token)),
    );
    items.extend(
        doc.actions
            .iter()
            .map(|a| PackItem::bare(a.id.clone(), ItemKind::Action)),
    );

    Ok(LibraryPack {
        id: self_entry.id.clone(),
        version: self_entry.version.clone(),
        source: source_kind,
        format: PackFormat::Zen,
        license: None,
        token_count: doc.tokens.tokens.len(),
        items,
    })
}

/// Build the [`LibraryPack`] metadata for an SVG icon library.
///
/// Metadata comes straight off the filenames and the manifest — no SVG is
/// parsed and no geometry is converted, so listing or searching a 1745-icon
/// library is as cheap as listing a two-component `.zen` pack. Conversion
/// happens only when an item is materialized.
///
/// Every icon is a component item. The pack's two stroke tokens are dependencies
/// rather than exported items (they are neither filter nor mask tokens), so they
/// contribute to [`LibraryPack::token_count`] but not to `items`.
pub fn svg_library_pack(lib: &SvgLibrary, source_kind: PackSource) -> LibraryPack {
    LibraryPack {
        id: lib.id.clone(),
        version: lib.version.clone(),
        source: source_kind,
        format: PackFormat::SvgDir,
        license: lib.license.clone(),
        token_count: SVG_PACK_TOKEN_COUNT,
        items: lib
            .icons
            .iter()
            .map(|icon| PackItem {
                id: icon.name.clone(),
                kind: ItemKind::Component,
                aliases: icon.aliases.clone(),
                tags: icon.tags.clone(),
                categories: icon.categories.clone(),
            })
            .collect(),
    }
}

/// Load every bundled pack: the [`EMBEDDED_PACKS`] `.zen` presets, then the
/// bundled SVG icon libraries.
///
/// An embedded `.zen` pack that fails to parse is skipped (embedded content is
/// shipped and tested, so this should not happen in practice); the returned
/// vector contains only the packs that loaded successfully.
pub fn load_embedded_packs() -> Vec<LibraryPack> {
    let mut packs: Vec<LibraryPack> = EMBEDDED_PACKS
        .iter()
        .filter_map(|(_, src)| parse_pack(src, PackSource::Preset).ok())
        .collect();
    packs.extend(
        embedded_svg_libraries()
            .iter()
            .map(|lib| svg_library_pack(lib, PackSource::Preset)),
    );
    packs
}

/// Scan `project_dir/libraries/` for packs of either format.
///
/// A `*.zen` FILE is a `.zen` pack. A SUBDIRECTORY holding at least one `*.svg`
/// is an SVG icon library — that is the whole opt-in: drop a folder of icons
/// there and it is a pack.
///
/// A missing `libraries/` directory (or a `project_dir` without one) yields an
/// empty vector. Entries that fail to read or parse are skipped — a note is
/// written to stderr — so one bad pack never aborts the whole listing.
pub fn load_project_packs(project_dir: &Path) -> Vec<LibraryPack> {
    let libraries_dir = project_dir.join("libraries");
    let entries = match std::fs::read_dir(&libraries_dir) {
        Ok(entries) => entries,
        // Missing directory (or any read error) → no project packs.
        Err(_) => return Vec::new(),
    };

    let mut packs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            if !is_svg_dir(&path) {
                continue;
            }
            match load_svg_dir(&path) {
                Ok(lib) => packs.push(svg_library_pack(&lib, PackSource::Project(path.clone()))),
                Err(e) => eprintln!("note: skipping '{}': {}", path.display(), e),
            }
            continue;
        }

        if path.extension().and_then(|e| e.to_str()) != Some("zen") {
            continue;
        }
        let source = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("note: skipping '{}': {}", path.display(), e);
                continue;
            }
        };
        match parse_pack(&source, PackSource::Project(path.clone())) {
            Ok(pack) => packs.push(pack),
            Err(e) => eprintln!("note: skipping '{}': {}", path.display(), e),
        }
    }
    packs
}

/// Resolve all available packs: project packs first, then embedded presets.
///
/// Project packs take precedence over embedded packs of the same id (a project
/// pack SHADOWS an embedded preset). Both are returned, each tagged with its
/// [`PackSource`], so callers that LIST can show every pack; callers that
/// MATERIALIZE should prefer the first pack for a given id. The result is sorted
/// by id for deterministic output (project before embedded on ties).
pub fn resolve_packs(project_dir: Option<&Path>) -> Vec<LibraryPack> {
    let mut packs = Vec::new();
    if let Some(dir) = project_dir {
        packs.extend(load_project_packs(dir));
    }
    packs.extend(load_embedded_packs());

    // Stable, deterministic order: by id, with project packs ahead of embedded
    // on ties (so the shadowing winner sorts first).
    packs.sort_by(|a, b| {
        a.id.cmp(&b.id)
            .then_with(|| source_rank(&a.source).cmp(&source_rank(&b.source)))
    });
    packs
}

/// Sort rank giving project packs precedence over embedded presets.
fn source_rank(source: &PackSource) -> u8 {
    match source {
        PackSource::Project(_) => 0,
        PackSource::Preset => 1,
    }
}

/// Read the `.zen` source text behind a [`PackFormat::Zen`] pack.
fn zen_pack_source(pack: &LibraryPack) -> Result<String, String> {
    match &pack.source {
        PackSource::Preset => EMBEDDED_PACKS
            .iter()
            .find(|(id, _)| *id == pack.id)
            .map(|(_, src)| (*src).to_owned())
            .ok_or_else(|| format!("embedded pack '{}' source not found", pack.id)),
        PackSource::Project(path) => std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read pack '{}': {e}", path.display())),
    }
}

/// Load the [`SvgLibrary`] behind a [`PackFormat::SvgDir`] pack.
fn svg_pack_library(pack: &LibraryPack) -> Result<SvgLibrary, String> {
    match &pack.source {
        PackSource::Preset => embedded_svg_libraries()
            .into_iter()
            .find(|lib| lib.id == pack.id)
            .ok_or_else(|| format!("bundled SVG library '{}' not found", pack.id)),
        PackSource::Project(path) => load_svg_dir(path),
    }
}

/// Obtain the pack's [`Document`] — the single funnel both formats meet at.
///
/// [`resolve_packs`] yields only pack METADATA; materialization needs the pack's
/// component/token/style/asset subtrees. A `.zen` pack is re-read and parsed; an
/// SVG library is SYNTHESIZED, converting exactly the icons `scope` admits.
///
/// Pass [`ItemScope::Only`] whenever the caller knows which item it wants: for a
/// 1745-icon library, [`ItemScope::All`] converts every icon.
///
/// # Errors
///
/// Returns a message when the pack's source cannot be located, read, converted,
/// or parsed.
pub fn pack_document(pack: &LibraryPack, scope: ItemScope<'_>) -> Result<Document, String> {
    let source = match pack.format {
        PackFormat::Zen => zen_pack_source(pack)?,
        PackFormat::SvgDir => {
            let lib = svg_pack_library(pack)?;
            synthesize_pack_source(&lib, scope)?
        }
    };
    KdlAdapter
        .parse(source.as_bytes())
        .map_err(|e| format!("pack '{}' failed to parse: {}", pack.id, e.message))
}

/// Resolve a theme reference — a bare theme name (`sunset`) or a full pack id
/// (`@zenith/theme.sunset`) — to its parsed [`Document`], for splicing its
/// token block into another document (`zenith new --theme`, `zenith theme
/// apply`).
///
/// A bare name (no leading `@`) is expanded to `@zenith/theme.<name>` before
/// matching. Resolution goes through [`resolve_packs`], so a project pack
/// (`<project_dir>/libraries/*.zen`) shadows an embedded preset of the same
/// id, exactly like every other pack lookup.
///
/// # Errors
///
/// Returns a plain error message (no CLI-specific error type — callers map it
/// into their own error struct) when no pack matches. A bare-name miss lists
/// the available embedded theme names (sorted, `@zenith/theme.` prefix
/// stripped) so the message stays actionable.
pub fn resolve_theme_pack(
    project_dir: Option<&Path>,
    name_or_id: &str,
) -> Result<Document, String> {
    let target_id = if name_or_id.starts_with('@') {
        name_or_id.to_owned()
    } else {
        format!("@zenith/theme.{name_or_id}")
    };

    let packs = resolve_packs(project_dir);
    let Some(pack) = packs.iter().find(|p| p.id == target_id) else {
        let mut available: Vec<&str> = EMBEDDED_PACKS
            .iter()
            .filter_map(|(id, _)| id.strip_prefix("@zenith/theme."))
            .collect();
        available.sort_unstable();
        return Err(format!(
            "unknown theme '{}' (available: {})",
            name_or_id,
            available.join(", ")
        ));
    };

    // A theme is a token block; an SVG icon library has no theme to give.
    match pack.format {
        PackFormat::Zen => {}
        PackFormat::SvgDir => {
            return Err(format!(
                "'{}' is an SVG icon library, not a theme",
                target_id
            ));
        }
    }

    pack_document(pack, ItemScope::All)
}
