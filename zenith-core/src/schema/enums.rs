//! Single source of truth for closed enum value lists.
//!
//! Validation checks, `node.invalid_value` messages, and schema type hints
//! all read these constants so the lists cannot drift apart.

/// `stroke-linecap` values.
pub(crate) const STROKE_LINECAPS: &[&str] = &["butt", "round", "square"];
/// `stroke-linejoin` values.
pub(crate) const STROKE_LINEJOINS: &[&str] = &["miter", "round", "bevel"];
/// `stroke-align` values.
pub(crate) const STROKE_ALIGNS: &[&str] = &["inside", "center", "outside"];
/// `fill-rule` values.
pub(crate) const FILL_RULES: &[&str] = &["nonzero", "evenodd"];
/// `h-align` values.
pub(crate) const H_ALIGNS: &[&str] = &["start", "center", "end"];
/// `v-align` values.
pub(crate) const V_ALIGNS: &[&str] = &["top", "middle", "bottom"];
/// Path point `kind` values.
pub(crate) const PATH_POINT_KINDS: &[&str] = &["corner", "smooth", "symmetric"];
/// Group `symmetry` mode values.
pub(crate) const SYMMETRY_MODES: &[&str] = &["radial", "mirror"];
/// Table `border-collapse` values.
pub(crate) const BORDER_COLLAPSES: &[&str] = &["separate", "collapse"];
/// Text `format` values.
pub(crate) const TEXT_FORMATS: &[&str] = &["markdown", "plain"];
/// Image `fit` values.
pub(crate) const IMAGE_FITS: &[&str] = &["contain", "cover", "stretch", "none"];
/// Light `kind` values.
pub(crate) const LIGHT_KINDS: &[&str] = &["ambient", "glow", "key", "rim"];
/// Mesh camera `kind` values.
pub(crate) const CAMERA_KINDS: &[&str] = &["orthographic", "perspective"];
/// Pattern `kind` values.
pub(crate) const PATTERN_KINDS: &[&str] = &["grid", "scatter"];
/// Shape `kind` values.
pub(crate) const SHAPE_KINDS: &[&str] = &["process", "decision", "terminator", "ellipse"];
/// Connector `route` values.
pub(crate) const CONNECTOR_ROUTES: &[&str] = &["straight", "orthogonal", "avoid"];
/// Connector marker values.
pub(crate) const CONNECTOR_MARKERS: &[&str] = &["none", "arrow"];
/// Construction guide `type` values.
pub(crate) const GUIDE_TYPES: &[&str] = &["segment", "circle"];
/// Chart `kind` values.
pub(crate) const CHART_KINDS: &[&str] = &["bar", "line", "area", "sparkline", "pie", "donut"];
/// Chart `bar-mode` values.
pub(crate) const CHART_BAR_MODES: &[&str] = &["grouped", "stacked"];
/// Chart `orientation` values.
pub(crate) const CHART_ORIENTATIONS: &[&str] = &["vertical", "horizontal"];
/// Chart `point-placement` values.
pub(crate) const CHART_POINT_PLACEMENTS: &[&str] = &["edge", "center"];
/// Chart `value-labels` values.
pub(crate) const CHART_VALUE_LABELS: &[&str] = &["auto", "none", "top", "center"];
/// Chart `legend-position` values.
pub(crate) const CHART_LEGEND_POSITIONS: &[&str] = &["right", "left", "top", "bottom"];
/// Chart `legend-layout` values.
pub(crate) const CHART_LEGEND_LAYOUTS: &[&str] = &["wrapped", "list"];
/// Chart `legend-align` values.
pub(crate) const CHART_LEGEND_ALIGNS: &[&str] = &["center", "left", "right"];
/// Document `colorspace` values.
pub(crate) const COLORSPACES: &[&str] = &["srgb", "cmyk"];
/// Document `page-progression` values.
pub(crate) const PAGE_PROGRESSIONS: &[&str] = &["ltr", "rtl"];
/// Page parity values (`page-parity-start`, page `parity`).
pub(crate) const PAGE_PARITIES: &[&str] = &["recto", "verso"];
/// Page number format values.
pub(crate) const PAGE_NUMBER_FORMATS: &[&str] = &["decimal", "lower-roman", "upper-roman"];
/// Page `line-jumps` style values.
pub(crate) const LINE_JUMP_STYLES: &[&str] = &["none", "arc", "gap"];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{attribute_type, attribute_type_for_kind};

    fn hint(list: &[&str]) -> String {
        format!("enum: {}", list.join("|"))
    }

    #[test]
    fn schema_hints_match_validation_lists() {
        let by_kind: &[(&str, &str, &[&str])] = &[
            ("shape", "kind", SHAPE_KINDS),
            ("pattern", "kind", PATTERN_KINDS),
            ("chart", "kind", CHART_KINDS),
            ("light", "kind", LIGHT_KINDS),
            ("mesh", "kind", CAMERA_KINDS),
            ("mesh", "stroke-linecap", STROKE_LINECAPS),
            ("path", "stroke-linejoin", STROKE_LINEJOINS),
            ("group", "symmetry-mode", SYMMETRY_MODES),
            ("chart", "legend-position", CHART_LEGEND_POSITIONS),
            ("chart", "legend-layout", CHART_LEGEND_LAYOUTS),
            ("chart", "legend-align", CHART_LEGEND_ALIGNS),
            ("chart", "bar-mode", CHART_BAR_MODES),
            ("chart", "orientation", CHART_ORIENTATIONS),
            ("chart", "point-placement", CHART_POINT_PLACEMENTS),
            ("chart", "value-labels", CHART_VALUE_LABELS),
            ("connector", "route", CONNECTOR_ROUTES),
        ];
        for (kind, name, list) in by_kind {
            assert_eq!(
                attribute_type_for_kind(kind, name),
                hint(list),
                "{kind}.{name}"
            );
        }
        let generic: &[(&str, &[&str])] = &[
            ("stroke-alignment", STROKE_ALIGNS),
            ("stroke-linecap", STROKE_LINECAPS),
            ("fill-rule", FILL_RULES),
            ("fit", IMAGE_FITS),
            ("parity", PAGE_PARITIES),
            ("page-parity-start", PAGE_PARITIES),
            ("page-progression", PAGE_PROGRESSIONS),
            ("colorspace", COLORSPACES),
            ("h-align", H_ALIGNS),
            ("v-align", V_ALIGNS),
        ];
        for (name, list) in generic {
            assert_eq!(attribute_type(name), hint(list), "{name}");
        }
    }
}
