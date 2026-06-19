//! Container-node compilation: `frame` (clip-only) and `group` (translate +
//! opacity cascade), plus the bounding-box helpers used to determine a group's
//! rotation pivot.

use std::collections::BTreeMap;

use zenith_core::{
    Diagnostic, Dimension, FontProvider, FrameNode, GroupNode, Node, Point, ResolvedToken, Style,
    Unit, dim_to_px,
};
use zenith_layout::RustybuzzEngine;

use crate::ir::SceneCommand;

use super::util::{resolve_property_dimension_px, rotation_degrees, unsupported_unit_diag};
use super::{RenderCtx, compile_node, node_role, style_prop};

// NOTE: compile_frame → compile_node → compile_frame recursion has no depth
// guard, consistent with the compile_group limitation in v0.
#[allow(clippy::too_many_arguments)]
pub(super) fn compile_frame(
    frame: &FrameNode,
    resolved: &BTreeMap<String, ResolvedToken>,
    style_map: &BTreeMap<&str, &Style>,
    fonts: &dyn FontProvider,
    engine: &RustybuzzEngine,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: RenderCtx,
) {
    // Entire subtree excluded when visible=false (no PushClip emitted).
    if frame.visible == Some(false) {
        return;
    }

    // All four geometry dimensions are required for a frame clip rectangle.
    // Resolve them BEFORE pushing any PushClip to keep push/pop balanced.
    let (Some(x_dim), Some(y_dim), Some(w_dim), Some(h_dim)) =
        (&frame.x, &frame.y, &frame.w, &frame.h)
    else {
        diagnostics.push(Diagnostic::advisory(
            "scene.missing_geometry",
            format!(
                "frame '{}' is missing one or more geometry properties (x, y, w, h); \
                 skipped",
                frame.id
            ),
            frame.source_span,
            Some(frame.id.clone()),
        ));
        return;
    };

    let Some(frame_x) = dim_to_px(x_dim.value, &x_dim.unit) else {
        diagnostics.push(unsupported_unit_diag(
            "frame",
            &frame.id,
            "x",
            frame.source_span,
        ));
        return;
    };
    let Some(frame_y) = dim_to_px(y_dim.value, &y_dim.unit) else {
        diagnostics.push(unsupported_unit_diag(
            "frame",
            &frame.id,
            "y",
            frame.source_span,
        ));
        return;
    };
    let Some(frame_w) = dim_to_px(w_dim.value, &w_dim.unit) else {
        diagnostics.push(unsupported_unit_diag(
            "frame",
            &frame.id,
            "w",
            frame.source_span,
        ));
        return;
    };
    let Some(frame_h) = dim_to_px(h_dim.value, &h_dim.unit) else {
        diagnostics.push(unsupported_unit_diag(
            "frame",
            &frame.id,
            "h",
            frame.source_span,
        ));
        return;
    };

    // Rotation bracket — outermost, wrapping PushClip + children + PopClip.
    // v0 limitation: the clip rectangle below stays axis-aligned even when the
    // frame is rotated; rotated children may extend past the axis-aligned clip.
    let frame_rot = rotation_degrees(frame.rotate.as_ref());
    if let Some(angle) = frame_rot {
        let cx = ctx.dx + frame_x + frame_w / 2.0;
        let cy = ctx.dy + frame_y + frame_h / 2.0;
        commands.push(SceneCommand::PushTransform {
            angle_deg: angle,
            cx,
            cy,
        });
    }

    // Clip rectangle is the frame's own bbox.
    commands.push(SceneCommand::PushClip {
        x: frame_x,
        y: frame_y,
        w: frame_w,
        h: frame_h,
    });

    // Frame clips only — it does NOT translate children (dx/dy unchanged).
    // Opacity cascades into all descendant alphas exactly as group does.
    let child_ctx = RenderCtx {
        opacity: ctx.opacity * frame.opacity.unwrap_or(1.0).clamp(0.0, 1.0),
        dx: ctx.dx, // clip-only: no translation
        dy: ctx.dy, // clip-only: no translation
    };

    if frame.layout.as_deref() == Some("flow") {
        compile_frame_flow(
            frame,
            frame_x,
            frame_y,
            frame_w,
            resolved,
            style_map,
            fonts,
            engine,
            commands,
            diagnostics,
            child_ctx,
        );
    } else {
        // Absolute (clip-only) model: children render at their own page coords.
        for child in &frame.children {
            compile_node(
                child,
                resolved,
                style_map,
                fonts,
                engine,
                commands,
                diagnostics,
                child_ctx,
            );
        }
    }

    commands.push(SceneCommand::PopClip);

    if frame_rot.is_some() {
        commands.push(SceneCommand::PopTransform);
    }
    // Frame emits no fill of its own in v0.
}

/// Lay a flow-frame's children out as a vertical stack inside its padded
/// content box, compiling each at the injected absolute coordinates.
///
/// Triggered only when `frame.layout == Some("flow")`. `frame_x`/`frame_y`/
/// `frame_w` are the already-resolved frame box in page coordinates (the same
/// values used for the surrounding `PushClip`). Children stack in source order
/// with `gap` between them; `padding` insets the content box uniformly. Both
/// `padding` and `gap` are token-only dimension style props on the frame's
/// style, defaulting to `0.0` when absent.
#[allow(clippy::too_many_arguments)]
fn compile_frame_flow(
    frame: &FrameNode,
    frame_x: f64,
    frame_y: f64,
    frame_w: f64,
    resolved: &BTreeMap<String, ResolvedToken>,
    style_map: &BTreeMap<&str, &Style>,
    fonts: &dyn FontProvider,
    engine: &RustybuzzEngine,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
    child_ctx: RenderCtx,
) {
    // Resolve padding / gap from the frame's style (token → px); 0.0 default.
    let pad = resolve_property_dimension_px(
        &style_prop(&frame.style, style_map, "padding").cloned(),
        resolved,
        0.0,
    );
    let gap = resolve_property_dimension_px(
        &style_prop(&frame.style, style_map, "gap").cloned(),
        resolved,
        0.0,
    );

    // Content box: uniform padding on all four sides.
    let content_left = frame_x + pad;
    let content_top = frame_y + pad;
    let content_w = (frame_w - 2.0 * pad).max(0.0);

    // Lay out children that participate (skip invisible and guide nodes) so a
    // trailing gap is only suppressed relative to the LAST laid-out child.
    let laid_out: Vec<&Node> = frame
        .children
        .iter()
        .filter(|c| !node_skipped_in_flow(c))
        .collect();
    let last_idx = laid_out.len().saturating_sub(1);

    let mut cursor_y = content_top;
    for (i, child) in laid_out.iter().enumerate() {
        // Cross-axis = start; child width = own declared `w` or the content
        // width. (A text child's own `align` still centers WITHIN its width.)
        let child_w = node_declared_w(child).unwrap_or(content_w);

        // Vertical extent: own declared `h` when present, else the MEASURED
        // intrinsic height returned by compiling the child (text/code wrapped
        // height; 0.0 for leaves with no declared height).
        let declared_h = node_declared_h(child);

        // Inject the absolute box onto a clone; compile with the SAME ctx
        // (dx/dy unchanged — injected coords are absolute page coords).
        let positioned = with_flow_box(child, content_left, cursor_y, child_w, declared_h);
        let measured_h = compile_node(
            &positioned,
            resolved,
            style_map,
            fonts,
            engine,
            commands,
            diagnostics,
            child_ctx,
        );

        // Advance by the declared height when present, otherwise the measured
        // intrinsic height read back from the compile.
        let advance = declared_h.unwrap_or(measured_h);
        cursor_y += advance;
        if i != last_idx {
            cursor_y += gap;
        }
    }
}

/// Whether a child is excluded from flow layout entirely (consumes no space):
/// `visible == Some(false)` or `role == "guide"`.
fn node_skipped_in_flow(node: &Node) -> bool {
    node_role(node) == Some("guide") || node_visible(node) == Some(false)
}

/// The `visible` flag of any node kind, if set (kinds without the property
/// — `Unknown` — yield `None`).
fn node_visible(node: &Node) -> Option<bool> {
    match node {
        Node::Rect(n) => n.visible,
        Node::Ellipse(n) => n.visible,
        Node::Line(n) => n.visible,
        Node::Text(n) => n.visible,
        Node::Code(n) => n.visible,
        Node::Frame(n) => n.visible,
        Node::Group(n) => n.visible,
        Node::Image(n) => n.visible,
        Node::Polygon(n) => n.visible,
        Node::Polyline(n) => n.visible,
        Node::Unknown(_) => None,
    }
}

/// The declared `w` of a node in pixels, if the node kind carries a `w`/`h`
/// box and the dimension resolves to pixels. Geometry-less kinds (line,
/// polygon, polyline, unknown) yield `None`.
fn node_declared_w(node: &Node) -> Option<f64> {
    let w = match node {
        Node::Rect(n) => n.w.as_ref(),
        Node::Ellipse(n) => n.w.as_ref(),
        Node::Text(n) => n.w.as_ref(),
        Node::Code(n) => n.w.as_ref(),
        Node::Frame(n) => n.w.as_ref(),
        Node::Group(n) => n.w.as_ref(),
        Node::Image(n) => n.w.as_ref(),
        Node::Line(_) | Node::Polygon(_) | Node::Polyline(_) | Node::Unknown(_) => None,
    }?;
    dim_to_px(w.value, &w.unit)
}

/// The declared `h` of a node in pixels, if the node kind carries a `w`/`h`
/// box and the dimension resolves to pixels. Geometry-less kinds yield `None`.
fn node_declared_h(node: &Node) -> Option<f64> {
    let h = match node {
        Node::Rect(n) => n.h.as_ref(),
        Node::Ellipse(n) => n.h.as_ref(),
        Node::Text(n) => n.h.as_ref(),
        Node::Code(n) => n.h.as_ref(),
        Node::Frame(n) => n.h.as_ref(),
        Node::Group(n) => n.h.as_ref(),
        Node::Image(n) => n.h.as_ref(),
        Node::Line(_) | Node::Polygon(_) | Node::Polyline(_) | Node::Unknown(_) => None,
    }?;
    dim_to_px(h.value, &h.unit)
}

/// Clone `node` and overwrite its `x`/`y`/`w`/`h` box with the injected
/// flow coordinates (all in absolute page px). `h` is set only when the flow
/// path resolved one (declared height); a `None` `h` leaves the clone's `h`
/// unset so the child auto-measures its own intrinsic height.
///
/// Kinds without an `x`/`y`/`w`/`h` box (`Line`/`Polygon`/`Polyline`/
/// `Unknown`) are returned unchanged — the flow path advances its cursor by
/// `0.0` for those.
fn with_flow_box(node: &Node, x: f64, y: f64, w: f64, h: Option<f64>) -> Node {
    let px = |v: f64| {
        Some(Dimension {
            value: v,
            unit: Unit::Px,
        })
    };
    let h_dim = h.and_then(px);

    let mut out = node.clone();
    match &mut out {
        Node::Rect(n) => {
            n.x = px(x);
            n.y = px(y);
            n.w = px(w);
            n.h = h_dim;
        }
        Node::Ellipse(n) => {
            n.x = px(x);
            n.y = px(y);
            n.w = px(w);
            n.h = h_dim;
        }
        Node::Text(n) => {
            n.x = px(x);
            n.y = px(y);
            n.w = px(w);
            n.h = h_dim;
        }
        Node::Code(n) => {
            n.x = px(x);
            n.y = px(y);
            n.w = px(w);
            n.h = h_dim;
        }
        Node::Image(n) => {
            n.x = px(x);
            n.y = px(y);
            n.w = px(w);
            n.h = h_dim;
        }
        Node::Frame(n) => {
            n.x = px(x);
            n.y = px(y);
            n.w = px(w);
            n.h = h_dim;
        }
        Node::Group(n) => {
            n.x = px(x);
            n.y = px(y);
            n.w = px(w);
            n.h = h_dim;
        }
        // Geometry-less kinds: no x/y/w/h box to inject.
        Node::Line(_) | Node::Polygon(_) | Node::Polyline(_) | Node::Unknown(_) => {}
    }
    out
}

// NOTE: compile_group → compile_node → compile_group recursion has no depth
// guard.  Pathologically deep group trees can overflow the stack.  This is a
// known v0 limitation; a guard will be added when nested documents are tested.
#[allow(clippy::too_many_arguments)]
pub(super) fn compile_group(
    group: &GroupNode,
    resolved: &BTreeMap<String, ResolvedToken>,
    style_map: &BTreeMap<&str, &Style>,
    fonts: &dyn FontProvider,
    engine: &RustybuzzEngine,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: RenderCtx,
) {
    // Entire subtree excluded when visible=false.
    if group.visible == Some(false) {
        return;
    }

    // Cascade opacity: multiply the group's own opacity into the inherited ctx.
    let group_opacity = group.opacity.unwrap_or(1.0).clamp(0.0, 1.0);
    let child_opacity = ctx.opacity * group_opacity;

    // Resolve group x/y to pixels; absent or unsupported-unit → 0.0 (no diagnostic).
    let group_x_px = group
        .x
        .as_ref()
        .and_then(|d| dim_to_px(d.value, &d.unit))
        .unwrap_or(0.0);
    let group_y_px = group
        .y
        .as_ref()
        .and_then(|d| dim_to_px(d.value, &d.unit))
        .unwrap_or(0.0);

    let child_dx = ctx.dx + group_x_px;
    let child_dy = ctx.dy + group_y_px;

    // Rotation bracket — outermost, wrapping all child commands.
    // Determine the pivot center:
    //   1. If the group has BOTH w and h declared → use the declared box center.
    //   2. Otherwise → compute the union bbox of direct children in device space.
    //   3. If neither yields a center (empty / geometry-less group) → skip
    //      the bracket entirely (v0 limitation, commented below).
    let group_rot = rotation_degrees(group.rotate.as_ref());
    let rot_center: Option<(f64, f64)> = if group_rot.is_some() {
        // Try declared box first.
        let declared = group
            .w
            .as_ref()
            .and_then(|wd| dim_to_px(wd.value, &wd.unit))
            .zip(
                group
                    .h
                    .as_ref()
                    .and_then(|hd| dim_to_px(hd.value, &hd.unit)),
            )
            .map(|(gw, gh)| (child_dx + gw / 2.0, child_dy + gh / 2.0));
        if declared.is_some() {
            declared
        } else {
            // Fall back to union bbox of direct children in device space.
            // v0 limitation: if the group is empty or contains only
            // geometry-less nodes no center is computable → rotation is
            // silently skipped.
            group_children_center(&group.children, child_dx, child_dy)
        }
    } else {
        None
    };

    if let (Some(angle), Some((cx, cy))) = (group_rot, rot_center) {
        commands.push(SceneCommand::PushTransform {
            angle_deg: angle,
            cx,
            cy,
        });
    }

    // Emit children in source order; the group itself produces no command.
    let child_ctx = RenderCtx {
        opacity: child_opacity,
        dx: child_dx,
        dy: child_dy,
    };
    for child in &group.children {
        compile_node(
            child,
            resolved,
            style_map,
            fonts,
            engine,
            commands,
            diagnostics,
            child_ctx,
        );
    }

    if group_rot.is_some() && rot_center.is_some() {
        commands.push(SceneCommand::PopTransform);
    }
}

/// Compute the axis-aligned bounding box of a `Point` list in authored coords.
///
/// Returns `(x_min, y_min, w, h)` in authored (pre-`base_dx`/`base_dy`) space,
/// or `None` when the list is empty or every point has a missing / unsupported-
/// unit coordinate.  Used by both the `Polygon` and `Polyline` arms of
/// [`group_children_center`] to avoid duplicating the accumulation loop.
fn points_bbox(pts: &[Point]) -> Option<(f64, f64, f64, f64)> {
    let mut px_min = f64::INFINITY;
    let mut py_min = f64::INFINITY;
    let mut px_max = f64::NEG_INFINITY;
    let mut py_max = f64::NEG_INFINITY;
    for pt in pts {
        let (Some(xd), Some(yd)) = (&pt.x, &pt.y) else {
            continue;
        };
        let (Some(px), Some(py)) = (dim_to_px(xd.value, &xd.unit), dim_to_px(yd.value, &yd.unit))
        else {
            continue;
        };
        px_min = px_min.min(px);
        py_min = py_min.min(py);
        px_max = px_max.max(px);
        py_max = py_max.max(py);
    }
    if px_min.is_finite() {
        Some((px_min, py_min, px_max - px_min, py_max - py_min))
    } else {
        None
    }
}

/// Compute the device-space center of a group's direct-child union bounding box.
///
/// `base_dx`/`base_dy` are the device-space origin of the group (i.e.
/// `ctx.dx + group_x_px` and `ctx.dy + group_y_px`).  Children are positioned
/// relative to those origins.
///
/// Returns `None` when no child yields a computable bbox (e.g. an empty group
/// or one containing only unknown/geometry-less nodes).
fn group_children_center(children: &[Node], base_dx: f64, base_dy: f64) -> Option<(f64, f64)> {
    // Accumulate min/max in device space.
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for child in children {
        // Helper: expand accumulated bounds by a device-space rect.
        macro_rules! expand {
            ($x:expr, $y:expr, $w:expr, $h:expr) => {
                if $w > 0.0 || $h > 0.0 {
                    min_x = min_x.min($x);
                    min_y = min_y.min($y);
                    max_x = max_x.max($x + $w);
                    max_y = max_y.max($y + $h);
                }
            };
        }

        match child {
            Node::Rect(n) => {
                let (Some(xd), Some(yd), Some(wd), Some(hd)) = (&n.x, &n.y, &n.w, &n.h) else {
                    continue;
                };
                let (Some(x), Some(y), Some(w), Some(h)) = (
                    dim_to_px(xd.value, &xd.unit),
                    dim_to_px(yd.value, &yd.unit),
                    dim_to_px(wd.value, &wd.unit),
                    dim_to_px(hd.value, &hd.unit),
                ) else {
                    continue;
                };
                expand!(base_dx + x, base_dy + y, w, h);
            }
            Node::Ellipse(n) => {
                let (Some(xd), Some(yd), Some(wd), Some(hd)) = (&n.x, &n.y, &n.w, &n.h) else {
                    continue;
                };
                let (Some(x), Some(y), Some(w), Some(h)) = (
                    dim_to_px(xd.value, &xd.unit),
                    dim_to_px(yd.value, &yd.unit),
                    dim_to_px(wd.value, &wd.unit),
                    dim_to_px(hd.value, &hd.unit),
                ) else {
                    continue;
                };
                expand!(base_dx + x, base_dy + y, w, h);
            }
            Node::Text(n) => {
                let (Some(xd), Some(yd), Some(wd), Some(hd)) = (&n.x, &n.y, &n.w, &n.h) else {
                    continue;
                };
                let (Some(x), Some(y), Some(w), Some(h)) = (
                    dim_to_px(xd.value, &xd.unit),
                    dim_to_px(yd.value, &yd.unit),
                    dim_to_px(wd.value, &wd.unit),
                    dim_to_px(hd.value, &hd.unit),
                ) else {
                    continue;
                };
                expand!(base_dx + x, base_dy + y, w, h);
            }
            Node::Code(n) => {
                let (Some(xd), Some(yd), Some(wd), Some(hd)) = (&n.x, &n.y, &n.w, &n.h) else {
                    continue;
                };
                let (Some(x), Some(y), Some(w), Some(h)) = (
                    dim_to_px(xd.value, &xd.unit),
                    dim_to_px(yd.value, &yd.unit),
                    dim_to_px(wd.value, &wd.unit),
                    dim_to_px(hd.value, &hd.unit),
                ) else {
                    continue;
                };
                expand!(base_dx + x, base_dy + y, w, h);
            }
            Node::Image(n) => {
                let (Some(xd), Some(yd), Some(wd), Some(hd)) = (&n.x, &n.y, &n.w, &n.h) else {
                    continue;
                };
                let (Some(x), Some(y), Some(w), Some(h)) = (
                    dim_to_px(xd.value, &xd.unit),
                    dim_to_px(yd.value, &yd.unit),
                    dim_to_px(wd.value, &wd.unit),
                    dim_to_px(hd.value, &hd.unit),
                ) else {
                    continue;
                };
                expand!(base_dx + x, base_dy + y, w, h);
            }
            Node::Frame(n) => {
                let (Some(xd), Some(yd), Some(wd), Some(hd)) = (&n.x, &n.y, &n.w, &n.h) else {
                    continue;
                };
                let (Some(x), Some(y), Some(w), Some(h)) = (
                    dim_to_px(xd.value, &xd.unit),
                    dim_to_px(yd.value, &yd.unit),
                    dim_to_px(wd.value, &wd.unit),
                    dim_to_px(hd.value, &hd.unit),
                ) else {
                    continue;
                };
                expand!(base_dx + x, base_dy + y, w, h);
            }
            Node::Line(n) => {
                // Line bbox is the degenerate rect spanning (x1,y1)-(x2,y2).
                let (Some(x1d), Some(y1d), Some(x2d), Some(y2d)) = (&n.x1, &n.y1, &n.x2, &n.y2)
                else {
                    continue;
                };
                let (Some(x1), Some(y1), Some(x2), Some(y2)) = (
                    dim_to_px(x1d.value, &x1d.unit),
                    dim_to_px(y1d.value, &y1d.unit),
                    dim_to_px(x2d.value, &x2d.unit),
                    dim_to_px(y2d.value, &y2d.unit),
                ) else {
                    continue;
                };
                let lx = x1.min(x2);
                let ly = y1.min(y2);
                let lw = (x2 - x1).abs();
                let lh = (y2 - y1).abs();
                expand!(base_dx + lx, base_dy + ly, lw, lh);
            }
            Node::Polygon(n) => {
                if let Some((x, y, w, h)) = points_bbox(&n.points) {
                    expand!(base_dx + x, base_dy + y, w, h);
                }
            }
            Node::Polyline(n) => {
                if let Some((x, y, w, h)) = points_bbox(&n.points) {
                    expand!(base_dx + x, base_dy + y, w, h);
                }
            }
            Node::Group(n) => {
                // Nested group: use its declared w/h if available, else skip.
                let (Some(xd), Some(yd), Some(wd), Some(hd)) = (&n.x, &n.y, &n.w, &n.h) else {
                    continue;
                };
                let (Some(x), Some(y), Some(w), Some(h)) = (
                    dim_to_px(xd.value, &xd.unit),
                    dim_to_px(yd.value, &yd.unit),
                    dim_to_px(wd.value, &wd.unit),
                    dim_to_px(hd.value, &hd.unit),
                ) else {
                    continue;
                };
                expand!(base_dx + x, base_dy + y, w, h);
            }
            // Unknown nodes have no geometry — skip.
            Node::Unknown(_) => {}
        }
    }

    if min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite() {
        Some(((min_x + max_x) / 2.0, (min_y + max_y) / 2.0))
    } else {
        None
    }
}
