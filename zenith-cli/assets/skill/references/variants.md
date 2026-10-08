# Variants

Zenith has two distinct "variant" tools — don't confuse them:

- **`zenith variant`** — one design at many **sizes/formats** (square/story/banner). Varies dimensions.
- **`zenith merge`** — one template + many **data rows** → many rendered outputs. Varies content.

---

## Size/format variants (`zenith variant`)

Turn **one canonical page into many named target sizes** — square, story, banner, ad slots — deterministically. This varies **dimensions** (and small per-target tweaks). Reach for `variant` for "the same design at 4 sizes". Reach for `merge` for "this design for 200 people".

For the `variants` block syntax, `override` props, and all command flags, run:

```bash
zenith variant --help
```

### Why it's reliable

- **Token propagation:** Variants inherit canonical page tokens. One token modification propagates to every size.
- **Anchored nodes:** Use `anchor` and `anchor-zone` from `layout.md` for placement across sizes. Free-coordinate decoration needs per-variant repositioning.
- **Deterministic.** Same source → byte-identical `.zen`, PNG or SVG, and manifest across runs.

### Workflow

1. Build and `zenith validate` the canonical page first — a broken source fails every variant. Variant-specific diagnostics: `variant.duplicate_id`, `variant.unknown_source`, `variant.invalid_dimension` (non-px or ≤ 0), `variant.override_unknown_node`.
2. Generate, then open a couple of the PNGs to eyeball reflow at the widest/tallest sizes.
3. For CI, pass `--manifest` and commit it so the batch is auditable and reproducible.

| Flag | Contract |
| --- | --- |
| `--format png` | Default image format. |
| `--format svg` | SVG images alongside `.zen` companions. |
| `--raster-scale F` | SVG fallback resolution. Finite `0 < F <= 4`, default 1. |
| `--manifest PATH` | Deterministic record of committed outputs. |
| `--json` | Per-variant report with diagnostics and committed paths. |

```bash
zenith variant poster.zen --out-dir out/ --format svg --raster-scale 2 --manifest run.json --json
```

Read `export.md` for fallback policy and partial output behavior.

---

## Data-driven variants / mail-merge (`zenith merge`)

Turn **one template + a data table into many rendered designs** — deterministically. Each row represents one locale, recipient, or product. Certificates and badges use the same bindings. One template, N rows, rendered PNG or SVG pages, each reproducible.

### How it works

1. Author a normal `.zen` template (tokens, layout, stable ids — all the usual discipline).
2. Mark the **variable** nodes with `role="data.<column>"`, where `<column>` matches a CSV header.
   - **Text nodes:** The row replaces the node's text.
   - **Image nodes:** The row replaces the asset path. The CSV cell contains a path.
   - **Other node kinds:** A `data.*` role produces an error.
3. Provide a CSV whose header row names the columns.
4. Run `merge` — one render per data row.

```kdl
// in the template, the headline is bound to the CSV "name" column:
text id="t.name" role="data.name" x=(px)60 y=(px)160 w=(px)680 h=(px)90
     fill=(token)"color.ink" font-family=(token)"font.h" font-size=(token)"size.h" { span "PLACEHOLDER" }
// a per-row image:
image id="img.logo" role="data.logo" asset="asset.placeholder" x=(px)60 y=(px)40 w=(px)160 h=(px)60 fit="contain"
```

```csv
name,logo
Alice,assets/alice.png
Bob,assets/bob.png
```

For the full command flags (including `--name-by`, `--manifest`, `--json`), run:

```bash
zenith merge --help
```

### Workflow

1. Build the template and `zenith validate` it once — fix every hard diagnostic before batching (a broken template fails every row).
2. Keep the placeholder text/image realistic (e.g. a long sample name) so you can eyeball that the box fits the widest row. Text-fit/overflow is per-row, so the longest value matters.
3. Dry-run small: merge the first few rows, open a couple of PNGs, then run the full set.
4. For production or CI, pass `--manifest` for reproducible output records. Declare asset `sha256` hashes for locked rendering where supported.

### Tips

- Everything stays tokenized — a brand/palette change re-renders all variants from one edit (see `references/brand.md`, `references/themes.md`).
- Pages vs rows: `merge` varies **content** across CSV rows. Different **sizes** (square/story/ banner) are separate pages in the template handled by `zenith variant` — see above.
- Localization: one column per text slot, one row per locale. Keep type large enough for the longest translation.

```bash
zenith merge card.zen people.csv --out-dir out/ --name-by name --format svg --raster-scale 2 --manifest run.json --json
```

CLI merge supports `--format png|svg`. PNG is the default. `--raster-scale` requires SVG output. MCP `zenith_merge` remains PNG only. No MCP variant tool exists.

Batch commands apply document, local, and global diagnostic policy. They have no `--deny` flags. Denied fallback writes nothing for the affected row or variant. I/O errors can leave earlier committed files. Reports and manifests retain those paths with failed status. Read `export.md` before production export.
