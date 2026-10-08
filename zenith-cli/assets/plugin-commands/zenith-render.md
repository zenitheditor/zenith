---
description: Render PNG, SVG, PDF, page batches, or a contact sheet and report committed paths.
argument-hint: "[path to .zen file] [png | svg | pdf | all-pages | all-pages-svg | contact-sheet]"
allowed-tools:
  - Bash(zenith:*)
  - Glob
---

Render the Zenith document referenced by: **$ARGUMENTS**

1. Run `zenith validate <file> --json`. If Errors exist, report them and stop.
2. Select the requested output. PNG is the default.

| Choice | Command |
| --- | --- |
| PNG | `zenith render <file> --png <file>.png --json` |
| SVG | `zenith render <file> --svg <file>.svg --json` |
| PDF | `zenith render <file> --pdf <file>.pdf --json` |
| All-pages PNG | `zenith render <file> --all-pages <DIR> --json` |
| All-pages SVG | `zenith render <file> --all-pages-svg <DIR> --json` |
| Contact sheet | `zenith render <file> --contact-sheet <file>.png --json` |

- **Page selection:** PNG and SVG default to page 1. PDF defaults to every page. Use `--page N` explicitly.
- **PNG resolution:** `--scale F` changes PNG dimensions. It requires finite `0 < F <= 4`.
- **Vector resolution:** `--raster-scale F` changes SVG/PDF fallback pixels only. Vector geometry remains unchanged.
- **Vector text:** SVG outlines text. Captured PDF text loses selection and search.
- **Fallback policy:** Check `render.svg_rasterized` and `render.pdf_rasterized`. Document and config policy apply. Render accepts `--allow`, `--warn`, and `--deny`.
- **Reports:** Report diagnostics, rasterized regions, and committed output paths from JSON.
- **Partial writes:** I/O errors can leave earlier outputs. Report committed paths even when status is blocked.
- **Critique:** Render and inspect a PNG preview when assessing visual quality.

Run `zenith render --help` for exact flags.
