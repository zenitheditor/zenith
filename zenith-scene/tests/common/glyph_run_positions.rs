//! Helper shared by the binaries that compare glyph-run origins.

use zenith_scene::ir::SceneCommand;

/// Collect every `DrawGlyphRun` `(x, y)` in command order.
pub fn glyph_run_positions(cmds: &[SceneCommand]) -> Vec<(f64, f64)> {
    cmds.iter()
        .filter_map(|c| match c {
            SceneCommand::DrawGlyphRun { x, y, .. } => Some((*x, *y)),
            _ => None,
        })
        .collect()
}
