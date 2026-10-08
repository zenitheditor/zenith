# Export

Zenith exports deterministic PNG, SVG, and PDF artifacts from editable `.zen` source.

## CLI outputs

| Flag | Contract |
| --- | --- |
| `--png PATH` | Writes page 1 unless `--page N` selects another page. |
| `--svg PATH` | Writes self-contained RGB SVG with outlined text. Defaults to page 1. |
| `--pdf PATH` | Writes every page into one PDF unless `--page N` selects one page. |
| `--all-pages DIR` | Writes `page-N.png` in document order. |
| `--all-pages-svg DIR` | Writes `page-N.svg` in document order. |
| `--contact-sheet PATH` | Writes one PNG containing the selected pages. |
| `--scale F` | Scales PNG geometry and effects. Requires finite `0 < F <= 4`. |
| `--raster-scale F` | Scales SVG/PDF fallback pixels only. Requires finite `0 < F <= 4`. Defaults to 1. |
| `--embed-full-fonts` | Embeds whole PDF fonts instead of subsets. Rasterized text remains unselectable. |
| `--json` | Reports diagnostics, committed output paths, and rasterized regions. |

```bash
zenith render examples/hello.zen --svg out/hello.svg --raster-scale 2 --json
zenith render examples/multipage.zen --pdf out/book.pdf --raster-scale 2 --json
zenith render examples/multipage.zen --all-pages-svg out/pages/ --json
zenith render examples/multipage.zen --contact-sheet out/sheet.png --scale 0.5 --json
```

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

CLI merge and variant support `--format png|svg`. PNG is the default. SVG variants retain their `.zen` companions. `--raster-scale` requires SVG output. Batch commands reject that flag for PNG, including an explicit value of 1.

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
| `scale` | PNG only. Finite `0 < F <= 4`, default 1. |
| `contact_sheet` | PNG only. Tiles all pages unless `page` selects one. |
| `out` | Optional protected filesystem write, alongside the artifact resource link. |
| `diagnostics` | Includes soft diagnostics when true. Fallback diagnostics remain visible regardless. |

`zenith_render` returns resource links instead of inline artifact bytes. Read them through `resources/read`. A failed `out` write reports the write error. `zenith_merge` exports PNG only. There is no MCP variant tool. Use CLI commands for batch SVG.
