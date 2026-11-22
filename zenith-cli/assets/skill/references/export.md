# Export

Zenith exports deterministic PNG, SVG, and PDF artifacts from editable `.zen` source.

## CLI outputs

| Flag | Contract |
| --- | --- |
| `--png PATH` | Writes page 1 unless `--page N` selects another page. |
| `--svg PATH` | Writes self-contained RGB SVG with outlined text. Defaults to page 1. |
| `--pdf PATH` | Writes every page into one PDF unless `--page N` selects one page. |
| `--colorspace srgb\|cmyk` | Overrides PDF color space (`srgb` for digital sharing, `cmyk` for print). Omit to follow document `colorspace` (default `srgb`). |
| `--print` | Print-ready PDF preset: forces `DeviceCMYK` and sets 300 DPI raster fallback (`--raster-scale 3.125`, unless `--raster-scale` is given). |
| `--icc-profile PATH` | Embeds a 4-channel CMYK ICC profile file as PDF/X `/DestOutputProfile` (implies CMYK). |
| `--impose MODE` | Arranges reading-order pages onto press sheets for `--pdf`: `booklet` (`bifold`, `saddle`), `trifold` (`letterfold`), `zfold` (`accordion`), `spread` (`2up`), `grid` (`nup`), or `cut-and-stack` (`stack`). Cannot combine with `--page`. |
| `--grid CxR` | Sheet grid dimensions (e.g. `2x4`, default `2x2`) for `--impose grid` or `--impose cut-and-stack`. |
| `--duplex EDGE` | Back-sheet flip for `--impose`: `short-edge` (default) or `long-edge` (180° back rotation). |
| `--creep MM` | Total saddle-stitch creep compensation in mm (`--impose booklet` only). |
| `--signature N` | Multi-signature section size in pages, positive multiple of 4 (`--impose booklet` only). |
| `--marks` | Draws K-only outer corner crop marks and internal fold (dashed) or cut (solid) ticks on `--impose` sheets. |
| `--color-bars` | Draws an 11-patch CMYK density control bar and 4-plate registration targets in the outer margin on `--impose` sheets. |
| `--all-pages DIR` | Writes `page-N.png` in document order. |
| `--all-pages-svg DIR` | Writes `page-N.svg` in document order. |
| `--contact-sheet PATH` | Writes one PNG containing the selected pages. |
| `--scale F` | Scales PNG geometry and effects. Requires finite `0 < F <= 4`. |
| `--raster-scale F` | Scales SVG/PDF fallback pixels only. Requires finite `0 < F <= 4`. Defaults to 1. |
| `--embed-full-fonts` | Embeds whole PDF fonts instead of subsets. Rasterized text remains unselectable. |
| `--json` | Reports diagnostics, committed output paths, and rasterized regions. |

```bash
zenith render examples/hello.zen --svg out/hello.svg --raster-scale 2 --json
zenith render examples/multipage.zen --pdf out/share.pdf --colorspace srgb --json
zenith render examples/multipage.zen --pdf out/print.pdf --print --impose booklet --json
zenith render examples/multipage.zen --all-pages-svg out/pages/ --json
zenith render examples/multipage.zen --contact-sheet out/sheet.png --scale 0.5 --json
```

## Print vs. digital sharing PDFs

Author multi-page leaflets, brochures, and booklets in **natural reading order** (`1, 2, 3, 4…`) at the finished panel size (for example, 4 × A5 portrait pages for an A4 bi-fold), then export both deliverables from the same `.zen` file:

- **Digital sharing (`--colorspace srgb`):** reading-order pages (`1→2→3→4`), `DeviceRGB`, default `1×` raster fallback — smaller file, reads naturally on phones and laptops.
- **Commercial print (`--print` + `--impose`):** `DeviceCMYK` with PDF/X `FOGRA39` `OutputIntent` across vectors, text, native SVGs, gradients, and images (keeping neutral grays/black K-only), `300 DPI` raster fallback (`3.125×`), outer-edge bleed preservation (inner fold edges flush), and press-imposed sheets:
  - `--impose booklet` (`bifold`, `saddle`): 2-up saddle-stitch (`[4|1]`, `[2|3]` for 4 pages; pads to multiple of 4). Supports `--signature N` (multi-signature books) and `--creep MM` (saddle-stitch shingling).
  - `--impose trifold` (`letterfold`): 3-up roll/letter fold (`[5|6|1]`, `[2|3|4]`; pads to multiple of 6).
  - `--impose zfold` (`accordion`): 3-up Z/accordion fold (`[5|4|1]`, `[2|3|6]`; pads to multiple of 6).
  - `--impose spread` (`2up`): 2-up reading-order spreads (`[1|2]`, `[3|4]`; pads to multiple of 2).
  - `--impose grid` (`nup`) with `--grid CxR` (default `2x2`): sequential N-up gang-up grid for cards, stickers, and flyers.
  - `--impose cut-and-stack` (`stack`) with `--grid CxR` (default `2x2`): stack-ordered N-up sheets so guillotine cuts stay in sequential order (also supported on `zenith merge --format pdf --combine`).
  - Add `--marks` for K-only corner crop marks + fold/cut ticks, `--color-bars` for an 11-patch CMYK density bar + 4-plate registration targets, `--icc-profile PATH` for strict PDF/X-4 `/DestOutputProfile` embedding, and `--duplex long-edge` when the printer tumbles on the long edge.
- **Vector SVG fidelity:** SVG linear-gradient fills, linear-gradient strokes, and non-square `objectBoundingBox` diagonal gradients emit as native PDF axial shadings (in both `DeviceRGB` and `DeviceCMYK`) without rasterizing.

Vector dimensions and geometry remain unchanged by `--raster-scale`. Existing bitmap assets retain their source resolution. PNG `--scale` changes output dimensions. Each side becomes `max(1, round(page_px * F))`, with half values rounded away from zero.

## Appearance and fallback

- **SVG text:** Outlines preserve appearance and lose editing, selection, and search.
- **SVG assets:** Image data embeds in the artifact. Colors use RGB.
- **SVG fallback:** Effects capture their complete scope. Non-normal blends capture the page. Crossed scopes and bitmap glyphs require capture.
- **PDF fallback:** Unsupported vector features capture their required scope or page to preserve appearance.
- **Captured content:** Text loses selection and search. Links lose click targets within captured ranges.
- **Fallback reports:** `render.svg_rasterized` and `render.pdf_rasterized` identify pages, command ranges, and reasons.
- **Strict errors:** CLI and MCP PDF exports reject capture and resource errors through `render.pdf_failed`.
- **Library compatibility:** Legacy PDF APIs returning `Vec<u8>` retain compatibility emission on capture errors. Appearance can degrade. Use report APIs for strict errors.

Fallback reports remain visible when MCP soft diagnostics are disabled. Policy can suppress or elevate diagnostics. Structured rasterized regions remain export metadata.

```bash
zenith render examples/hello.zen --svg out/hello.svg --deny render.svg_rasterized --json
zenith render examples/hello.zen --pdf out/hello.pdf --deny render.pdf_rasterized --json
```

Render prepares outputs and applies diagnostic policy before writes. A blocking diagnostic leaves requested outputs untouched. Source validation cannot predict every export fallback. Check the export report too.

## Batch outputs

CLI merge and variant support `--format png|svg|pdf`. PNG is the default. Variants retain their `.zen` companions. A PDF merge row is one file with all its pages and the `page bleed` print boxes. `merge --combine FILE` also writes one PDF of all successful rows. `--raster-scale` requires SVG or PDF output. Batch commands reject that flag for PNG, including an explicit value of 1.

Batch commands have no `--allow`, `--warn`, or `--deny` flags. Document, local, and global policy apply. A denied fallback prevents writes for that row or variant. Other successful items can remain.

Read `variants.md` for copyable batch commands and data bindings.

## Protected writes

- **File replacement:** Each file uses a temporary sibling and rename. A write error preserves the existing destination.
- **Batch errors:** Writes commit sequentially. An error can leave earlier outputs. There is no batch rollback.
- **Reports:** JSON output paths identify committed files. Partial rows and variants carry failed status and retain committed paths.
- **Manifests:** Partial entries retain committed paths and failed status. Manifest write errors retain batch error counts.
- **Read-only destinations:** Writes reject read-only files. Replacement preserves existing permissions.
- **Links:** Symlink outputs replace the resolved target and retain the link. Cycles and excessive link chains produce errors. Replacement leaves other hard links pointing at the previous content.
- **Durability:** Protected writes provide no crash durability guarantee.

Inspect committed paths after an I/O error. A blocked status does not imply zero writes after an I/O error.

## MCP rendering

| Parameter | Contract |
| --- | --- |
| `doc` | Document path or document identity. |
| `format` | Required: `png`, `svg`, `pdf`, or `scene`. |
| `page` | One-based page. Defaults to every page for PDF and page 1 for other formats. |
| `raster_scale` | SVG/PDF only. Finite `0 < F <= 4`, default 1. |
| `colorspace` | PDF only. `srgb` or `cmyk`. Omit to follow document `colorspace`. |
| `print` | PDF only. Print preset: `DeviceCMYK` + `300 DPI` fallback (`raster_scale: 3.125` unless set). |
| `impose` | PDF only (omit `page`). `booklet` (`bifold`, `saddle`), `trifold` (`letterfold`), `zfold` (`accordion`), `spread` (`2up`), `grid` (`nup`), or `cut-and-stack` (`stack`). |
| `grid` | `impose=grid\|cut-and-stack` only. Grid dimensions `<cols>x<rows>` (default `2x2`). |
| `color_bars` | `impose` only. Draw 11-patch CMYK density bar and 4-plate registration targets. |
| `scale` | PNG only. Finite `0 < F <= 4`, default 1. |
| `contact_sheet` | PNG only. Tiles all pages unless `page` selects one. |
| `out` | Optional protected filesystem write, alongside the artifact resource link. |
| `diagnostics` | Includes soft diagnostics when true. Fallback diagnostics remain visible regardless. |

`zenith_render` returns resource links instead of inline artifact bytes. Read them through `resources/read`. A failed `out` write reports the write error. There is no MCP variant tool. Use CLI variant for size batches.

## MCP merge

| Parameter | Contract |
| --- | --- |
| `doc` | Required template path or document identity. |
| `data` | Required CSV path. |
| `out_dir` | Required directory for committed row pages. |
| `format` | `png` or `svg`. Defaults to `png`. |
| `raster_scale` | SVG only. Finite `0 < F <= 4`, default 1. |
| `name_by` | Optional CSV column for output names. |
| `manifest` | Optional protected manifest write. |

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "zenith_merge",
    "arguments": {
      "doc": "card.zen",
      "data": "people.csv",
      "out_dir": "out/",
      "format": "svg",
      "raster_scale": 2,
      "name_by": "name",
      "manifest": "out/run.json"
    }
  }
}
```

MCP merge applies the same document, local, and global batch policy as CLI merge. PNG rejects `raster_scale`, including an explicit value of 1. Denied fallback prevents writes for the affected row.

Structured responses include `total_rows`, `written`, `failed`, `files_written`, `outputs`, and `rows`. Partial rows retain committed paths with failed status. `outputs` lists committed paths including `out_dir`. A manifest write error adds `io.write_failed` and marks the tool result as an error. Row counts and committed paths remain available. Other successful files remain. There is no batch rollback.
