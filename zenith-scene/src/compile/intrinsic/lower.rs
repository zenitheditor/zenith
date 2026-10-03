//! Lowering of node lists that exist only at compile time: projected master
//! children and expanded component instances.

use zenith_core::{Diagnostic, Node, subtree_uses_layout};

use crate::layout::lower_nodes;

use super::super::NodeCtx;
use super::IntrinsicEnv;

/// Lower the layout frames in `nodes` with the compile context `cx`, and
/// append the layout diagnostics to `diagnostics`.
///
/// A list without a layout frame is left untouched.
pub(in crate::compile) fn lower_expanded(
    nodes: &mut [Node],
    cx: NodeCtx<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !subtree_uses_layout(nodes) {
        return;
    }
    let lowered = lower_nodes(nodes, IntrinsicEnv::from_node_ctx(cx));
    diagnostics.extend(lowered.diagnostics);
}
