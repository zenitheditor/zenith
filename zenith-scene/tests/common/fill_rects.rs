//! Helper shared by the binaries that read `FillRect` geometry.

use zenith_scene::CompileResult;
use zenith_scene::ir::SceneCommand;

/// Collect every FillRect command's (x, y, w, h) in emission order.
pub fn fill_rects(result: &CompileResult) -> Vec<(f64, f64, f64, f64)> {
    result
        .scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::FillRect { x, y, w, h, .. } => Some((*x, *y, *w, *h)),
            _ => None,
        })
        .collect()
}
