//! Helper shared by the binaries that count wrapped text lines.

use zenith_scene::ir::SceneCommand;

/// Count distinct glyph-run baseline y values (≈ the number of text lines).
pub fn distinct_line_count(cmds: &[SceneCommand]) -> usize {
    let mut ys: std::collections::BTreeSet<i64> = std::collections::BTreeSet::new();
    for c in cmds {
        if let SceneCommand::DrawGlyphRun { y, .. } = c {
            ys.insert((*y * 100.0) as i64);
        }
    }
    ys.len()
}
