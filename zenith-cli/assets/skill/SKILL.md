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

1. For a flyer, letterhead, handout, deck, poster, menu, card, or report, run `zenith new <path> --starter <kind>`. Add `--theme` and a size flag as needed. The output lists every text slot and one `tx --ops` command that fills them. Change the text, tokens, and nodes. Do not rewrite the file.
2. For any other design, run `zenith schema write` once. It holds a working document, the layout rules, the common attributes, and the common `tx` ops as runnable JSON. Run `zenith schema node <kind>` or `zenith schema op <name>` only for what it does not list.
3. Run `zenith validate`, then `zenith render --png`, and look at the PNG before delivery. Export the requested format alongside the `.zen` source.

To revise an existing document, start with `zenith tx --help`, which lists the common edits as runnable ops. Read a reference only when a step fails or the task is not covered above. Never invent attributes or transaction fields.

If the CLI is unavailable, use the installer at https://github.com/zenitheditor/zenith#install. Never fabricate execution results.

| Task | Read |
| --- | --- |
| Tokens, primitives, transactions, the full loop | `references/authoring-workflow.md` |
| A brief no starter matches | `references/by-kind.md` |
| Export flags, fallback reports, MCP rendering, output errors | `references/export.md` |
| Auto-layout, anchors, safe zones, frames | `references/layout.md` |
| Themes, styles, defaults | `references/themes.md` |
| Diagnostics, repair, policy, contrast, fonts | `references/diagnostics.md` |
| Visual critique | `references/design-critique.md` |
| Brand kit | `references/brand.md` |
| Icons | `references/icons.md` |
| Candidates, history, MCP workflow | `references/agentic-workflow.md` |
| Pointer edits with a live preview (`zenith edit`, `zenith_editor_*` MCP tools) | `references/authoring-workflow.md`, `references/agentic-workflow.md` |
| Size variants or CSV mail-merge | `references/variants.md` |
| Patterns | `references/pattern.md` |
| Recipe provenance | `references/recipes-model.md` |
| Error or feature report | `references/reporting-issues.md` |

Photo generation and raster editing belong to other tools. Import their image assets into the editable design.
