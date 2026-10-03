//! Chart text roles, their size and weight relative to the chart base size,
//! and the glyph-run source ids that attribute each drawn string to its chart.
//!
//! Every chart glyph run carries `source_node_id = "<chart-id>/<role>/<index>"`
//! (`<index>` is unique per role within the chart: the tick, category, legend
//! entry, or `series × categories + category` of a value label). The page lint
//! reads the id back to judge chart text size and contrast, and its
//! diagnostics name the chart and the role.

/// What a chart string is. The base size is the chart style `font-size`, else
/// the engine default (see `look::default_base`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::compile) enum ChartTextRole {
    /// Chart title above the plot: base × 1.25, weight 600.
    Title,
    /// Caption below the plot: base.
    Caption,
    /// Value-axis tick labels: base.
    Axis,
    /// Category labels: base.
    Category,
    /// Legend entry labels: base.
    Legend,
    /// Value labels on the plot background: base × 0.875.
    Value,
    /// Value labels drawn on a bar or slice fill: base × 0.875.
    ValueInside,
}

/// Every role, in source-id order.
pub(in crate::compile) const CHART_TEXT_ROLES: [ChartTextRole; 7] = [
    ChartTextRole::Title,
    ChartTextRole::Caption,
    ChartTextRole::Axis,
    ChartTextRole::Category,
    ChartTextRole::Legend,
    ChartTextRole::Value,
    ChartTextRole::ValueInside,
];

impl ChartTextRole {
    /// The role name in source ids and diagnostics.
    pub(in crate::compile) const fn as_str(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Caption => "caption",
            Self::Axis => "axis",
            Self::Category => "category",
            Self::Legend => "legend",
            Self::Value => "value",
            Self::ValueInside => "value-inside",
        }
    }

    /// Font size as a multiple of the chart base size.
    pub(in crate::compile) const fn scale(self) -> f64 {
        match self {
            Self::Title => 1.25,
            Self::Value | Self::ValueInside => 0.875,
            Self::Caption | Self::Axis | Self::Category | Self::Legend => 1.0,
        }
    }

    /// Font weight.
    pub(in crate::compile) const fn weight(self) -> u16 {
        match self {
            Self::Title => 600,
            Self::Caption
            | Self::Axis
            | Self::Category
            | Self::Legend
            | Self::Value
            | Self::ValueInside => 400,
        }
    }

    /// Whether the contrast check judges this role. A value label inside a
    /// bar or slice sits on the chart's own mark ink, which the backdrop
    /// sampler does not model, so it is not judged.
    pub(in crate::compile) const fn contrast_judged(self) -> bool {
        match self {
            Self::ValueInside => false,
            Self::Title
            | Self::Caption
            | Self::Axis
            | Self::Category
            | Self::Legend
            | Self::Value => true,
        }
    }

    /// The glyph-run source id of string `index` of this role in `chart_id`.
    pub(in crate::compile) fn source_id(self, chart_id: &str, index: usize) -> String {
        format!("{chart_id}/{}/{index}", self.as_str())
    }

    fn parse_name(name: &str) -> Option<Self> {
        CHART_TEXT_ROLES.into_iter().find(|r| r.as_str() == name)
    }
}

/// A chart glyph-run source id split into `(chart id, role, index)`. The chart
/// id may itself hold `/` (an expanded instance prefixes its ids).
pub(in crate::compile) fn parse_chart_source(source: &str) -> Option<(&str, ChartTextRole, usize)> {
    let (rest, index) = source.rsplit_once('/')?;
    let index: usize = index.parse().ok()?;
    let (chart, role) = rest.rsplit_once('/')?;
    if chart.is_empty() {
        return None;
    }
    Some((chart, ChartTextRole::parse_name(role)?, index))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_ids_round_trip() {
        for role in CHART_TEXT_ROLES {
            let id = role.source_id("card/sales", 3);
            assert_eq!(parse_chart_source(&id), Some(("card/sales", role, 3)));
        }
    }

    #[test]
    fn non_chart_sources_do_not_parse() {
        assert_eq!(parse_chart_source("title"), None);
        assert_eq!(parse_chart_source("box/label"), None);
        assert_eq!(parse_chart_source("a/axis/x"), None);
        assert_eq!(parse_chart_source("a/unknown/0"), None);
        assert_eq!(parse_chart_source("/axis/0"), None);
    }

    #[test]
    fn role_sizes_follow_the_base() {
        assert_eq!(ChartTextRole::Title.scale(), 1.25);
        assert_eq!(ChartTextRole::Value.scale(), 0.875);
        assert_eq!(ChartTextRole::Axis.scale(), 1.0);
        assert!(!ChartTextRole::ValueInside.contrast_judged());
    }
}
