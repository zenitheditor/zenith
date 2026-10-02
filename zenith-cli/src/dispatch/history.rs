//! Dispatch for the history commands: `history`, `undo`, `redo`, `version`,
//! `restore`, and `sync`.

use std::process::ExitCode;

use crate::cli::{HistoryArgs, RedoArgs, RestoreArgs, SyncArgs, UndoArgs, VersionArgs};
use crate::history;
use crate::report::CliError;

use super::output::warn;

pub(super) fn dispatch_history(args: HistoryArgs) -> ExitCode {
    let view = match history::history_view(&args.path) {
        Ok(view) => view,
        Err(msg) => return CliError::new("history.failed", msg, 2).emit(args.json),
    };
    if args.json {
        let versions_json: Vec<serde_json::Value> = view
            .versions
            .iter()
            .map(|v| {
                serde_json::json!({
                    "id": v.id,
                    "seq": v.seq,
                    "label": v.label,
                    "op_kind": v.op_kind,
                    "timestamp_ms": v.timestamp_ms,
                })
            })
            .collect();
        let obj = serde_json::json!({
            "doc_id": view.doc_id,
            "has_session": view.has_session,
            "versions": versions_json,
        });
        println!("{}", crate::commands::serialize_pretty(&obj));
        return ExitCode::SUCCESS;
    }
    println!("doc-id: {}", view.doc_id);
    if view.versions.is_empty() {
        println!("(no versions recorded yet)");
    }
    for v in &view.versions {
        let label = v.label.as_deref().unwrap_or("");
        let op = v.op_kind.as_deref().unwrap_or("");
        println!("{:>4}  {}  {} {}", v.seq, v.id, op, label);
    }
    ExitCode::SUCCESS
}

pub(super) fn dispatch_undo(args: UndoArgs) -> ExitCode {
    match history::undo_edit(&args.path) {
        Ok(history::NavOutcome::Moved) => {
            println!("undid last edit to '{}'", args.path.display());
            ExitCode::SUCCESS
        }
        Ok(history::NavOutcome::NothingToDo) => {
            println!("nothing to undo");
            ExitCode::SUCCESS
        }
        Err(msg) => CliError::new("history.undo_failed", msg, 2).emit(false),
    }
}

pub(super) fn dispatch_redo(args: RedoArgs) -> ExitCode {
    match history::redo_edit(&args.path) {
        Ok(history::NavOutcome::Moved) => {
            println!("redid last undone edit to '{}'", args.path.display());
            ExitCode::SUCCESS
        }
        Ok(history::NavOutcome::NothingToDo) => {
            println!("nothing to redo");
            ExitCode::SUCCESS
        }
        Err(msg) => CliError::new("history.redo_failed", msg, 2).emit(false),
    }
}

pub(super) fn dispatch_version(args: VersionArgs) -> ExitCode {
    match history::name_version(&args.path, &args.name) {
        Ok(id) => {
            println!("saved version '{}' as {}", args.name, id);
            ExitCode::SUCCESS
        }
        Err(msg) => CliError::new("history.version_failed", msg, 2).emit(false),
    }
}

pub(super) fn dispatch_restore(args: RestoreArgs) -> ExitCode {
    match history::restore(&args.path, &args.rev) {
        Ok(outcome) => {
            if let Some(w) = &outcome.warning {
                warn(w);
            }
            println!(
                "restored '{}' to {}",
                args.path.display(),
                outcome.version_id
            );
            ExitCode::SUCCESS
        }
        Err(msg) => CliError::new("history.restore_failed", msg, 2).emit(false),
    }
}

pub(super) fn dispatch_sync(args: SyncArgs) -> ExitCode {
    match history::sync_external(&args.path) {
        Ok(history::SyncOutcome::Captured { id }) => {
            println!("captured external change as {id}");
            ExitCode::SUCCESS
        }
        Ok(history::SyncOutcome::AlreadyInSync) => {
            println!("already in sync");
            ExitCode::SUCCESS
        }
        Err(msg) => CliError::new("history.sync_failed", msg, 2).emit(false),
    }
}
