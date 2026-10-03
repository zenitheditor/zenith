---
description: Validate a .zen document and report diagnostics (no changes made).
argument-hint: "[path to .zen file]"
allowed-tools:
  - Bash(zenith:*)
  - Glob
---

Validate the Zenith document at: **$ARGUMENTS** (if empty, find the relevant `.zen` file).

Run `zenith validate <file> --json` and report:

- Pass/fail and the count of Error / Warning / Advisory diagnostics.
- For each Error: the code, the node id, and a concrete fix.
- Which diagnostics `zenith fix <file>` (dry-run) resolves.

Every Error blocks. If asked to fix, run `zenith fix <file> --apply`, fix the rest at the source, then re-validate.
