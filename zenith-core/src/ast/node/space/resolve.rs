//! Resolve a geometry property (`x` / `y` / `w` / `h`) to px.

use std::collections::BTreeMap;

use crate::ast::value::{PropertyValue, dim_to_px};
use crate::tokens::{ResolvedToken, ResolvedValue};

/// Resolve an optional geometry property to pixels.
///
/// A raw dimension (`(px)120`) resolves directly. A dimension token ref
/// (`(token)"dim.h"`) resolves through `resolved`. Every other shape gives
/// `None`: an absent value, a literal, a data ref, an unresolved or
/// non-dimension token, or a unit without a px value (`pct`, `deg`).
pub fn resolve_geometry_px(
    prop: Option<&PropertyValue>,
    resolved: &BTreeMap<String, ResolvedToken>,
) -> Option<f64> {
    match prop? {
        PropertyValue::TokenRef(id) => match resolved.get(id.as_str()) {
            Some(rt) => match &rt.value {
                ResolvedValue::Dimension(d) => dim_to_px(d.value, &d.unit),
                ResolvedValue::Color(_)
                | ResolvedValue::CmykColor { .. }
                | ResolvedValue::Number(_)
                | ResolvedValue::FontFamily(_)
                | ResolvedValue::FontWeight(_)
                | ResolvedValue::Gradient(_)
                | ResolvedValue::Shadow(_)
                | ResolvedValue::Filter(_)
                | ResolvedValue::Mask(_) => None,
            },
            None => None,
        },
        PropertyValue::Dimension(d) => dim_to_px(d.value, &d.unit),
        PropertyValue::Literal(_) | PropertyValue::DataRef(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::value::{Dimension, Unit};

    #[test]
    fn raw_px_and_pt_resolve() {
        let r = BTreeMap::new();
        let px = PropertyValue::Dimension(Dimension {
            value: 12.0,
            unit: Unit::Px,
        });
        let pt = PropertyValue::Dimension(Dimension {
            value: 72.0,
            unit: Unit::Pt,
        });
        assert_eq!(resolve_geometry_px(Some(&px), &r), Some(12.0));
        assert_eq!(resolve_geometry_px(Some(&pt), &r), Some(96.0));
    }

    #[test]
    fn absent_pct_literal_and_missing_token_give_none() {
        let r = BTreeMap::new();
        let pct = PropertyValue::Dimension(Dimension {
            value: 50.0,
            unit: Unit::Pct,
        });
        assert_eq!(resolve_geometry_px(None, &r), None);
        assert_eq!(resolve_geometry_px(Some(&pct), &r), None);
        let lit = PropertyValue::Literal("10".to_owned());
        assert_eq!(resolve_geometry_px(Some(&lit), &r), None);
        let tok = PropertyValue::TokenRef("dim.x".to_owned());
        assert_eq!(resolve_geometry_px(Some(&tok), &r), None);
    }
}
