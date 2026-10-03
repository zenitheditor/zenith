---
name: zenith
description: "Author, edit, and render deterministic .zen design documents (posters, decks/slides, social graphics, flyers, books, magazines, diagrams, ads) with the zenith CLI. Use when the task is to create or change a visual design as structured, editable, version-controllable source — not a flat AI image. Covers: themes with ui.* styles and defaults, auto-layout frames (row/column/grid, hug/fill), design tokens & color (sRGB + CMYK), typography and text fit, anchors, images, recipes & procedural backgrounds, transactions (typed edits), `zenith fix` auto-repair, layout/overlap lint, variants/mail-merge, PNG/PDF/contact-sheet rendering, brand kits, and the agent loop new --theme -> validate --json -> fix -> render --contact-sheet -> tx. Triggers: design, poster, deck, slide, social graphic, flyer, brochure, banner, diagram, flowchart, chart, bar chart, line chart, pie chart, donut chart, data visualization, graph, legend, .zen, zenith, brand kit, render to PNG/PDF."
allowed-tools:
  - Bash(zenith:*)
  - Read
  - Write
  - Edit
  - Glob
  - Grep
---

# Zenith

Plain-text `.zen` source (KDL) → validate → render to pixel-exact PNG or print PDF.
Drive it with the `zenith` CLI, not an image model.

- **Use** for posters, decks, social, flyers, diagrams, charts, ads, and variants.
- **Do not use** for photos (place them as `image`), pure code tasks, or raster editing.

## CLI is the source of truth

This skill is judgment and routing. Syntax lives in the CLI. Never invent attribute names or op fields.

| Need | Run |
| --- | --- |
| Node attributes | `zenith schema nodes` · `zenith schema node <kind>` |
| Transaction ops | `zenith schema ops` · `zenith schema op <name>` |
| Token types, document tokens | `zenith schema token <type>` · `zenith tokens <file>` |
| Styles, defaults | `zenith schema style` · `zenith schema defaults` |
| Diagnostic codes | `zenith schema diagnostics` |
| Command flags | `zenith <cmd> --help` |
| Packs, fonts | `zenith library list` · `zenith fonts` |

If `zenith --version` fails, give the installer from https://github.com/zenitheditor/zenith#install. Never fake a workflow.

## Core loop (fastest path)

1. **Scaffold.** `zenith new <path> --theme <name>` plus canvas flags. If `.zenith/brand.md` exists in or above the directory, use its tokens (`references/brand.md`). Invent a palette only when the brand and every theme fail the brief.
   - A print `--format` (A4, Letter, A5, …) scales the theme type to a 16 px body on A4. Use the scaled `size.*` tokens. Do not add smaller ones.
2. **Author with layout and defaults.**
   - Stacks, rows, cards, chips, and lists go in `frame layout="row|column|grid"`. Do not compute child x/y by hand.
   - A themed document styles bare `text`, `shape`, and `connector` through its `defaults` block. Omit font, fill, and radius attributes.
   - Set `style="ui.h1"`, `"ui.card"`, `"ui.button"`, and so on for a role.
3. **Validate.** `zenith validate <file> --json`. With no Errors it also compiles every page and reports overflow, contrast, and lint.
4. **Fix.** `zenith fix <file>` previews the diff. `zenith fix <file> --apply` writes it. Fix what remains by hand.
5. **Preview.** `zenith render <file> --contact-sheet <png> --scale 0.5 --json`. Open the PNG. `status: blocked` means Errors remain.
6. **Iterate** with `zenith tx <file> <tx.json>` (dry-run, then `--apply`). `zenith inspect <file> --json` gives each node's final `box`.
   - The dry-run prints the source diff and every moved or resized box. Check them before `--apply`.
   - `tx --apply` rewrites the file in canonical form. Re-read it before text edits.
7. **Finish.** Render full scale with `--png <out.png>`, `--all-pages <DIR>`, or `--pdf <out.pdf>`. Report the changed ids, the validate result, and the output path.

A clean validate does not mean a good design. Critique the PNG with `references/design-critique.md`.

### Token dialects (one per document)

| Source | When | Ids |
| --- | --- | --- |
| Theme | No project brand | `color.primary`, `color.base.100`, `radius.box`, `size.h1`, `ui.*` styles → `references/themes.md` |
| Project brand | `.zenith/brand.md` and a kit exist | Project roles, often `color.brand` / `color.ink` → `references/brand.md` |
| Bespoke | Explicit one-off look | Still tokenized, with stable role ids |

Never mix theme ids and brand ids without a deliberate map. After scaffolding, use only the ids `zenith tokens <doc>` lists, plus any you add.

## By brief — pick tools first

Read the matching section of `references/by-kind.md` before authoring new work.

| Brief | Scaffold flags | Core tools |
| --- | --- | --- |
| Social / story / banner | default 1080² · `--width 1080 --height 1920` · `--width 1600 --height 400` | column or row frame, CTA `shape id="cta.button" style="ui.button"`, anchors |
| Poster / flyer | `--format a4` / `tabloid` | hierarchy, one depth motif |
| Deck / slides | `--format letter --landscape --pages N` | master chrome, one idea per page, contact sheet |
| Flow / architecture | any | `shape` + `connector`, `@zenith/flowchart`, Lucide icons |
| Numbers / tables | any | `chart`, `table` |
| Long article / report | `--format a4 --pages N` | `text format="markdown"`, `src=`, `chain` |
| Photo + type | any | `zenith asset import` + `image` + `@zenith/masks` / `@zenith/filters` |
| Many sizes / many rows | one master / template + CSV | `zenith variant` / `zenith merge` |
| Brand hexes only | — | `zenith theme new`, then `zenith theme apply` |

**Built-in packs** (never rebuild these): `@zenith/theme.*`, `@zenith/icons-lucide`, `@zenith/flowchart`,
`@zenith/filters`, `@zenith/masks`, `@zenith/brand-kit`. Search with `zenith library search`. Add with
`zenith library add <pack>#<item> --into <doc>`. Add only what you apply.

## Non-negotiables

- **Stable ids** — `hero.title`, `cta.button`. No anonymous nodes.
- **Tokens for visuals** — fill, font, size, stroke, and shadow reference tokens. Geometry can be raw px.
- **`zenith fix` mints tokens** — it turns a raw literal into `color.custom.<hex>` or `size.<n>`. Re-point a minted token to a theme role when one fits.
- **Layout over coordinates** — x/y belong only on top-level blocks, `position="absolute"` children, and free decoration. `tx` rejects x/y on an in-flow child (`tx.layout_managed`). Change flow with `set_layout`, or reorder.
- **Labeled box → `shape`** — never `rect` + floating `text`. The label ink pairs with the fill (`X` → `X.content`).
- **Text fits its box** — omit `h` on text in a layout frame. On an absolute text, the overflow message names the `h` and font size that fit. Grow the box before shrinking type. Report any shrink.
- **Muted text on dark themes** — captions use `color.base.content` (optional `opacity`), never `color.base.300`.
- **Icons on dark themes** — recolor Lucide strokes before the first render (`references/icons.md`).
- **Intentional overlap** — set `role="decoration"` or `role="background"`. The node and its whole subtree then leave overlap, occlusion, and crossing lint. They also leave `frame.child_overflow` and `layout.off_canvas`.
- **Shared chrome** — decks and books use `create_master` + `set_page_master`, never copied footers.
- **Right primitive** — flow: `shape` + `connector`. Data: `chart` / `table`. Things: Lucide icons. Prose: `text`.
- **Assets external** — `zenith asset import` + `image`. Never bake layout into a flat picture.
- **Look at the PNG** — schema for syntax, eyes for judgment.

Polish after structure works. Pick one motif: `pattern`, `mesh`, `light`, gradient, or noise.

## Routing (load on demand)

| Need | Open / run |
| --- | --- |
| Document-type recipes (start here for new work) | `references/by-kind.md` |
| Auto-layout, anchors, safe zones, frames | `references/layout.md` |
| Themes, `ui.*` styles, defaults, `theme new` | `references/themes.md` |
| Diagnostics, `zenith fix`, lint codes, policy, contrast, fonts | `references/diagnostics.md` |
| Design critique with `inspect --json` | `references/design-critique.md` |
| Brand kit / `.zenith/brand.md` | `references/brand.md` · `templates/brand.md` · `templates/brand-kit.zen` |
| Icons craft | `references/icons.md` |
| Multi-candidate workflow, history, MCP | `references/agentic-workflow.md` |
| Size variants vs mail-merge | `references/variants.md` |
| Pattern node, `detach_pattern` | `references/pattern.md` |
| Recipe provenance block | `references/recipes-model.md` |
| Bug or feature report | `references/reporting-issues.md` |
| Path craft, logo outlines | `zenith inspect path <doc> <id> --json` · `zenith outline-text --help` · `zenith perceive --help` |
| Font OT features, alternates | `zenith fonts features <family> --json` · `zenith fonts alternates <family> --char A --json` |
| Live import of another `.zen` | `zenith schema node instance` · `zenith imports --help` |

`zenith variant` varies size. `zenith merge` varies content rows. Prefer `imports` + `instance` over copying shared lockups.
