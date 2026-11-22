---
name: zenith
description: "Author and modify editable .zen designs with the Zenith CLI. Use for posters, slides, diagrams, charts, brand kits, size variants, and CSV mail-merge. Export PNG, self-contained SVG with outlined text, or PDF with explicit raster fallback reports. Use other tools for photo generation, raster editing, and unrelated coding tasks."
allowed-tools:
  - Bash(zenith:*)
  - Read
  - Write
  - Edit
  - Glob
  - Grep
---

# Zenith

Zenith agents author editable `.zen` documents and render deterministic outputs through the CLI.

A new design:

1. For a flyer, letterhead, handout, deck, poster, menu, card, or report, run `zenith new <path> --starter <kind>`. Add `--theme` and a size flag as needed. The output lists every text slot and one `tx --ops` command that fills them.
2. For any other design, run `zenith schema write` once. It holds a working document, the layout rules, the common attributes, and the common `tx` ops.
3. Run `zenith validate`, then `zenith render --png`, and look at the PNG. Export the requested format beside the `.zen` source.

To revise a document:

1. Run `zenith inspect <file>`. It lists page order, ids, boxes, text, styles, overrides, assets, masters, and components. Run `zenith tokens <file>` for token values. Read the whole source only when these miss a fact.
2. Run `zenith tx --help` once. It lists the common edits as runnable ops. Run `zenith schema op <name>` for any other op.
3. Check an edit on page N in the same call: `zenith tx <file> --ops '[...]' --apply --png out.png --page N`. Check many pages with `zenith render --contact-sheet`.

When no op covers an edit, edit the `.zen` source with your file-edit tool. Never invent attributes or transaction fields. Never fabricate execution results. Without the CLI, use the installer at https://github.com/zenitheditor/zenith#install.

Read a reference only when a step fails or the task is not covered above:

| Task | Read |
| --- | --- |
| Tokens, primitives, transactions, pointer edits (`zenith edit`) | `references/authoring-workflow.md` |
| A brief no starter matches | `references/by-kind.md` |
| Export, print CMYK, imposition, fallback reports | `references/export.md` |
| Auto-layout, anchors, safe zones | `references/layout.md` |
| Themes, styles, defaults | `references/themes.md` |
| Diagnostics, contrast, fonts | `references/diagnostics.md` |
| Visual critique | `references/design-critique.md` |
| Brand kit | `references/brand.md` |
| Icons | `references/icons.md` |
| Candidates, history, MCP | `references/agentic-workflow.md` |
| Size variants, CSV mail-merge | `references/variants.md` |
| Patterns | `references/pattern.md` |
| Recipe provenance | `references/recipes-model.md` |
| Error or feature report | `references/reporting-issues.md` |

Photo generation and raster editing belong to other tools. Import their images as assets.
