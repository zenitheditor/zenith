//! Text layout and frame geometry for Zenith.
//!
//! Owns the text-layout adapter trait (Parley is the first engine behind it),
//! text measurement and shaping into positioned glyph runs, frame and group
//! bbox resolution, and the layout result type consumed by scene compilation.
//! Downstream crates never see engine-specific types; they receive only
//! Zenith's own glyph-run records.
