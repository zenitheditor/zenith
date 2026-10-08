# Diagnostics — fix loop, lint, policy

Codes, severities, and summaries: `zenith schema diagnostics`. The full catalog with Errors: add `--json`.

## Where diagnostics come from

- `zenith validate <file> --json` checks the source.
- With no Errors, `validate` also compiles every page. That adds overflow, contrast, and lint.
- `zenith render <file> … --json` always prints JSON with every diagnostic.
- `render` `status` is `ok` or `blocked`. Diagnostic blocking leaves outputs untouched before writes.
- I/O errors can leave earlier committed outputs. Inspect the report paths. There is no batch rollback.
- Source validation reports source and compile diagnostics. Export-specific diagnostics require the export report.

## `zenith fix`

`zenith fix <file>` previews fixes and a source diff. `--apply` writes and prints a summary. add `--diff` to print the diff too. Fix table: `zenith fix --help`.

- A machine-fixable diagnostic carries a structured `fix` object in `validate --json`.
- Raw visual literals become the same-value token, else a minted one (`color.custom.<hex>`, `size.<n>`, …).
- Re-point a minted color to a theme role when one fits.
- Typo'd property names, token ids, and enum values become their unique did-you-mean.
- Compile-stage fixes apply too. `text.ink_overlap` moves the text's authored `y`.
- The rest stays in `remaining`. Fix it by hand, then re-run `validate --json`.

Author freely, then run `fix --apply` once. Never hand-tokenize literals one at a time.

## Text overflow

Text `overflow` sets what happens when glyph ink leaves the box. The check measures ink, not line boxes.

| `overflow` | Behavior |
| --- | --- |
| `clip` (default) | Clips at the box edge. Warns `text.overflow`. |
| `visible` | Paints past the box. No diagnostic. |
| `fit` | Keeps the size. Errors `text.fit_failed`. |
| `autofit` | Shrinks within [`font-size-min`, `font-size`]. Errors `text.fit_failed` at the floor. |

- The message names the `h` that fits at the declared size and the font size that fits.
- Set that `h` first. Shrink type only on purpose, and report it.
- In a layout frame, omit `h`. The compile measures the text.
- `label.overflow` is the shape-label case: ink crosses an ellipse, diamond, or pill outline.

## Compile-stage lint

| Code | Catches |
| --- | --- |
| `text.ink_overlap` | Glyph ink of two texts or labels intersects |
| `text.occluded` | A later opaque node hides over half of a glyph |
| `label.overflow` | Shape label ink crosses the shape outline |
| `layout.block_overlap` | Two sibling blocks overlap in part. Containment and children of one `group` are silent |
| `chart.overflow` | Chart text ink leaves the chart box. raise `h` / `w` to the named size |
| `align.near_miss` | An edge, center, or baseline sits 0.75-3 px off a shared value |
| `spacing.uneven_gap` | Three or more siblings have nearly equal gaps |
| `connector.crosses_node` | A connector route runs through an unconnected node |
| `text.edge_crowding` | Glyph ink sits too close to the trim edge |
| `text.too_small` | Font size is below the page-relative floor |
| `type.near_duplicate_size` | Two font sizes in one group or frame are within 1 px or 8% |

- `align.near_miss` and `spacing.uneven_gap` come from hand-placed siblings. Move them into a layout frame.
- `role="decoration"` or `role="background"` exempts a node and its whole subtree from overlap, occlusion, and crossing lint. The exemption also covers `frame.child_overflow` and `layout.off_canvas`.
- Use those roles for intentional overlap: glows, watermarks, background type.

## Contrast

The check judges text against the paint behind its glyphs, with APCA `Lc`.

| Code | Severity | Meaning |
| --- | --- | --- |
| `contrast.invisible` | warning | `\|Lc\|` < 15. A real defect, even when `valid` is `true`. |
| `contrast.low` | advisory | Legible but under threshold. Often deliberate. |
| `contrast.indeterminate_backdrop` | advisory | Backdrop is an image, path, filter, or blended fill. |

- For an indeterminate backdrop, set `contrast-bg=(token)"<color>"` to the color the viewer sees.
- `color.base.200` and `color.base.300` are surfaces. Text on `color.base.100` uses `color.base.content`.

## Policy

Policy changes reporting only. Rendered output never changes. Error codes are immutable.

| Verb | Effect |
| --- | --- |
| `allow "<code>" ["<subject-id>" …]` | Suppress the code, optionally for listed subjects only |
| `warn "<code>"` | Force Warning |
| `deny "<code>"` | Elevate to a blocking Error (CI gate) |

Sources, last wins:

1. `~/.config/zenith/config.kdl`
2. `./.zenith.kdl` (walked up from the document)
3. In-file `diagnostics { … }` block at the document root
4. `--allow` / `--warn` / `--deny` on `validate` and `render` (repeatable)

## Fonts and CI

- A non-bundled `fontFamily` emits `font.local`. That render is not deterministic across machines.
- Use a Bundled family (`zenith fonts`), or declare the font as a project `font` asset.
- `font.local` is raised at render time. Gate it with `render --deny font.local`, not `validate`.

## Export diagnostics

| Code | Contract |
| --- | --- |
| `render.svg_rasterized` | SVG fallback captures a scope or page. |
| `render.pdf_rasterized` | PDF fallback captures a scope or page. |
| `render.pdf_failed` | Strict PDF capture or resource error blocks export. |
| `io.write_failed` | An output write fails. Earlier committed files can remain. |

Fallback diagnostics identify pages, command ranges, and reasons. Captured text loses selection and search. Captured links lose click targets. MCP returns fallback diagnostics even when soft diagnostics are disabled. Structured rasterized regions remain metadata despite diagnostic policy.

CLI render supports `--deny render.pdf_rasterized` and `--deny render.svg_rasterized`. Batch commands have no policy flags. Document, local, and global policy apply to batches. Read `export.md` for resolution controls and protected writes.
