//! Pure logic for `zenith fix`.
//!
//! [`run`] works on in-memory source. The caller reads the file and, with
//! `--apply`, writes `outcome.source_after`.

use std::path::Path;

use serde::Serialize;
use zenith_core::fix::{AppliedFix, FixOutcome, fix_source, unified_diff};

use crate::commands::validate::{Collected, collect};
use crate::commands::{format_diagnostic_line, serialize_pretty};
use crate::config::CliPolicyFlags;
use crate::json_types::DiagnosticJson;

/// An error that stops `zenith fix` before any fix runs.
#[derive(Debug)]
pub struct FixCmdErr {
    /// Human-readable message.
    pub message: String,
    /// Exit code (2: the document does not parse).
    pub exit_code: u8,
}

/// The rendered result of a fix run.
#[derive(Debug)]
pub struct FixCmdOutcome {
    /// The core fix result.
    pub outcome: FixOutcome,
    /// What `zenith validate` reports for `outcome.source_after`, including
    /// compile-stage diagnostics once no error remains.
    pub remaining: Collected,
    /// Human summary plus unified diff.
    pub human: String,
    /// The `zenith-fix-v1` JSON envelope.
    pub json_str: String,
    /// The validate exit code of `remaining`: 0 clean, 1 errors remain.
    pub exit_code: u8,
}

/// One applied fix in the JSON envelope.
#[derive(Debug, Serialize)]
pub struct AppliedFixJson {
    pub code: String,
    pub subject_id: String,
    pub property: String,
    pub from: String,
    pub to: String,
}

/// The `zenith-fix-v1` JSON envelope.
#[derive(Debug, Serialize)]
pub struct FixOutputJson {
    pub schema: &'static str,
    pub applied: Vec<AppliedFixJson>,
    pub remaining: Vec<DiagnosticJson>,
}

/// Fix `src` and render the result.
///
/// `label` names the file in the diff headers. `project_dir` is the
/// document's directory, for the validate pipeline (assets, config,
/// imports). `apply` only changes the summary wording; the caller writes the
/// file.
///
/// # Errors
///
/// Returns [`FixCmdErr`] with exit code 2 when `src` does not parse.
pub fn run(
    src: &str,
    label: &str,
    project_dir: Option<&Path>,
    apply: bool,
) -> Result<FixCmdOutcome, FixCmdErr> {
    let outcome = fix_source(src).map_err(|e| FixCmdErr {
        message: format!(
            "error[parse.error]: {}; fix the syntax, then run `zenith fix` again",
            e.message
        ),
        exit_code: 2,
    })?;
    let remaining = collect(
        &outcome.source_after,
        project_dir,
        &CliPolicyFlags::default(),
    );
    let exit_code = remaining.exit_code;
    let human = render_human(&outcome, &remaining, label, apply);
    let json_str = render_json(&outcome, &remaining);
    Ok(FixCmdOutcome {
        outcome,
        remaining,
        human,
        json_str,
        exit_code,
    })
}

fn applied_line(f: &AppliedFix) -> String {
    format!(
        "  {} ({}) {}: {} → {}",
        f.code, f.subject_id, f.property, f.from, f.to
    )
}

fn render_human(outcome: &FixOutcome, remaining: &Collected, label: &str, apply: bool) -> String {
    let errors = remaining
        .diagnostics
        .iter()
        .filter(|d| d.is_error())
        .count();
    let mode = match (apply, outcome.changed()) {
        (_, false) => "nothing to write",
        (true, true) => "written",
        (false, true) => "dry-run; pass --apply to write",
    };
    let mut out = format!(
        "fix: {} applied, {errors} error(s) remaining ({mode})",
        outcome.applied.len()
    );
    if !outcome.applied.is_empty() {
        out.push_str("\napplied:");
        for f in &outcome.applied {
            out.push('\n');
            out.push_str(&applied_line(f));
        }
    }
    if !outcome.minted.is_empty() {
        out.push_str("\nminted tokens:");
        for t in &outcome.minted {
            out.push_str(&format!("\n  {} ({}) = {}", t.id, t.token_type, t.value));
        }
    }
    if !remaining.diagnostics.is_empty() {
        out.push_str("\nremaining:");
        for d in &remaining.diagnostics {
            out.push_str("\n  ");
            out.push_str(&format_diagnostic_line(d));
        }
    }
    let diff = unified_diff(label, &outcome.source_before, &outcome.source_after);
    if !diff.is_empty() {
        out.push('\n');
        out.push_str(diff.trim_end());
    }
    out
}

fn render_json(outcome: &FixOutcome, remaining: &Collected) -> String {
    serialize_pretty(&FixOutputJson {
        schema: "zenith-fix-v1",
        applied: outcome
            .applied
            .iter()
            .map(|f| AppliedFixJson {
                code: f.code.clone(),
                subject_id: f.subject_id.clone(),
                property: f.property.clone(),
                from: f.from.clone(),
                to: f.to.clone(),
            })
            .collect(),
        remaining: DiagnosticJson::located_all_in(
            &remaining.diagnostics,
            &outcome.source_after,
            &remaining.files,
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = r##"zenith version=1 {
  project id="proj.f" name="Fix"
  tokens format="zenith-token-v1" {
    token id="color.ink" type="color" value="#112233"
  }
  styles {
  }
  document id="doc.f" title="Fix" {
    page id="page.f" w=(px)200 h=(px)200 {
      rect id="r.1" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill="#112233"
    }
  }
}
"##;

    #[test]
    fn exact_token_fix_reports_and_diffs() {
        let out = run(DOC, "d.zen", None, false).expect("parses");
        assert_eq!(out.exit_code, 0, "{}", out.human);
        assert!(
            out.human.contains(
                "token.raw_visual_literal (r.1) fill: \"#112233\" → (token)\"color.ink\""
            ),
            "{}",
            out.human
        );
        assert!(out.human.contains("--- a/d.zen"), "{}", out.human);
        assert!(out.json_str.contains("\"schema\": \"zenith-fix-v1\""));
    }

    #[test]
    fn parse_error_is_exit_two() {
        let err = run("not {{{ kdl", "d.zen", None, false).expect_err("must fail");
        assert_eq!(err.exit_code, 2);
    }
}
