//! Overflow measurement and diagnostics for a sized `text` node.
//!
//! [`measure_overflow`] decides WHETHER the drawn content overflows its box
//! (the one predicate behind the clip bracket, the `text.overflow` warning, the
//! `text.fit_failed` error, and the autofit search). The block axis compares
//! the glyph INK against the box, so a line box taller than the box does not
//! overflow while the glyphs fit. The inline axis flags one unbreakable line
//! wider than the box. [`overflow_diagnostic`] turns the measured facts into
//! the per-mode message, naming the box size that fits at the laid-out font
//! size and the largest font size that fits the box.

use zenith_core::{Diagnostic, Span};

use crate::compile::text::ink::{BlockOverflow, EPSILON, InkRect, block_overflow};
use crate::compile::text::overflow_mode::{TextOverflow, fmt_px};

/// Inputs to [`measure_overflow`] — the drawn ink and the box.
pub(super) struct OverflowCheck {
    /// Resolved box width in px, when available.
    pub box_w_opt: Option<f64>,
    /// Resolved box height in px, when available.
    pub box_h_opt: Option<f64>,
    /// Box top in scene px.
    pub box_y: f64,
    /// Ink bounds of the node's draws (`None` when nothing paints).
    pub ink: Option<InkRect>,
    /// Line count after emit (1 on the fast path, wrapped count otherwise).
    pub fit_line_count: usize,
    /// Whether the wrap path was taken.
    pub needs_wrap: bool,
    /// Total single-line advance width in px.
    pub total_advance: f64,
    /// Resolved node font size in px.
    pub font_size: f32,
}

/// The measured overflow of a node whose content does NOT fit its box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct OverflowFacts {
    /// Laid-out line count.
    pub line_count: usize,
    /// Font size the content was laid out at, in px.
    pub font_size: f64,
    /// Single-line content width, in px.
    pub content_w: f64,
    /// Box width, in px.
    pub box_w: f64,
    /// Box height, in px.
    pub box_h: f64,
    /// Ink leaves the box along the block axis.
    pub block: Option<BlockOverflow>,
    /// A single unbreakable line is wider than the box.
    pub width_overflow: bool,
}

impl OverflowFacts {
    /// The facts as seen from a host box `pad` px larger on every side (a
    /// shape around its label box): box and needed sizes grow by `2 × pad`.
    pub(super) fn padded(self, pad: f64) -> Self {
        let grow = 2.0 * pad;
        Self {
            content_w: self.content_w + grow,
            box_w: self.box_w + grow,
            box_h: self.box_h + grow,
            block: self.block.map(|b| BlockOverflow {
                need_h: b.need_h + grow,
                ..b
            }),
            ..self
        }
    }
}

/// What a sized compile produced: the laid-out content height and, when the
/// content overflows a complete box, the measured overflow.
pub(super) struct SizedOutcome {
    /// Laid-out content height in px (the flow-layout advance).
    pub height: f64,
    /// `Some` only when both box dimensions resolve and the content overflows.
    pub overflow: Option<OverflowFacts>,
}

impl SizedOutcome {
    /// An outcome with no overflow facts (early returns and the branches that
    /// own their overflow handling: chain member, markdown, tab leader).
    pub(super) fn plain(height: f64) -> Self {
        Self {
            height,
            overflow: None,
        }
    }
}

/// Measure whether the drawn content overflows its box.
///
/// Evaluated only when BOTH box dimensions resolve; a partial box cannot
/// overflow. Block axis: the ink extends below the box bottom or above the box
/// top. Inline axis: the wrap path produced ONE line wider than the box, which
/// happens only when no break point exists (the greedy packer splits any line
/// it can). The fast path never overflows inline (it is taken only when the
/// line fits).
pub(super) fn measure_overflow(c: OverflowCheck) -> Option<OverflowFacts> {
    let (box_w, box_h) = (c.box_w_opt?, c.box_h_opt?);
    let block = block_overflow(c.ink, c.box_y, box_h);
    let width_overflow = c.needs_wrap && c.fit_line_count == 1 && c.total_advance > box_w + EPSILON;
    (block.is_some() || width_overflow).then_some(OverflowFacts {
        line_count: c.fit_line_count,
        font_size: f64::from(c.font_size),
        content_w: c.total_advance,
        box_w,
        box_h,
        block,
        width_overflow,
    })
}

/// Build the diagnostic for an overflowing node in `mode`. `who` opens the
/// message (`text 'title'`); `id` is the subject.
///
/// `fitting_px` is the largest integer font size below the laid-out size that
/// fits the box (from the autofit search), or `None` when no size fits.
/// `overflow="visible"` overflows on purpose and yields no diagnostic.
pub(super) fn overflow_diagnostic(
    who: &str,
    id: &str,
    span: Option<Span>,
    mode: TextOverflow,
    facts: &OverflowFacts,
    fitting_px: Option<i64>,
) -> Option<Diagnostic> {
    let need = need_clause(facts);
    let remedy = set_clause(facts);
    let subject = Some(id.to_owned());
    match mode {
        TextOverflow::Visible => None,
        TextOverflow::Clip => {
            let keep = if remedy.starts_with("set w=") || remedy.starts_with("set h=") {
                " to keep the type scale"
            } else {
                ""
            };
            Some(Diagnostic::warning(
                "text.overflow",
                format!(
                    "{who}: clipped at the box edge: {need} — {remedy}{keep}{}",
                    font_clause("font-size", fitting_px)
                ),
                span,
                subject,
            ))
        }
        TextOverflow::Fit => Some(Diagnostic::error(
            "text.fit_failed",
            format!(
                "{who}: overflow=\"fit\" failed: {need} — {remedy}{}",
                font_clause("font-size", fitting_px)
            ),
            span,
            subject,
        )),
        TextOverflow::Autofit => Some(Diagnostic::error(
            "text.fit_failed",
            format!(
                "{who}: overflow=\"autofit\" failed at its {}px floor: {need} — {remedy}{}",
                fmt_px(facts.font_size),
                font_clause("font-size-min", fitting_px)
            ),
            span,
            subject,
        )),
    }
}

/// `3 lines at 64px need 262px height in a 60px box` (and the width / rise
/// variants).
fn need_clause(f: &OverflowFacts) -> String {
    let one = f.line_count == 1;
    let lines = if one {
        "1 line".to_owned()
    } else {
        format!("{} lines", f.line_count)
    };
    let (verb, rise_verb) = if one {
        ("needs", "rises")
    } else {
        ("need", "rise")
    };
    let fs = fmt_px(f.font_size);
    let need_w = f.content_w.ceil();
    let (bw, bh) = (fmt_px(f.box_w), fmt_px(f.box_h));
    let below = f.block.filter(|b| b.below).map(|b| b.need_h);
    let rise = f.block.map_or(0.0, |b| b.rise);
    let head = match (f.width_overflow, below) {
        (true, Some(h)) => format!(
            "{lines} at {fs}px {verb} {need_w:.0}px width and {h:.0}px height in a {bw}x{bh}px box"
        ),
        (true, None) => {
            format!("{lines} at {fs}px {verb} {need_w:.0}px width in a {bw}px-wide box")
        }
        (false, Some(h)) => format!("{lines} at {fs}px {verb} {h:.0}px height in a {bh}px box"),
        (false, None) => {
            return format!(
                "{lines} at {fs}px {rise_verb} {rise:.0}px above the top of a {bh}px box"
            );
        }
    };
    if rise > 0.0 {
        format!("{head}, and the ink rises {rise:.0}px above the box top")
    } else {
        head
    }
}

/// `set h=(px)262` (and the width variants) — the box that fits at this size.
/// Ink that only rises above the box top has no box fix: paint it past the box.
fn set_clause(f: &OverflowFacts) -> String {
    let need_w = f.content_w.ceil();
    let below = f.block.filter(|b| b.below).map(|b| b.need_h);
    match (f.width_overflow, below) {
        (true, Some(h)) => format!("set w=(px){need_w:.0} h=(px){h:.0}"),
        (true, None) => format!("set w=(px){need_w:.0}"),
        (false, Some(h)) => format!("set h=(px){h:.0}"),
        (false, None) => "set overflow=\"visible\"".to_owned(),
    }
}

/// `, or font-size ≤ 18px fits`; empty when no font size fits the box.
fn font_clause(attr: &str, fitting_px: Option<i64>) -> String {
    fitting_px.map_or_else(String::new, |px| format!(", or {attr} ≤ {px}px fits"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(lines: usize, w_over: bool, need_h: Option<f64>, rise: f64) -> OverflowFacts {
        OverflowFacts {
            line_count: lines,
            font_size: 64.0,
            content_w: 299.2,
            box_w: 120.0,
            box_h: 60.0,
            block: (need_h.is_some() || rise > 0.0).then(|| BlockOverflow {
                need_h: need_h.unwrap_or(60.0),
                below: need_h.is_some(),
                rise,
            }),
            width_overflow: w_over,
        }
    }

    fn ink(top: f64, bottom: f64) -> Option<InkRect> {
        Some(InkRect {
            left: 0.0,
            top,
            right: 50.0,
            bottom,
        })
    }

    fn check(ink: Option<InkRect>, lines: usize, needs_wrap: bool, adv: f64) -> OverflowCheck {
        OverflowCheck {
            box_w_opt: Some(100.0),
            box_h_opt: Some(40.0),
            box_y: 10.0,
            ink,
            fit_line_count: lines,
            needs_wrap,
            total_advance: adv,
            font_size: 16.0,
        }
    }

    #[test]
    fn measure_requires_a_complete_box() {
        let mut c = check(ink(10.0, 90.0), 3, true, 250.0);
        c.box_h_opt = None;
        assert!(measure_overflow(c).is_none());
    }

    #[test]
    fn measure_uses_ink_not_line_boxes() {
        // Ink inside the box fits, however many line boxes were laid out.
        assert!(measure_overflow(check(ink(12.0, 49.0), 2, true, 150.0)).is_none());
        let tall = measure_overflow(check(ink(12.0, 70.2), 3, true, 250.0));
        assert!(tall.is_some_and(|f| f.block.is_some_and(|b| b.below && b.need_h == 61.0)));
        let wide = measure_overflow(check(ink(12.0, 30.0), 1, true, 250.0));
        assert!(wide.is_some_and(|f| f.width_overflow && f.block.is_none()));
    }

    #[test]
    fn clip_message_names_fitting_height_and_size() {
        let d = overflow_diagnostic(
            "text 'title'",
            "title",
            None,
            TextOverflow::Clip,
            &facts(3, false, Some(262.0), 0.0),
            Some(18),
        )
        .expect("clip overflow warns");
        assert_eq!(d.code, "text.overflow");
        assert_eq!(d.severity, zenith_core::Severity::Warning);
        assert_eq!(
            d.message,
            "text 'title': clipped at the box edge: 3 lines at 64px need 262px height \
             in a 60px box — set h=(px)262 to keep the type scale, or font-size ≤ 18px fits"
        );
        assert_eq!(d.subject_id.as_deref(), Some("title"));
    }

    #[test]
    fn fit_and_autofit_messages() {
        let fit = overflow_diagnostic(
            "text 'title'",
            "title",
            None,
            TextOverflow::Fit,
            &facts(1, true, None, 0.0),
            None,
        )
        .expect("fit overflow errors");
        assert_eq!(fit.code, "text.fit_failed");
        assert_eq!(fit.severity, zenith_core::Severity::Error);
        assert_eq!(
            fit.message,
            "text 'title': overflow=\"fit\" failed: 1 line at 64px needs 300px width \
             in a 120px-wide box — set w=(px)300"
        );
        let auto = overflow_diagnostic(
            "text 'title'",
            "title",
            None,
            TextOverflow::Autofit,
            &facts(2, true, Some(262.0), 0.0),
            Some(9),
        )
        .expect("autofit overflow errors");
        assert_eq!(
            auto.message,
            "text 'title': overflow=\"autofit\" failed at its 64px floor: 2 lines at 64px \
             need 300px width and 262px height in a 120x60px box — set w=(px)300 h=(px)262, \
             or font-size-min ≤ 9px fits"
        );
    }

    #[test]
    fn rise_above_the_top_suggests_visible() {
        let d = overflow_diagnostic(
            "text 'title'",
            "title",
            None,
            TextOverflow::Clip,
            &facts(1, false, None, 4.0),
            Some(50),
        )
        .expect("rise warns");
        assert_eq!(
            d.message,
            "text 'title': clipped at the box edge: 1 line at 64px rises 4px above the top \
             of a 60px box — set overflow=\"visible\", or font-size ≤ 50px fits"
        );
    }

    #[test]
    fn visible_is_silent() {
        assert!(
            overflow_diagnostic(
                "text 'title'",
                "title",
                None,
                TextOverflow::Visible,
                &facts(3, false, Some(262.0), 0.0),
                Some(1)
            )
            .is_none()
        );
    }
}
