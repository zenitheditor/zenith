//! Lowering of node lists that exist only at compile time: projected master
//! children and expanded component instances.

use std::cell::Cell;

use zenith_core::{Diagnostic, Node, subtree_uses_layout};

use crate::layout::{NodeBoxes, lower_nodes, may_depend_on_position, settle};

use super::super::{NodeCtx, RenderCtx};
use super::{IntrinsicEnv, ProbeHints};

/// Lower the layout frames in `nodes` with the compile context `cx`, and
/// append the layout diagnostics to `diagnostics`.
///
/// `base_ctx` is the render context the list compiles under; measure probes
/// use it, and position-dependent text settles against the page's runaround
/// boxes. A list without a layout frame is left untouched. No anchor resolves
/// here: compile resolves anchors on page children only.
pub(in crate::compile) fn lower_expanded(
    nodes: &mut [Node],
    cx: NodeCtx<'_>,
    base_ctx: RenderCtx,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !subtree_uses_layout(nodes) {
        return;
    }
    let probes = Cell::new(0);
    let no_hints = ProbeHints::new();
    let probe_env = IntrinsicEnv::from_node_ctx(cx, base_ctx, &no_hints, &probes);
    if !may_depend_on_position(nodes, &probe_env) {
        let lowered = lower_nodes(nodes, probe_env, None);
        diagnostics.extend(lowered.diagnostics);
        return;
    }
    let original = nodes.to_vec();
    let (lowered, _) = settle(
        &original,
        nodes,
        NodeBoxes::new(),
        None,
        &mut |list, hints, _| {
            lower_nodes(
                list,
                IntrinsicEnv::from_node_ctx(cx, base_ctx, hints, &probes),
                None,
            )
        },
    );
    diagnostics.extend(lowered.diagnostics);
}
