//! Sheet-level press marks: K-only corner crop marks, internal fold/cut ticks,
//! 4-plate registration targets, and CMYK density control bars.

use zenith_scene::{Color, LineCap, Paint, SceneCommand};

/// Minimum outer slug margin (in px, ~6.35 mm) when `--marks` or `--color-bars` is active.
pub(super) const MARKS_MIN_SLUG_PX: f64 = 24.0;

/// Gap between trim edge and crop/fold mark start (~2.1 mm).
const MARK_OFFSET_PX: f64 = 8.0;

/// Length of each crop/fold mark segment (~3.2 mm).
const MARK_LEN_PX: f64 = 12.0;

/// Hairline mark stroke width (`0.5 pt` at 96 px/in).
const MARK_STROKE_PX: f64 = 0.67;

/// Height of each swatch in the CMYK density control bar.
const BAR_HEIGHT_PX: f64 = 8.0;

/// Nominal width of each swatch in the CMYK density control bar.
const BAR_SWATCH_W_PX: f64 = 10.0;

/// Radius of each 4-plate registration target circle.
const REG_RADIUS_PX: f64 = 4.0;

/// Half-length of each 4-plate registration target crosshair arm.
const REG_ARM_PX: f64 = 6.0;

/// Emit K-only corner crop marks at the 4 sheet trim corners and internal fold
/// (dashed) or cut (solid) tick marks in the outer margins.
pub(super) fn emit_sheet_marks(
    commands: &mut Vec<SceneCommand>,
    (left, top, trim_w, trim_h): (f64, f64, f64, f64),
    (cols, rows): (usize, usize),
    (panel_trim_w, panel_trim_h): (f64, f64),
    dashed_internal: bool,
) {
    let right = left + trim_w;
    let bottom = top + trim_h;
    let black = Color::cmyk(0.0, 0.0, 0.0, 100.0, 0, 0, 0);
    let mut line = |x1: f64, y1: f64, x2: f64, y2: f64, dash: Option<f64>| {
        commands.push(SceneCommand::StrokeLine {
            x1,
            y1,
            x2,
            y2,
            color: black,
            stroke_width: MARK_STROKE_PX,
            stroke_dash: dash,
            stroke_gap: dash,
            stroke_linecap: Some(LineCap::Butt),
        });
    };

    // 4 outer trim corners (2 solid segments per corner, strictly outside trim).
    let o = MARK_OFFSET_PX;
    let l = MARK_LEN_PX;
    // Top-left
    line(left - o - l, top, left - o, top, None);
    line(left, top - o - l, left, top - o, None);
    // Top-right
    line(right + o, top, right + o + l, top, None);
    line(right, top - o - l, right, top - o, None);
    // Bottom-left
    line(left - o - l, bottom, left - o, bottom, None);
    line(left, bottom + o, left, bottom + o + l, None);
    // Bottom-right
    line(right + o, bottom, right + o + l, bottom, None);
    line(right, bottom + o, right, bottom + o + l, None);

    // Internal column fold/cut tick marks (strictly outside trim).
    let tick_dash = dashed_internal.then_some(3.0);
    for col in 1..cols {
        let fx = left + panel_trim_w * col as f64;
        line(fx, top - o - l, fx, top - o, tick_dash);
        line(fx, bottom + o, fx, bottom + o + l, tick_dash);
    }
    // Internal row cut tick marks (strictly outside trim).
    for row in 1..rows {
        let fy = top + panel_trim_h * row as f64;
        line(left - o - l, fy, left - o, fy, tick_dash);
        line(right + o, fy, right + o + l, fy, tick_dash);
    }
}

/// Emit a centered 11-patch CMYK density control bar in the top outer slug and
/// 4-plate registration targets in the left and right outer margins.
pub(super) fn emit_color_bars(
    commands: &mut Vec<SceneCommand>,
    (left, top, trim_w, trim_h): (f64, f64, f64, f64),
    rows: usize,
) {
    // 11 standard press density patches: C, M, Y, K, C+M, C+Y, M+Y, K75, K50, K25, K10.
    const PATCHES: [(f32, f32, f32, f32, u8, u8, u8); 11] = [
        (100.0, 0.0, 0.0, 0.0, 0, 174, 239),
        (0.0, 100.0, 0.0, 0.0, 236, 0, 140),
        (0.0, 0.0, 100.0, 0.0, 255, 242, 0),
        (0.0, 0.0, 0.0, 100.0, 0, 0, 0),
        (100.0, 100.0, 0.0, 0.0, 46, 49, 146),
        (100.0, 0.0, 100.0, 0.0, 0, 166, 81),
        (0.0, 100.0, 100.0, 0.0, 237, 28, 36),
        (0.0, 0.0, 0.0, 75.0, 64, 64, 64),
        (0.0, 0.0, 0.0, 50.0, 128, 128, 128),
        (0.0, 0.0, 0.0, 25.0, 191, 191, 191),
        (0.0, 0.0, 0.0, 10.0, 230, 230, 230),
    ];

    let max_bar_w = (trim_w - 20.0).max(0.0);
    if max_bar_w >= 22.0 {
        let swatch_w = BAR_SWATCH_W_PX.min(max_bar_w / PATCHES.len() as f64);
        let bar_w = swatch_w * PATCHES.len() as f64;
        let bar_x = left + (trim_w - bar_w) / 2.0;
        let bar_y = top - MARK_OFFSET_PX - BAR_HEIGHT_PX;
        let border_k = Color::cmyk(0.0, 0.0, 0.0, 100.0, 0, 0, 0);

        for (i, &(c, m, y, k, r, g, b)) in PATCHES.iter().enumerate() {
            let x = bar_x + swatch_w * i as f64;
            commands.push(SceneCommand::FillRect {
                x,
                y: bar_y,
                w: swatch_w,
                h: BAR_HEIGHT_PX,
                paint: Paint::solid(Color::cmyk(c, m, y, k, r, g, b)),
            });
        }
        commands.push(SceneCommand::StrokeRect {
            x: bar_x,
            y: bar_y,
            w: bar_w,
            h: BAR_HEIGHT_PX,
            color: border_k,
            stroke_width: MARK_STROKE_PX,
            stroke_dash: None,
            stroke_gap: None,
            stroke_linecap: None,
        });
    }

    // 4-plate registration targets (C=100, M=100, Y=100, K=100) in left/right margins.
    // Offset vertically to 1/4 height on even row counts so targets never collide with
    // center horizontal cut ticks.
    let reg_y = if rows.is_multiple_of(2) {
        top + trim_h * 0.25
    } else {
        top + trim_h * 0.5
    };
    let right = left + trim_w;
    let reg_offset = MARK_OFFSET_PX + REG_ARM_PX;
    emit_registration_target(commands, left - reg_offset, reg_y);
    emit_registration_target(commands, right + reg_offset, reg_y);
}

/// Emit one 4-plate registration target (circle + crosshair) centered at `(cx, cy)`.
fn emit_registration_target(commands: &mut Vec<SceneCommand>, cx: f64, cy: f64) {
    let registration = Color::cmyk(100.0, 100.0, 100.0, 100.0, 0, 0, 0);
    let d = REG_RADIUS_PX * 2.0;
    commands.push(SceneCommand::StrokeEllipse {
        x: cx - REG_RADIUS_PX,
        y: cy - REG_RADIUS_PX,
        w: d,
        h: d,
        color: registration,
        stroke_width: MARK_STROKE_PX,
        stroke_dash: None,
        stroke_gap: None,
        stroke_linecap: None,
        rx: None,
        ry: None,
    });
    for (x1, y1, x2, y2) in [
        (cx - REG_ARM_PX, cy, cx + REG_ARM_PX, cy),
        (cx, cy - REG_ARM_PX, cx, cy + REG_ARM_PX),
    ] {
        commands.push(SceneCommand::StrokeLine {
            x1,
            y1,
            x2,
            y2,
            color: registration,
            stroke_width: MARK_STROKE_PX,
            stroke_dash: None,
            stroke_gap: None,
            stroke_linecap: Some(LineCap::Butt),
        });
    }
}
