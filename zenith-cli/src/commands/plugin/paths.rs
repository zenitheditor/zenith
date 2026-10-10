//! Per-(agent, scope) target paths for the Zenith skill.

use std::path::{Path, PathBuf};

use super::agent::{Agent, Scope};

/// The skill's stable name — used as directory slug and file stem.
pub const SKILL_NAME: &str = "zenith";

/// Where a skill is written for an agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillTarget {
    /// A folder skill: the whole tree is written under this directory.
    Folder(PathBuf),
    /// A single rule/markdown file written at this path.
    Rule(PathBuf),
}

impl SkillTarget {
    /// The root path on disk (the directory for a folder, the file for a rule).
    pub fn root(&self) -> &Path {
        match self {
            SkillTarget::Folder(p) | SkillTarget::Rule(p) => p,
        }
    }
}

/// Resolve `$HOME` for user-scope installs.
pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Compute the skill target for `(agent, scope)`, with `project_root` as the
/// base for project-scope paths. Returns `None` when the combination has no
/// automatic filesystem target (e.g. Cursor/Windsurf user scope are UI-only).
pub fn skill_target(agent: Agent, scope: Scope, project_root: &Path) -> Option<SkillTarget> {
    // Folder agents: a `<base>/<conf>/skills/zenith/` directory.
    let folder = |conf_project: &Path, conf_user: Option<PathBuf>| -> Option<SkillTarget> {
        let base = match scope {
            Scope::Project => project_root.join(conf_project),
            Scope::User => conf_user?,
        };
        Some(SkillTarget::Folder(base.join("skills").join(SKILL_NAME)))
    };
    // Rule agents: a single file `<dir>/<file>`.
    let rule =
        |dir_project: PathBuf, dir_user: Option<PathBuf>, file: &str| -> Option<SkillTarget> {
            let dir = match scope {
                Scope::Project => project_root.join(dir_project),
                Scope::User => dir_user?,
            };
            Some(SkillTarget::Rule(dir.join(file)))
        };
    let home = home_dir();

    match agent {
        Agent::ClaudeCode => folder(
            Path::new(".claude"),
            home.as_ref().map(|h| h.join(".claude")),
        ),
        // Codex adopts the cross-agent `.agents/skills/` location.
        Agent::Codex => folder(
            Path::new(".agents"),
            home.as_ref().map(|h| h.join(".agents")),
        ),
        // OpenCode: project `.opencode/`, global `~/.config/opencode/`.
        Agent::OpenCode => folder(
            Path::new(".opencode"),
            home.as_ref().map(|h| h.join(".config").join("opencode")),
        ),
        // Cursor rules — project-only (`.mdc`); user scope is UI-managed.
        Agent::Cursor => rule(PathBuf::from(".cursor").join("rules"), None, "zenith.mdc"),
        // Windsurf rules — project-only; user scope is UI-managed.
        Agent::Windsurf => rule(PathBuf::from(".windsurf").join("rules"), None, "zenith.md"),
        Agent::Aider => rule(
            PathBuf::from(".aider"),
            home.as_ref().map(|h| h.join(".aider")),
            "zenith-skill.md",
        ),
        Agent::Zed => rule(
            PathBuf::from(".zed"),
            home.as_ref().map(|h| h.join(".zed")),
            "zenith-skill.md",
        ),
        Agent::Gemini => rule(
            PathBuf::from(".gemini"),
            home.as_ref().map(|h| h.join(".gemini")),
            "zenith-skill.md",
        ),
        Agent::Copilot => rule(
            PathBuf::from(".copilot"),
            home.as_ref().map(|h| h.join(".copilot")),
            "zenith-skill.md",
        ),
        Agent::Continue => rule(
            PathBuf::from(".continue").join("skills"),
            home.as_ref().map(|h| h.join(".continue").join("skills")),
            "zenith.md",
        ),
        Agent::Kiro => rule(
            PathBuf::from(".kiro").join("steering"),
            home.as_ref().map(|h| h.join(".kiro").join("steering")),
            "zenith-skill.md",
        ),
        Agent::Antigravity => rule(
            PathBuf::from(".antigravity"),
            home.as_ref().map(|h| h.join(".antigravity")),
            "zenith-skill.md",
        ),
    }
}

/// Slash-command directory for agents that support project commands. Returns
/// `None` for agents with no known command convention.
pub fn command_dir(agent: Agent, scope: Scope, project_root: &Path) -> Option<PathBuf> {
    match agent {
        Agent::ClaudeCode => Some(match scope {
            Scope::Project => project_root.join(".claude").join("commands"),
            Scope::User => home_dir()?.join(".claude").join("commands"),
        }),
        Agent::OpenCode => Some(match scope {
            Scope::Project => project_root.join(".opencode").join("command"),
            Scope::User => home_dir()?.join(".config").join("opencode").join("command"),
        }),
        Agent::Codex
        | Agent::Cursor
        | Agent::Windsurf
        | Agent::Aider
        | Agent::Zed
        | Agent::Gemini
        | Agent::Copilot
        | Agent::Continue
        | Agent::Kiro
        | Agent::Antigravity => None,
    }
}
