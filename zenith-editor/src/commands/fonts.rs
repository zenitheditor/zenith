//! `fonts.required`: the font faces the document needs and the build does
//! not hold.

use serde_json::{Value, json};
use zenith_core::FontMissLog;
use zenith_pipeline::validate_source;

use crate::ctx::Ctx;
use crate::error::EditorError;
use crate::fonts::{embedded_files, needed_faces};

/// Compile every page of the current text (as `doc.diagnose` does) while
/// recording each face the shaper asks for, and list the ones with no
/// face.
///
/// An entry whose `file` is set names a bundled font file: fetch it and
/// pass it in the project fonts. An entry with `file: null` is a family
/// that is neither bundled nor a project font asset; supply it as a font
/// asset in the project files. A document can ask for a further face only
/// after the first is supplied (an unresolved family falls back to Noto
/// Sans), so the page repeats until `faces` is empty. `complete` is
/// `false` when an error stops compilation, so the list can be short until
/// the error is fixed.
pub(crate) fn run(ctx: &mut Ctx<'_, '_>, _raw: Value) -> Result<Value, EditorError> {
    let log = FontMissLog::new();
    let env = ctx.env;
    ctx.work.validations += 1;
    let validation = validate_source(
        env.host.with_font_log(&log),
        &ctx.session.text,
        env.project_dir,
        env.flags,
    );
    Ok(json!({
        "faces": needed_faces(&log),
        "embedded": embedded_files(),
        "complete": validation.exit_code == 0,
    }))
}
