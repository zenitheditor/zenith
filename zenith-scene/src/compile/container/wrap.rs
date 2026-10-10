//! Shared attached-effect wrapping for container subtrees.

use crate::ir::{MaskSpec, SceneCommand};

use super::super::NodeCtx;
use super::super::boxes::StreamId;
use super::super::paint::{NodeEffect, emit_node_with_effects};

/// Compile a container body into a scratch stream with `body`, then splice
/// it into `commands` wrapped by `effect` and `mask`.
///
/// With a box recorder, `body` records into a [`nested`] recorder. Its base
/// is the state open at the splice point, and its offset puts each
/// `command_index` into the final stream. With both an effect and a mask
/// the body lands twice: records point at the first copy, the sharp base.
///
/// [`nested`]: super::super::boxes::BoxRecorder::nested
pub(super) fn emit_wrapped_container(
    cx: NodeCtx,
    commands: &mut Vec<SceneCommand>,
    connector_strokes: &mut Vec<usize>,
    (effect, mask): (Option<NodeEffect>, Option<MaskSpec>),
    body: impl FnOnce(NodeCtx, &mut Vec<SceneCommand>, &mut Vec<usize>),
) {
    // Commands the wrapper puts before the first copy of the body.
    let lead = match (&mask, &effect) {
        (None, None) | (Some(_), Some(_)) => 0,
        (None, Some(_)) | (Some(_), None) => 1,
    };
    let propagate_connectors = !matches!((&mask, &effect), (Some(_), Some(_)));
    let mut draws = Vec::new();
    let mut local_connector_strokes = Vec::new();
    match cx.boxes {
        Some(recorder) => {
            let nested = recorder.nested(StreamId::of(commands), commands, lead);
            body(
                NodeCtx {
                    boxes: Some(&nested),
                    ..cx
                },
                &mut draws,
                &mut local_connector_strokes,
            );
            recorder.adopt(nested);
        }
        None => body(cx, &mut draws, &mut local_connector_strokes),
    }
    let base = commands.len();
    emit_node_with_effects(commands, draws, effect, mask);
    if propagate_connectors {
        connector_strokes.extend(
            local_connector_strokes
                .into_iter()
                .map(|idx| base + lead + idx),
        );
    }
}
