---
name: zenith
description: "Author, edit, and render deterministic .zen design documents (posters, decks/slides, social graphics, flyers, books, magazines, diagrams, ads) with the zenith CLI. Use when the task is to create or change a visual design as structured, editable, version-controllable source — not a flat AI image. Covers: design tokens & color (sRGB + CMYK), gradients, typography, layout & anchors, frames/groups, images, visual recipes & procedural backgrounds, transactions (typed edits), variants/mail-merge, PNG/PDF rendering, validation, brand kits, and the agentic author->validate->render->inspect->edit loop. Triggers: design, poster, deck, slide, social graphic, flyer, brochure, banner, diagram, flowchart, chart, bar chart, line chart, pie chart, donut chart, data visualization, graph, legend, .zen, zenith, brand kit, render to PNG/PDF."
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
Editable, addressable, deterministic. Drive it with the `zenith` CLI — not an image model.

**Use** for posters, decks, social, flyers, diagrams, charts, ads, variants — source you can
diff and re-render. **Don't use** for photographic *pictures* (generate elsewhere, place as
`image`), pure code tasks, or in-place raster editing.

## CLI is the source of truth

Do **not** invent attribute names or op fields:

```bash
zenith --version                  # missing? https://github.com/zenitheditor/zenith#install
zenith --help · zenith <cmd> --help
zenith schema nodes | ops | tokens
zenith schema node <kind> · zenith schema op <name> · zenith schema token <type>
zenith validate <file> --json
zenith inspect <file> --json      # geometry + role facts for critique
zenith inspect path <file> <id> --json   # path topology + extrema bounds (+ --craft)
zenith tokens <file>              # palette/type already on the document
zenith library list               # every embedded + project pack
zenith fonts                      # Bundled (portable) vs Local
zenith fonts features "Noto Sans" --json # OT feature tags for a face
zenith fonts alternates "Noto Sans" --char A --json
```

If `zenith` is missing, give the one-line installer from the repo README — do not fake a workflow.
This skill is **judgment + routing**. Syntax lives in `zenith schema`.

## Core loop

1. **Match the brief** → canvas + primitives (`By brief` below; details in `references/by-kind.md`).
2. **Tokens first.** Project brand (`.zenith/brand.md` / `libraries/*.zen`) → else
   `zenith new <path> --theme <name>`. Only invent a palette when neither fits.
3. **Author.** Scaffold with `zenith new` (don't hand-write the outer skeleton). Prefer
   `zenith tx` for later edits (dry-run by default). KDL: one node per line (or `\`);
   booleans `#true` / `#false`.
4. **Validate.** `zenith validate <file> --json` after every change. Never finalize with Errors.
5. **Render and look.** `zenith render <file> --png out.png` (or `--all-pages` / `--pdf`), then
   **open the PNG**. Clean validate ≠ good design. Critique:
   `references/design-critique.md` + `inspect --json`.
6. **Report** briefly: ids/tokens changed, validate result, output path.

`zenith fmt <file>` is idempotent. After placing siblings, prefer layout ops over eyeballing:
`align_nodes`, `distribute_nodes`, `align_to_edge` (`zenith schema op <name>`).

### Token dialects (one per document)

| Source | When | Ids |
| --- | --- | --- |
| **Theme** | No project brand | `color.primary`, `color.base.100`, `color.base.content`, `radius.box`, `size.h1`, … → `references/themes.md` |
| **Project brand** | `.zenith/brand.md` + kit | Project roles (often `color.brand` / `color.ink`) → `references/brand.md` |
| **Bespoke** | Explicit one-off look | Still tokenize; stable role ids |

Do not mix theme-contract ids and brand ids without a deliberate map. After scaffold:
`zenith tokens <doc>` and use **only** those ids (plus any you add).

## By brief — pick tools first

Read `references/by-kind.md` for the full recipe of the matching row. Always:

```bash
zenith new <path> --theme <name> [canvas flags…]   # or brand tokens
zenith tokens <path>                               # use these
zenith library list                                # packs available now
```

| Brief smells like… | Scaffold | Core tools (use these, not fakes) |
| --- | --- | --- |
| Social square | default 1080² | `text` + accent `rect` + CTA **`shape`** + optional icon |
| Story / reel | `--width 1080 --height 1920` | same; safe-zone; strong hierarchy |
| Banner / header | e.g. `--width 1600 --height 400` | horizontal lockup; anchors for logo/CTA |
| Poster / flyer | `--format a4` / `tabloid` / custom | hierarchy; optional `pattern`/`light` depth |
| Deck / slides | `--format letter --landscape --pages N` | one idea/page; `role`s; contact-sheet render |
| Flow / process | any size | **`shape` + `connector`** or `@zenith/flowchart#process\|decision\|terminator` |
| Architecture / product map | any | `zenith library search` icons + `connector` (+ ports) |
| Numbers / KPIs | any | **`chart`** (bar/line/area/pie/donut/sparkline) — not hand-drawn bars |
| Table / schedule | any | **`table`** |
| Long article / report | `--format a4` + pages | `text` `format="markdown"` / `src=` / `chain`; `footnote`/`toc`/`code` |
| Photo + type | any | `asset import` + `image` + **`@zenith/masks`** / **`@zenith/filters`** |
| Fancy background | any | `pattern`, `mesh`, `light`, gradient/noise tokens — see polish |
| Many sizes | one master + anchors | **`zenith variant`** |
| Many rows / people | template + CSV | **`zenith merge`** (`role="data.<col>"`) |
| Brand hexes only | — | **`zenith theme new`** then `theme apply` |

**Canvas cheatsheet:** square `1080²` (default) · story `1080×1920` · landscape banner custom ·
print `--format a4|a3|letter|tabloid` · deck `--format letter --landscape --pages N` ·
`zenith new --help` for full list.

## Built-in packs (don't rebuild these)

Run `zenith library list` for the live catalog. Embedded presets:

| Pack | Use for | Add |
| --- | --- | --- |
| `@zenith/theme.*` (10) | Full token contract | `zenith new --theme <name>` or `theme apply` |
| `@zenith/icons-lucide` | Devices, cloud, lock, UI affordances (~1745) | `library search` → `library add …#icon --into doc --page id --at X,Y` |
| `@zenith/flowchart` | process / decision / terminator components | `library add @zenith/flowchart#decision --into … --page … --at X,Y` |
| `@zenith/filters` | Photo/grade looks (duotone, noir, vintage, …) | `library add @zenith/filters#duotone-gold --into doc` then `filter=(token)"…"` |
| `@zenith/masks` | Vignette, spotlight, soft card/portrait clips | `library add @zenith/masks#vignette --into doc` then `mask=(token)"…"` |
| `@zenith/brand-kit` | Re-skin actions | `library list` / apply via `tx` or brand workflow |

Icons judgment: `references/icons.md`. Never invent decorative icons for abstract concepts.

## Non-negotiables

- **Stable ids** — `hero.title`, `cta.button`. No anonymous node soup.
- **Tokenize visuals** — fill/font/size/stroke/shadow via `(token)"id"`. Geometry `x y w h` may be raw px. Values: `(px)28`, weight `700`, `"#hex"` — not CSS strings. **`create_token`** supports scalars plus structured **`shadow`** (`layers`), **`filter`** (`filter_ops`), **`gradient`** (`stops` + `angle`/`radial`), **`mask`** (`shape`/`feather`/`radius`) — see `zenith schema op create_token`. Pack filters/masks also via `library add`.
- **Shared chrome** — decks/books: `create_master` + `add_node` into the master + `set_page_master` on each page (not copy-paste footers).
- **Labeled box → `shape`** with `text-style` pointing at a style that uses the readable ink
  (theme: `color.primary.content` on `color.primary` fill). Not `rect` + floating `text`.
  Create styles with **`create_style`** / **`set_style_property`** (`zenith schema style`,
  `zenith schema op create_style`) — or hand-author `styles { }` then `zenith sync`.
- **Right primitive** — flow: `shape`+`connector`; data: `chart`/`table`; things: Lucide icons;
  prose: `text` (+ markdown/`src`/`chain`).
- **Type box ≥ type size** — theme `size.h1` is 64px → give the text node **h ≈ 90+** (body 28 →
  h ≈ 40+). First overflow fix is **grow the box**, not shrink the token.
- **Muted text on dark themes** — captions/page numbers use `color.base.content` (optionally lower
  `opacity`), never a dark surface token like `color.base.300` as fill on `color.base.100`.
- **Icons on dark themes** — Lucide defaults to near-black stroke. After `library add`, recolor
  **before** first render (shared `lib.icons.stroke` or every `icon.N` path). Resize with
  `set_geometry` on the instance (`w`/`h`/`x`/`y`) or source. See `references/icons.md`.
- **Overflow preserves type** — enlarge/reflow/`chain` before shrink/`autofit`; report if you shrink.
- **Group motifs** — `group`/`frame` with stable id for one-op move/dim/delete.
- **Assets external** — `zenith asset import` + `image`; never bake layout into a flat picture.
- **Align with ops** — `align_nodes` / `distribute_nodes` / `align_to_edge` after rough placement.
- **Only add packs you use** — unused filter/mask tokens leave advisories; strip or apply them.
- **Look at the PNG** — schema for syntax; eyes for judgment.

### Optional polish (after structure works)

Depth without clutter: `pattern` (grid/scatter) · `mesh` · `light` · gradient/`shadow` tokens ·
`@zenith/filters` · `@zenith/masks` · noise filter (`zenith schema token filter`).
One strong motif beats five competing effects. Pattern details: `references/pattern.md`.

## Routing (load on demand)

| Need | Open / run |
| --- | --- |
| **Document-type recipes** (start here for new work) | `references/by-kind.md` |
| Layout, anchors, safe zones, frames | `references/layout.md` |
| Themes catalog / apply / `theme new` | `references/themes.md` |
| Brand kit / `.zenith/brand.md` | `references/brand.md` · `templates/brand.md` |
| Icons craft | `references/icons.md` · `library search` |
| Design critique | `references/design-critique.md` · `inspect --json` |
| Multi-candidate / MCP | `references/agentic-workflow.md` |
| Size variants vs mail-merge | `references/variants.md` |
| Styles block / create_style | `zenith schema style` · `zenith schema op create_style` |
| Diagnostics, contrast, fonts | `references/diagnostics.md` |
| Pattern / detach | `references/pattern.md` |
| Recipe provenance block | `references/recipes-model.md` |
| Bug/feature report | `references/reporting-issues.md` |
| Any node/op/flag syntax | `zenith schema …` · `zenith <cmd> --help` |
| Path craft / logo outlines | `zenith schema node path` · `zenith inspect path <doc> <id> --json` · `zenith outline-text --help` · `zenith perceive --help` |
| Font OT features / alternates | `zenith fonts features <family|file> --json` · `zenith fonts alternates … --char A --json` |
| Live import of another `.zen` | `zenith schema node instance` · `page` |

**Two "variant" tools:** `zenith variant` = size/format; `zenith merge` = content rows.

## Project config

1. `.zenith/brand.md` exists (walk up) → read and conform.
2. Else `zenith new <path> --theme <name>` (or later `theme apply`).
3. Prefer `imports` + `instance`/`page source=…` over copying shared lockups.
4. Invent a palette only when brand and themes both fail the brief.
