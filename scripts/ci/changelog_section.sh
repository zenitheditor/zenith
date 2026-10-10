#!/usr/bin/env bash
# Extract one version's section from CHANGELOG.md.
#
#   scripts/ci/changelog_section.sh <base_version> [out_file]   # e.g. 0.0.11
#
# Writes the body of `## [<base_version>]` — everything up to the next `## [` —
# to `out_file` (default /tmp/changelog-section.md) and echoes it. Exits
# non-zero if CHANGELOG.md is missing or that section is absent or empty.
#
# Called by release.yml twice: as the gate in `validate-version`, and to build
# the GitHub release body. One implementation, so the check and the notes
# never disagree about what a section contains.
#
# Pass the BASE version: a `v0.0.12-beta.1` tag is a candidate for the
# `[0.0.12]` section, so a prerelease rehearses the notes of the final release.
#
# Run it from the repository root.

set -euo pipefail

VERSION="${1:?usage: changelog_section.sh <base_version> [out_file]}"
OUTPUT="${2:-/tmp/changelog-section.md}"

if [[ ! -f CHANGELOG.md ]]; then
    echo "::error::CHANGELOG.md is required to cut a release. Run this script from the repository root."
    exit 1
fi

# Leading and trailing blank lines and the `---` rule between sections are
# page furniture, not release notes. The version is matched literally, so `.`
# is not a wildcard.
awk -v version="$VERSION" '
    index($0, "## [" version "]") == 1 { found = 1; next }
    found && /^## \[/ { exit }
    found && n == 0 && /^[[:space:]]*$/ { next }
    found { lines[n++] = $0 }
    END {
        while (n > 0 && (lines[n - 1] ~ /^[[:space:]]*$/ || lines[n - 1] ~ /^-{3,}[[:space:]]*$/)) {
            n--
        }
        for (i = 0; i < n; i++) print lines[i]
    }
' CHANGELOG.md > "$OUTPUT"

if ! grep -q '[^[:space:]]' "$OUTPUT"; then
    echo "::error::CHANGELOG.md has no non-empty '## [$VERSION]' section. Add a '## [$VERSION] — YYYY-MM-DD' section that describes the release, then push the tag again."
    exit 1
fi

echo "Changelog section for [$VERSION]:"
cat "$OUTPUT"
