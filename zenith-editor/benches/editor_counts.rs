//! Instruction and allocation counts of the editor loops (see
//! `common/mod.rs`). Wall-clock time swings on a loaded machine; these
//! counts do not.
//!
//! ```text
//! cd zenith-editor/benches && cargo bench --bench editor_counts --no-run
//! perf stat -e instructions:u <bench binary> <scenario> <iterations>
//! ```
//!
//! The binary opens the deck and runs one warm-up step (process-wide font
//! cache), then `iterations` steps of `<scenario>` (`typing`,
//! `typing_batch`, `typing_heavy`, `drag`, `drag_snap`, `drag_multi`,
//! `buffer`, `render`, or `all`). Instructions per step = (count at N −
//! count at 0) / N. It prints the engine work per step and the heap
//! allocations per step, counted by fluxbench's `TrackingAllocator`.

use fluxbench::{TrackingAllocator, current_allocation, reset_allocation_counter};
use zenith_editor_bench::Fixture;

#[global_allocator]
static GLOBAL: TrackingAllocator = TrackingAllocator;

/// The scenario names.
const SCENARIOS: [&str; 8] = [
    "typing",
    "typing_batch",
    "typing_heavy",
    "drag",
    "drag_snap",
    "drag_multi",
    "buffer",
    "render",
];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // `cargo bench` passes `--bench`: run every scenario once.
    let (which, iterations) = match args.as_slice() {
        [name, n] => (name.as_str(), n.parse::<u32>().unwrap_or(1)),
        [name] if !name.starts_with('-') => (name.as_str(), 1),
        _ => ("all", 1),
    };
    let fx = Fixture::new();
    let names: Vec<&str> = if which == "all" {
        SCENARIOS.to_vec()
    } else {
        vec![which]
    };
    for name in names {
        fx.run(name, fx.start(name));
        reset_allocation_counter();
        let mut work = None;
        for _ in 0..iterations {
            work = Some(fx.run(name, fx.start(name)));
        }
        let (bytes, count) = current_allocation();
        let per = u64::from(iterations.max(1));
        println!(
            "{name}: work {:?}, allocations {} ({} bytes) per step",
            work,
            count / per,
            bytes / per
        );
    }
}
