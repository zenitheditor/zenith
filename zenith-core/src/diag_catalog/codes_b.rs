//! Diagnostic catalog entries, group b. Part of the single catalog assembled
//! in `super::catalog`; ordering across groups is preserved on assembly.

use super::entry::{DiagnosticCodeInfo, info};
use crate::diagnostics::Severity;

/// Group b of the diagnostic-code catalog (see `super::catalog::DIAGNOSTIC_CODES`).
pub(super) const CODES: &[DiagnosticCodeInfo] = &[
    info(
        "construction.degenerate_guide",
        Severity::Warning,
        "Construction segment guide has identical endpoints.",
    ),
    info(
        "construction.invalid_geometry",
        Severity::Warning,
        "Construction guide property is not a finite px/pt dimension.",
    ),
    info(
        "construction.invalid_radius",
        Severity::Warning,
        "Construction circle guide radius is not positive.",
    ),
    info(
        "construction.missing_geometry",
        Severity::Warning,
        "Construction guide lacks a required geometry property.",
    ),
    info(
        "construction.unknown_guide_type",
        Severity::Error,
        "Construction guide `type` is not a recognized guide type.",
    ),
    info(
        "contrast.invisible",
        Severity::Warning,
        "Text/background APCA Lc contrast is near zero, so the text is effectively invisible.",
    ),
    info(
        "contrast.indeterminate_backdrop",
        Severity::Advisory,
        "Text sits on an image or other backdrop that cannot be sampled during validation.",
    ),
    info(
        "contrast.low",
        Severity::Advisory,
        "Text/background APCA Lc contrast is below the WCAG 3 draft threshold.",
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
        Severity::Error,
        "Document `colorspace` value is unrecognized.",
    ),
    info(
        "document.invalid_page_parity_start",
        Severity::Error,
        "Document `page-parity-start` value is unrecognized.",
    ),
    info(
        "document.invalid_page_progression",
        Severity::Error,
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
        Severity::Error,
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
        "font.invalid_feature",
        Severity::Warning,
        "OpenType feature or alternate spec is malformed.",
    ),
    info(
        "font.parse_failed",
        Severity::Error,
        "Font file bytes cannot be parsed.",
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
        "group.invalid_symmetry",
        Severity::Error,
        "Group `symmetry-mode` is not a recognized mode.",
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
        Severity::Error,
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
        "image.svg_style_on_non_svg",
        Severity::Warning,
        "SVG-only style properties (svg-stroke/svg-fill/svg-stroke-width) are \
         set on an image whose asset is not of kind `svg`; they are ignored.",
    ),
    info(
        "image.upscale",
        Severity::Advisory,
        "Image is being scaled up beyond its native resolution.",
    ),
    info(
        "io.write_failed",
        Severity::Error,
        "Output file cannot be written.",
    ),
    info(
        "kerning.duplicate_pair",
        Severity::Warning,
        "A text-bearing node declares the same `kern-pair` more than once.",
    ),
    info(
        "kerning.empty_pair",
        Severity::Error,
        "`kern-pair` left and right strings must be non-empty.",
    ),
    info(
        "layout.off_canvas",
        Severity::Advisory,
        "A node extends outside the page bounds.",
    ),
    info(
        "library.unknown_property",
        Severity::Error,
        "Unrecognized property on a `library` declaration.",
    ),
    info(
        "margin.violation",
        Severity::Advisory,
        "Content intrudes into a declared book live-area margin.",
    ),
    info(
        "light.angle_ignored",
        Severity::Warning,
        "Light `angle` uses a unit other than deg and is ignored.",
    ),
    info(
        "light.unknown_kind",
        Severity::Error,
        "Light `kind` is not a recognized light kind.",
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
        "mesh.invalid_columns",
        Severity::Error,
        "Mesh `columns` is not greater than zero.",
    ),
    info(
        "mesh.invalid_rows",
        Severity::Error,
        "Mesh `rows` is not greater than zero.",
    ),
    info(
        "mesh.unknown_kind",
        Severity::Error,
        "Mesh camera `kind` is not a recognized kind.",
    ),
    info(
        "node.invalid_geometry",
        Severity::Error,
        "A node geometry attribute uses an unknown unit.",
    ),
    info(
        "node.invalid_value",
        Severity::Error,
        "A node property has a value outside its allowed set.",
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
        "node.opacity_out_of_range",
        Severity::Warning,
        "Light or mesh `opacity` is outside 0.0–1.0.",
    ),
    info(
        "node.unknown_kind",
        Severity::Warning,
        "A node kind is not recognized.",
    ),
    info(
        "node.unknown_property",
        Severity::Error,
        "A node carries a property name this engine does not recognize.",
    ),
    info(
        "node.unsupported_child",
        Severity::Error,
        "A child node was authored under a kind that does not accept it and was discarded.",
    ),
    info(
        "page.invalid_bleed",
        Severity::Warning,
        "Page `bleed` value is invalid.",
    ),
    info(
        "page.invalid_line_jumps",
        Severity::Error,
        "Page `line-jumps` value is not recognized.",
    ),
    info(
        "page.invalid_parity",
        Severity::Error,
        "Page `parity` value is not recognized.",
    ),
    info(
        "chart.category_count_mismatch",
        Severity::Advisory,
        "Chart `categories` label count does not match the series data-point count.",
    ),
    info(
        "chart.invalid_bar_mode",
        Severity::Error,
        "Chart `bar-mode` is not `grouped` or `stacked`.",
    ),
    info(
        "chart.invalid_kind",
        Severity::Error,
        "Chart `kind` is not one of bar|line|sparkline|pie|donut.",
    ),
    info(
        "chart.invalid_legend_align",
        Severity::Error,
        "Chart `legend-align` is not one of `center`, `left`, or `right`.",
    ),
    info(
        "chart.invalid_legend_layout",
        Severity::Error,
        "Chart `legend-layout` is not `wrapped` or `list`.",
    ),
    info(
        "chart.invalid_legend_position",
        Severity::Error,
        "Chart `legend-position` is not one of `right`, `left`, `top`, or `bottom`.",
    ),
    info(
        "chart.invalid_orientation",
        Severity::Error,
        "Chart `orientation` is not `vertical` or `horizontal`.",
    ),
    info(
        "chart.invalid_point_placement",
        Severity::Error,
        "Chart `point-placement` is not `edge` or `center`.",
    ),
    info(
        "chart.invalid_value_labels",
        Severity::Error,
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
];
