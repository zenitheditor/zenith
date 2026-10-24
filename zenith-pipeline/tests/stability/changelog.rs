//! The corpus freeze matches a CHANGELOG.md section.
//!
//! - `FROZEN_AT` at or below the workspace version names a released
//!   `## [X.Y.Z]` section.
//! - `FROZEN_AT` above the workspace version is a refresh for the next
//!   release, so `## [Unreleased]` stands for it.
//! - Any refresh after the first freeze is a declared break. Its section must
//!   hold a `### Breaking` heading.

use super::frozen::{corpus_dir, frozen_at, parse_version};

/// The version of the first corpus freeze. It declares no break.
const FIRST_FREEZE: &str = "0.0.14";

/// CHANGELOG.md, relative to the crate directory.
const CHANGELOG: &str = "../CHANGELOG.md";

/// The body of the `## [<name>]` section of `changelog`: every line up to the
/// next `## ` heading. `None` when there is no such section.
fn section<'a>(changelog: &'a str, name: &str) -> Option<Vec<&'a str>> {
    let heading = format!("## [{name}]");
    let mut lines = changelog.lines();
    lines.find(|line| {
        line.strip_prefix(&heading)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
    })?;
    Some(lines.take_while(|line| !line.starts_with("## ")).collect())
}

#[test]
fn changelog_has_a_section_for_the_corpus_freeze() {
    let version = frozen_at(&corpus_dir());
    let frozen = parse_version(&version).expect("FROZEN_AT is X.Y.Z");
    let workspace = parse_version(env!("CARGO_PKG_VERSION")).expect("workspace version X.Y.Z");
    let name = if frozen > workspace {
        "Unreleased"
    } else {
        version.as_str()
    };
    let changelog = std::fs::read_to_string(CHANGELOG)
        .unwrap_or_else(|e| panic!("cannot read {CHANGELOG}: {e}"));
    let Some(body) = section(&changelog, name) else {
        panic!(
            "corpus FROZEN_AT is {version}, but CHANGELOG.md has no `## [{name}]` section. \
             Add it in the commit that refreshes the corpus"
        );
    };
    if version != FIRST_FREEZE {
        assert!(
            body.iter().any(|line| line.trim_end() == "### Breaking"),
            "corpus refreshed at {version}: CHANGELOG.md `## [{name}]` needs a `### Breaking` \
             section that lists the break"
        );
    }
}

#[test]
fn section_reads_up_to_the_next_release_heading() {
    let text = "## [Unreleased]\n\n### Breaking\n\n- x\n\n## [0.0.1] — d\n\n### Added\n";
    let unreleased = section(text, "Unreleased").expect("unreleased section");
    assert!(unreleased.contains(&"### Breaking"));
    assert!(!unreleased.contains(&"### Added"));
    let released = section(text, "0.0.1").expect("released section");
    assert_eq!(released, vec!["", "### Added"]);
    assert!(section(text, "0.0.2").is_none());
    assert!(section("## [0.0.10]\n", "0.0.1").is_none());
}
