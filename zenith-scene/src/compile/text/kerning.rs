//! Scene-side resolution for authored manual kerning pair adjustments.

use std::collections::BTreeMap;

use zenith_core::KerningPair;
use zenith_core::tokens::ResolvedToken;
use zenith_layout::KerningPairAdjustment;

use crate::compile::util::resolve_property_dimension_px;

pub(in crate::compile) fn resolve_kerning_pairs(
    pairs: &[KerningPair],
    resolved: &BTreeMap<String, ResolvedToken>,
) -> Vec<KerningPairAdjustment> {
    pairs
        .iter()
        .map(|pair| KerningPairAdjustment {
            left: pair.left.clone(),
            right: pair.right.clone(),
            adjustment_px: resolve_property_dimension_px(Some(&pair.by), resolved, 0.0) as f32,
        })
        .collect()
}
