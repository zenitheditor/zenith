//! The fast single-line emit: each shaped span draws its background rects,
//! decorations, then its glyph run, left to right from the aligned origin.
//!
//! Taken when the whole line fits the box (or there is no box width). Shaping
//! a span whole differs glyph-for-glyph from shaping its words apart, so this
//! path stays exact for every fitting node.

use zenith_layout::TextDirection;

use crate::compile::text::shape::{CODE_BG, run_to_scene_glyphs};
use crate::ir::{Paint, SceneCommand};

use super::spans::{ShapedSet, ShapedSpan};
use super::style::{BoxLayout, NodeStyle};

/// Where one span run sits: left edge, baseline, and advance width.
#[derive(Clone, Copy)]
struct RunPlace {
    x: f64,
    baseline_y: f64,
    advance: f64,
}

/// Emit every span of `set` as one line.
pub(super) fn emit_single_line(
    style: &NodeStyle,
    layout: BoxLayout,
    set: ShapedSet,
    commands: &mut Vec<SceneCommand>,
) {
    let ShapedSet {
        mut spans,
        total_advance,
        node_ascent,
    } = set;
    let is_rtl = style.direction == TextDirection::Rtl;
    let mut x_cursor = layout.text_x + x_offset(layout, total_advance, is_rtl);

    // RTL: reverse the span order so the first logical span sits rightmost. Each
    // run is already in visual RTL order from the shaper.
    if is_rtl {
        spans.reverse();
    }
    for shaped in spans {
        let advance = shaped.run.advance_width as f64;
        // A super/subscript span sits on the shared full-size baseline plus its
        // shift. A plain span keeps its own run ascent. Without a full-size span,
        // fall back to the span's own ascent.
        let baseline_y = if shaped.look.vertical_align {
            layout.text_y
                + node_ascent.unwrap_or(shaped.run.ascent as f64)
                + shaped.look.baseline_dy
        } else {
            layout.text_y + shaped.run.ascent as f64
        };
        let place = RunPlace {
            x: x_cursor,
            baseline_y,
            advance,
        };
        emit_backgrounds(&shaped, place, commands);
        emit_decorations(&shaped, place, layout.deco_thickness, commands);
        emit_glyphs(style, shaped, place, commands);
        x_cursor += advance;
    }
}

/// The line's x offset from the box left for the alignment.
///
/// LTR `start` is the origin. Under RTL the anchor flips: `start` right-anchors,
/// `end` left-anchors, and `center` is symmetric. With no box width the line
/// start-anchors at the origin.
fn x_offset(layout: BoxLayout, total_advance: f64, is_rtl: bool) -> f64 {
    let Some(box_w) = layout.box_w else {
        return 0.0;
    };
    let free = box_w - total_advance;
    match (layout.align, is_rtl) {
        ("center", _) => free / 2.0,
        ("end", false) => free,
        ("end", true) => 0.0,
        // `start` / `justify` / unknown: a single fitting line is start-aligned.
        (_, false) => 0.0,
        (_, true) => free,
    }
}

/// Highlight (author color) and code (internal `CODE_BG`) rects. They come
/// first so glyphs and decorations paint on top.
fn emit_backgrounds(shaped: &ShapedSpan, p: RunPlace, commands: &mut Vec<SceneCommand>) {
    let top = p.baseline_y - shaped.run.ascent as f64;
    let height = (shaped.run.ascent + shaped.run.descent) as f64;
    let mut rect = |paint: Paint| {
        commands.push(SceneCommand::FillRect {
            x: p.x,
            y: top,
            w: p.advance,
            h: height,
            paint,
        });
    };
    if let Some(hl_color) = shaped.look.highlight {
        rect(Paint::solid(hl_color));
    }
    if shaped.look.code {
        rect(Paint::solid(CODE_BG));
    }
}

/// Underline and strikethrough rules in the span color, across the run
/// advance. Position derives from the span's own font size. They come before
/// the glyphs so the text sits on top.
fn emit_decorations(
    shaped: &ShapedSpan,
    p: RunPlace,
    thickness: f64,
    commands: &mut Vec<SceneCommand>,
) {
    let look = &shaped.look;
    let mut rule = |y: f64| {
        commands.push(SceneCommand::FillRect {
            x: p.x,
            y,
            w: p.advance,
            h: thickness,
            paint: Paint::solid(look.color),
        });
    };
    if look.underline {
        rule(p.baseline_y + look.font_size as f64 * 0.12);
    }
    if look.strikethrough {
        rule(p.baseline_y - look.font_size as f64 * 0.30);
    }
}

/// The span's glyph run.
fn emit_glyphs(
    style: &NodeStyle,
    shaped: ShapedSpan,
    p: RunPlace,
    commands: &mut Vec<SceneCommand>,
) {
    let glyphs = run_to_scene_glyphs(&shaped.run);
    commands.push(SceneCommand::DrawGlyphRun {
        x: p.x,
        y: p.baseline_y,
        font_id: shaped.run.font_id,
        font_size: shaped.run.font_size,
        color: shaped.look.color,
        stroke_color: style.glyph_stroke.0,
        stroke_width: style.glyph_stroke.1,
        link: shaped.look.link,
        selectable: true,
        source_node_id: Some(style.text.id.clone()),
        glyphs,
    });
}
