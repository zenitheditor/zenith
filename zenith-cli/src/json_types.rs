//! Serialisable DTO types for JSON output.
//!
//! These types are defined in the CLI crate — we do NOT add serde to
//! zenith-core.  Each type maps from zenith-core/zenith-scene types to a
//! schema-versioned JSON shape.

use serde::Serialize;

use crate::report::ImportFiles;

/// JSON representation of a [`zenith_core::Diagnostic`].
///
/// `line` and `col` are 1-based. They are present only when the diagnostic
/// has a span and the caller supplied the source text. `file` names the
/// imported file the span indexes into; it is absent for spans in the host
/// document. `cause` names a root cause shared by many subjects.
#[derive(Debug, Serialize)]
pub struct DiagnosticJson {
    pub code: String,
    pub severity: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub col: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
    /// Structured machine fix (`zenith fix` applies it).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<zenith_core::FixHint>,
}

impl DiagnosticJson {
    /// An error entry with no subject, location, or cause.
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity: "error".to_owned(),
            message: message.into(),
            subject_id: None,
            line: None,
            col: None,
            file: None,
            cause: None,
            fix: None,
        }
    }

    /// Convert `d`, adding `line`/`col` from its span over the host `src`.
    ///
    /// A diagnostic from an import has no known file here, so it gets no
    /// `line`/`col`. Use [`Self::located_all_in`] when imports are known.
    pub fn located(d: &zenith_core::Diagnostic, src: &str) -> Self {
        Self::located_with(d, &mut crate::report::Locator::new(src))
    }

    /// Convert every diagnostic in `diagnostics`, located over the host `src`.
    pub fn located_all(diagnostics: &[zenith_core::Diagnostic], src: &str) -> Vec<Self> {
        Self::located_all_in(diagnostics, src, &ImportFiles::default())
    }

    /// Convert every diagnostic in `diagnostics`.
    ///
    /// A span with no import indexes into the host `src`. A span from an import
    /// indexes into that import's file in `files`: `file` names it and
    /// `line`/`col` come from its text. When the file is unknown or unreadable,
    /// `line`/`col` are omitted.
    pub fn located_all_in(
        diagnostics: &[zenith_core::Diagnostic],
        src: &str,
        files: &ImportFiles,
    ) -> Vec<Self> {
        let mut locator = crate::report::Locator::with_files(src, files);
        diagnostics
            .iter()
            .map(|d| Self::located_with(d, &mut locator))
            .collect()
    }

    fn located_with(d: &zenith_core::Diagnostic, locator: &mut crate::report::Locator<'_>) -> Self {
        let mut out = Self::from(d);
        let loc = locator.locate(d);
        out.file = loc.file;
        if let Some((line, col)) = loc.line_col {
            out.line = Some(line);
            out.col = Some(col);
        }
        out
    }
}

impl From<&zenith_core::Diagnostic> for DiagnosticJson {
    fn from(d: &zenith_core::Diagnostic) -> Self {
        Self {
            code: d.code.clone(),
            severity: severity_str(&d.severity).to_owned(),
            message: d.message.clone(),
            subject_id: d.subject_id.clone(),
            line: None,
            col: None,
            file: None,
            cause: d.cause().map(str::to_owned),
            fix: d.fix().cloned(),
        }
    }
}

pub(crate) fn severity_str(s: &zenith_core::Severity) -> &'static str {
    match s {
        zenith_core::Severity::Error => "error",
        zenith_core::Severity::Warning => "warning",
        zenith_core::Severity::Advisory => "advisory",
    }
}

/// Top-level JSON envelope for `validate`.
#[derive(Debug, Serialize)]
pub struct ValidateOutput {
    pub schema: &'static str,
    pub valid: bool,
    pub diagnostics: Vec<DiagnosticJson>,
}

/// Top-level JSON envelope for `fmt`.
#[derive(Debug, Serialize)]
pub struct FmtOutput {
    pub schema: &'static str,
    pub changed: bool,
    pub hash: String,
}

/// A single token entry for `tokens` output.
#[derive(Debug, Serialize)]
pub struct TokenEntry {
    pub id: String,
    pub token_type: String,
    pub resolved_value: String,
}

/// Top-level JSON envelope for `tokens`.
#[derive(Debug, Serialize)]
pub struct TokensOutput {
    pub schema: &'static str,
    pub tokens: Vec<TokenEntry>,
    pub diagnostics: Vec<DiagnosticJson>,
}

/// Top-level JSON envelope for `render`.
///
/// `status` is `"ok"` when every requested output was written, else
/// `"blocked"`. `outputs` lists the written paths in write order. `images`
/// lists every written PNG with its pixel size and raster scale, in write
/// order; it is omitted when no PNG was written.
#[derive(Debug, Serialize)]
pub struct RenderOutput {
    pub schema: &'static str,
    pub status: &'static str,
    pub outputs: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<RenderImageJson>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rasterized_regions: Vec<RenderRasterizedRegionJson>,
    pub diagnostics: Vec<DiagnosticJson>,
}

/// One raster fallback range in a written vector output.
#[derive(Debug, Serialize)]
pub struct RenderRasterizedRegionJson {
    pub path: String,
    pub format: &'static str,
    pub page: usize,
    pub command_start: usize,
    pub command_end: usize,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raster_scale: Option<f64>,
}

/// One written PNG in the `render` envelope.
///
/// `kind` is `png`, `page`, `spread`, or `contact_sheet`. `pages` holds the
/// 1-based page numbers the image shows. `columns` and `rows` are set for a
/// contact sheet only.
#[derive(Debug, Clone, Serialize)]
pub struct RenderImageJson {
    pub path: String,
    pub kind: &'static str,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
    pub pages: Vec<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub columns: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rows: Option<u32>,
}

/// JSON result of `library add --json`.
///
/// `written` is false under `--dry-run`. `source` holds the resulting document
/// text under `--dry-run` only.
#[derive(Debug, Serialize)]
pub struct LibraryAddOutput {
    pub schema: &'static str,
    pub path: String,
    pub written: bool,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// The `--parent` container the instance was placed into, when given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

/// JSON envelope for a failure outside a command's own output shape: a
/// missing file, a bad flag, or a parse error before the command runs.
#[derive(Debug, Serialize)]
pub struct ErrorOutput {
    pub schema: &'static str,
    pub diagnostics: Vec<DiagnosticJson>,
}

/// Top-level JSON envelope for `tx`.
#[derive(Debug, Serialize)]
pub struct TxOutputJson {
    pub schema: &'static str,
    pub status: String,
    pub affected: Vec<String>,
    pub diagnostics: Vec<DiagnosticJson>,
    pub changed: bool,
    /// Unified diff of the canonical source before and after. Present on a
    /// dry-run, or with `--apply --diff`, when the source changed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_diff: Option<String>,
    /// Nodes whose compiled page box changed, sorted by id. Present with
    /// `source_diff`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boxes: Option<Vec<crate::commands::tx::BoxDelta>>,
}

/// One skipped-token entry in the `zenith theme apply --json` extras.
///
/// `doc_type` is `None` when the theme token has no same-id counterpart in the
/// document at all (an unencodable theme-side value, not a doc-side clash).
#[derive(Debug, Serialize)]
pub struct ThemeApplySkipJson {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_type: Option<String>,
    pub theme_type: String,
    pub reason: &'static str,
}

impl From<&crate::commands::theme::SkippedToken> for ThemeApplySkipJson {
    fn from(s: &crate::commands::theme::SkippedToken) -> Self {
        Self {
            id: s.id.clone(),
            doc_type: s.doc_type.clone(),
            theme_type: s.theme_type.clone(),
            reason: s.reason.label(),
        }
    }
}

/// One skipped style or `defaults` entry in the `zenith theme apply --json`
/// extras. `id` is the style id, or the node kind of a `defaults` entry.
#[derive(Debug, Serialize)]
pub struct ThemeApplyBlockSkipJson {
    pub id: String,
    pub reason: &'static str,
}

impl From<&crate::commands::theme::SkippedBlockItem> for ThemeApplyBlockSkipJson {
    fn from(s: &crate::commands::theme::SkippedBlockItem) -> Self {
        Self {
            id: s.id.clone(),
            reason: s.reason.label(),
        }
    }
}

/// Per-row result in the `merge --json` batch report.
#[derive(Debug, Serialize)]
pub struct MergeRowResult {
    pub row: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub status: &'static str,
    pub outputs: Vec<String>,
    pub diagnostics: Vec<DiagnosticJson>,
}

/// Top-level JSON envelope for `merge --json`.
#[derive(Debug, Serialize)]
pub struct MergeOutput {
    pub schema: &'static str,
    pub total_rows: usize,
    pub written: usize,
    pub failed: usize,
    pub rows: Vec<MergeRowResult>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<DiagnosticJson>,
}

/// One manifest row with committed files, including partial writes.
#[derive(Debug, Serialize)]
pub struct ManifestRow {
    pub row: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub outputs: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<&'static str>,
}

/// Deterministic generation manifest for `zenith merge --manifest`.
#[derive(Debug, Serialize)]
pub struct MergeManifest {
    pub schema: &'static str,
    /// Manifest format version. Bumped only when the manifest structure changes
    /// (never on a routine crate release), so identical inputs stay byte-identical.
    pub generator: &'static str,
    pub source_sha256: String,
    pub data_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_by: Option<String>,
    pub rows: Vec<ManifestRow>,
}

// ── Variant JSON types ────────────────────────────────────────────────────────

/// Per-variant result in the `variant --json` envelope.
#[derive(Debug, Serialize)]
pub struct VariantResultJson {
    pub id: String,
    pub source: String,
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outputs_zen: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outputs_png: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outputs_svg: Option<String>,
    pub diagnostics: Vec<DiagnosticJson>,
}

/// Top-level JSON envelope for `variant --json`.
#[derive(Debug, Serialize)]
pub struct VariantOutput {
    pub schema: &'static str,
    pub total_variants: usize,
    pub generated: usize,
    pub failed: usize,
    pub variants: Vec<VariantResultJson>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<DiagnosticJson>,
}

/// One manifest target with committed files, including partial writes.
#[derive(Debug, Serialize)]
pub struct VariantManifestTarget {
    pub id: String,
    pub source: String,
    pub outputs_zen: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub outputs_png: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outputs_svg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<&'static str>,
}

/// Deterministic generation manifest for `zenith variant --manifest`.
#[derive(Debug, Serialize)]
pub struct VariantManifest {
    pub schema: &'static str,
    /// Manifest format version. Bumped only when the manifest structure changes
    /// (never on a routine crate release), so identical inputs stays byte-identical.
    pub generator: &'static str,
    pub source_sha256: String,
    pub targets: Vec<VariantManifestTarget>,
}

// ── Schema JSON types ─────────────────────────────────────────────────────────

/// A single node-kind entry in the `schema nodes` JSON output.
#[derive(Debug, Serialize)]
pub struct SchemaNodeEntry {
    pub kind: String,
    pub summary: String,
}

/// A single attribute entry in the `schema node <kind>` and `schema page/asset/document` JSON output.
#[derive(Debug, Serialize)]
pub struct SchemaAttr {
    pub name: String,
    pub ty: String,
    /// Value used when the attribute is absent; omitted when it has none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

/// A single node-kind detail entry in the `schema node <kind>` JSON output.
#[derive(Debug, Serialize)]
pub struct SchemaNodeDetail {
    pub kind: String,
    pub summary: String,
    pub attributes: Vec<SchemaAttr>,
    /// Minimal full-node authoring example, when one is available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<String>,
    /// Child-content descriptor for kinds that accept authorable children.
    /// Absent from JSON for kinds with no authorable child content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<SchemaNodeContent>,
}

/// Child-content descriptor embedded in [`SchemaNodeDetail`].
#[derive(Debug, Serialize)]
pub struct SchemaNodeContent {
    pub description: String,
    pub example: String,
}

/// A single op entry in the `schema ops` JSON output.
#[derive(Debug, Serialize)]
pub struct SchemaOpEntry {
    pub op: String,
    pub summary: String,
}

/// A single op field entry in the `schema op <name>` JSON output.
#[derive(Debug, Serialize)]
pub struct SchemaOpFieldEntry {
    pub name: String,
    pub ty: String,
    pub required: bool,
}

/// Full detail entry in the `schema op <name>` JSON output.
#[derive(Debug, Serialize)]
pub struct SchemaOpDetail {
    pub op: String,
    pub summary: String,
    pub fields: Vec<SchemaOpFieldEntry>,
    pub example: String,
}

/// Top-level JSON envelope for `schema nodes`.
#[derive(Debug, Serialize)]
pub struct SchemaNodesOutput {
    pub schema: &'static str,
    pub nodes: Vec<SchemaNodeEntry>,
}

/// Top-level JSON envelope for `schema node <kind>`.
#[derive(Debug, Serialize)]
pub struct SchemaNodeOutput {
    pub schema: &'static str,
    pub node: SchemaNodeDetail,
}

/// Top-level JSON envelope for `schema ops`.
#[derive(Debug, Serialize)]
pub struct SchemaOpsOutput {
    pub schema: &'static str,
    pub ops: Vec<SchemaOpEntry>,
}

/// Top-level JSON envelope for `schema op <name>`.
#[derive(Debug, Serialize)]
pub struct SchemaOpOutput {
    pub schema: &'static str,
    pub op: SchemaOpDetail,
}

/// Top-level JSON envelope for bare `zenith schema` (overview).
#[derive(Debug, Serialize)]
pub struct SchemaOverviewOutput {
    pub schema: &'static str,
    pub node_kinds: usize,
    pub tx_ops: usize,
    pub token_types: usize,
}

/// A single token-type entry in the `schema tokens` JSON output.
#[derive(Debug, Serialize)]
pub struct SchemaTokenEntry {
    pub ty: String,
    pub summary: String,
}

/// Full detail for one token type in the `schema token <type>` JSON output.
#[derive(Debug, Serialize)]
pub struct SchemaTokenDetail {
    pub ty: String,
    pub summary: String,
    pub value_form: String,
    pub child_nodes: String,
    pub example: String,
}

/// Top-level JSON envelope for `schema tokens`.
#[derive(Debug, Serialize)]
pub struct SchemaTokensOutput {
    pub schema: &'static str,
    pub token_types: Vec<SchemaTokenEntry>,
}

/// Top-level JSON envelope for `schema token <type>`.
#[derive(Debug, Serialize)]
pub struct SchemaTokenOutput {
    pub schema: &'static str,
    pub token: SchemaTokenDetail,
}

/// Top-level JSON envelope for `schema page`, `schema asset`, `schema document`.
#[derive(Debug, Serialize)]
pub struct SchemaSurfaceOutput {
    pub schema: &'static str,
    /// Which non-node surface this describes: `"page"`, `"asset"`, or `"document"`.
    pub surface: &'static str,
    pub summary: String,
    pub attributes: Vec<SchemaAttr>,
}

/// A single governable diagnostic-code entry in the `schema diagnostics` JSON.
#[derive(Debug, Serialize)]
pub struct SchemaDiagnosticCode {
    pub code: String,
    /// `"error"`, `"warning"`, or `"advisory"`.
    pub severity: String,
    pub summary: String,
    /// True when an `allow`/`deny`/`warn` entry can adjust this code (Warning /
    /// Advisory). Error-severity codes are immutable and report `false`.
    pub governable: bool,
}

/// Top-level JSON envelope for `schema diagnostics`.
#[derive(Debug, Serialize)]
pub struct SchemaDiagnosticsOutput {
    pub schema: &'static str,
    pub summary: String,
    /// The policy verbs: `allow`, `deny`, `warn`.
    pub verbs: Vec<String>,
    /// Canonical KDL policy-entry forms.
    pub syntax: Vec<String>,
    /// Precedence note: in-file `diagnostics { … }` now; CLI flags/config later.
    pub precedence: &'static str,
    /// Every diagnostic code in the catalog (governable and always-Error).
    pub codes: Vec<SchemaDiagnosticCode>,
}

/// One override property entry in the `schema variant` JSON output.
#[derive(Debug, Serialize)]
pub struct SchemaOverridePropEntry {
    pub name: String,
    pub ty: String,
    pub required: bool,
}

/// Top-level JSON envelope for `schema variant`.
#[derive(Debug, Serialize)]
pub struct SchemaVariantOutput {
    pub schema: &'static str,
    pub summary: String,
    pub block_structure: String,
    pub variant_node: String,
    pub override_entry: String,
    pub override_props: Vec<SchemaOverridePropEntry>,
    pub example: String,
}

/// One `port` property entry in the `schema ports` JSON output.
#[derive(Debug, Serialize)]
pub struct SchemaPortPropEntry {
    pub name: String,
    pub ty: String,
    pub required: bool,
}

/// Top-level JSON envelope for `schema ports`.
#[derive(Debug, Serialize)]
pub struct SchemaPortsOutput {
    pub schema: &'static str,
    pub summary: String,
    pub placement: String,
    pub block_structure: String,
    pub port_props: Vec<SchemaPortPropEntry>,
    pub example: String,
}

/// Top-level JSON envelope for `schema brand`.
#[derive(Debug, Serialize)]
pub struct SchemaBrandOutput {
    pub schema: &'static str,
    pub summary: String,
    pub placement: &'static str,
    pub child_nodes: Vec<SchemaBrandChildNode>,
    pub absent_means: &'static str,
    pub diagnostic_codes: Vec<SchemaBrandDiagCode>,
    pub example: &'static str,
}

/// One child-node descriptor in the `schema brand` JSON output.
#[derive(Debug, Serialize)]
pub struct SchemaBrandChildNode {
    pub node: &'static str,
    pub syntax: &'static str,
    pub description: &'static str,
}

/// One diagnostic code entry in the `schema brand` JSON output.
#[derive(Debug, Serialize)]
pub struct SchemaBrandDiagCode {
    pub code: &'static str,
    pub severity: &'static str,
    pub summary: &'static str,
}

// ── Fonts JSON types ──────────────────────────────────────────────────────────

/// Top-level JSON envelope for `zenith fonts --json`.
#[derive(Debug, Serialize)]
pub struct FontsOutput {
    pub schema: &'static str,
    /// Family names bundled in the binary (name-table case), sorted
    /// case-insensitively. These are portable: any machine with this Zenith
    /// binary will resolve them identically.
    pub bundled: Vec<String>,
    /// Family names found on this machine only (name-table case), sorted
    /// case-insensitively, after excluding any family already in `bundled`.
    /// Using these trips a `font.local` advisory and renders may differ on
    /// another machine.
    pub local: Vec<String>,
}

/// One feature tag entry in [`FontsFeaturesOutput`].
#[derive(Debug, Serialize)]
pub struct FontsFeatureEntry {
    pub tag: String,
    /// Tables advertising this tag: `"GSUB"` and/or `"GPOS"`.
    pub tables: Vec<&'static str>,
}

/// Top-level JSON envelope for `zenith fonts features --json`.
#[derive(Debug, Serialize)]
pub struct FontsFeaturesOutput {
    pub schema: &'static str,
    /// Resolved target label (family name or font path).
    pub target: String,
    pub weight: u16,
    pub style: &'static str,
    pub has_kern_table: bool,
    pub features: Vec<FontsFeatureEntry>,
}

/// Top-level JSON envelope for `zenith fonts alternates --json`.
#[derive(Debug, Serialize)]
pub struct FontsAlternatesOutput {
    pub schema: &'static str,
    /// Resolved target label (family name or font path).
    pub target: String,
    pub weight: u16,
    pub style: &'static str,
    /// Queried character as a single-char string.
    pub char: String,
    /// Codepoint in `U+XXXX` form.
    pub codepoint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub glyph_index: Option<u16>,
    /// Alternate glyph IDs from GSUB AlternateSubstitution (sorted).
    pub alternate_glyph_ids: Vec<u16>,
    /// Honest enumeration limits.
    pub limits: String,
}

// ── Recipe inspect JSON types ─────────────────────────────────────────────────

/// A single `param` entry within a [`RecipeInspectJson`].
#[derive(Debug, Serialize)]
pub struct RecipeParamInspectJson {
    pub name: String,
    /// Canonical string representation of the parameter value: a token ref is
    /// rendered as `"<id>"`, a literal as its raw string, a dimension as
    /// `"(<unit>)<value>"`.
    pub value: String,
}

/// A single recipe entry in the `recipes` array of [`crate::commands::inspect::InspectOutput`].
#[derive(Debug, Serialize)]
pub struct RecipeInspectJson {
    pub id: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounds: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detached: Option<bool>,
    pub params: Vec<RecipeParamInspectJson>,
    pub palette: Vec<String>,
    pub expanded: Vec<String>,
}
