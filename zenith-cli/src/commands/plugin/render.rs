//! Render the per-agent contents of a rule-format skill file.
//!
//! Folder-format agents receive the embedded tree verbatim (its `SKILL.md`
//! frontmatter already follows the Agent Skills standard), so only the
//! single-file rule agents need rendering here.

use super::agent::Agent;
use super::assets::{SKILL_FILES, skill_description, skill_file, skill_md_body};

const RULE_NOTE: &str = "> **Single-file install.** Authoring and export constraints appear below. \
Repository links provide optional guidance. No bundled reference files are installed. \
Run `zenith --help` and `zenith <command> --help` for exact flags.";

fn rule_body() -> String {
    let mut body = [
        skill_file("references/authoring-workflow.md").unwrap_or_default(),
        skill_file("references/export.md").unwrap_or_default(),
    ]
    .join("\n");
    body = body.replace(
        "Read the matching section of `references/by-kind.md` before authoring new work.",
        "Consult `references/by-kind.md` for optional guidance by brief.",
    );
    body = body.replace(
        "Read `variants.md` for copyable batch commands and data bindings.",
        "Consult `variants.md` for optional batch guidance and data bindings.",
    );
    let mut routes: Vec<_> = SKILL_FILES
        .iter()
        .filter(|(path, _)| path.starts_with("references/") || path.starts_with("templates/"))
        .map(|(path, _)| *path)
        .collect();
    routes.sort_by_key(|path| std::cmp::Reverse(path.len()));
    for path in routes {
        let link = format!(
            "[{path}](https://github.com/zenitheditor/zenith/blob/main/zenith-cli/assets/skill/{path})"
        );
        body = body.replace(&format!("`{path}`"), &link);
        if let Some(name) = path.strip_prefix("references/") {
            body = body.replace(&format!("`{name}`"), &link);
        }
    }
    body
}

/// Render the file contents for a rule-format `agent`.
pub fn render_rule(agent: Agent) -> String {
    let body = if agent.format() == super::agent::SkillFormat::Folder {
        skill_md_body().to_owned()
    } else {
        rule_body()
    };
    let desc = skill_description().unwrap_or_default();
    match agent {
        // Cursor `.mdc` rules: frontmatter with `description` + `alwaysApply`.
        Agent::Cursor => format!(
            "---\nalwaysApply: false\ndescription: {desc}\n---\n\n{RULE_NOTE}\n\n{body}",
            desc = yaml_scalar(&desc),
        ),
        // Windsurf rules: bare markdown.
        Agent::Windsurf => format!("{RULE_NOTE}\n\n{body}"),
        // Everything else: plain markdown with an identifying H1.
        Agent::Aider
        | Agent::Zed
        | Agent::Gemini
        | Agent::Copilot
        | Agent::Continue
        | Agent::Kiro
        | Agent::Antigravity => format!("# Zenith\n\n{RULE_NOTE}\n\n{body}"),
        // Folder agents never reach here.
        Agent::ClaudeCode | Agent::Codex | Agent::OpenCode => body,
    }
}

/// Quote a YAML scalar only when it contains characters that would otherwise
/// break parsing.
fn yaml_scalar(s: &str) -> String {
    let needs = s.contains(':')
        || s.contains('#')
        || s.contains('"')
        || s.contains('\'')
        || s.starts_with(char::is_whitespace)
        || s.ends_with(char::is_whitespace);
    if needs {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        s.to_owned()
    }
}
