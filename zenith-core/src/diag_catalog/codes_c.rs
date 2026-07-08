//! Diagnostic catalog entries, group c. Part of the single catalog assembled
//! in `super::catalog`; ordering across groups is preserved on assembly.

use super::entry::{DiagnosticCodeInfo, info};
use crate::diagnostics::Severity;

/// Group c of the diagnostic-code catalog (see `super::catalog::DIAGNOSTIC_CODES`).
pub(super) const CODES: &[DiagnosticCodeInfo] = &[
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
        "scene.invalid_import_source",
        Severity::Advisory,
        "An imported composition source is malformed and cannot be parsed.",
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
        "scene.unknown_import",
        Severity::Advisory,
        "An instance references an import that is not present in the scene import graph.",
    ),
    info(
        "scene.unknown_import_component",
        Severity::Advisory,
        "An instance references a component that is not present in the imported document.",
    ),
    info(
        "scene.unknown_import_page",
        Severity::Advisory,
        "A page source references a page that is not present in the imported document.",
    ),
    info(
        "scene.unresolved_token",
        Severity::Advisory,
        "A paint references a token that could not be resolved at compile time.",
    ),
    info(
        "scene.unsupported_import_source",
        Severity::Advisory,
        "An imported composition source is not yet supported by scene compilation.",
    ),
    info(
        "scene.unsupported_import_target",
        Severity::Advisory,
        "An imported composition source targets a kind that scene compilation does not support.",
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
