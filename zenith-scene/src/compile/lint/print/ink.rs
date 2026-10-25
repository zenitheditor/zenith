//! `ink.rich_black` and `ink.total_coverage` over the CMYK colors one print
//! page draws.
//!
//! Only CMYK-origin colors count (a `cmyk(c,m,y,k)` token). An sRGB color
//! has no plate values, so it is skipped.
//!
//! - `ink.total_coverage`: a fill, stroke, text, or gradient stop color with
//!   C+M+Y+K over 300%. Reported once per node, role, and color.
//! - `ink.rich_black`: text drawn under 24 px (18 pt) whose fill has K of
//!   60% or more and two or more of C, M, Y above 0. Two inks or more on a
//!   thin stroke can misregister. Reported once per node. The drawn size
//!   counts every ancestor instance fit scale.

use std::collections::BTreeSet;

use zenith_core::Diagnostic;

use super::fmt::{fmt_cmyk, fmt_num};
use super::owner::{Owners, Subject, subject};
use super::run::PrintInput;
use crate::ir::{Color, Paint, SceneCommand};

/// The total ink limit, in percent of one plate.
pub(super) const MAX_TOTAL_INK: f32 = 300.0;
/// Text drawn under this size, in px (18 pt), is small text.
pub(super) const SMALL_TEXT_PX: f64 = 24.0;
/// A fill with at least this much K, in percent, reads as black.
pub(super) const RICH_BLACK_MIN_K: f32 = 60.0;

/// What a color paints in a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Role {
    Fill,
    GradientStop,
    Stroke,
    Text,
    TextStroke,
}

impl Role {
    fn as_str(self) -> &'static str {
        match self {
            Role::Fill => "fill",
            Role::GradientStop => "gradient stop",
            Role::Stroke => "stroke",
            Role::Text => "text fill",
            Role::TextStroke => "text stroke",
        }
    }
}

/// Every `ink.*` diagnostic of the page, in command order.
pub(super) fn ink(input: &PrintInput<'_, '_>, owners: &Owners) -> Vec<Diagnostic> {
    let mut coverage_seen: BTreeSet<(String, Role, String)> = BTreeSet::new();
    let mut black_seen: BTreeSet<String> = BTreeSet::new();
    let mut out = Vec::new();
    for (index, command) in input.env.commands.iter().enumerate() {
        let colors = colors_of(command);
        if colors.iter().all(|(_, c)| c.cmyk.is_none()) {
            continue;
        }
        let subject = subject(input, owners, index, command);
        for (role, color) in colors {
            let Some(cmyk) = color.cmyk else {
                continue;
            };
            if total(cmyk) > MAX_TOTAL_INK
                && coverage_seen.insert((subject.id.to_owned(), role, fmt_cmyk(cmyk)))
            {
                out.push(coverage(&subject, role, cmyk));
            }
        }
        if let SceneCommand::DrawGlyphRun {
            font_size, color, ..
        } = command
            && let Some(cmyk) = color.cmyk
        {
            let size = f64::from(*font_size) * subject.scale;
            if is_rich_black(cmyk)
                && size < SMALL_TEXT_PX
                && black_seen.insert(subject.id.to_owned())
            {
                out.push(rich_black(&subject, size, cmyk));
            }
        }
    }
    out
}

/// The colors `command` paints, with their roles.
fn colors_of(command: &SceneCommand) -> Vec<(Role, Color)> {
    let paint = |p: &Paint| match p {
        Paint::Solid { color } => vec![(Role::Fill, *color)],
        Paint::Gradient(g) => g
            .stops
            .iter()
            .map(|s| (Role::GradientStop, s.color))
            .collect(),
    };
    match command {
        SceneCommand::FillRect { paint: p, .. }
        | SceneCommand::FillRoundedRect { paint: p, .. }
        | SceneCommand::FillEllipse { paint: p, .. }
        | SceneCommand::FillPolygon { paint: p, .. }
        | SceneCommand::FillPath { paint: p, .. } => paint(p),
        SceneCommand::StrokeRect { color, .. }
        | SceneCommand::StrokeRoundedRect { color, .. }
        | SceneCommand::StrokeEllipse { color, .. }
        | SceneCommand::StrokeLine { color, .. }
        | SceneCommand::StrokePolyline { color, .. }
        | SceneCommand::StrokePath { color, .. } => vec![(Role::Stroke, *color)],
        SceneCommand::DrawGlyphRun {
            color,
            stroke_color,
            ..
        } => {
            let mut out = vec![(Role::Text, *color)];
            out.extend(stroke_color.map(|c| (Role::TextStroke, c)));
            out
        }
        SceneCommand::DrawImage { .. }
        | SceneCommand::DrawSvgAsset { .. }
        | SceneCommand::PushClip { .. }
        | SceneCommand::PushClipRoundedRect { .. }
        | SceneCommand::PopClip
        | SceneCommand::PushLayer { .. }
        | SceneCommand::PopLayer
        | SceneCommand::PushTransform { .. }
        | SceneCommand::PushScaleTranslate { .. }
        | SceneCommand::PushTransformMatrix { .. }
        | SceneCommand::PopTransform
        | SceneCommand::BeginShadow { .. }
        | SceneCommand::EndShadow
        | SceneCommand::BeginBlur { .. }
        | SceneCommand::EndBlur
        | SceneCommand::BeginFilter { .. }
        | SceneCommand::EndFilter
        | SceneCommand::BeginMask { .. }
        | SceneCommand::EndMask => Vec::new(),
    }
}

/// C+M+Y+K, in percent.
fn total(cmyk: [f32; 4]) -> f32 {
    cmyk.iter().sum()
}

/// K at [`RICH_BLACK_MIN_K`] or more, and two or more of C, M, Y above 0.
fn is_rich_black(cmyk: [f32; 4]) -> bool {
    let [c, m, y, k] = cmyk;
    let inks = [c, m, y].iter().filter(|v| **v > 0.0).count();
    k >= RICH_BLACK_MIN_K && inks >= 2
}

/// The color with C, M, Y scaled down so the total is at most
/// [`MAX_TOTAL_INK`], K kept. Channels round down to whole percent.
fn within_limit(cmyk: [f32; 4]) -> [f32; 4] {
    let [c, m, y, k] = cmyk;
    let cmy = c + m + y;
    if cmy <= 0.0 {
        return cmyk;
    }
    let gain = ((MAX_TOTAL_INK - k) / cmy).clamp(0.0, 1.0);
    [
        (c * gain).floor(),
        (m * gain).floor(),
        (y * gain).floor(),
        k,
    ]
}

fn coverage(subject: &Subject<'_>, role: Role, cmyk: [f32; 4]) -> Diagnostic {
    let fixed = within_limit(cmyk);
    let message = format!(
        "{} {} {} totals {}% ink > {}% limit — lower C, M, or Y, for example {} at {}%",
        subject.name,
        role.as_str(),
        fmt_cmyk(cmyk),
        fmt_num(f64::from(total(cmyk))),
        fmt_num(f64::from(MAX_TOTAL_INK)),
        fmt_cmyk(fixed),
        fmt_num(f64::from(total(fixed))),
    );
    Diagnostic::warning(
        "ink.total_coverage",
        message,
        subject.span,
        Some(subject.id.to_owned()),
    )
}

fn rich_black(subject: &Subject<'_>, size: f64, cmyk: [f32; 4]) -> Diagnostic {
    let message = format!(
        "{} is {}px rich black {} under the {}px (18pt) floor — fill it with K only, \
         cmyk(0,0,0,100), or set font-size to at least {}px",
        subject.name,
        fmt_num(size),
        fmt_cmyk(cmyk),
        fmt_num(SMALL_TEXT_PX),
        fmt_num(SMALL_TEXT_PX),
    );
    Diagnostic::warning(
        "ink.rich_black",
        message,
        subject.span,
        Some(subject.id.to_owned()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rich_black_needs_heavy_k_and_two_inks() {
        assert!(is_rich_black([60.0, 40.0, 40.0, 100.0]));
        assert!(!is_rich_black([0.0, 0.0, 0.0, 100.0]), "K only");
        assert!(!is_rich_black([0.0, 0.0, 50.0, 100.0]), "one non-K ink");
        assert!(!is_rich_black([60.0, 40.0, 0.0, 20.0]), "light K: a color");
    }

    #[test]
    fn total_and_fix_stay_within_the_limit() {
        let heavy = [80.0, 70.0, 70.0, 100.0];
        assert_eq!(total(heavy), 320.0);
        let fixed = within_limit(heavy);
        assert_eq!(fixed[3], 100.0, "K is kept");
        assert!(total(fixed) <= MAX_TOTAL_INK, "{fixed:?}");
        assert_eq!(within_limit([0.0, 0.0, 0.0, 100.0]), [0.0, 0.0, 0.0, 100.0]);
    }

    #[test]
    fn colors_of_reads_text_fill_and_stroke() {
        let black = Color::cmyk(60.0, 40.0, 40.0, 100.0, 0, 0, 0);
        let run = SceneCommand::DrawGlyphRun {
            x: 0.0,
            y: 0.0,
            font_id: "f".to_owned(),
            font_size: 12.0,
            color: black,
            stroke_color: Some(black),
            stroke_width: Some(1.0),
            link: None,
            selectable: true,
            source_node_id: None,
            glyphs: Vec::new(),
        };
        let roles: Vec<Role> = colors_of(&run).into_iter().map(|(r, _)| r).collect();
        assert_eq!(roles, vec![Role::Text, Role::TextStroke]);
        assert!(colors_of(&SceneCommand::PopClip).is_empty());
    }
}
