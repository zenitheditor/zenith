---
description: Render PNG or SVG row pages from a .zen template and CSV.
argument-hint: "[template.zen] [data.csv] [png | svg]"
allowed-tools:
  - Bash(zenith:*)
  - Read
  - Write
  - Glob
---

Mail-merge task: **$ARGUMENTS**

1. Bind text and image nodes with `role="data.<column>"`, matching CSV headers. Preserve ids and tokens.
2. Run `zenith validate <template> --json`. Correct hard diagnostics before batch rendering.
3. Select PNG or SVG. PNG is the default.

```bash
zenith merge <template.zen> <data.csv> --out-dir out/ --name-by <col> --manifest manifest.json --json
zenith merge <template.zen> <data.csv> --out-dir out/ --format svg --raster-scale 2 --manifest manifest.json --json
```

- **SVG resolution:** `--raster-scale F` requires SVG and finite `0 < F <= 4`. Vector geometry remains unchanged.
- **Fallback policy:** Document, local, and global config policy apply. Merge has no `--allow`, `--warn`, or `--deny`.
- **Denied fallback:** The affected row writes no pages. Other successful rows can remain.
- **Partial rows:** A later write error can leave earlier committed pages. Report their paths and row diagnostics.
- **Manifest errors:** Report the manifest error alongside actual row counts and committed paths. There is no batch rollback.
- **Critique:** Inspect PNG previews, including the longest text row. Generate PNG previews separately for SVG batches.
- **Delivery:** Report successful and failed rows, actual committed files, and the manifest path when written.

Run `zenith merge --help` for exact flags.
