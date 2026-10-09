//! Engine work of the bench loops on the showcase deck (see
//! `benches/common/mod.rs`). Work counts are deterministic, so they gate
//! the loops where wall-clock time cannot.

#[path = "../../benches/common/mod.rs"]
mod loops;

use loops::Fixture;
use zenith_editor::Work;

fn work(parses: u32, validations: u32, tx_runs: u32, compiles: u32, rasters: u32) -> Work {
    Work {
        parses,
        validations,
        tx_runs,
        patches: 0,
        compiles,
        rasters,
    }
}

#[test]
fn showcase_loops_do_the_expected_work() {
    let fx = Fixture::new();
    let cases = [
        // Five calls: each parses the new text again.
        ("typing", work(5, 1, 0, 1, 1)),
        // One batch: one parse, one validation, one page render.
        ("typing_batch", work(1, 1, 0, 1, 1)),
        ("typing_heavy", work(1, 1, 0, 1, 1)),
        // The plan compiles the page for boxes, the preview compiles and
        // rasterizes the result.
        ("drag", work(1, 0, 1, 2, 1)),
        // Snapping reads the boxes the plan compiled: no extra compile.
        ("drag_snap", work(1, 0, 1, 2, 1)),
        // Three nodes share one page compile.
        ("drag_multi", work(1, 0, 1, 2, 1)),
        ("buffer", work(1, 1, 0, 0, 0)),
        ("render", work(1, 0, 0, 1, 1)),
    ];
    for (name, want) in cases {
        assert_eq!(fx.run(name, fx.start(name)), want, "{name}");
    }
}
