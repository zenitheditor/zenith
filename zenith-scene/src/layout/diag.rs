//! Layout diagnostics and the sink that collects them.
//!
//! A frame is solved several times: to measure it as a child, then once more
//! to place its children. Only the placing pass reports, through a live
//! [`Sink`]; measuring passes use [`Sink::none`], so each finding appears once.

use zenith_core::{Diagnostic, Node};

use super::model::{Axis, Mode};

/// Where a solve pass sends its diagnostics, if anywhere.
pub(super) struct Sink<'d> {
    out: Option<&'d mut Vec<Diagnostic>>,
}

impl<'d> Sink<'d> {
    /// A sink that drops every diagnostic (measuring passes).
    pub(super) fn none() -> Self {
        Self { out: None }
    }

    /// A sink that keeps every diagnostic in `out`.
    pub(super) fn to(out: &'d mut Vec<Diagnostic>) -> Self {
        Self { out: Some(out) }
    }

    pub(super) fn push(&mut self, d: Diagnostic) {
        if let Some(out) = self.out.as_deref_mut() {
            out.push(d);
        }
    }
}

/// `layout.unsized_child` (Error): a flow child hugs an axis but has no
/// content to size from.
pub(super) fn unsized_child(child: &Node, frame_id: &str, mode: Mode, axis: Axis) -> Diagnostic {
    let (id, span) = child.id_and_span();
    let kind = child.kind_str();
    let attr = axis.size_attr();
    Diagnostic::error(
        "layout.unsized_child",
        format!(
            "{kind} '{id}' in {} frame '{frame_id}' has no {attr}: a {kind} has no content \
             to hug; set {attr}, or set {attr}=\"fill\"",
            mode.as_str()
        ),
        span,
        Some(id.to_owned()),
    )
}

/// `layout.fill_in_hug_parent` (Advisory): a main-axis `fill` child in a
/// frame whose main axis hugs.
pub(super) fn fill_in_hug_parent(child: &Node, frame_id: &str, axis: Axis) -> Diagnostic {
    let (id, span) = child.id_and_span();
    let attr = axis.size_attr();
    Diagnostic::advisory(
        "layout.fill_in_hug_parent",
        format!(
            "{} '{id}': {attr}=\"fill\" sits in frame '{frame_id}', whose {attr} hugs its \
             children, so it takes its content size; set a fixed {attr} on frame \
             '{frame_id}', or set {attr} on '{id}'",
            child.kind_str()
        ),
        span,
        Some(id.to_owned()),
    )
}

/// `layout.child_overflow` (Advisory): laid-out children need more than the
/// frame's fixed content box along `axis`.
pub(super) fn child_overflow(
    frame_id: &str,
    span: Option<zenith_core::Span>,
    axis: Axis,
    need: f64,
    have: f64,
) -> Diagnostic {
    let (name, attr) = match axis {
        Axis::X => ("x", "w"),
        Axis::Y => ("y", "h"),
    };
    Diagnostic::advisory(
        "layout.child_overflow",
        format!(
            "frame '{frame_id}': laid-out children need {}px along {name}, but the content \
             box is {}px; raise {attr}, set wrap=#true, or shrink the children",
            fmt_px(need),
            fmt_px(have)
        ),
        span,
        Some(frame_id.to_owned()),
    )
}

/// `layout.conflicting_size` (Error): a resolved `min-*` above its `max-*`.
pub(super) fn conflicting_min_max(child: &Node, axis: Axis, min: f64, max: f64) -> Diagnostic {
    let (id, span) = child.id_and_span();
    let attr = axis.size_attr();
    Diagnostic::error(
        "layout.conflicting_size",
        format!(
            "node '{id}': min-{attr} {}px is above max-{attr} {}px; lower min-{attr} or \
             raise max-{attr}",
            fmt_px(min),
            fmt_px(max)
        ),
        span,
        Some(id.to_owned()),
    )
}

/// `layout.conflicting_size` (Error): a hugging text whose height depends
/// on its own position kept moving for `passes` layout passes.
pub(super) fn unsettled(id: &str, span: Option<zenith_core::Span>, passes: usize) -> Diagnostic {
    Diagnostic::error(
        "layout.conflicting_size",
        format!(
            "node '{id}': its hug height depends on its own position (text-exclusion \
             runaround or the baseline grid), and layout did not settle after {passes} \
             passes; set h on '{id}'"
        ),
        span,
        Some(id.to_owned()),
    )
}

/// A px value rounded to two decimals for messages.
fn fmt_px(v: f64) -> String {
    let rounded = (v * 100.0).round() / 100.0;
    format!("{rounded}")
}
