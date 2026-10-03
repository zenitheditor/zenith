//! The one builder behind every overflow diagnostic of a `text` node.
//!
//! Single box, shape label, chain member, and markdown stack all word their
//! message the same way per [`TextOverflow`] mode. Only the opening (`who`),
//! the clip wording, the autofit note, and the tail differ per site.

use zenith_core::{Diagnostic, Span};

use super::ink::BlockOverflow;
use super::overflow_mode::{TextOverflow, fmt_px};

/// The parts of an overflow message that differ per call site.
#[derive(Clone, Copy)]
pub(in crate::compile) struct OverflowReport<'a> {
    /// Opens the message (`text 'title'`, `label of shape 'box'`).
    pub(in crate::compile) who: &'a str,
    /// The diagnostic subject id.
    pub(in crate::compile) subject: &'a str,
    pub(in crate::compile) span: Option<Span>,
    /// Clip-mode wording (`clipped at the box edge`).
    pub(in crate::compile) clip_what: &'a str,
    /// Text between `failed` and the colon in `autofit` mode (empty or
    /// ` at its 12px floor`).
    pub(in crate::compile) autofit_note: &'a str,
    /// The measurement and remedy for this mode.
    pub(in crate::compile) tail: &'a str,
}

impl OverflowReport<'_> {
    /// The diagnostic for `mode`. `visible` overflows on purpose: `None`.
    pub(in crate::compile) fn diagnostic(&self, mode: TextOverflow) -> Option<Diagnostic> {
        let Self {
            who,
            clip_what,
            autofit_note,
            tail,
            ..
        } = *self;
        let (error, code, message) = match mode {
            TextOverflow::Visible => return None,
            TextOverflow::Clip => (
                false,
                "text.overflow",
                format!("{who}: {clip_what}: {tail}"),
            ),
            TextOverflow::Fit => (
                true,
                "text.fit_failed",
                format!("{who}: overflow=\"fit\" failed: {tail}"),
            ),
            TextOverflow::Autofit => (
                true,
                "text.fit_failed",
                format!("{who}: overflow=\"autofit\" failed{autofit_note}: {tail}"),
            ),
        };
        let subject = Some(self.subject.to_owned());
        Some(if error {
            Diagnostic::error(code, message, self.span, subject)
        } else {
            Diagnostic::warning(code, message, self.span, subject)
        })
    }
}

/// The tail for a block-axis overflow of a content stack that keeps its sizes
/// (chain member, markdown). `noun` names the content (`chain content`);
/// `below_remedy` follows the `set h=` fix when the ink runs below the box.
pub(in crate::compile) fn block_tail(
    noun: &str,
    o: BlockOverflow,
    box_h: f64,
    below_remedy: &str,
) -> String {
    if o.below {
        format!(
            "{noun} needs {:.0}px height in a {}px box — set h=(px){:.0}, {below_remedy}",
            o.need_h,
            fmt_px(box_h),
            o.need_h
        )
    } else {
        format!(
            "{noun} rises {:.0}px above the top of a {}px box — set overflow=\"visible\"",
            o.rise,
            fmt_px(box_h)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(tail: &str) -> OverflowReport<'_> {
        OverflowReport {
            who: "text 'a'",
            subject: "a",
            span: None,
            clip_what: "clipped at the box edge",
            autofit_note: " (kept)",
            tail,
        }
    }

    #[test]
    fn modes_word_the_message() {
        let r = report("tail");
        assert!(r.diagnostic(TextOverflow::Visible).is_none());
        let clip = r.diagnostic(TextOverflow::Clip).expect("clip");
        assert_eq!(clip.code, "text.overflow");
        assert_eq!(clip.message, "text 'a': clipped at the box edge: tail");
        let fit = r.diagnostic(TextOverflow::Fit).expect("fit");
        assert_eq!(fit.message, "text 'a': overflow=\"fit\" failed: tail");
        let auto = r.diagnostic(TextOverflow::Autofit).expect("autofit");
        assert_eq!(auto.code, "text.fit_failed");
        assert_eq!(
            auto.message,
            "text 'a': overflow=\"autofit\" failed (kept): tail"
        );
    }

    #[test]
    fn block_tail_below_and_above() {
        let below = BlockOverflow {
            need_h: 90.0,
            below: true,
            rise: 0.0,
        };
        assert_eq!(
            block_tail("chain content", below, 60.0, "add a box"),
            "chain content needs 90px height in a 60px box — set h=(px)90, add a box"
        );
        let above = BlockOverflow {
            need_h: 0.0,
            below: false,
            rise: 4.0,
        };
        assert_eq!(
            block_tail("chain content", above, 60.0, "x"),
            "chain content rises 4px above the top of a 60px box — set overflow=\"visible\""
        );
    }
}
