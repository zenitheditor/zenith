//! Dispatch for `zenith reflow`.

use std::process::ExitCode;

use crate::cli::ReflowArgs;
use crate::cli_helpers::read_file;
use crate::commands::reflow::{ReflowJson, run_reflow};
use crate::commands::serialize_pretty;
use crate::report::CliError;

use super::output::write_error;

pub(super) fn dispatch_reflow(args: ReflowArgs) -> ExitCode {
    let json = args.json;
    let doc_src = match read_file(&args.path) {
        Ok(s) => s,
        Err(e) => return e.emit(json),
    };

    let report = match run_reflow(&doc_src, args.path.parent(), args.max_pages) {
        Ok(r) => r,
        Err(e) => {
            return CliError::new(e.code, format!("error[{}]: {}", e.code, e.message), 2)
                .emit(json);
        }
    };

    let mut applied = false;
    if args.apply && !report.spawned_pages.is_empty() {
        let mut guard = match crate::output_file::OutputGuard::new(&[]) {
            Ok(g) => g,
            Err(e) => return write_error(&args.path, &e).emit(json),
        };
        if let Err(e) = guard.write(&args.path, report.source_after.as_bytes()) {
            return write_error(&args.path, &e).emit(json);
        }
        applied = true;
    }

    let pages_added = report.spawned_pages.len();
    if json {
        let payload = ReflowJson {
            schema: "zenith-reflow-v1",
            path: args.path.display().to_string(),
            applied,
            pages_before: report.pages_before,
            pages_after: report.pages_after,
            pages_added,
            spawned_pages: report.spawned_pages,
            still_overflowing: report.still_overflowing,
            diff: report.diff,
        };
        println!("{}", serialize_pretty(&payload));
    } else if pages_added == 0 {
        println!(
            "no overflow detected ({} page(s)); nothing to reflow",
            report.pages_before
        );
    } else {
        let verb = if applied { "applied" } else { "dry run" };
        println!(
            "reflow ({verb}): spawned {} page(s) ({} -> {}): {}",
            pages_added,
            report.pages_before,
            report.pages_after,
            report.spawned_pages.join(", "),
        );
        if !applied {
            println!("add --apply to write '{}'", args.path.display());
        }
        if !report.diff.is_empty() {
            println!("{}", report.diff);
        }
    }

    if report.still_overflowing {
        if !json {
            eprintln!(
                "warning: chained text still overflows after spawning {} page(s)",
                pages_added
            );
        }
        ExitCode::from(1u8)
    } else {
        ExitCode::SUCCESS
    }
}
