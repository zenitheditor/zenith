//! A `text.overflow` on an in-flow child of a row or column frame gets a note
//! when its remedy resizes the box along the frame's main axis: `set h=` in a
//! column, `set w=` in a row. That advice is right for the box and wrong for
//! the layout around it, because the siblings after it move, and an agent that
//! follows it reflows the whole stack. A remedy on the cross axis, or no
//! resize at all (tab leaders), moves no sibling and gets no note.

use std::collections::BTreeMap;

use zenith_core::{Diagnostic, LayoutKind, LayoutPosition, Node};

/// A stack frame: its id and layout.
#[derive(Clone, Copy)]
struct Stack<'a> {
    frame: &'a str,
    layout: &'static str,
    /// The resize remedy in the overflow message that moves the siblings.
    main_axis_remedy: &'static str,
    /// What the resize makes the box.
    grows: &'static str,
}

/// The stack frame of every in-flow child of a stack frame, keyed by child id.
fn flow_parents<'a>(children: &'a [Node], out: &mut BTreeMap<&'a str, Stack<'a>>) {
    for node in children {
        let Some(kids) = node.children() else {
            continue;
        };
        if let (Node::Frame(frame), Some(frame_id)) = (node, node.id()) {
            let name = match &frame.layout {
                Some(LayoutKind::Row) => Some(("row", "set w=(px)", "wider")),
                Some(LayoutKind::Column) => Some(("column", "set h=(px)", "taller")),
                Some(LayoutKind::Absolute | LayoutKind::Grid | LayoutKind::Unknown(_)) | None => {
                    None
                }
            };
            if let Some((layout, main_axis_remedy, grows)) = name {
                for kid in kids {
                    let in_flow = kid
                        .layout_item()
                        .and_then(|item| item.position.as_ref())
                        .is_none_or(|p| match p {
                            LayoutPosition::Auto => true,
                            LayoutPosition::Absolute | LayoutPosition::Unknown(_) => false,
                        });
                    if let (true, Some(id)) = (in_flow, kid.id()) {
                        out.insert(
                            id,
                            Stack {
                                frame: frame_id,
                                layout,
                                main_axis_remedy,
                                grows,
                            },
                        );
                    }
                }
            }
        }
        flow_parents(kids, out);
    }
}

/// Append the flow note to each `text.overflow` whose subject is an in-flow
/// child of a row or column frame in `children` and whose remedy resizes the
/// box along that frame's main axis. The remedy is read from the message the
/// overflow report wrote, so a change to that wording fails the tests.
pub(in crate::compile) fn note_flow_on_overflow(children: &[Node], diagnostics: &mut [Diagnostic]) {
    let mut parents = BTreeMap::new();
    flow_parents(children, &mut parents);
    if parents.is_empty() {
        return;
    }
    for d in diagnostics.iter_mut().filter(|d| d.code == "text.overflow") {
        let Some(stack) = d
            .subject_id
            .as_deref()
            .and_then(|id| parents.get(id))
            .copied()
        else {
            continue;
        };
        if !d.message.contains(stack.main_axis_remedy) {
            continue;
        }
        d.message.push_str(&format!(
            " The box is in the flow of {} frame '{}': a {} box moves the siblings after it, a smaller font-size does not.",
            stack.layout, stack.frame, stack.grows
        ));
    }
}
