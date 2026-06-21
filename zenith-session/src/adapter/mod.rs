//! Injected adapter traits for filesystem, clock, and RNG access.
//!
//! Module root: declarations and re-exports only.

pub mod clock;
pub mod fs;
pub mod rng;

pub use clock::{Clock, FakeClock, OsClock};
pub use fs::{Fs, MemFs, OsFs};
pub use rng::{FakeRng, OsRng, Rng};
