//! Helper shared by the chain binaries that count glyph runs by baseline band.

use zenith_scene::ir::SceneCommand;

/// Count the `DrawGlyphRun` commands whose baseline `y` falls in `[lo, hi)`.
/// Used to attribute glyph runs to a particular chain member's box.
pub fn glyph_runs_in_y(cmds: &[SceneCommand], lo: f64, hi: f64) -> usize {
    cmds.iter()
        .filter(|c| match c {
            SceneCommand::DrawGlyphRun { y, .. } => *y >= lo && *y < hi,
            _ => false,
        })
        .count()
}
