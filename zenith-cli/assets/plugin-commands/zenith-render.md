---
description: Render a .zen document to PNG (or PDF / all pages) and report the output path.
argument-hint: "[path to .zen file] [optional: pdf | all-pages]"
allowed-tools:
  - Bash(zenith:*)
  - Glob
---

Render the Zenith document referenced by: **$ARGUMENTS**

1. Run `zenith validate <file> --json`. If Errors exist, report them and stop.
2. Render:
   - default → `zenith render <file> --png <file>.png`
   - `pdf` → `zenith render <file> --pdf <file>.pdf` (print-ready)
   - `all-pages` → `zenith render <file> --contact-sheet <file>.png` (one image, every page)
3. Report the output path(s). Flags such as `--page`, `--scale`, `--all-pages`: `zenith render --help`.
