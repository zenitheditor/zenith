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
- For each hard (Error) diagnostic: the code, the offending node id, and a concrete fix.

Treat every Error as blocking. If asked, apply the fixes at the source (tokenize literals, fix
overflow/contrast, resolve missing assets/tokens), then re-validate. Do not finalize while hard
diagnostics remain.
