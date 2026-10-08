//! Diagnostic catalog entries, group c. Part of the single catalog assembled
//! in `super::catalog`; ordering across groups is preserved on assembly.

use super::entry::{DiagnosticCodeInfo, info};
use crate::diagnostics::Severity;

/// Group c of the diagnostic-code catalog (see `super::catalog::DIAGNOSTIC_CODES`).
pub(super) const CODES: &[DiagnosticCodeInfo] = &[
    info(
        "parse.error",
        Severity::Error,
        "Document source cannot be parsed.",
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
        Severity::Error,
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
        "render.no_pages",
        Severity::Error,
        "Document has no pages to render.",
    ),
    info(
        "render.page_out_of_range",
        Severity::Error,
        "Requested page index is outside the document.",
    ),
    info(
        "render.raster_failed",
        Severity::Error,
        "Rasterizing a page failed.",
    ),
    info(
        "render.scene_serialize_failed",
        Severity::Error,
        "Scene serialization failed.",
    ),
    info(
        "render.spread_failed",
        Severity::Error,
        "Rendering a page spread failed.",
    ),
    info("render.pdf_failed", Severity::Error, "PDF export failed."),
    info(
        "render.pdf_rasterized",
        Severity::Advisory,
        "PDF export embeds a rasterized command range. Text and links lose selection and click targets.",
    ),
    info("render.svg_failed", Severity::Error, "SVG export failed."),
    info(
        "render.svg_rasterized",
        Severity::Advisory,
        "SVG export embeds a rasterized command range. Links in that range lose click targets.",
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
        "scene.invalid_symmetry",
        Severity::Warning,
        "Group symmetry parameter is out of range or non-finite at compile time.",
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
        "scene.text_outline_failed",
        Severity::Error,
        "Text outline materialization failed for a node.",
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
        "scene.unsupported_fit",
        Severity::Advisory,
        "An instance uses a `fit` value the scene compiler does not support; the instance is skipped.",
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
        Severity::Error,
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
        Severity::Error,
        "Shape `h-align` value is not recognized.",
    ),
    info(
        "shape.invalid_stroke_alignment",
        Severity::Error,
        "Shape `stroke-alignment` value is not recognized.",
    ),
    info(
        "shape.invalid_v_align",
        Severity::Error,
        "Shape `v-align` value is not recognized.",
    ),
    info(
        "shape.unknown_kind",
        Severity::Error,
        "Shape `kind` is not a recognized preset shape.",
    ),
    info(
        "spacing.uneven_gap",
        Severity::Advisory,
        "Three or more siblings in a row or column have nearly but not exactly equal gaps.",
    ),
    info(
        "style.align_unsupported",
        Severity::Warning,
        "A shape or table takes a style `align` value its `h-align` does not accept.",
    ),
    info(
        "style.invalid_value",
        Severity::Error,
        "An enum-valued style key (`align`, `v-align`) holds a value outside its list.",
    ),
    info(
        "style.unknown_property",
        Severity::Error,
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
        Severity::Error,
        "Table `border-collapse` value is not recognized.",
    ),
    info(
        "table.invalid_h_align",
        Severity::Error,
        "Table `h-align` value is not recognized.",
    ),
    info(
        "table.invalid_v_align",
        Severity::Error,
        "Table `v-align` value is not recognized.",
    ),
    info(
        "text-exclusion.unresolved_ref",
        Severity::Warning,
        "Text `text-exclusion` references an unknown node.",
    ),
    info(
        "text.edge_crowding",
        Severity::Advisory,
        "A text's glyph ink sits closer to the trim edge than max(8px, 1.5% of the shorter page side) while still inside the trim; skipped when the page declares margins or a safe zone. The message names the gap, the floor, and the `x` / `y` that reaches it, with a `set_property` fix when the text has an authored absolute `x` / `y`.",
    ),
    info(
        "text.fit_failed",
        Severity::Error,
        "Text with `overflow=\"fit\"` (or `\"autofit\"` at its floor) overflows its box; the message names the box size and font size that fit.",
    ),
    info(
        "text.invalid_format",
        Severity::Error,
        "Text `format` value is not recognized.",
    ),
    info(
        "text.invalid_v_align",
        Severity::Error,
        "Text `v-align` value is not recognized.",
    ),
    info(
        "text.forced_break",
        Severity::Warning,
        "A text line was force-broken during wrapping to prevent infinite layout.",
    ),
    info(
        "text.ink_overlap",
        Severity::Warning,
        "The glyph ink of two texts (text nodes or shape / connector labels) intersects over more than 4 px²; the message names the overlap and the `y` that clears it, with a `set_property` fix when the text has an authored absolute `y`.",
    ),
    info(
        "text.occluded",
        Severity::Warning,
        "A node painted later with an opaque fill or image at full opacity covers more than half of at least one glyph of a text; the message names the covering node and the hidden glyph count.",
    ),
    info(
        "text.overflow",
        Severity::Warning,
        "Text content overflows its box and is clipped at the box edge (`overflow=\"clip\"`, the default); the message names the box size that keeps the type scale and the font size that fits.",
    ),
    info(
        "text.src_missing",
        Severity::Error,
        "A text node's `src` file was not found or could not be read at render time.",
    ),
    info(
        "text.too_small",
        Severity::Advisory,
        "A text's effective font size is below max(9px, 0.9% of the shorter page side); the message names the floor and the `font-size` that reaches it, with a `replace_value` fix when `font-size` is a literal on the node.",
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
        "token.mask_extra_shape",
        Severity::Error,
        "A `mask` token has more than one shape child; only the first is read.",
    ),
    info(
        "token.set_partially_used",
        Severity::Advisory,
        "A multi-token provenance `set` has some but not all of its tokens referenced. Theme pack sets (`@zenith/theme.*`) are exempt.",
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
        Severity::Error,
        "A token declares an unrecognized type.",
    ),
    info(
        "token.unused",
        Severity::Advisory,
        "A token is declared but never referenced.",
    ),
    info(
        "tx.coordinate_unresolved",
        Severity::Advisory,
        "A transaction moved a node between containers, but a container origin does not resolve \
         to px, so the node's x/y stay unchanged; check its position.",
    ),
    info(
        "tx.duplicate_id",
        Severity::Error,
        "A transaction tried to add a node or asset with an id already in use.",
    ),
    info(
        "tx.flow_placed",
        Severity::Advisory,
        "A reparent, group, or ungroup puts a node into a flow slot of a row, column, or grid \
         frame; the frame places it, so its x/y and size are ignored and siblings reflow. Set \
         position=\"absolute\" or use set_geometry to keep its page box.",
    ),
    info(
        "tx.geometry_required",
        Severity::Error,
        "A set_geometry null removes x, y, w, or h from a node that needs it: no row, column, or \
         grid frame places the node in flow, and no anchor places it. Keep the attribute or set \
         a value.",
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
        "tx.layout_managed",
        Severity::Error,
        "A transaction op sets the x/y of a node that a row, column, or grid frame places in flow; \
         set position=\"absolute\" with set_layout or reorder the node instead.",
    ),
    info(
        "tx.locked_skipped",
        Severity::Warning,
        "A transaction op was skipped because the target node is locked.",
    ),
    info(
        "tx.no_text_outlines",
        Severity::Error,
        "Text outline materialization produced no path geometry.",
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
        "tx.page_box_changed",
        Severity::Warning,
        "A reparent, group, or ungroup changes the compiled page box of a node it moves; the \
         op is meant to keep page position. Check the reported box and the reflowed siblings.",
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
        "tx.unknown_master",
        Severity::Error,
        "Master referenced by a transaction op does not exist.",
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
        "type.near_duplicate_size",
        Severity::Advisory,
        "Two distinct authored font sizes inside one top-level group or frame differ by 1px or less, or by under 8%; the message names both sizes and their sources. Chart, table, and code internals are skipped.",
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
        Severity::Error,
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
    // ── CLI error envelope (emitted by zenith-cli outside a command's own output) ──
    info(
        "asset.exists",
        Severity::Error,
        "The destination asset file exists with different bytes.",
    ),
    info(
        "asset.import",
        Severity::Error,
        "`zenith asset import` failed.",
    ),
    info(
        "asset.import_failed",
        Severity::Error,
        "`zenith asset import` failed; the error envelope names the cause.",
    ),
    info(
        "asset.zpx_bake",
        Severity::Error,
        "`zenith asset` ZPX bake failed.",
    ),
    info(
        "asset.zpx_bake_failed",
        Severity::Error,
        "`zenith asset` ZPX bake failed; the error envelope names the cause.",
    ),
    info(
        "cli.invalid_argument",
        Severity::Error,
        "A command-line argument or flag combination is invalid.",
    ),
    info(
        "data.load_failed",
        Severity::Error,
        "The `--data` file is missing, unreadable, or not valid JSON or CSV.",
    ),
    info(
        "fix.failed",
        Severity::Error,
        "`zenith fix` failed; the document does not parse.",
    ),
    info("fmt.failed", Severity::Error, "`zenith fmt` failed."),
    info(
        "fmt.format_failed",
        Severity::Error,
        "The document failed to parse, so `zenith fmt` cannot format it.",
    ),
    info(
        "font.missing",
        Severity::Error,
        "A font file named on the command line cannot be read.",
    ),
    info(
        "font.parse",
        Severity::Error,
        "A font file cannot be parsed.",
    ),
    info(
        "fonts.family_not_found",
        Severity::Error,
        "`zenith fonts` finds no face for the requested family.",
    ),
    info("fonts.failed", Severity::Error, "`zenith fonts` failed."),
    info(
        "history.failed",
        Severity::Error,
        "A history command failed.",
    ),
    info(
        "history.redo_failed",
        Severity::Error,
        "`zenith history redo` failed.",
    ),
    info(
        "history.restore_failed",
        Severity::Error,
        "`zenith history restore` failed.",
    ),
    info(
        "history.sync_failed",
        Severity::Error,
        "`zenith history sync` failed.",
    ),
    info(
        "history.undo_failed",
        Severity::Error,
        "`zenith history undo` failed.",
    ),
    info(
        "history.version_failed",
        Severity::Error,
        "`zenith history version` failed.",
    ),
    info(
        "imports.failed",
        Severity::Error,
        "`zenith imports` failed.",
    ),
    info(
        "inspect.failed",
        Severity::Error,
        "`zenith inspect` failed.",
    ),
    info(
        "inspect.node_not_found",
        Severity::Error,
        "`zenith inspect` names a node id the document does not contain.",
    ),
    info(
        "io.not_utf8",
        Severity::Error,
        "A file or formatted output is not valid UTF-8.",
    ),
    info("io.read_failed", Severity::Error, "A file cannot be read."),
    info(
        "library.add_failed",
        Severity::Error,
        "`zenith library add` failed.",
    ),
    info(
        "library.asset_write_failed",
        Severity::Error,
        "`zenith library` cannot write an asset file.",
    ),
    info(
        "library.show_failed",
        Severity::Error,
        "`zenith library show` failed.",
    ),
    info(
        "merge.no_data_nodes",
        Severity::Error,
        "The merge template has no node with a `data.*` role.",
    ),
    info(
        "merge.row_failed",
        Severity::Error,
        "One merge row failed to render.",
    ),
    info(
        "merge.setup_failed",
        Severity::Error,
        "`zenith merge` failed before it rendered any row.",
    ),
    info("new.failed", Severity::Error, "`zenith new` failed."),
    info(
        "perceive.failed",
        Severity::Error,
        "`zenith perceive` failed.",
    ),
    info(
        "perceive.node_not_found",
        Severity::Error,
        "`zenith perceive` names a path node the document does not contain.",
    ),
    info(
        "text_outline.failed",
        Severity::Error,
        "`zenith text-outline` failed.",
    ),
    info(
        "theme.apply_failed",
        Severity::Error,
        "`zenith theme apply` failed.",
    ),
    info(
        "theme.new_failed",
        Severity::Error,
        "`zenith theme new` failed.",
    ),
    info("tokens.failed", Severity::Error, "`zenith tokens` failed."),
    info(
        "tx.engine",
        Severity::Error,
        "The transaction engine rejected the transaction.",
    ),
    info("tx.failed", Severity::Error, "`zenith tx` failed."),
    info(
        "tx.parse",
        Severity::Error,
        "The transaction file cannot be parsed.",
    ),
    info("update.failed", Severity::Error, "`zenith update` failed."),
    info(
        "variant.failed",
        Severity::Error,
        "One variant failed to render.",
    ),
    info(
        "variant.setup_failed",
        Severity::Error,
        "`zenith variant` failed before it rendered any variant.",
    ),
    info(
        "workspace.failed",
        Severity::Error,
        "A `zenith workspace` command failed.",
    ),
];
