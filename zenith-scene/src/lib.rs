//! Backend-neutral scene IR and scene compilation for Zenith.
//!
//! Owns the display-list primitives (FillRect, DrawGlyphRun, PushClip, etc.),
//! scene compilation from a validated AST and layout results into a z-ordered
//! display list, opacity/visibility/clip resolution, and exclusion of
//! non-printing nodes. This crate is the stable contract boundary that every
//! future backend — GPU, PDF, SVG export — will consume.
