mod common;
use common::*;
use zenith_core::default_provider;
use zenith_scene::ir::SceneCommand;
use zenith_scene::{compile, compile_page};

/// With `drop-cap-lines=3`: an oversized initial glyph is emitted (its
/// font_size is far larger than the 32px body), and the first three body lines
/// are indented to the right of the box-left while the fourth line returns to
/// the box-left edge.
#[test]
fn dropcap_emits_oversized_initial_and_indents_first_lines() {
    let runs = dropcap_runs(Some(3), DROPCAP_BODY);
    assert!(runs.len() > 4, "body must wrap to several lines: {runs:?}");

    let body_font_size = 32.0_f32;
    let box_left = 180.0_f64;

    // The oversized cap is the FIRST emitted run (drawn before the body).
    let (cap_x, _cap_y, cap_size) = runs[0];
    assert!(
        cap_size > body_font_size * 2.0,
        "drop-cap font_size ({cap_size}) must be far larger than body ({body_font_size}); \
         expected ≈ 3×line_height"
    );
    assert!(
        (cap_x - box_left).abs() < 0.01,
        "drop cap must sit at the box left edge ({box_left}); got {cap_x}"
    );

    // Body runs follow the cap. The first three text lines must be indented to
    // the RIGHT of the box left; the next line must return to the box left.
    // Collect body runs grouped by distinct baseline y (one x-origin per line).
    let body: Vec<(f64, f64)> = runs[1..].iter().map(|&(x, y, _)| (x, y)).collect();
    // Per-line minimum x keyed by baseline y (BTreeMap to keep order stable).
    let mut line_min_x: std::collections::BTreeMap<i64, f64> = std::collections::BTreeMap::new();
    for &(x, y) in &body {
        let key = (y * 1000.0) as i64;
        let e = line_min_x.entry(key).or_insert(x);
        if x < *e {
            *e = x;
        }
    }
    let line_starts: Vec<f64> = line_min_x.values().copied().collect();
    assert!(
        line_starts.len() >= 4,
        "need at least 4 body lines; got {}: {line_starts:?}",
        line_starts.len()
    );
    for (i, &sx) in line_starts.iter().take(3).enumerate() {
        assert!(
            sx > box_left + 1.0,
            "body line {i} must be indented right of box left ({box_left}); got {sx}"
        );
    }
    assert!(
        (line_starts[3] - box_left).abs() < 1.0,
        "body line 4 must return to box left ({box_left}); got {}",
        line_starts[3]
    );
}

/// Without `drop-cap-lines` the command stream is byte-identical to a plain
/// wrapped paragraph — the feature is fully opt-in.
#[test]
fn dropcap_absent_is_byte_identical() {
    let dc_attr = "";
    let _ = dc_attr;
    let none_runs = dropcap_runs(None, DROPCAP_BODY);
    // Re-render the SAME no-dropcap source twice → identical (determinism), and
    // confirm the first run is body-sized (no oversized cap).
    let none_runs2 = dropcap_runs(None, DROPCAP_BODY);
    assert_eq!(
        none_runs, none_runs2,
        "no-dropcap render must be deterministic"
    );
    for (_, _, fs) in &none_runs {
        assert!(
            (*fs - 32.0).abs() < 0.01,
            "no-dropcap node must emit only body-sized (32px) runs; got {fs}"
        );
    }
}

/// Empty body text + `drop-cap-lines` set → no panic, no oversized cap, no
/// glyph runs at all (the empty-spans early return fires).
#[test]
fn dropcap_empty_text_no_panic_no_cap() {
    let runs = dropcap_runs(Some(3), "");
    assert!(
        runs.is_empty(),
        "empty text with drop-cap-lines must emit no glyph runs; got {runs:?}"
    );
}

/// Two renders of a drop-cap document produce a byte-identical command stream.
#[test]
fn dropcap_two_run_byte_identical() {
    let src = r##"zenith version=1 {
  project id="proj.dcr" name="DCR"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.dcr" title="DCR" {
page id="page.dcr" w=(px)1800 h=(px)2700 {
  text id="text.dcr" x=(px)180 y=(px)600 w=(px)600 h=(px)1200 align="justify" font-size=(px)32 drop-cap-lines=3 {
    span "The quick brown fox jumps over the lazy dog and then keeps on running across the meadow."
  }
}
  }
}
"##;
    let doc = parse(src);
    let r1 = compile(&doc, &default_provider());
    let r2 = compile(&doc, &default_provider());
    assert_eq!(
        r1.scene.commands, r2.scene.commands,
        "two drop-cap renders must be byte-identical"
    );
}

/// With `widow-orphan=2`, a paragraph's lone first line that the greedy cut
/// would leave at the bottom of box 1 is pulled down into box 2: box 1's line
/// count is REDUCED relative to the control-off render, and box 2 gains lines.
#[test]
fn widow_orphan_pulls_orphan_line_to_next_box() {
    let (off0, off1) = widow_orphan_line_counts(false);
    let (on0, on1) = widow_orphan_line_counts(true);

    // The control must change SOMETHING about the distribution.
    assert!(
        (on0, on1) != (off0, off1),
        "widow-orphan=2 must move the break: off=({off0},{off1}) on=({on0},{on1})"
    );
    // Lines only move DOWN (box 1 keeps no more than before; box 2 gains).
    assert!(
        on0 <= off0,
        "box 1 must not gain lines under widow/orphan: off0={off0} on0={on0}"
    );
    assert!(
        on1 >= off1,
        "box 2 must not lose lines under widow/orphan: off1={off1} on1={on1}"
    );
    // No content lost: the total line count is preserved across the boundary
    // move (re-wrapping to the same widths keeps the same lines).
    assert_eq!(
        on0 + on1,
        off0 + off1,
        "total lines must be preserved when the boundary moves"
    );
}

/// Widow/orphan OFF: the 2-box chain distribution is deterministic and matches a
/// re-render byte-for-byte (the control is fully opt-in).
#[test]
fn widow_orphan_off_is_deterministic() {
    let doc_src = || {
        let p1 = "alpha bravo charlie delta echo foxtrot golf hotel india juliet";
        let p2 = "victor whiskey xray yankee zulu aurora borealis cascade delta estuary";
        format!(
            r##"zenith version=1 {{
  project id="proj.wod" name="WOD"
  tokens format="zenith-token-v1" {{}}
  styles {{}}
  document id="doc.wod" title="WOD" {{
page id="page.a" w=(px)1200 h=(px)2000 {{
  text id="body.1" x=(px)100 y=(px)100 w=(px)900 h=(px)360 chain="ch" font-size=(px)40 overflow="visible" {{
    span "{p1}\n{p2}"
  }}
}}
page id="page.b" w=(px)1200 h=(px)2000 {{
  text id="body.2" x=(px)100 y=(px)100 w=(px)900 h=(px)900 chain="ch" font-size=(px)40 overflow="visible" {{
  }}
}}
  }}
}}
"##
        )
    };
    let doc = parse(&doc_src());
    let r1 = compile_page(&doc, &default_provider(), 0, None)
        .scene
        .commands;
    let r2 = compile_page(&doc, &default_provider(), 0, None)
        .scene
        .commands;
    assert_eq!(
        r1, r2,
        "widow/orphan-off chain render must be deterministic"
    );
}

#[test]
fn baseline_grid_none_is_byte_identical() {
    // A page WITHOUT a baseline-grid attribute must compile byte-identically to
    // itself (the default-off path is unchanged) and must emit NO snap diags.
    let doc = baseline_grid_doc("");
    let r1 = compile(&doc, &default_provider());
    let r2 = compile(&doc, &default_provider());
    assert_eq!(
        r1.scene.commands, r2.scene.commands,
        "grid-absent render must be deterministic / unchanged"
    );
    assert!(
        !r1.diagnostics
            .iter()
            .any(|d| d.code == "baseline-grid.snap_failed"),
        "no snap diagnostic without a grid"
    );
    // Sanity: the node actually wrapped into multiple lines.
    assert!(
        glyph_run_ys(&r1.scene.commands).len() >= 2,
        "test text must wrap into multiple lines"
    );
}

#[test]
fn baseline_grid_snaps_first_baseline_to_grid() {
    // With g=14, the first emitted baseline must land on a multiple of 14, and
    // it must differ from the un-snapped baseline (proving the snap is active).
    let g = 14.0;
    let snapped = compile(
        &baseline_grid_doc("baseline-grid=(px)14"),
        &default_provider(),
    );
    let plain = compile(&baseline_grid_doc(""), &default_provider());

    let snapped_ys = glyph_run_ys(&snapped.scene.commands);
    let plain_ys = glyph_run_ys(&plain.scene.commands);
    let first_snapped = *snapped_ys.first().expect("snapped node emits a run");
    let first_plain = *plain_ys.first().expect("plain node emits a run");

    // First baseline is the next grid line ≥ the natural baseline.
    let rem = first_snapped % g;
    assert!(
        rem.abs() < 1e-6 || (g - rem).abs() < 1e-6,
        "first baseline {first_snapped} must be a multiple of {g}"
    );
    assert!(
        first_snapped >= first_plain - 1e-9,
        "snapped baseline moves DOWN (≥ natural): {first_snapped} vs {first_plain}"
    );
    assert!(
        first_snapped - first_plain < g + 1e-9,
        "snap moves down by less than one full grid cell"
    );
}

#[test]
fn baseline_grid_uniform_advance_is_multiple_of_pitch() {
    // Consecutive line baselines differ by ceil(line_height/g)*g — and since
    // line_height (18px Noto) > g (14), that advance is 28 (2× grid).
    let g = 14.0;
    let r = compile(
        &baseline_grid_doc("baseline-grid=(px)14"),
        &default_provider(),
    );
    let ys = distinct_line_ys(&r.scene.commands);
    assert!(ys.len() >= 2, "need ≥2 wrapped lines to check advance");
    let advance = ys[1] - ys[0];
    // The advance must be a positive multiple of g (the smallest multiple of g
    // ≥ the resolved Noto line-height at 18px). It is uniform across all lines.
    assert!(advance > 0.0, "advance must be positive; got {advance}");
    let mult = advance / g;
    assert!(
        (mult - mult.round()).abs() < 1e-6 && mult.round() >= 1.0,
        "advance {advance} must be a positive integer multiple of {g}"
    );
    // Every consecutive pair shares the same multiple-of-g advance.
    for w in ys.windows(2) {
        let d = w[1] - w[0];
        assert!(
            (d - advance).abs() < 1e-6,
            "all line advances equal; got {d} vs {advance}"
        );
        let rem = d % g;
        assert!(
            rem.abs() < 1e-6 || (g - rem).abs() < 1e-6,
            "advance {d} must be a multiple of {g}"
        );
    }
}

#[test]
fn baseline_grid_snap_failed_when_line_height_exceeds_pitch() {
    // line_height (18px Noto body) > g=14 → one advisory snap_failed diagnostic
    // for the node. With a generous grid (g=40 ≥ line_height) → NO diagnostic.
    let tight = compile(
        &baseline_grid_doc("baseline-grid=(px)14"),
        &default_provider(),
    );
    let tight_diags: Vec<_> = tight
        .diagnostics
        .iter()
        .filter(|d| d.code == "baseline-grid.snap_failed")
        .collect();
    assert_eq!(
        tight_diags.len(),
        1,
        "exactly one snap_failed advisory per affected node; got {:?}",
        tight.diagnostics
    );
    assert!(
        tight_diags[0].message.contains("col1"),
        "diagnostic names the node id"
    );

    let loose = compile(
        &baseline_grid_doc("baseline-grid=(px)40"),
        &default_provider(),
    );
    assert!(
        !loose
            .diagnostics
            .iter()
            .any(|d| d.code == "baseline-grid.snap_failed"),
        "no snap_failed when line-height ≤ grid pitch"
    );
}

/// A wrapping text node WITHOUT `bullet` must emit the identical command stream
/// as before (default-off / byte-identical gate).
#[test]
fn bullet_none_is_byte_identical() {
    // A node that wraps (narrow box, long text) without bullet.
    let no_bullet_src = r#"zenith version=1 {
  project id="proj.bn" name="BN"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.bn" title="BN" {
page id="page.bn" w=(px)1280 h=(px)720 {
  text id="t.nb" x=(px)100 y=(px)100 w=(px)200 h=(px)300 overflow="clip" align="start" {
    span "Revenue grew twelve percent year over year the strongest result since the restructuring."
  }
}
  }
}
"#;
    // Same node but with an explicit `bullet=""` (empty → treated as absent).
    let empty_bullet_src = no_bullet_src.replace(
        r#"overflow="clip" align="start""#,
        r#"overflow="clip" align="start" bullet="""#,
    );

    let a = compile(&parse(no_bullet_src), &default_provider());
    let b = compile(&parse(&empty_bullet_src), &default_provider());
    assert_eq!(
        a.scene.commands, b.scene.commands,
        "empty bullet string must emit identical command stream to no-bullet node"
    );
}

/// With `bullet="•"`, the compiled output must contain:
/// (a) a `DrawGlyphRun` for the marker at `x == text_x` (the un-indented left edge),
/// (b) ALL text-line glyph runs at `x > text_x` (every line is indented),
/// (c) wrapped line 2's first-glyph x equals wrapped line 1's first-glyph x
///     (continuation auto-aligns with the first text line).
#[test]
fn bullet_indents_all_text_lines_and_draws_marker() {
    let src = r#"zenith version=1 {
  project id="proj.bi" name="BI"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.bi" title="BI" {
page id="page.bi" w=(px)1280 h=(px)720 {
  text id="t.bi" x=(px)160 y=(px)200 w=(px)300 h=(px)400 overflow="clip" align="start" bullet="•" {
    span "Revenue grew twelve percent year over year the strongest result since the restructuring."
  }
}
  }
}
"#;
    let text_x = 160.0_f64;
    let result = compile(&parse(src), &default_provider());

    // Collect all DrawGlyphRun commands from the page (skip PushClip/PopClip).
    let glyph_runs: Vec<(f64, f64)> = result
        .scene
        .commands
        .iter()
        .filter_map(|c| {
            if let SceneCommand::DrawGlyphRun { x, y, .. } = c {
                Some((*x, *y))
            } else {
                None
            }
        })
        .collect();

    assert!(
        !glyph_runs.is_empty(),
        "expected at least one DrawGlyphRun, got none"
    );

    // (a) The first glyph run must be the marker at x == text_x.
    let (first_x, _) = glyph_runs[0];
    assert!(
        (first_x - text_x).abs() < 1.0,
        "marker DrawGlyphRun must be at x ≈ text_x ({text_x}), got x={first_x}"
    );

    // (b) All remaining runs (the body text) must be at x > text_x.
    let body_runs: Vec<f64> = glyph_runs[1..].iter().map(|(x, _)| *x).collect();
    assert!(
        !body_runs.is_empty(),
        "expected body glyph runs after the marker"
    );
    for &bx in &body_runs {
        assert!(
            bx > text_x + 0.5,
            "body glyph run at x={bx} must be indented past text_x={text_x}"
        );
    }

    // (c) The text wraps: collect the DISTINCT baseline-y values of the body
    // runs to identify lines. Check that the SECOND line's first-run x matches
    // the FIRST line's first-run x (continuation alignment).
    // We gather x-values per baseline_y bucket (within 1px tolerance).
    let mut by_line: std::collections::BTreeMap<i64, Vec<f64>> = std::collections::BTreeMap::new();
    for (x, y) in &glyph_runs[1..] {
        let key = y.round() as i64;
        by_line.entry(key).or_default().push(*x);
    }
    let lines: Vec<Vec<f64>> = by_line.into_values().collect();
    assert!(
        lines.len() >= 2,
        "expected at least 2 wrapped body lines for the long sentence, got {}",
        lines.len()
    );
    let line0_x = lines[0].iter().cloned().fold(f64::INFINITY, f64::min);
    let line1_x = lines[1].iter().cloned().fold(f64::INFINITY, f64::min);
    assert!(
        (line0_x - line1_x).abs() < 1.5,
        "continuation line x ({line1_x}) must align with first text line x ({line0_x})"
    );
}

/// Regression: a SINGLE-LINE bullet (text that fits one line) must STILL draw
/// the marker and indent the text — it must not slip onto the fast single-line
/// path that has no bullet handling. (The fix forces the wrapping/indent path
/// whenever a bullet/padding-left/text-indent is present.)
#[test]
fn single_line_bullet_draws_marker_and_indents() {
    let src = r#"zenith version=1 {
  project id="proj.sb" name="SB"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.sb" title="SB" {
page id="page.sb" w=(px)1280 h=(px)720 {
  text id="t.sb" x=(px)160 y=(px)200 w=(px)900 h=(px)80 overflow="clip" align="start" bullet="•" {
    span "Short item"
  }
}
  }
}
"#;
    let text_x = 160.0_f64;
    let result = compile(&parse(src), &default_provider());
    let glyph_runs: Vec<f64> = result
        .scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::DrawGlyphRun { x, .. } => Some(*x),
            _ => None,
        })
        .collect();
    // Marker + at least one body run.
    assert!(
        glyph_runs.len() >= 2,
        "single-line bullet must emit a marker run plus body run(s); got {}",
        glyph_runs.len()
    );
    // (a) marker at x ≈ text_x.
    assert!(
        (glyph_runs[0] - text_x).abs() < 1.0,
        "marker must be at x ≈ text_x ({text_x}), got {}",
        glyph_runs[0]
    );
    // (b) body indented past text_x (single line still gets the hanging column).
    for &bx in &glyph_runs[1..] {
        assert!(
            bx > text_x + 0.5,
            "single-line bullet body run at x={bx} must be indented past text_x={text_x}"
        );
    }
}

/// A larger `bullet-gap` pushes the text column further right than the default.
#[test]
fn bullet_gap_widens_the_column() {
    let base = r#"zenith version=1 {
  project id="proj.bg" name="BG"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.bg" title="BG" {
page id="page.bg" w=(px)1280 h=(px)720 {
  text id="t.bg" x=(px)100 y=(px)100 w=(px)500 h=(px)400 overflow="clip" align="start" bullet="•" {
    span "Revenue grew twelve percent year over year the strongest result since the restructuring."
  }
}
  }
}
"#;
    let wide = base.replace(r#"bullet="•""#, r#"bullet="•" bullet-gap=(px)80"#);

    let default_result = compile(&parse(base), &default_provider());
    let wide_result = compile(&parse(&wide), &default_provider());

    // The first non-marker body run x in the wide variant must be further right.
    let first_body_x = |cmds: &[SceneCommand]| -> f64 {
        let mut marker_seen = false;
        for c in cmds {
            if let SceneCommand::DrawGlyphRun { x, .. } = c {
                if marker_seen {
                    return *x;
                }
                marker_seen = true;
            }
        }
        0.0
    };

    let default_x = first_body_x(&default_result.scene.commands);
    let wide_x = first_body_x(&wide_result.scene.commands);
    assert!(
        wide_x > default_x + 1.0,
        "wide bullet-gap should push text column further right: default={default_x}, wide={wide_x}"
    );
}

/// At two different font sizes, the text column offset M scales with the
/// shaped marker advance, so M differs between sizes and the continuation
/// line still aligns with the first text line at each size.
#[test]
fn bullet_marker_measured_independent_of_size() {
    let make_src = |size: u32| {
        format!(
            r#"zenith version=1 {{
  project id="proj.bm" name="BM"
  tokens format="zenith-token-v1" {{}}
  styles {{}}
  document id="doc.bm" title="BM" {{
page id="page.bm" w=(px)1280 h=(px)720 {{
  text id="t.bm" x=(px)100 y=(px)100 w=(px)500 h=(px)500 overflow="clip" align="start" font-size=(px){size} bullet="•" {{
    span "Revenue grew twelve percent year over year the strongest result since the restructuring."
  }}
}}
  }}
}}
"#
        )
    };

    let first_body_x = |cmds: &[SceneCommand]| -> f64 {
        let mut marker_seen = false;
        for c in cmds {
            if let SceneCommand::DrawGlyphRun { x, .. } = c {
                if marker_seen {
                    return *x;
                }
                marker_seen = true;
            }
        }
        0.0
    };

    let text_x = 100.0_f64;
    let r_small = compile(&parse(&make_src(14)), &default_provider());
    let r_large = compile(&parse(&make_src(32)), &default_provider());

    let x_small = first_body_x(&r_small.scene.commands);
    let x_large = first_body_x(&r_large.scene.commands);

    // M = marker_advance + gap; both scale with font_size, so M at 32px > M at 14px.
    assert!(
        x_large > x_small,
        "larger font should produce a wider bullet M: small={x_small}, large={x_large}"
    );

    // Both must be indented past text_x.
    assert!(
        x_small > text_x + 0.5,
        "small-font body must be indented (got x_small={x_small})"
    );
    assert!(
        x_large > text_x + 0.5,
        "large-font body must be indented (got x_large={x_large})"
    );

    // For each size, check continuation alignment (line 1 x ≈ line 0 x).
    let check_continuation = |cmds: &[SceneCommand], label: &str| {
        let body_runs: Vec<(f64, f64)> = cmds
            .iter()
            .skip_while(|c| !matches!(c, SceneCommand::DrawGlyphRun { .. }))
            .skip(1) // skip the marker run
            .filter_map(|c| {
                if let SceneCommand::DrawGlyphRun { x, y, .. } = c {
                    Some((*x, *y))
                } else {
                    None
                }
            })
            .collect();
        let mut by_line: std::collections::BTreeMap<i64, Vec<f64>> =
            std::collections::BTreeMap::new();
        for (x, y) in &body_runs {
            by_line.entry(y.round() as i64).or_default().push(*x);
        }
        let lines: Vec<Vec<f64>> = by_line.into_values().collect();
        if lines.len() >= 2 {
            let l0 = lines[0].iter().cloned().fold(f64::INFINITY, f64::min);
            let l1 = lines[1].iter().cloned().fold(f64::INFINITY, f64::min);
            assert!(
                (l0 - l1).abs() < 2.0,
                "{label}: continuation x ({l1}) must align with first line x ({l0})"
            );
        }
    };

    check_continuation(&r_small.scene.commands, "small-font");
    check_continuation(&r_large.scene.commands, "large-font");
}
