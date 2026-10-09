//! Editor loop benches over the showcase deck (see `common/mod.rs`).
//!
//! ```text
//! cd zenith-editor/benches && cargo bench --bench editor
//! ```
//!
//! Time is informational on a loaded machine. Judge a change by the
//! instruction and allocation counts of `editor_counts`, and by the work
//! counts `tests/editor/showcase.rs` asserts. The p95 gates come from the
//! first measurement (load average about 3, 3.7 GHz) with headroom: the
//! typing target of 50 ms on page 2 (measured 35 ms), 60 ms on the heaviest
//! page (measured 50 ms), and warnings for the drag preview, the render, and
//! `buffer.set`. This binary runs on the system allocator: the counting
//! allocator (`editor_counts`) adds two atomic adds per allocation, and a
//! keystroke allocates about 390 000 times.

use std::sync::OnceLock;

use fluxbench::prelude::*;
use fluxbench::{bench, verify};
use zenith_editor_bench::Fixture;

/// The deck, opened once per worker process.
fn fixture() -> &'static Fixture {
    static FIXTURE: OnceLock<Fixture> = OnceLock::new();
    FIXTURE.get_or_init(Fixture::default)
}

#[bench(group = "editor")]
fn typing_loop(b: &mut Bencher) {
    let fx = fixture();
    b.iter_with_setup(|| fx.start("typing"), |s| fx.run("typing", s));
}

#[bench(group = "editor", severity = "critical")]
fn typing_batch(b: &mut Bencher) {
    let fx = fixture();
    b.iter_with_setup(|| fx.start("typing_batch"), |s| fx.run("typing_batch", s));
}

#[bench(group = "editor", severity = "critical")]
fn typing_heavy(b: &mut Bencher) {
    let fx = fixture();
    b.iter_with_setup(|| fx.start("typing_heavy"), |s| fx.run("typing_heavy", s));
}

#[bench(group = "editor")]
fn drag_preview(b: &mut Bencher) {
    let fx = fixture();
    b.iter_with_setup(|| fx.start("drag"), |s| fx.run("drag", s));
}

#[bench(group = "editor")]
fn drag_snap_preview(b: &mut Bencher) {
    let fx = fixture();
    b.iter_with_setup(|| fx.start("drag_snap"), |s| fx.run("drag_snap", s));
}

#[bench(group = "editor")]
fn drag_multi_preview(b: &mut Bencher) {
    let fx = fixture();
    b.iter_with_setup(|| fx.start("drag_multi"), |s| fx.run("drag_multi", s));
}

#[bench(group = "editor")]
fn buffer_set(b: &mut Bencher) {
    let fx = fixture();
    b.iter_with_setup(|| fx.start("buffer"), |s| fx.run("buffer", s));
}

#[bench(group = "editor")]
fn render_viewport(b: &mut Bencher) {
    let fx = fixture();
    b.iter_with_setup(|| fx.start("render"), |s| fx.run("render", s));
}

/// The typing-loop target: p95 under 50 ms (ns), for the batch the page
/// sends on page 2.
#[verify(expr = "typing_batch_p95 < 50000000", severity = "critical")]
struct TypingUnder50ms;

/// The heaviest page: p95 under 60 ms (measured 50 ms).
#[verify(expr = "typing_heavy_p95 < 60000000", severity = "critical")]
struct HeavyTypingUnder60ms;

/// One drag step: p95 under 40 ms (measured 31 ms).
#[verify(expr = "drag_preview_p95 < 40000000", severity = "warning")]
struct DragUnder40ms;

/// One snapped drag step: under the same 40 ms as the plain one.
#[verify(expr = "drag_snap_preview_p95 < 40000000", severity = "warning")]
struct SnapDragUnder40ms;

/// A snapped three-node drag step: under the same 40 ms.
#[verify(expr = "drag_multi_preview_p95 < 40000000", severity = "warning")]
struct MultiDragUnder40ms;

/// The viewport render: p95 under 40 ms (measured 31 ms).
#[verify(expr = "render_viewport_p95 < 40000000", severity = "warning")]
struct RenderUnder40ms;

/// `buffer.set`: p95 under 25 ms (measured 19 ms).
#[verify(expr = "buffer_set_p95 < 25000000", severity = "warning")]
struct BufferUnder25ms;

fn main() {
    // The verify macros register the gates; the structs only name them.
    let _ = (
        TypingUnder50ms,
        HeavyTypingUnder60ms,
        DragUnder40ms,
        SnapDragUnder40ms,
        MultiDragUnder40ms,
        RenderUnder40ms,
        BufferUnder25ms,
    );
    if let Err(e) = fluxbench::run() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
