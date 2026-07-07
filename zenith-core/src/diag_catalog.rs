//! The single source of truth for every diagnostic code the engine can emit.
//!
//! This catalog is hand-maintained and drift-guarded (mirroring [`crate::schema`]'s
//! node-kind tables). It drives BOTH:
//! - the diagnostic-policy validation in [`crate::validate()`] (which codes are
//!   *governable* by a `diagnostics { … }` block, and which are always Errors), and
//! - the `zenith schema diagnostics` surface (re-exposed through [`crate::schema`]).
//!
//! ## Adding a new diagnostic code
//!
//! Any new diagnostic code emitted anywhere in the workspace **MUST** be added to
//! [`DIAGNOSTIC_CODES`] below, with its real [`Severity`] and a one-line summary.
//! A code that is not in this catalog is treated as *unknown* by the policy
//! validator: a `diagnostics { … }` entry naming it produces `policy.unknown_code`.
//!
//! ## Governable vs. always-Error
//!
//! A code is **governable** when its catalog severity is `Warning` or `Advisory`:
//! an `allow`/`deny`/`warn` entry can adjust how it is reported. A code whose
//! catalog severity is `Error` is **always-Error** and immutable — `allow`/`warn`
//! cannot weaken it (the validator emits `policy.ineffective_on_error`); a `deny`
//! on it is a silent no-op (it is already an Error).

use crate::diagnostics::Severity;

/// One catalog entry: a stable diagnostic `code`, the [`Severity`] the engine
/// emits it at, and a one-line human summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiagnosticCodeInfo {
    /// The stable dot-separated code, e.g. `"layout.off_canvas"`.
    pub code: &'static str,
    /// The severity the engine emits this diagnostic at.
    pub severity: Severity,
    /// One-line description of what the diagnostic means.
    pub summary: &'static str,
}

impl DiagnosticCodeInfo {
    /// True when a `diagnostics { … }` policy entry can adjust this code — i.e.
    /// its severity is `Warning` or `Advisory`. Error-severity codes are
    /// immutable.
    pub fn is_governable(&self) -> bool {
        match self.severity {
            Severity::Error => false,
            Severity::Warning | Severity::Advisory => true,
        }
    }
}

/// The three policy verbs accepted inside a `diagnostics { … }` block, in
/// canonical order.
pub const DIAGNOSTIC_VERBS: &[&str] = &["allow", "deny", "warn"];

/// The complete catalog of diagnostic codes the engine can emit.
///
/// Sorted by code for deterministic output. The `#[cfg(test)]` drift guard below
/// asserts the list is non-empty and that every code is unique.
///
/// NOTE: any new diagnostic code MUST be added here (see the module docs).
pub const DIAGNOSTIC_CODES: &[DiagnosticCodeInfo] = &[
    info(
        "anchor.cycle",
        Severity::Error,
        "Anchor parent chain forms a cycle.",
    ),
    info(
        "anchor.edge_without_sibling",
        Severity::Warning,
        "`anchor-edge` set without an `anchor-sibling`.",
    ),
    info(
        "anchor.gap_invalid_unit",
        Severity::Warning,
        "`anchor-gap` uses a non-pixel unit.",
    ),
    info(
        "anchor.parent_without_anchor",
        Severity::Warning,
        "`anchor-parent` set without an `anchor`.",
    ),
    info(
        "anchor.sibling_without_anchor",
        Severity::Warning,
        "`anchor-sibling` set without an `anchor`.",
    ),
    info(
        "anchor.unknown_edge",
        Severity::Error,
        "`anchor-edge` value is not a recognized edge.",
    ),
    info(
        "anchor.unknown_value",
        Severity::Error,
        "`anchor` value is not a recognized anchor point.",
    ),
    info(
        "anchor.unresolvable_parent",
        Severity::Error,
        "`anchor-parent` references an unknown node.",
    ),
    info(
        "anchor.unresolved_sibling",
        Severity::Error,
        "`anchor-sibling` references an unknown node.",
    ),
    info(
        "anchor.unresolved_zone",
        Severity::Error,
        "`anchor-zone` references an unknown zone.",
    ),
    info(
        "anchor.zone_without_anchor",
        Severity::Warning,
        "`anchor-zone` set without an `anchor`.",
    ),
    info(
        "asset.invalid_kind",
        Severity::Error,
        "`asset` kind is not a recognized asset kind.",
    ),
    info(
        "asset.invalid_src",
        Severity::Error,
        "`asset` src is empty or malformed.",
    ),
    info(
        "asset.missing",
        Severity::Error,
        "A declared asset file was not found on disk at render time.",
    ),
    info(
        "asset.unknown_property",
        Severity::Warning,
        "Unrecognized property on an `asset` declaration.",
    ),
    info(
        "asset.unknown_reference",
        Severity::Error,
        "Image references an undeclared asset id.",
    ),
    info(
        "baseline-grid.snap_failed",
        Severity::Warning,
        "A text column could not be snapped to the baseline grid.",
    ),
    info(
        "brand.color_off_palette",
        Severity::Warning,
        "A color token's value is not in the document brand palette.",
    ),
    info(
        "brand.font_not_allowed",
        Severity::Warning,
        "A font-family token's value is not in the approved brand font list.",
    ),
    info(
        "brand.weight_not_allowed",
        Severity::Warning,
        "A font-weight token's value is not in the approved brand weight list.",
    ),
    info(
        "component.unknown_override_target",
        Severity::Warning,
        "Instance override targets an unknown component child.",
    ),
    info(
        "component.unknown_reference",
        Severity::Error,
        "Instance references an undeclared component id.",
    ),
    info(
        "connector.invalid_anchor",
        Severity::Warning,
        "Connector anchor value is not recognized.",
    ),
    info(
        "connector.invalid_marker",
        Severity::Warning,
        "Connector marker value is not recognized.",
    ),
    info(
        "connector.invalid_route",
        Severity::Warning,
        "Connector route value is not recognized.",
    ),
    info(
        "connector.missing_target",
        Severity::Warning,
        "Connector is missing a `from` or `to` target.",
    ),
    info(
        "connector.unknown_target",
        Severity::Warning,
        "Connector `from`/`to` references an unknown node.",
    ),
    info(
        "contrast.low",
        Severity::Warning,
        "Text/background contrast is below the WCAG 2.2 threshold.",
    ),
    info(
        "data.missing_field",
        Severity::Advisory,
        "A `(data)` property references a field path not present in the data context.",
    ),
    info(
        "data.no_context",
        Severity::Advisory,
        "A `(data)` property was encountered but no data context was provided at compile time.",
    ),
    info(
        "document.invalid_colorspace",
        Severity::Warning,
        "Document `colorspace` value is unrecognized.",
    ),
    info(
        "document.invalid_page_parity_start",
        Severity::Warning,
        "Document `page-parity-start` value is unrecognized.",
    ),
    info(
        "document.invalid_page_progression",
        Severity::Warning,
        "Document `page-progression` value is unrecognized.",
    ),
    info(
        "document.invalid_spread_gutter",
        Severity::Warning,
        "Document `spread-gutter` value is invalid.",
    ),
    info(
        "document.no_pages",
        Severity::Error,
        "Document body contains no pages.",
    ),
    info(
        "field.unknown_type",
        Severity::Warning,
        "Field `type` is not a recognized field type.",
    ),
    info(
        "field.unresolved_ref",
        Severity::Warning,
        "Field references an unknown target.",
    ),
    info(
        "filter.duotone_missing_color",
        Severity::Error,
        "Duotone filter op is missing a shadow/highlight color.",
    ),
    info(
        "filter.invalid_amount",
        Severity::Error,
        "Filter op `amount` is out of range.",
    ),
    info(
        "filter.invalid_scale",
        Severity::Error,
        "Filter op `scale` is out of range.",
    ),
    info(
        "filter.no_ops",
        Severity::Error,
        "Filter token declares no ops.",
    ),
    info(
        "fold.content_crossing",
        Severity::Advisory,
        "Content crosses a declared fold line.",
    ),
    info(
        "font.glyph_missing",
        Severity::Warning,
        "Text contains character(s) with no glyph in any registered font.",
    ),
    info(
        "font.local",
        Severity::Advisory,
        "A font resolved from a local/system source; rendering is not deterministic across machines.",
    ),
    info(
        "font.unresolved",
        Severity::Advisory,
        "A text/code node's font family is unavailable; falling back to a default.",
    ),
    info(
        "footnote.body_overlap",
        Severity::Advisory,
        "A footnote body overlaps another footnote or the page live area.",
    ),
    info(
        "footnote.no_live_area",
        Severity::Advisory,
        "Footnotes cannot be placed: the page has no live area defined.",
    ),
    info(
        "footnote.unresolved_ref",
        Severity::Warning,
        "Text span footnote-ref names an unknown footnote.",
    ),
    info(
        "frame.child_overflow",
        Severity::Advisory,
        "A frame child extends outside the frame box.",
    ),
    info(
        "gradient.invalid_radius",
        Severity::Error,
        "Radial gradient radius is invalid.",
    ),
    info(
        "gradient.stop_unresolved",
        Severity::Error,
        "Gradient stop color references an unknown token.",
    ),
    info(
        "gradient.stop_wrong_type",
        Severity::Error,
        "Gradient stop color token is not a color.",
    ),
    info(
        "gradient.too_few_stops",
        Severity::Error,
        "Gradient declares fewer than two stops.",
    ),
    info(
        "grid.missing_columns",
        Severity::Advisory,
        "Table grid layout has no column definitions.",
    ),
    info(
        "group.invalid_intensity",
        Severity::Warning,
        "Group `intensity` is out of the 0.0–1.0 range.",
    ),
    info(
        "id.duplicate",
        Severity::Error,
        "An id is used more than once in the document.",
    ),
    info(
        "image.invalid_fit",
        Severity::Warning,
        "Image `fit` value is not recognized.",
    ),
    info(
        "image.invalid_src_rect",
        Severity::Error,
        "Image source-crop rectangle is invalid.",
    ),
    info(
        "image.overflow",
        Severity::Advisory,
        "Image pixel content overflows the node box at the current fit mode.",
    ),
    info(
        "image.partial_src_rect",
        Severity::Error,
        "Image source-crop rectangle is only partially specified.",
    ),
    info(
        "image.upscale",
        Severity::Advisory,
        "Image is being scaled up beyond its native resolution.",
    ),
    info(
        "layout.off_canvas",
        Severity::Advisory,
        "A node extends outside the page bounds.",
    ),
    info(
        "library.unknown_property",
        Severity::Warning,
        "Unrecognized property on a `library` declaration.",
    ),
    info(
        "margin.violation",
        Severity::Advisory,
        "Content intrudes into a declared book live-area margin.",
    ),
    info(
        "mask.invalid_feather",
        Severity::Error,
        "Mask `feather` value is invalid.",
    ),
    info(
        "mask.invalid_radius",
        Severity::Error,
        "Mask `radius` value is invalid.",
    ),
    info(
        "master.unknown_reference",
        Severity::Error,
        "A page references an undeclared master id.",
    ),
    info(
        "node.invalid_geometry",
        Severity::Error,
        "A node geometry attribute uses an unknown unit.",
    ),
    info(
        "node.locked",
        Severity::Error,
        "A transaction attempted to modify a locked node.",
    ),
    info(
        "node.missing_geometry",
        Severity::Error,
        "A node is missing a required geometry attribute.",
    ),
    info(
        "node.unknown_kind",
        Severity::Warning,
        "A node kind is not recognized by this engine version.",
    ),
    info(
        "node.unknown_property",
        Severity::Warning,
        "A node carries a property this engine does not recognize.",
    ),
    info(
        "page.invalid_bleed",
        Severity::Warning,
        "Page `bleed` value is invalid.",
    ),
    info(
        "page.invalid_line_jumps",
        Severity::Warning,
        "Page `line-jumps` value is not recognized.",
    ),
    info(
        "page.invalid_parity",
        Severity::Warning,
        "Page `parity` value is not recognized.",
    ),
    info(
        "chart.category_count_mismatch",
        Severity::Advisory,
        "Chart `categories` label count does not match the series data-point count.",
    ),
    info(
        "chart.invalid_bar_mode",
        Severity::Warning,
        "Chart `bar-mode` is not `grouped` or `stacked`.",
    ),
    info(
        "chart.invalid_kind",
        Severity::Error,
        "Chart `kind` is not one of bar|line|sparkline|pie|donut.",
    ),
    info(
        "chart.invalid_legend_align",
        Severity::Warning,
        "Chart `legend-align` is not one of `center`, `left`, or `right`.",
    ),
    info(
        "chart.invalid_legend_layout",
        Severity::Warning,
        "Chart `legend-layout` is not `wrapped` or `list`.",
    ),
    info(
        "chart.invalid_legend_position",
        Severity::Warning,
        "Chart `legend-position` is not one of `right`, `left`, `top`, or `bottom`.",
    ),
    info(
        "chart.invalid_orientation",
        Severity::Warning,
        "Chart `orientation` is not `vertical` or `horizontal`.",
    ),
    info(
        "chart.invalid_point_placement",
        Severity::Warning,
        "Chart `point-placement` is not `edge` or `center`.",
    ),
    info(
        "chart.invalid_value_labels",
        Severity::Warning,
        "Chart `value-labels` is not one of `auto`, `none`, `top`, or `center`.",
    ),
    info(
        "pattern.grid_missing_spacing",
        Severity::Error,
        "Grid pattern is missing required `spacing`.",
    ),
    info(
        "pattern.invalid_count",
        Severity::Error,
        "Pattern `count` is out of range.",
    ),
    info(
        "pattern.invalid_spacing",
        Severity::Error,
        "Pattern `spacing` is invalid.",
    ),
    info(
        "pattern.jitter_out_of_range",
        Severity::Warning,
        "Pattern `jitter` is out of the 0.0–1.0 range.",
    ),
    info(
        "pattern.scatter_missing_count",
        Severity::Error,
        "Scatter pattern is missing required `count`.",
    ),
    info(
        "pattern.unknown_kind",
        Severity::Error,
        "Pattern `kind` is not `grid` or `scatter`.",
    ),
    info(
        "provenance.unknown_library",
        Severity::Error,
        "A provenance origin names an undeclared library.",
    ),
    info(
        "provenance.unknown_node",
        Severity::Error,
        "A provenance origin names an unknown node.",
    ),
    info(
        "provenance.unknown_property",
        Severity::Warning,
        "Unrecognized property on a provenance `origin`.",
    ),
    info(
        "recipe.duplicate_id",
        Severity::Error,
        "Two recipes declare the same id.",
    ),
    info(
        "recipe.unknown_bounds",
        Severity::Error,
        "Recipe `bounds` references an unknown node.",
    ),
    info(
        "recipe.unknown_expanded_node",
        Severity::Error,
        "Recipe `expanded` names an unknown node.",
    ),
    info(
        "recipe.unknown_palette_token",
        Severity::Error,
        "Recipe palette references an unknown or non-color token.",
    ),
    info(
        "safe_zone.violation",
        Severity::Advisory,
        "Content violates a declared safe/dead zone.",
    ),
    info(
        "scene.invalid_color",
        Severity::Advisory,
        "A paint value could not be resolved to a valid color at compile time.",
    ),
    info(
        "scene.missing_geometry",
        Severity::Advisory,
        "A node's geometry could not be resolved; the node is skipped in the scene.",
    ),
    info(
        "scene.no_pages",
        Severity::Advisory,
        "The document has no pages; an empty scene is produced.",
    ),
    info(
        "scene.page_out_of_range",
        Severity::Advisory,
        "The requested page index is outside the document's page range.",
    ),
    info(
        "scene.text_unshaped",
        Severity::Advisory,
        "A text or code node could not be shaped and is omitted from the scene.",
    ),
    info(
        "scene.unknown_component",
        Severity::Advisory,
        "An instance references a component that could not be found at compile time.",
    ),
    info(
        "scene.unresolved_token",
        Severity::Advisory,
        "A paint references a token that could not be resolved at compile time.",
    ),
    info(
        "scene.unsupported_node",
        Severity::Advisory,
        "A node kind is not supported by the scene compiler and is skipped.",
    ),
    info(
        "scene.unsupported_unit",
        Severity::Advisory,
        "A dimension uses a unit the scene compiler cannot resolve to pixels.",
    ),
    info(
        "scene.wrong_token_type",
        Severity::Advisory,
        "A paint references a token that is not a color type.",
    ),
    info(
        "section.duplicate_start_page",
        Severity::Error,
        "Two sections start on the same page.",
    ),
    info(
        "section.invalid_folio_style",
        Severity::Warning,
        "Section `folio-style` value is not recognized.",
    ),
    info(
        "section.unknown_start_page",
        Severity::Error,
        "Section `start-page` references an unknown page.",
    ),
    info(
        "shadow.layer_unresolved",
        Severity::Error,
        "Shadow layer color references an unknown token.",
    ),
    info(
        "shadow.layer_wrong_type",
        Severity::Error,
        "Shadow layer color token is not a color.",
    ),
    info(
        "shadow.no_layers",
        Severity::Error,
        "Shadow token declares no layers.",
    ),
    info(
        "shape.insufficient_points",
        Severity::Error,
        "A polygon/polyline has too few points.",
    ),
    info(
        "shape.invalid_h_align",
        Severity::Warning,
        "Shape `h-align` value is not recognized.",
    ),
    info(
        "shape.invalid_stroke_alignment",
        Severity::Warning,
        "Shape `stroke-alignment` value is not recognized.",
    ),
    info(
        "shape.invalid_v_align",
        Severity::Warning,
        "Shape `v-align` value is not recognized.",
    ),
    info(
        "shape.unknown_kind",
        Severity::Warning,
        "Shape `kind` is not a recognized preset shape.",
    ),
    info(
        "style.unknown_property",
        Severity::Warning,
        "A style block carries an unrecognized property.",
    ),
    info(
        "style.unknown_reference",
        Severity::Error,
        "A node references an undeclared style id.",
    ),
    info(
        "table.cell_overflow",
        Severity::Error,
        "A table cell's content overflows its cell box.",
    ),
    info(
        "table.flow_overflow",
        Severity::Advisory,
        "Table text content overflows the table body area.",
    ),
    info(
        "table.invalid_border_collapse",
        Severity::Warning,
        "Table `border-collapse` value is not recognized.",
    ),
    info(
        "table.invalid_h_align",
        Severity::Warning,
        "Table `h-align` value is not recognized.",
    ),
    info(
        "table.invalid_v_align",
        Severity::Warning,
        "Table `v-align` value is not recognized.",
    ),
    info(
        "text-exclusion.unresolved_ref",
        Severity::Warning,
        "Text `text-exclusion` references an unknown node.",
    ),
    info(
        "text.fit_failed",
        Severity::Error,
        "Text with `overflow: fit` could not be scaled to fit its frame.",
    ),
    info(
        "text.invalid_format",
        Severity::Warning,
        "Text `format` value is not recognized; treated as plain.",
    ),
    info(
        "text.invalid_v_align",
        Severity::Warning,
        "Text `v-align` value is not recognized.",
    ),
    info(
        "text.forced_break",
        Severity::Warning,
        "A text line was force-broken during wrapping to prevent infinite layout.",
    ),
    info(
        "text.overflow",
        Severity::Warning,
        "Text content overflows its containing frame; preserve type scale first and shrink only when intended or constrained.",
    ),
    info(
        "text.src_missing",
        Severity::Error,
        "A text node's `src` file was not found or could not be read at render time.",
    ),
    info(
        "toc.no_selector",
        Severity::Warning,
        "A `toc` node declares no selector.",
    ),
    info(
        "token.cyclic_reference",
        Severity::Error,
        "A token reference chain forms a cycle.",
    ),
    info(
        "token.duplicate_id",
        Severity::Error,
        "Two tokens declare the same id.",
    ),
    info(
        "token.incompatible_property",
        Severity::Error,
        "A token is referenced by an incompatible property.",
    ),
    info(
        "token.invalid_value",
        Severity::Error,
        "A token has an invalid value for its type.",
    ),
    info(
        "token.raw_visual_literal",
        Severity::Error,
        "A visual property uses a raw literal instead of a token.",
    ),
    info(
        "token.set_partially_used",
        Severity::Advisory,
        "A multi-token provenance `set` has some but not all of its tokens referenced.",
    ),
    info(
        "token.type_mismatch",
        Severity::Error,
        "A token value does not match its declared type.",
    ),
    info(
        "token.unknown_reference",
        Severity::Error,
        "A property references an undeclared token id.",
    ),
    info(
        "token.unknown_type",
        Severity::Warning,
        "A token declares an unrecognized type.",
    ),
    info(
        "token.unused",
        Severity::Advisory,
        "A token is declared but never referenced.",
    ),
    info(
        "tx.duplicate_id",
        Severity::Error,
        "A transaction tried to add a node or asset with an id already in use.",
    ),
    info(
        "tx.geometry_unresolved",
        Severity::Warning,
        "An align/distribute target has no resolvable geometry and was skipped.",
    ),
    info(
        "tx.invalid_geometry",
        Severity::Error,
        "A transaction geometry operation produced invalid geometry.",
    ),
    info(
        "tx.invalid_geometry_tolerance",
        Severity::Error,
        "A transaction geometry tolerance is invalid.",
    ),
    info(
        "tx.invalid_node_spec",
        Severity::Error,
        "A transaction `add` op specifies a node that cannot be parsed.",
    ),
    info(
        "tx.invalid_parent",
        Severity::Error,
        "A transaction op targets an invalid or incompatible parent node.",
    ),
    info(
        "tx.invalid_path_anchor",
        Severity::Error,
        "A transaction path-anchor operation found invalid anchor coordinates.",
    ),
    info(
        "tx.invalid_value",
        Severity::Error,
        "A transaction op carries a value that fails validation.",
    ),
    info(
        "tx.locked_skipped",
        Severity::Warning,
        "A transaction op was skipped because the target node is locked.",
    ),
    info(
        "tx.noop",
        Severity::Advisory,
        "A transaction op produced no change (the document is already in the requested state).",
    ),
    info(
        "tx.not_a_pattern",
        Severity::Error,
        "A pattern-expand op targets a node that is not a pattern.",
    ),
    info(
        "tx.out_of_range",
        Severity::Error,
        "A transaction op value is outside the allowed range for the target property.",
    ),
    info(
        "tx.pattern_not_expandable",
        Severity::Error,
        "A pattern cannot be expanded because its bounds or count are not resolved.",
    ),
    info(
        "tx.pattern_unresolved_bounds",
        Severity::Error,
        "A pattern-expand op could not resolve the pattern bounds node.",
    ),
    info(
        "tx.unknown_node",
        Severity::Error,
        "A transaction op references a node id that does not exist in the document.",
    ),
    info(
        "tx.unknown_recipe",
        Severity::Error,
        "A transaction op references a recipe id that is not declared.",
    ),
    info(
        "tx.unknown_style",
        Severity::Error,
        "A transaction op references a style id that is not declared.",
    ),
    info(
        "tx.unknown_token",
        Severity::Error,
        "A transaction op references a token id that is not declared.",
    ),
    info(
        "tx.unsupported_closed_path",
        Severity::Error,
        "A transaction path operation does not support closed paths.",
    ),
    info(
        "tx.unsupported_path_handles",
        Severity::Error,
        "A transaction path operation does not support Bézier handles.",
    ),
    info(
        "tx.unsupported_property",
        Severity::Error,
        "A transaction op targets a property that cannot be set on the node kind.",
    ),
    info(
        "tx.wrong_node_type",
        Severity::Error,
        "A transaction op targets a node of the wrong kind for that operation.",
    ),
    info(
        "value.out_of_range",
        Severity::Error,
        "A numeric value is outside its allowed range.",
    ),
    info(
        "variant.duplicate_id",
        Severity::Error,
        "Two variants declare the same id.",
    ),
    info(
        "variant.invalid_dimension",
        Severity::Error,
        "A variant `w`/`h` value is invalid.",
    ),
    info(
        "variant.override_unknown_node",
        Severity::Error,
        "A variant override targets an unknown node.",
    ),
    info(
        "variant.override_unknown_property",
        Severity::Warning,
        "A variant override carries an unrecognized property.",
    ),
    info(
        "variant.unknown_source",
        Severity::Error,
        "A variant `source` references an unknown page.",
    ),
    // ── Diagnostic-policy self-validation (emitted by the policy checker) ──────
    info(
        "policy.unknown_code",
        Severity::Warning,
        "A `diagnostics { … }` entry names a code the engine does not emit.",
    ),
    info(
        "policy.ineffective_on_error",
        Severity::Warning,
        "`allow`/`warn` cannot weaken an always-Error diagnostic code.",
    ),
];

/// `const`-friendly constructor for a [`DiagnosticCodeInfo`] table entry.
const fn info(code: &'static str, severity: Severity, summary: &'static str) -> DiagnosticCodeInfo {
    DiagnosticCodeInfo {
        code,
        severity,
        summary,
    }
}

/// Look up a code's catalog entry, or `None` if the code is not in the catalog.
pub fn lookup(code: &str) -> Option<&'static DiagnosticCodeInfo> {
    DIAGNOSTIC_CODES.iter().find(|e| e.code == code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn catalog_is_nonempty() {
        assert!(
            !DIAGNOSTIC_CODES.is_empty(),
            "the diagnostic-code catalog must not be empty"
        );
    }

    #[test]
    fn catalog_codes_are_unique() {
        let mut seen: BTreeSet<&'static str> = BTreeSet::new();
        for entry in DIAGNOSTIC_CODES {
            assert!(
                !entry.code.is_empty(),
                "a catalog entry has an empty code string"
            );
            assert!(
                seen.insert(entry.code),
                "duplicate diagnostic code in the catalog: {}",
                entry.code,
            );
        }
    }

    #[test]
    fn governable_codes_are_warning_or_advisory() {
        for entry in DIAGNOSTIC_CODES {
            assert_eq!(
                entry.is_governable(),
                entry.severity != Severity::Error,
                "is_governable() must mirror (severity != Error) for {}",
                entry.code,
            );
        }
    }

    #[test]
    fn lookup_finds_known_and_misses_unknown() {
        assert!(lookup("layout.off_canvas").is_some());
        assert!(lookup("policy.unknown_code").is_some());
        assert!(lookup("not.a_real_code").is_none());
    }

    #[test]
    fn compile_font_diagnostics_are_catalogued_and_governable() {
        // The compile-stage font diagnostics (emitted by zenith-scene) must be in
        // the catalog so the diagnostic policy can govern them on the render path.
        let unresolved = lookup("font.unresolved").expect("font.unresolved must be catalogued");
        assert_eq!(unresolved.severity, Severity::Advisory);
        assert!(
            unresolved.is_governable(),
            "font.unresolved must be governable"
        );

        let glyph_missing =
            lookup("font.glyph_missing").expect("font.glyph_missing must be catalogued");
        assert_eq!(glyph_missing.severity, Severity::Warning);
        assert!(
            glyph_missing.is_governable(),
            "font.glyph_missing must be governable"
        );

        // `font.local` must be a governable Advisory so `deny "font.local"` can
        // gate CI renders that resolved a machine-local font.
        let local = lookup("font.local").expect("font.local must be catalogued");
        assert_eq!(local.severity, Severity::Advisory);
        assert!(local.is_governable(), "font.local must be governable");
    }

    #[test]
    fn scene_diagnostics_are_catalogued() {
        // Codes emitted by zenith-scene during compilation must be in the catalog
        // so that `diagnostics { … }` blocks can govern them.
        let cases: &[(&str, Severity)] = &[
            ("baseline-grid.snap_failed", Severity::Warning),
            ("footnote.body_overlap", Severity::Advisory),
            ("footnote.no_live_area", Severity::Advisory),
            ("scene.invalid_color", Severity::Advisory),
            ("scene.missing_geometry", Severity::Advisory),
            ("scene.no_pages", Severity::Advisory),
            ("scene.page_out_of_range", Severity::Advisory),
            ("scene.text_unshaped", Severity::Advisory),
            ("scene.unknown_component", Severity::Advisory),
            ("scene.unresolved_token", Severity::Advisory),
            ("scene.unsupported_node", Severity::Advisory),
            ("scene.unsupported_unit", Severity::Advisory),
            ("scene.wrong_token_type", Severity::Advisory),
            ("table.flow_overflow", Severity::Advisory),
            ("text.fit_failed", Severity::Error),
            ("text.forced_break", Severity::Warning),
            ("text.overflow", Severity::Warning),
        ];
        for (code, expected_severity) in cases {
            let entry =
                lookup(code).unwrap_or_else(|| panic!("{code} must be in the diagnostic catalog"));
            assert_eq!(
                entry.severity, *expected_severity,
                "{code} catalog severity mismatch"
            );
        }
    }

    #[test]
    fn render_diagnostics_are_catalogued() {
        // Codes emitted by zenith-cli at render time must be in the catalog.
        let asset_missing =
            lookup("asset.missing").expect("asset.missing must be in the diagnostic catalog");
        assert_eq!(asset_missing.severity, Severity::Error);

        let text_src_missing =
            lookup("text.src_missing").expect("text.src_missing must be in the diagnostic catalog");
        assert_eq!(text_src_missing.severity, Severity::Error);
        assert!(
            !text_src_missing.is_governable(),
            "text.src_missing must not be governable (always-Error)"
        );

        let image_overflow =
            lookup("image.overflow").expect("image.overflow must be in the diagnostic catalog");
        assert_eq!(image_overflow.severity, Severity::Advisory);
        assert!(
            image_overflow.is_governable(),
            "image.overflow must be governable"
        );

        let image_upscale =
            lookup("image.upscale").expect("image.upscale must be in the diagnostic catalog");
        assert_eq!(image_upscale.severity, Severity::Advisory);
        assert!(
            image_upscale.is_governable(),
            "image.upscale must be governable"
        );
    }

    #[test]
    fn transaction_diagnostics_are_catalogued() {
        // Codes emitted by zenith-tx must be in the catalog so that policy
        // validation can recognise them as known codes.
        let known_tx_codes: &[(&str, Severity)] = &[
            ("node.locked", Severity::Error),
            ("tx.duplicate_id", Severity::Error),
            ("tx.geometry_unresolved", Severity::Warning),
            ("tx.invalid_geometry", Severity::Error),
            ("tx.invalid_geometry_tolerance", Severity::Error),
            ("tx.invalid_node_spec", Severity::Error),
            ("tx.invalid_parent", Severity::Error),
            ("tx.invalid_path_anchor", Severity::Error),
            ("tx.invalid_value", Severity::Error),
            ("tx.locked_skipped", Severity::Warning),
            ("tx.noop", Severity::Advisory),
            ("tx.not_a_pattern", Severity::Error),
            ("tx.out_of_range", Severity::Error),
            ("tx.pattern_not_expandable", Severity::Error),
            ("tx.pattern_unresolved_bounds", Severity::Error),
            ("tx.unknown_node", Severity::Error),
            ("tx.unknown_recipe", Severity::Error),
            ("tx.unknown_style", Severity::Error),
            ("tx.unknown_token", Severity::Error),
            ("tx.unsupported_closed_path", Severity::Error),
            ("tx.unsupported_path_handles", Severity::Error),
            ("tx.unsupported_property", Severity::Error),
            ("tx.wrong_node_type", Severity::Error),
        ];
        for (code, expected_severity) in known_tx_codes {
            let entry =
                lookup(code).unwrap_or_else(|| panic!("{code} must be in the diagnostic catalog"));
            assert_eq!(
                entry.severity, *expected_severity,
                "{code} catalog severity mismatch"
            );
        }
    }
}
