//! Contrast of `shape` and `connector` labels, judged on drawn geometry.
//!
//! The scene compiler measures each label after it draws it: the glyph ink
//! box, the run colours, the run font size, and the label's rotation. This
//! module samples the backdrop over that ink box the same way a text node's
//! glyph ink is sampled. Labels carry no `contrast-bg` hint, so a label over
//! an unsampled backdrop (image, effect) is skipped.

use crate::ast::node::Node;
use crate::diagnostics::Diagnostic;

use super::geometry::{RectPx, Rotation};
use super::ink::sample_points;
use super::props::{font_weight_of, style_property};
use super::text::{lc_threshold, select_contrast_sample};
use super::types::{BackdropCandidate, ContrastEnv, ContrastSample, INVISIBLE_LC_FLOOR, PaintCtx};

/// The measured ink of one drawn label, in page px (trim-box origin).
#[derive(Debug, Clone, PartialEq)]
pub struct LabelInk {
    /// Ink box left edge, before `rotation`.
    pub x: f64,
    /// Ink box top edge, before `rotation`.
    pub y: f64,
    /// Ink box width.
    pub w: f64,
    /// Ink box height.
    pub h: f64,
    /// Rotation `(angle_deg, cx, cy)` the label draws under, if any.
    pub rotation: Option<(f64, f64, f64)>,
    /// Distinct fill colours of the label's glyph runs.
    pub colors: Vec<(u8, u8, u8)>,
    /// Largest glyph-run font size, in px.
    pub font_size_px: f64,
}

/// Judge the label of `node` when the label pass measured one for it.
pub(super) fn check_label(
    node: &Node,
    ctx: PaintCtx<'_>,
    candidates: &[BackdropCandidate],
    env: ContrastEnv<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let (kind, id, span, text_style) = match node {
        Node::Shape(s) => ("shape", &s.id, s.source_span, s.text_style.as_deref()),
        Node::Connector(c) => ("connector", &c.id, c.source_span, c.text_style.as_deref()),
        Node::Rect(_)
        | Node::Ellipse(_)
        | Node::Line(_)
        | Node::Text(_)
        | Node::Code(_)
        | Node::Frame(_)
        | Node::Group(_)
        | Node::Image(_)
        | Node::Polygon(_)
        | Node::Polyline(_)
        | Node::Path(_)
        | Node::Instance(_)
        | Node::Field(_)
        | Node::Footnote(_)
        | Node::Toc(_)
        | Node::Table(_)
        | Node::Pattern(_)
        | Node::Chart(_)
        | Node::Light(_)
        | Node::Mesh(_)
        | Node::Unknown(_) => return,
    };
    let Some(ink) = env.inks.and_then(|inks| inks.labels.get(id.as_str())) else {
        return;
    };
    let ink_box = RectPx {
        x: ink.x,
        y: ink.y,
        w: ink.w,
        h: ink.h,
    };
    // Sample the rotated ink: each sample point turns with the label.
    let turn = ink.rotation.map(|(angle_deg, cx, cy)| Rotation {
        angle_deg: -angle_deg,
        cx,
        cy,
    });
    let points = ink_box.sample_points().map(|(x, y)| match turn {
        Some(turn) => turn.inverse_map(x, y),
        None => (x, y),
    });
    let (samples, indeterminate, hidden) =
        sample_points(points, ctx.clip, candidates, ctx.page_bg_rgb);
    if indeterminate || hidden {
        return;
    }
    let mut worst: Option<ContrastSample> = None;
    for rgb in &ink.colors {
        if let Some(sample) = select_contrast_sample(*rgb, None, &samples, ctx.page_bg_rgb)
            && worst.is_none_or(|w| sample.lc < w.lc)
        {
            worst = Some(sample);
        }
    }
    let Some(sample) = worst else {
        return;
    };
    let weight = font_weight_of(
        style_property(text_style, "font-weight", env.style_map),
        env.resolved_tokens,
    );
    let size = ink.font_size_px;
    let threshold = lc_threshold(size, weight);
    if sample.lc >= threshold {
        return;
    }
    let subject = format!("{kind} '{id}' label");
    let fix = "set a label fill with more contrast through the `text-style` fill or a span `fill`";
    let diagnostic = if sample.lc < INVISIBLE_LC_FLOOR {
        Diagnostic::warning(
            "contrast.invisible",
            format!(
                "{subject}: APCA contrast Lc {:.1} is effectively invisible against {} (Lc below {:.0}); {fix}",
                sample.lc, sample.source, INVISIBLE_LC_FLOOR
            ),
            span,
            Some(id.clone()),
        )
    } else {
        Diagnostic::advisory(
            "contrast.low",
            format!(
                "{subject}: APCA contrast Lc {:.1} against {} is below the WCAG 3 draft threshold (Lc {:.0}); {fix}",
                sample.lc, sample.source, threshold
            ),
            span,
            Some(id.clone()),
        )
    };
    diagnostics.push(diagnostic);
}
