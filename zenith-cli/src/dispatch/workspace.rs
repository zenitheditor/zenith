//! Dispatch logic for `zenith workspace`.

use std::process::ExitCode;

use crate::cli::{self, WorkspaceArgs};
use crate::commands;
use crate::report::CliError;

use super::output::warn;

pub(super) fn dispatch_workspace(args: WorkspaceArgs) -> ExitCode {
    match args.command {
        cli::WorkspaceSub::Scratch(scratch_args) => match scratch_args.command {
            cli::ScratchSub::New(a) => {
                let doc_bytes = match std::fs::read(&a.doc) {
                    Ok(b) => b,
                    Err(e) => {
                        return CliError::new(
                            "io.read_failed",
                            format!("error reading '{}': {}", a.doc.display(), e),
                            2,
                        )
                        .emit(false);
                    }
                };
                match commands::workspace::scratch_new(&doc_bytes, &a.doc, &a) {
                    Ok(outcome) => {
                        if let Some(w) = &outcome.warning {
                            warn(w);
                        }
                        println!("{}", outcome.id);
                        ExitCode::SUCCESS
                    }
                    Err(e) => CliError::new("workspace.failed", e.to_string(), 2).emit(false),
                }
            }
            cli::ScratchSub::List(a) => match commands::workspace::scratch_list(&a.doc, a.json) {
                Ok(out) => {
                    println!("{}", out);
                    ExitCode::SUCCESS
                }
                Err(e) => CliError::new("workspace.failed", e.to_string(), 2).emit(a.json),
            },
            cli::ScratchSub::Show(a) => {
                match commands::workspace::scratch_show(&a.doc, &a.candidate, a.json) {
                    Ok(out) => {
                        println!("{}", out);
                        ExitCode::SUCCESS
                    }
                    Err(e) => CliError::new("workspace.failed", e.to_string(), 2).emit(a.json),
                }
            }
        },
        cli::WorkspaceSub::Candidate(a) => {
            match commands::workspace::candidate_set_status(&a.doc, &a.candidate, &a.status) {
                Ok(out) => {
                    println!("{}", out);
                    ExitCode::SUCCESS
                }
                Err(e) => CliError::new("workspace.failed", e.to_string(), 2).emit(false),
            }
        }
        cli::WorkspaceSub::Promote(a) => {
            match commands::workspace::promote(&a.doc, &a.candidate, &a.into, &a.id_suffix) {
                Ok(out) => {
                    println!("{}", out);
                    ExitCode::SUCCESS
                }
                Err(e) => CliError::new("workspace.failed", e.to_string(), 2).emit(false),
            }
        }
        cli::WorkspaceSub::Finalize(a) => match commands::workspace::finalize(&a.doc, a.json) {
            Ok(out) => {
                println!("{}", out);
                ExitCode::SUCCESS
            }
            Err(e) => CliError::new("workspace.failed", e.to_string(), 2).emit(a.json),
        },
        cli::WorkspaceSub::Bundle(a) => match commands::workspace::bundle_doc(&a.doc, &a.out) {
            Ok(out) => {
                println!("{}", out);
                ExitCode::SUCCESS
            }
            Err(e) => CliError::new("workspace.failed", e.to_string(), 2).emit(false),
        },
        cli::WorkspaceSub::Unbundle(a) => match commands::workspace::unbundle_doc(&a.bundle) {
            Ok(doc_id) => {
                println!("{}", doc_id);
                ExitCode::SUCCESS
            }
            Err(e) => CliError::new("workspace.failed", e.to_string(), 2).emit(false),
        },
    }
}
