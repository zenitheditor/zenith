//! The `overflow` mode of a `text` node and the shared box-clip bracket.
//!
//! One parse of the raw attribute drives every text path (single box, chain
//! member, markdown block stack), so the modes cannot drift apart:
//!
//! - `clip` (also the default when `overflow` is absent, and the reading of an
//!   unrecognized value): overflowing ink is clipped at the box edge and a
//!   `text.overflow` warning names the box size that fits.
//! - `visible`: overflowing ink paints past the box; no diagnostic.
//! - `fit`: the declared size is kept; overflow is the `text.fit_failed` error.
//! - `autofit`: the font shrinks to the largest size in
//!   `[font-size-min, font-size]` that fits; overflow at the floor is the
//!   `text.fit_failed` error.
//!
//! The clip bracket is emitted ONLY when the content overflows, so a node whose
//! content fits emits a byte-identical command stream in every mode.

use crate::ir::SceneCommand;

/// The parsed `overflow` attribute of a `text` node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::compile) enum TextOverflow {
    /// Clip overflowing ink at the box edge and warn (`text.overflow`).
    Clip,
    /// Paint overflowing ink past the box, silently.
    Visible,
    /// Keep the declared size; overflow is a `text.fit_failed` error.
    Fit,
    /// Shrink-to-fit down to the floor; overflow at the floor is a
    /// `text.fit_failed` error.
    Autofit,
}

impl TextOverflow {
    /// Parse the raw attribute. Absent and unrecognized values read as `Clip`
    /// (the validator reports an unrecognized value as `node.invalid_value`).
    pub(in crate::compile) fn from_attr(raw: Option<&str>) -> Self {
        match raw {
            Some("visible") => Self::Visible,
            Some("fit") => Self::Fit,
            Some("autofit") => Self::Autofit,
            Some(_) | None => Self::Clip,
        }
    }
}

/// An axis-aligned box in scene px (`x`, `y`, `w`, `h`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::compile) struct ClipBox {
    pub(in crate::compile) x: f64,
    pub(in crate::compile) y: f64,
    pub(in crate::compile) w: f64,
    pub(in crate::compile) h: f64,
}

/// Wrap every command appended since `start` in a `PushClip(box)` / `PopClip`
/// bracket. `None` leaves the stream untouched (byte-identical).
pub(in crate::compile) fn clip_commands_since(
    commands: &mut Vec<SceneCommand>,
    start: usize,
    clip: Option<ClipBox>,
) {
    let Some(b) = clip else {
        return;
    };
    let draws = commands.split_off(start.min(commands.len()));
    commands.push(SceneCommand::PushClip {
        x: b.x,
        y: b.y,
        w: b.w,
        h: b.h,
    });
    commands.extend(draws);
    commands.push(SceneCommand::PopClip);
}

/// Format a px quantity for a diagnostic: whole values print without a
/// fraction, others with one decimal.
pub(in crate::compile) fn fmt_px(v: f64) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{:.0}", v.round())
    } else {
        format!("{v:.1}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_px_keeps_one_decimal_for_fractions() {
        assert_eq!(fmt_px(19.2), "19.2");
        assert_eq!(fmt_px(64.0), "64");
        assert_eq!(fmt_px(59.98), "60");
    }

    #[test]
    fn absent_and_unknown_read_as_clip() {
        assert_eq!(TextOverflow::from_attr(None), TextOverflow::Clip);
        assert_eq!(TextOverflow::from_attr(Some("clip")), TextOverflow::Clip);
        assert_eq!(TextOverflow::from_attr(Some("scroll")), TextOverflow::Clip);
        assert_eq!(
            TextOverflow::from_attr(Some("visible")),
            TextOverflow::Visible
        );
        assert_eq!(TextOverflow::from_attr(Some("fit")), TextOverflow::Fit);
        assert_eq!(
            TextOverflow::from_attr(Some("autofit")),
            TextOverflow::Autofit
        );
    }

    #[test]
    fn clip_bracket_wraps_only_new_commands() {
        let mut cmds = vec![SceneCommand::PopLayer, SceneCommand::PopTransform];
        clip_commands_since(
            &mut cmds,
            1,
            Some(ClipBox {
                x: 1.0,
                y: 2.0,
                w: 3.0,
                h: 4.0,
            }),
        );
        assert_eq!(cmds.len(), 4);
        assert!(matches!(cmds[0], SceneCommand::PopLayer));
        assert!(matches!(
            cmds[1],
            SceneCommand::PushClip {
                x: 1.0,
                y: 2.0,
                w: 3.0,
                h: 4.0
            }
        ));
        assert!(matches!(cmds[2], SceneCommand::PopTransform));
        assert!(matches!(cmds[3], SceneCommand::PopClip));
    }

    #[test]
    fn no_clip_leaves_stream_untouched() {
        let mut cmds = vec![SceneCommand::PopLayer];
        clip_commands_since(&mut cmds, 0, None);
        assert_eq!(cmds.len(), 1);
    }
}
