//! The blur / shadow / filter pick of a sized node.

use zenith_core::{TextNode, dim_to_px};

use crate::compile::paint::{NodeEffect, resolve_property_filter, resolve_property_shadow};
use crate::compile::style_prop;
use crate::compile::text::ctx::TextCompileEnv;

/// The winning node effect. Blur beats shadow, which beats filter; at most one
/// is chosen. A node with no shaped spans carries no effect. `shadow` falls
/// back to the node style's `shadow`.
pub(super) fn resolve_effect(
    text: &TextNode,
    env: TextCompileEnv,
    has_spans: bool,
) -> Option<NodeEffect> {
    let resolved = env.resolved;
    if !has_spans {
        return None;
    }
    let blur_sigma = text
        .blur
        .as_ref()
        .and_then(|d| dim_to_px(d.value, &d.unit))
        .filter(|&s| s > 0.0);
    if let Some(sigma) = blur_sigma {
        return Some(NodeEffect::Blur(sigma));
    }
    if let Some(shadows) = text
        .shadow
        .as_ref()
        .or_else(|| style_prop(&text.style, env.style_map, "shadow"))
        .and_then(|p| resolve_property_shadow(p, resolved, &text.id))
    {
        return Some(NodeEffect::Shadow(shadows));
    }
    text.filter
        .as_ref()
        .and_then(|p| resolve_property_filter(p, resolved, &text.id))
        .map(NodeEffect::Filter)
}
