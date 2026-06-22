---
description: Batch-render variants from a .zen template and a CSV (mail-merge).
argument-hint: "[template.zen] [data.csv]"
allowed-tools:
  - Bash(zenith:*)
  - Read
  - Write
  - Glob
---

Mail-merge task: **$ARGUMENTS**

Read `references/variants.md` from the zenith skill, then:

1. Ensure the template's variable nodes are bound with `role="data.<column>"` (text and/or
   image nodes), matching the CSV header columns. If the template isn't set up yet, add the
   roles (keep ids/tokens intact).
2. `zenith validate <template> --json` and fix any hard diagnostics first — a broken template
   fails every row.
3. Run the batch:
   `zenith merge <template.zen> <data.csv> --out-dir out/ --name-by <col> --manifest manifest.json`
4. Spot-check a couple of output PNGs (watch the longest row for text overflow), then report
   how many rows rendered and where the outputs + manifest are.

Run `zenith merge --help` for exact flags.
