//! Physical sheet composition: panel placement, outer-bleed clipping, duplex
//! back-side rotation, and press-mark attachment.

use zenith_scene::{Rect, Scene, SceneCommand};

use super::marks::{MARKS_MIN_SLUG_PX, emit_color_bars, emit_sheet_marks};
use super::plan::{PanelGeometry, SheetSpec};
use super::{DuplexMode, ImposeMode, ImposeOptions};

/// Compose one physical sheet by placing each panel slot on a `cols × rows` grid,
/// preserving outer bleed while meeting flush at internal fold/cut edges.
pub(super) fn compose_sheet(
    scenes: &[Scene],
    spec: &SheetSpec,
    geom: PanelGeometry,
    opts: ImposeOptions,
) -> Scene {
    let PanelGeometry {
        bleed_x,
        bleed_y,
        trim_w,
        trim_h,
    } = geom;
    let cols = spec.cols;
    let rows = spec.rows;
    let sheet_trim_w = trim_w * cols as f64;
    let sheet_trim_h = trim_h * rows as f64;
    let needs_slug = opts.marks || opts.color_bars;
    let margin_x = if needs_slug {
        bleed_x.max(MARKS_MIN_SLUG_PX)
    } else {
        bleed_x
    };
    let margin_y = if needs_slug {
        bleed_y.max(MARKS_MIN_SLUG_PX)
    } else {
        bleed_y
    };
    let sheet_w = sheet_trim_w + 2.0 * margin_x;
    let sheet_h = sheet_trim_h + 2.0 * margin_y;

    let mut sheet = Scene::new(sheet_w, sheet_h);
    if margin_x > 0.0 || margin_y > 0.0 {
        sheet.trim = Some(Rect {
            x: margin_x,
            y: margin_y,
            w: sheet_trim_w,
            h: sheet_trim_h,
        });
    }

    let rotate_back = spec.is_back && opts.duplex == DuplexMode::LongEdge;
    if rotate_back {
        sheet.commands.push(SceneCommand::PushTransform {
            angle_deg: 180.0,
            cx: sheet_w / 2.0,
            cy: sheet_h / 2.0,
        });
    }

    for (idx, page_slot) in spec.slots.iter().enumerate() {
        let Some(page_idx) = *page_slot else {
            continue;
        };
        let Some(page) = scenes.get(page_idx) else {
            continue;
        };
        let row = idx / cols;
        let col = idx % cols;
        let panel_trim_x = margin_x + trim_w * col as f64;
        let panel_trim_y = margin_y + trim_h * row as f64;
        // Outer sheet edges preserve bleed; internal fold/cut edges clip flush at trim.
        let clip_x = if col == 0 {
            margin_x - bleed_x
        } else {
            panel_trim_x
        };
        let clip_right = if col + 1 == cols {
            margin_x + sheet_trim_w + bleed_x
        } else {
            panel_trim_x + trim_w
        };
        let clip_y = if row == 0 {
            margin_y - bleed_y
        } else {
            panel_trim_y
        };
        let clip_bottom = if row + 1 == rows {
            margin_y + sheet_trim_h + bleed_y
        } else {
            panel_trim_y + trim_h
        };

        // Saddle-stitch creep shifts left panel right (+creep) and right panel left (-creep).
        let creep_shift = if spec.creep_px > 0.0 && cols == 2 && rows == 1 {
            if col == 0 {
                spec.creep_px
            } else {
                -spec.creep_px
            }
        } else {
            0.0
        };
        let tx = (panel_trim_x - bleed_x) + creep_shift;
        let ty = panel_trim_y - bleed_y;

        // Strip per-page trailing crop marks (8 StrokeLines emitted by scene compiler when bleed > 0)
        // so only clean sheet-level marks appear when `--marks` is enabled.
        let panel_cmds = strip_panel_crop_marks(page);

        sheet.commands.push(SceneCommand::PushClip {
            x: clip_x,
            y: clip_y,
            w: clip_right - clip_x,
            h: clip_bottom - clip_y,
        });
        if tx != 0.0 || ty != 0.0 {
            sheet.commands.push(SceneCommand::PushScaleTranslate {
                sx: 1.0,
                sy: 1.0,
                tx,
                ty,
            });
        }
        sheet.commands.extend(panel_cmds.iter().cloned());
        if tx != 0.0 || ty != 0.0 {
            sheet.commands.push(SceneCommand::PopTransform);
        }
        sheet.commands.push(SceneCommand::PopClip);
    }

    if opts.marks {
        let dashed_internal = matches!(
            opts.mode,
            ImposeMode::Booklet | ImposeMode::Trifold | ImposeMode::Zfold | ImposeMode::Spread
        );
        emit_sheet_marks(
            &mut sheet.commands,
            (margin_x, margin_y, sheet_trim_w, sheet_trim_h),
            (cols, rows),
            (trim_w, trim_h),
            dashed_internal,
        );
    }

    if opts.color_bars {
        emit_color_bars(
            &mut sheet.commands,
            (margin_x, margin_y, sheet_trim_w, sheet_trim_h),
            rows,
        );
    }

    if rotate_back {
        sheet.commands.push(SceneCommand::PopTransform);
    }

    sheet
}

/// If `page` has a print bleed, strip the 8 trailing per-page corner crop-mark
/// `StrokeLine` commands emitted by the page compiler so imposed sheets never
/// carry stray interior crop marks.
fn strip_panel_crop_marks(page: &Scene) -> &[SceneCommand] {
    if page.trim.is_some()
        && let Some(split_at) = page.commands.len().checked_sub(8)
        && let Some((head, tail)) = page.commands.split_at_checked(split_at)
        && tail
            .iter()
            .all(|cmd| matches!(cmd, SceneCommand::StrokeLine { .. }))
    {
        head
    } else {
        &page.commands
    }
}
