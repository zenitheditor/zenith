//! One nudged attribute: offset an authored value by a px delta and keep
//! its unit. Shared by `nudge_geometry`, `nudge_line_points`, and
//! `nudge_anchor_gap`.

use zenith_core::{Diagnostic, Dimension, PropertyValue, Unit, dim_to_px};

/// `d` plus `delta_px`, written in the unit of `d`. `None` when the unit has
/// no px conversion (`pct`, `deg`, or an unknown unit).
pub(in crate::engine) fn offset_dimension(d: &Dimension, delta_px: f64) -> Option<Dimension> {
    let delta = match &d.unit {
        Unit::Px => delta_px,
        Unit::Pt => delta_px * 72.0 / 96.0,
        Unit::Pct | Unit::Deg | Unit::Unknown(_) => return None,
    };
    Some(Dimension {
        value: d.value + delta,
        unit: d.unit.clone(),
    })
}

/// The px value of `d`, when its unit converts.
pub(in crate::engine) fn dimension_px(d: &Dimension) -> Option<f64> {
    dim_to_px(d.value, &d.unit)
}

/// An authored property value as `.zen` source text, for messages.
pub(in crate::engine) fn shown(value: &PropertyValue) -> String {
    match value {
        PropertyValue::Dimension(d) => d.to_kdl_string(),
        PropertyValue::TokenRef(id) => format!("(token){id:?}"),
        PropertyValue::Literal(s) => format!("{s:?}"),
        PropertyValue::DataRef(s) => format!("(data){s:?}"),
    }
}

/// Context for the per-attribute diagnostics of one op.
#[derive(Clone, Copy)]
pub(in crate::engine) struct AxisCtx<'a> {
    /// The op name, e.g. `nudge_geometry`.
    pub op: &'static str,
    /// The target node id.
    pub node_id: &'a str,
    /// The attribute name, e.g. `x` or `anchor-gap`.
    pub attr: &'static str,
}

impl AxisCtx<'_> {
    /// An Error with `code` and this attribute's message prefix.
    pub(in crate::engine) fn error(&self, code: &str, tail: String) -> Diagnostic {
        Diagnostic::error(
            code,
            format!(
                "{}: {} of node {:?} {tail}",
                self.op, self.attr, self.node_id
            ),
            None,
            Some(self.node_id.to_owned()),
        )
    }

    /// `tx.value_unresolved`: the value `shown` has no px value to offset.
    pub(in crate::engine) fn unresolved(&self, shown: &str) -> Diagnostic {
        self.error(
            "tx.value_unresolved",
            format!(
                "is {shown}, which has no px value to offset. Write an absolute value with \
                 set_geometry instead."
            ),
        )
    }

    /// `offset_dimension` of `d`, or `tx.value_unresolved`.
    pub(in crate::engine) fn offset(
        &self,
        d: &Dimension,
        delta: f64,
    ) -> Result<Dimension, Diagnostic> {
        offset_dimension(d, delta).ok_or_else(|| self.unresolved(&d.to_kdl_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dim(value: f64, unit: Unit) -> Dimension {
        Dimension { value, unit }
    }

    #[test]
    fn px_and_pt_keep_their_unit() {
        assert_eq!(
            offset_dimension(&dim(10.0, Unit::Px), 3.0),
            Some(dim(13.0, Unit::Px))
        );
        // 3 px = 2.25 pt.
        assert_eq!(
            offset_dimension(&dim(10.0, Unit::Pt), 3.0),
            Some(dim(12.25, Unit::Pt))
        );
        assert_eq!(
            dimension_px(&dim(12.25, Unit::Pt)),
            Some(12.25 * 96.0 / 72.0)
        );
    }

    #[test]
    fn pct_deg_and_unknown_units_do_not_offset() {
        assert_eq!(offset_dimension(&dim(50.0, Unit::Pct), 1.0), None);
        assert_eq!(offset_dimension(&dim(5.0, Unit::Deg), 1.0), None);
        assert_eq!(
            offset_dimension(&dim(5.0, Unit::Unknown("mm".into())), 1.0),
            None
        );
    }

    #[test]
    fn shown_prints_source_forms() {
        assert_eq!(
            shown(&PropertyValue::TokenRef("s.x".into())),
            r#"(token)"s.x""#
        );
        assert_eq!(
            shown(&PropertyValue::Dimension(dim(4.0, Unit::Pt))),
            "(pt)4"
        );
        assert_eq!(
            shown(&PropertyValue::DataRef("a.b".into())),
            r#"(data)"a.b""#
        );
        assert_eq!(shown(&PropertyValue::Literal("auto".into())), r#""auto""#);
    }
}
