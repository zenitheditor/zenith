//! Unit tests for print sheet imposition planning, composition, and press marks.

use zenith_scene::{Rect, Scene, SceneCommand};

use super::plan::{SheetSpec, sheet_plan};
use super::{DuplexMode, ImposeMode, ImposeOptions, impose_scenes, parse_grid_spec};

fn slots_only(plan: &[SheetSpec]) -> Vec<Vec<Option<usize>>> {
    plan.iter().map(|s| s.slots.clone()).collect()
}

#[test]
fn booklet_plan_for_4_and_8_pages_and_multi_signature() {
    assert_eq!(
        slots_only(&sheet_plan(4, ImposeOptions::new(ImposeMode::Booklet))),
        vec![vec![Some(3), Some(0)], vec![Some(1), Some(2)]]
    );
    assert_eq!(
        slots_only(&sheet_plan(8, ImposeOptions::new(ImposeMode::Booklet))),
        vec![
            vec![Some(7), Some(0)],
            vec![Some(1), Some(6)],
            vec![Some(5), Some(2)],
            vec![Some(3), Some(4)],
        ]
    );
    // 8 pages split into two 4-page signatures: [4|1],[2|3] then [8|5],[6|7].
    let multi = ImposeOptions {
        signature: Some(4),
        ..ImposeOptions::new(ImposeMode::Booklet)
    };
    assert_eq!(
        slots_only(&sheet_plan(8, multi)),
        vec![
            vec![Some(3), Some(0)],
            vec![Some(1), Some(2)],
            vec![Some(7), Some(4)],
            vec![Some(5), Some(6)],
        ]
    );
}

#[test]
fn trifold_zfold_and_spread_plans() {
    assert_eq!(
        slots_only(&sheet_plan(6, ImposeOptions::new(ImposeMode::Trifold))),
        vec![
            vec![Some(4), Some(5), Some(0)],
            vec![Some(1), Some(2), Some(3)],
        ]
    );
    assert_eq!(
        slots_only(&sheet_plan(6, ImposeOptions::new(ImposeMode::Zfold))),
        vec![
            vec![Some(4), Some(3), Some(0)],
            vec![Some(1), Some(2), Some(5)],
        ]
    );
    assert_eq!(
        slots_only(&sheet_plan(3, ImposeOptions::new(ImposeMode::Spread))),
        vec![vec![Some(0), Some(1)], vec![Some(2), None]]
    );
}

#[test]
fn impose_validates_sizes_creep_and_signature() {
    let mismatched = vec![Scene::new(561.28, 793.6), Scene::new(600.0, 793.6)];
    let err = impose_scenes(&mismatched, ImposeOptions::new(ImposeMode::Booklet)).unwrap_err();
    assert!(
        err.message.contains("uniform page sizes"),
        "{}",
        err.message
    );

    let pages = vec![Scene::new(100.0, 140.0); 4];
    let bad_sig = ImposeOptions {
        signature: Some(6),
        ..ImposeOptions::new(ImposeMode::Booklet)
    };
    assert!(impose_scenes(&pages, bad_sig).is_err());

    let bad_creep = ImposeOptions {
        creep_mm: 1.0,
        ..ImposeOptions::new(ImposeMode::Trifold)
    };
    assert!(impose_scenes(&pages, bad_creep).is_err());
}

#[test]
fn impose_preserves_outer_bleed_and_adds_marks_duplex_and_creep() {
    let bleed = 12.0;
    let mut pages = Vec::new();
    for _ in 0..8 {
        let mut s = Scene::new(100.0 + 2.0 * bleed, 140.0 + 2.0 * bleed);
        s.trim = Some(Rect {
            x: bleed,
            y: bleed,
            w: 100.0,
            h: 140.0,
        });
        pages.push(s);
    }
    let opts = ImposeOptions {
        mode: ImposeMode::Booklet,
        duplex: DuplexMode::LongEdge,
        creep_mm: 2.54, // 9.6 px on inner sheet
        signature: None,
        marks: true,
        color_bars: false,
    };
    let sheets = impose_scenes(&pages, opts).unwrap();
    assert_eq!(sheets.len(), 4);
    // Marks force 24 px slug margin on each side around 200x140 trim box.
    assert!((sheets[0].width - 248.0).abs() < 1e-6);
    assert!((sheets[0].height - 188.0).abs() < 1e-6);
    assert_eq!(
        sheets[0].trim,
        Some(Rect {
            x: 24.0,
            y: 24.0,
            w: 200.0,
            h: 140.0,
        })
    );
    // Front sheet has no 180° rotation; back sheet starts with 180° PushTransform.
    assert!(!matches!(
        sheets[0].commands.first(),
        Some(SceneCommand::PushTransform { .. })
    ));
    assert!(matches!(
        sheets[1].commands.first(),
        Some(SceneCommand::PushTransform { angle_deg, .. }) if (*angle_deg - 180.0).abs() < 1e-6
    ));
    // Marks emitted: 8 corner crop lines + 2 fold ticks = 10 StrokeLines.
    let mark_lines = sheets[0]
        .commands
        .iter()
        .filter(|c| matches!(c, SceneCommand::StrokeLine { .. }))
        .count();
    assert_eq!(mark_lines, 10);
}

#[test]
fn grid_and_cut_and_stack_plans_and_2d_marks() {
    // 10 pages on a 2x2 grid (4-up) -> 3 sheets.
    assert_eq!(
        slots_only(&sheet_plan(
            10,
            ImposeOptions::new(ImposeMode::Grid { cols: 2, rows: 2 })
        )),
        vec![
            vec![Some(0), Some(1), Some(2), Some(3)],
            vec![Some(4), Some(5), Some(6), Some(7)],
            vec![Some(8), Some(9), None, None],
        ]
    );
    // Cut-and-stack for 10 pages on 2x2 (S=3 sheets):
    // sheet 0 = [1, 4, 7, 10], sheet 1 = [2, 5, 8, _], sheet 2 = [3, 6, 9, _].
    assert_eq!(
        slots_only(&sheet_plan(
            10,
            ImposeOptions::new(ImposeMode::CutAndStack { cols: 2, rows: 2 })
        )),
        vec![
            vec![Some(0), Some(3), Some(6), Some(9)],
            vec![Some(1), Some(4), Some(7), None],
            vec![Some(2), Some(5), Some(8), None],
        ]
    );

    let pages = vec![Scene::new(100.0, 60.0); 10];
    let opts = ImposeOptions {
        marks: true,
        ..ImposeOptions::new(ImposeMode::CutAndStack { cols: 2, rows: 2 })
    };
    let sheets = impose_scenes(&pages, opts).unwrap();
    assert_eq!(sheets.len(), 3);
    // 2x2 of 100x60 = 200x120 trim + 24px slug on each side = 248x168.
    assert!((sheets[0].width - 248.0).abs() < 1e-6);
    assert!((sheets[0].height - 168.0).abs() < 1e-6);
    // 8 corner lines + 2 column cut ticks + 2 row cut ticks = 12 solid StrokeLines.
    let mark_lines: Vec<_> = sheets[0]
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::StrokeLine { stroke_dash, .. } => Some(*stroke_dash),
            _ => None,
        })
        .collect();
    assert_eq!(mark_lines.len(), 12);
    assert!(
        mark_lines.iter().all(Option::is_none),
        "cut-and-stack cut ticks must be solid"
    );
}

#[test]
fn color_bars_emit_11_cmyk_patches_and_two_registration_targets() {
    let pages = vec![Scene::new(100.0, 140.0); 4];
    let opts = ImposeOptions {
        marks: true,
        color_bars: true,
        ..ImposeOptions::new(ImposeMode::Booklet)
    };
    let sheets = impose_scenes(&pages, opts).unwrap();
    let sheet = &sheets[0];
    // 11 CMYK density patches (FillRect) + 1 framing StrokeRect.
    let fill_rects = sheet
        .commands
        .iter()
        .filter(|c| matches!(c, SceneCommand::FillRect { .. }))
        .count();
    let stroke_rects = sheet
        .commands
        .iter()
        .filter(|c| matches!(c, SceneCommand::StrokeRect { .. }))
        .count();
    let reg_circles = sheet
        .commands
        .iter()
        .filter(|c| matches!(c, SceneCommand::StrokeEllipse { .. }))
        .count();
    assert_eq!(fill_rects, 11);
    assert_eq!(stroke_rects, 1);
    assert_eq!(reg_circles, 2);
}

#[test]
fn parse_grid_spec_validates_bounds() {
    assert_eq!(parse_grid_spec("2x4"), Ok((2, 4)));
    assert_eq!(parse_grid_spec("3X2"), Ok((3, 2)));
    assert!(parse_grid_spec("1x1").is_err());
    assert!(parse_grid_spec("0x2").is_err());
    assert!(parse_grid_spec("33x2").is_err());
    assert!(parse_grid_spec("2by4").is_err());
}
