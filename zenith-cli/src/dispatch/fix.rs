//! Dispatch for `zenith fix`.
//!
//! With `--apply`, the fixed source reaches disk (and history) before the
//! result prints, so a write error is the only output.

use std::process::ExitCode;

use crate::cli::FixArgs;
use crate::cli_helpers::read_file;
use crate::commands;
use crate::report::CliError;

use super::output::{apply_edit, print_outcome};

pub(super) fn dispatch_fix(args: FixArgs) -> ExitCode {
    let json = args.json;
    let src = match read_file(&args.path) {
        Ok(s) => s,
        Err(e) => return e.emit(json),
    };
    let label = args.path.display().to_string();
    let out = match commands::fix::run(&src, &label, args.path.parent(), args.apply) {
        Ok(o) => o,
        Err(e) => return CliError::new("fix.failed", e.message, e.exit_code).emit(json),
    };
    if args.apply
        && out.outcome.changed()
        && let Err(e) = apply_edit(&args.path, out.outcome.source_after.as_bytes(), "fix.apply")
    {
        return e.emit(json);
    }
    print_outcome(json, &out.json_str, &out.human);
    ExitCode::from(out.exit_code)
}
