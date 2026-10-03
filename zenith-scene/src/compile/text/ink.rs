//! Ink bounds of drawn text and the block-axis overflow they imply.
//!
//! Text overflows its box when its INK leaves the box, not when its line box
//! does: a single-line label whose line height exceeds the box but whose
//! glyphs fit paints nothing outside the box. The ink of a glyph is its font
//! bounding box (from `zenith-layout`), placed at the glyph's pen position.

use zenith_layout::TextLayoutEngine;

use crate::ir::SceneCommand;

use super::ctx::ShapeEnv;

/// Tolerance in px before ink counts as leaving the box.
pub(in crate::compile) const EPSILON: f64 = 0.5;

/// The union of painted extents, in scene px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::compile) struct InkRect {
    pub(in crate::compile) left: f64,
    pub(in crate::compile) top: f64,
    pub(in crate::compile) right: f64,
    pub(in crate::compile) bottom: f64,
}

impl InkRect {
    fn union(self, o: InkRect) -> InkRect {
        InkRect {
            left: self.left.min(o.left),
            top: self.top.min(o.top),
            right: self.right.max(o.right),
            bottom: self.bottom.max(o.bottom),
        }
    }
}

/// The ink bounds of the glyph runs and filled rects (highlights, code
/// backgrounds, decorations) in `cmds`. A stroked run grows by half its stroke
/// width. `None` when nothing in `cmds` paints.
pub(in crate::compile) fn ink_bounds(cmds: &[SceneCommand], env: ShapeEnv) -> Option<InkRect> {
    let mut acc: Option<InkRect> = None;
    let mut add = |r: InkRect| acc = Some(acc.map_or(r, |a| a.union(r)));
    for cmd in cmds {
        if let SceneCommand::DrawGlyphRun {
            x,
            y,
            font_id,
            font_size,
            stroke_color,
            stroke_width,
            glyphs,
            ..
        } = cmd
        {
            let grow = match (stroke_color, stroke_width) {
                (Some(_), Some(w)) if *w > 0.0 => w / 2.0,
                _ => 0.0,
            };
            for g in glyphs {
                let Some(b) = env
                    .engine
                    .glyph_ink_box(font_id, g.glyph_id, *font_size, env.fonts)
                else {
                    continue;
                };
                let (gx, gy) = (x + f64::from(g.dx), y + f64::from(g.dy));
                add(InkRect {
                    left: gx + f64::from(b.x_min) - grow,
                    top: gy + f64::from(b.y_min) - grow,
                    right: gx + f64::from(b.x_max) + grow,
                    bottom: gy + f64::from(b.y_max) + grow,
                });
            }
        } else if let SceneCommand::FillRect { x, y, w, h, .. } = cmd {
            add(InkRect {
                left: *x,
                top: *y,
                right: x + w,
                bottom: y + h,
            });
        }
    }
    acc
}

/// How ink leaves a box along the block axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::compile) struct BlockOverflow {
    /// Box height that holds the ink below the box top: `ceil(ink.bottom - box_y)`.
    pub(in crate::compile) need_h: f64,
    /// Ink extends below the box bottom.
    pub(in crate::compile) below: bool,
    /// Px the ink rises above the box top (`0` when it does not).
    pub(in crate::compile) rise: f64,
}

/// The block-axis overflow of `ink` against the box `[box_y, box_y + box_h]`,
/// or `None` when the ink fits (or nothing paints).
pub(in crate::compile) fn block_overflow(
    ink: Option<InkRect>,
    box_y: f64,
    box_h: f64,
) -> Option<BlockOverflow> {
    let ink = ink?;
    let below = ink.bottom > box_y + box_h + EPSILON;
    let above = ink.top < box_y - EPSILON;
    (below || above).then(|| BlockOverflow {
        need_h: (ink.bottom - box_y).ceil().max(box_h),
        below,
        rise: if above { (box_y - ink.top).ceil() } else { 0.0 },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ink(top: f64, bottom: f64) -> Option<InkRect> {
        Some(InkRect {
            left: 0.0,
            top,
            right: 10.0,
            bottom,
        })
    }

    #[test]
    fn ink_inside_the_box_fits() {
        assert_eq!(block_overflow(ink(12.0, 33.0), 10.0, 24.0), None);
        assert_eq!(block_overflow(ink(9.6, 34.4), 10.0, 24.0), None);
        assert_eq!(block_overflow(None, 10.0, 24.0), None);
    }

    #[test]
    fn ink_below_names_the_height_that_holds_it() {
        let o = block_overflow(ink(12.0, 40.2), 10.0, 24.0).expect("overflows");
        assert!(o.below);
        assert_eq!(o.need_h, 31.0);
        assert_eq!(o.rise, 0.0);
    }

    #[test]
    fn ink_above_reports_the_rise() {
        let o = block_overflow(ink(6.5, 20.0), 10.0, 24.0).expect("overflows");
        assert!(!o.below);
        assert_eq!(o.rise, 4.0);
        assert_eq!(o.need_h, 24.0);
    }

    #[test]
    fn fill_rects_count_as_ink() {
        let provider = zenith_core::default_provider();
        let store = zenith_layout::FontFaceStore::new(&provider);
        let engine = zenith_layout::RustybuzzEngine::new(&store);
        let env = ShapeEnv {
            engine: &engine,
            fonts: &provider,
        };
        let cmds = vec![SceneCommand::FillRect {
            x: 1.0,
            y: 2.0,
            w: 3.0,
            h: 4.0,
            paint: crate::ir::Paint::solid(crate::ir::Color::srgb(0, 0, 0, 255)),
        }];
        assert_eq!(
            ink_bounds(&cmds, env),
            Some(InkRect {
                left: 1.0,
                top: 2.0,
                right: 4.0,
                bottom: 6.0
            })
        );
        assert_eq!(ink_bounds(&[], env), None);
    }
}
