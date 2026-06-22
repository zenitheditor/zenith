# Variants & mail-merge (`zenith merge`)

Turn **one template + a data table into many rendered designs** — deterministically. This is
the high-volume path: localized posts (one row per language), personalized graphics (one row
per recipient), campaign/product variants, certificates, badges, price cards. One template,
N rows, N PNGs, each reproducible.

## How it works

1. Author a normal `.zen` template (tokens, layout, stable ids — all the usual discipline).
2. Mark the **variable** nodes with `role="data.<column>"`, where `<column>` matches a CSV
   header. Supported on:
   - **text nodes** — the node's text is replaced per row.
   - **image nodes** — the node's asset path is replaced per row (the CSV cell is a path).
     A `data.*` role on any other node kind is an error.
3. Provide a CSV whose header row names the columns.
4. Run `merge` — one render per data row.

```kdl
# in the template, the headline is bound to the CSV "name" column:
text id="t.name" role="data.name" x=(px)60 y=(px)160 w=(px)680 h=(px)90
     fill=(token)"color.ink" font-family=(token)"font.h" font-size=(token)"size.h" { span "PLACEHOLDER" }
# a per-row image:
image id="img.logo" role="data.logo" asset="asset.placeholder" x=(px)60 y=(px)40 w=(px)160 h=(px)60 fit="contain"
```

```csv
name,logo
Alice,assets/alice.png
Bob,assets/bob.png
```

## The command

```bash
zenith merge <template.zen> <data.csv> --out-dir out/ \
    [--name-by <column>] [--json] [--manifest manifest.json]
```

- `--out-dir <DIR>` (required) — one PNG per row is written here.
- `--name-by <COL>` — name each file by that column's value (e.g. `Alice.png`); default
  `row-NNNN.png`.
- `--json` — machine-readable batch report (per-row provenance: which row → which file).
- `--manifest <PATH>` — a deterministic generation manifest (schema `zenith-merge-manifest-v1`)
  recording the template `source_sha256`, the `data_sha256`, `name_by`, and per-row keys. Commit
  or archive it for **CI-reproducible** batches — same template + same data → same outputs.

## Workflow

1. Build the template and `zenith validate` it once — fix every hard diagnostic before batching
   (a broken template fails every row).
2. Keep the placeholder text/image realistic (e.g. a long sample name) so you can eyeball that
   the box fits the widest row; text-fit/overflow is per-row, so the longest value matters.
3. Dry-run small: merge the first few rows, open a couple of PNGs, then run the full set.
4. For production/CI, pass `--manifest` (and render assets with `--locked` where applicable via
   the template's `sha256` asset hashes) so the batch is auditable and reproducible.

## Tips

- Everything stays tokenized — a brand/palette change re-renders all variants from one edit
  (`references/brand.md`, `references/themes.md`).
- Pages vs rows: `merge` varies **content** across CSV rows; different **sizes** (square/story/
  banner) are separate pages in the template — see `references/layout.md`.
- Localization: one column per text slot, one row per locale; keep type large enough for the
  longest translation.

Run `zenith merge --help` for the exact flags.
