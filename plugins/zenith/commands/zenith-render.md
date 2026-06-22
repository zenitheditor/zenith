---
description: Render a .zen document to PNG (or PDF / all pages) and report the output path.
argument-hint: "[path to .zen file] [optional: pdf | all-pages]"
allowed-tools:
  - Bash(zenith:*)
  - Glob
---

Render the Zenith document referenced by: **$ARGUMENTS**

1. First `zenith validate <file> --json`; if hard diagnostics exist, report them and stop (a
   broken document should not be silently rendered).
2. Then render:
   - default → `zenith render <file> --png <file>.png`
   - `pdf` → `zenith render <file> --pdf <file>.pdf` (print-ready)
   - `all-pages` → `zenith render <file> --all-pages <dir>/` (contact sheet, one PNG per page)
3. Report the output path(s). Run `zenith render --help` for `--page`, `--spread`, `--locked`.
