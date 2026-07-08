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

Zenith turns design into **plain-text `.zen` source** (KDL) that you read, diff, validate,
edit with typed transactions, and render **deterministically** to pixel-exact PNG or
print-ready PDF. You drive it through the `zenith` CLI. This is the opposite of an image
model: you produce an editable, addressable _document_, not a bag of pixels.

## When to use

**Use this skill when** the task is to create or change a visual design that should be
editable and reproducible: posters, decks/slides, social graphics, flyers, brochures,
banners, book/magazine pages, diagrams/flowcharts, ads, or on-brand variants — and when the
user wants the result as source they can review, version, and re-render.

**Don't use it for**: generating a photographic/illustrative _picture_ (use an image model
for that, then compose the resulting asset into a `.zen` document — see `zenith schema node image`);
pure backend/code tasks; or editing existing raster files.

## The CLI is the source of truth

The CLI is self-documenting and **authoritative** for node/attribute/op syntax. Always reach for
it before this skill:

```bash
zenith --version                  # if missing, see https://github.com/zenitheditor/zenith#install
zenith --help                     # command list + the loop, in the tool itself
zenith <command> --help           # exact flags and an EXAMPLE for any command
zenith schema nodes               # all node kinds
zenith schema node <kind>         # attributes, types, and required/optional for one kind
zenith schema ops                 # all transaction op names
zenith schema op <name>           # fields, types, and semantics for one op
zenith validate <file> --json     # actionable diagnostics ("did you mean?", raw-literal hints, fit font-size)
zenith inspect <file> --json      # resolved per-node geometry (px, tokens resolved) + role — the facts for a design critique
zenith schema tokens              # all token types
zenith schema token <type>        # exact value form + example for one token type
zenith schema diagnostics         # governable diagnostic codes
zenith fonts                      # list Bundled vs Local/system font families
```

**Do not look up attribute names or op fields in this skill** — `zenith schema` emits them
directly, with types and required/optional status. This skill holds what the CLI can't: conceptual
guidance, design workflow, recipes, gotchas, and judgment calls.

If `zenith` is not installed, tell the user the one-line installer
(`curl -fsSL https://raw.githubusercontent.com/zenitheditor/zenith/main/scripts/install.sh | sh`)
rather than guessing — do not fabricate a workflow you can't run.

## The core loop (always follow this)

Zenith is deterministic and auditable; lean into that instead of guessing.

1. **Plan in source.** Capture the brief, palette, and layer plan as `note`/`role="guide"`
   content or a sidecar, so the design can be traced back to intent.
2. **Author / edit.** For **new** work, start from `zenith new <path> --theme <name>` when no
   project brand exists (`.zenith/brand.md` / `libraries/*.zen`) — it scaffolds the full theme
   token contract (10 embedded themes: `cobalt`, `ember`, `harbor`, `lagoon`, `pine`, `poppy`,
   `prism`, `sorbet`, `sunset`, `volt`; `zenith library list` shows them) on top of the minimal
   top-level skeleton (`zenith version=1 { project … tokens … styles … document { page … } }`)
   with a doc-id already minted. Fall back to a bare `zenith new <path>` only when the doc really
   should start unthemed. Edit the scaffold rather than hand-writing the outer structure from
   memory (run `zenith schema document` / `zenith schema page` for the exact top-level shape). For **changes**
   to existing nodes, apply a typed transaction with `zenith tx` — prefer it for edits: it is dry-run
   by default, shows a source + scene diff, and enforces id-uniqueness and referential integrity.
   Note: KDL nodes are **one per line** — a node and all its attributes go on a single line (or end
   each line with `\` to continue), and booleans are written `#true` / `#false`.
3. **Validate.** Run `zenith validate <file> --json` after every change. **Never finalize
   while hard (Error) diagnostics remain.** Fix at the source.
4. **Render to inspect — then actually LOOK.** `zenith render <file> --png out.png` (or
   `--all-pages <dir>` for a contact sheet, `--pdf` for print), then OPEN the PNG and visually
   verify before declaring success. Check, concretely: text is centered where you intended (both
   axes), nothing overlaps or is clipped, labels sit inside their boxes, spacing/alignment look
   deliberate. A clean `validate` does NOT mean it looks right — validation can't see a label
   stuck to the top of a button or two boxes overlapping. If something is off, fix it; never
   report "done" on a render you didn't inspect. For a deeper, **style-neutral critique** —
   composition, balance, consistency, noise, semantic accuracy, and reading alignment/spacing/role
   facts from `zenith inspect <file> --json` — see `references/design-critique.md`.
5. **Iterate**, then report what changed and where the output is.

Run `zenith fmt <file>` to canonicalize source; it is idempotent.

## Non-negotiable best practices

These make designs editable, on-brand, and reproducible — and keep the agentic loop sound.

- **Stable, meaningful ids.** Every node carries an `id`. Use semantic, hierarchical ids
  (`bg.grunge`, `hero.title`, `cta.button`) so later edits and transactions target exactly
  the right node. Anonymous node soup cannot be edited later.
- **Tokenize everything.** Colors, fonts, sizes, gradients, shadows go through `token`s and
  are referenced with `(token)"id"`. Never embed raw hex/sizes in nodes — a palette/brand
  swap must be a token-value change, not a geometry rewrite. Token values use KDL typed
  literals: `(px)28`, numeric weight `700`, color `"#hex"` — not CSS strings like `"28px"` or
  `"900"`. Gradients, shadows, filters, and masks use child nodes inside the token, not a
  bare `value=`. Run `zenith schema token <type>` (or `zenith schema tokens`) for the exact
  value form and a working example per token type.
- **Resolve text overflow by preserving intent first.** When `text.overflow` reports clipped
  text, treat declared type scale as design intent, especially for titles, wordmarks, body systems,
  and brand tokens. First enlarge the text box, move neighboring nodes, split content, reflow the
  layout, or link boxes with `chain`. Shrink font size, lower shared size tokens, or use
  `overflow="autofit"` only when smaller type is intended, the source explicitly opts into fitting,
  or the layout/section constraints make geometry expansion impossible. If you shrink type to clear
  overflow, report that tradeoff explicitly.
- **Use the right primitive for a labeled box.** For anything that is a box WITH centered text
  — a button/CTA, a flowchart node, a labelled card — prefer the `shape` node, not a `rect` plus a
  separately-positioned `text`. `shape` carries an owned label (child `span`s) that it centers for
  you via `h-align`/`v-align`, and `shape kind="decision"` gives a diamond, `terminator` a
  pill, `process` a rounded box (run `zenith schema node shape`). Connectors anchor cleanly to a
  `shape`'s real outline. If you DO overlay a standalone `text` on a box, set `v-align="middle"`
  (and `align="center"`) on the text so the label is actually centered — a bare `text` sits at the
  TOP of its box. For flowcharts and architecture diagrams, connect nodes with `connector`
  (`from`/`to` + `marker-end`), never hand-drawn lines. Use divided anchors like
  `from-anchor="35/60"` when several links leave the same outline, and use page/component
  `ports { port node="..." id="..." anchor="..." }` plus `from="node#port"` for reusable
  semantic attachment points.
- **Use native icon components for real-world diagram objects.** Do not represent devices,
  clouds, servers, databases, files, folders, locks, networks, or search/settings affordances as
  generic labeled boxes when an icon is the visual object. Discover the embedded pack with
  `zenith library list`; inspect an icon with
  `zenith library show @zenith/icons-lucide#monitor`; materialize with
  `zenith library add @zenith/icons-lucide#monitor --into <file> --page <page-id> --at X,Y`.
  The Lucide pack includes `monitor`, `smartphone`, `tablet`, `server`, `database`, `cloud`,
  `hard-drive`, `cpu`, `network`, `wifi`, `globe`, `box`, `file`, `folder`, `lock`, `key`,
  `search`, `settings`, `arrow-right-left`, `sync`, `upload-cloud`, and `download-cloud`.
  Restyle materialized icon instances with `override ref="icon" svg-stroke=(token)"..."` and
  `svg-stroke-width=(token)"..."`; use `svg-fill` only for icons that intentionally fill from
  `currentColor`. Add labels beside or below icon instances when text is needed; do not redraw the
  icon by hand.
- **For quantitative/data content** (comparisons, trends, proportions) use the `chart` node
  (bar/line/area/pie/donut/sparkline) — run `zenith schema node chart` for the series/categories
  child syntax; bind a series to a `--data` JSON array or CSV column with `data-ref`.
- **Group semantically.** Put related layers in `group`/`frame` with a stable id so a whole
  motif can be moved, dimmed (`set_opacity`), or removed in one operation.
- **Validate before render, render before finalize.** Hard diagnostics block finalization.
  Suppress a known-OK advisory or gate CI with a `diagnostics` policy — see `references/diagnostics.md`.
- **Keep real-object pixels external.** Photos/illustrations from an image model are declared
  as `assets` (with `sha256` for lockable provenance) and placed as `image` nodes — never bake
  text or layout into a flattened picture. See `zenith schema node image`.
- **Determinism.** Same source + backend → same bytes. No reliance on time/randomness. If you
  generate many nodes procedurally, record the parameters/seed in a note so it is replayable.
- **Verify syntax against reality, not memory.** Exact node/attribute syntax lives in
  `zenith schema node <kind>` (authoritative). When unsure of a property, run `zenith schema node
<kind>` — then validate. Do not invent syntax.

## Command surface

Discover commands with `zenith --help` and flags with `zenith <cmd> --help` (each includes an
example). Most commands support `--json` for machine-readable output. The groups, in brief:
**author** (`new` — scaffold a fresh document; `validate`, `fmt`, `tokens`, `inspect`), **render**
(`render`), **edit** (`tx` — typed, dry-run by default), **asset** (`asset import` — bring a local
image/svg/font file in as a frozen, hash-pinned asset declaration; `asset zpx-bake` — bake a
hand-authored `.zpx` raster manifest into a frozen PNG asset declaration), **variants**
(`variant` — size/format variants from one page; `merge` — CSV data mail-merge), **library**
(`library list`/`add`), **theme** (`theme new`, `theme apply`), **fonts** (`fonts` — list Bundled
vs Local/system families), **workspace** (`workspace scratch`/`candidate`/`promote`/`finalize`/
`bundle`/`unbundle` — store-backed scratch candidates), and **history** (`history`, `undo`,
`redo`, `version`, `restore`, `sync`). Do not memorize flags from this file — ask the CLI.

> Two different "variant" tools — don't confuse them: `zenith variant` varies **size/format**
> (one design → square/story/banner), `zenith merge` varies **content** (one template → many
> data rows). `references/variants.md` covers both.

## Routing — load a reference pack on demand

Read only the pack you need for the current sub-task (progressive disclosure). Each lives in
`references/` next to this file.

| The task involves…                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | Read / command                                                                                                   |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------- |
| Starting a brand-new document, or the top-level file structure (`zenith { project tokens styles document { page } }`)                                                                                                                                                                                                                                                                                                                                                                                              | `zenith new <path> --theme <name>` · `zenith schema document` · `zenith schema page`                             |
| Node/attribute names, types, required/optional status                                                                                                                                                                                                                                                                                                                                                                                                                                                              | `zenith schema node <kind>` · `zenith schema nodes`                                                              |
| Transaction op fields and semantics                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | `zenith schema op <name>` · `zenith schema ops`                                                                  |
| Command flags and usage                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            | `zenith <cmd> --help`                                                                                            |
| Syntax errors, type mismatches, "did you mean?" — act on the diagnostic                                                                                                                                                                                                                                                                                                                                                                                                                                            | `zenith validate <file> --json`                                                                                  |
| The full agent run: scratch experiments, multiple candidates, select, promote, clean up, provenance                                                                                                                                                                                                                                                                                                                                                                                                                | `references/agentic-workflow.md` + `zenith workspace --help`                                                     |
| Judging/improving a finished render — composition, balance, consistency, noise, semantic accuracy; computing alignment/spacing/margins/role-divergence from resolved geometry (style-neutral)                                                                                                                                                                                                                                                                                                                      | `references/design-critique.md` + `zenith inspect <file> --json`                                                 |
| Driving Zenith where the CLI can't run (remote/CI/sandboxed/hosted agents) — the MCP server                                                                                                                                                                                                                                                                                                                                                                                                                        | `references/agentic-workflow.md` (MCP section) + `zenith mcp --help`                                             |
| Procedural grid/scatter tiling — the `pattern` node or the `detach_pattern` op                                                                                                                                                                                                                                                                                                                                                                                                                                     | `references/pattern.md`                                                                                          |
| Recording a generated motif as a `recipes` block (provenance, seed/params, recipe `tx` ops)                                                                                                                                                                                                                                                                                                                                                                                                                        | `references/recipes-model.md`                                                                                    |
| Picking or applying a ready-made visual theme (palette + shape language)                                                                                                                                                                                                                                                                                                                                                                                                                                           | `zenith new --theme <name>` · `zenith theme apply <name> <doc>` · `references/themes.md`                         |
| Generating a theme from a brand (logo, website, brand colors)                                                                                                                                                                                                                                                                                                                                                                                                                                                      | `references/themes.md` → `zenith theme new --help`                                                               |
| Color, gradients, glows, texture, typography, "make it premium" — visual effects                                                                                                                                                                                                                                                                                                                                                                                                                                   | `zenith schema node <kind>` · `zenith schema token <type>` + design judgment                                     |
| Page setup, anchors, safe zones, frames, grids, spreads                                                                                                                                                                                                                                                                                                                                                                                                                                                            | `references/layout.md`                                                                                           |
| Structured data tables — columns, rows, cells, header rows, borders, alignment                                                                                                                                                                                                                                                                                                                                                                                                                                     | `zenith schema node table`                                                                                       |
| Long-form prose kept lean — inline markdown (`**bold**`, `==highlight==`, `[link](url)`, …), external `.md`/`.txt` files, or data-bound body text; **block-level markdown** (`format="markdown"`: headings h1–h6, paragraphs, blockquotes, lists, fenced code, `---` rules) with per-role styling via `block role="…"` decls (cascade doc > page > text); overflow fires `text.overflow` (preserve type scale first: enlarge/reflow/link boxes with `chain`; shrink only when intended or geometry is constrained) | `zenith schema node text` · `zenith schema block`                                                                |
| Bringing in a photo/illustration asset and composing around it                                                                                                                                                                                                                                                                                                                                                                                                                                                     | `zenith asset import` (register the file) · `zenith schema node image` · `zenith schema asset`                   |
| Baking a hand-authored `.zpx` raster manifest (painted layers, blend modes, gradient-map adjustments) into a frozen PNG asset                                                                                                                                                                                                                                                                                                                                                                                      | `zenith asset zpx-bake`                                                                                          |
| Defining or applying a brand/identity, or per-project style                                                                                                                                                                                                                                                                                                                                                                                                                                                        | `references/brand.md`                                                                                            |
| Many outputs from one design — **sizes/formats** (`zenith variant`) or **data** rows (`zenith merge`, binding nodes with `role="data.<column>"`)                                                                                                                                                                                                                                                                                                                                                                   | `references/variants.md` · `zenith merge --help`                                                                 |
| Per-variant tweaks (the `variants`/`override` block: hide nodes, swap text/fill, reposition with x/y/w/h)                                                                                                                                                                                                                                                                                                                                                                                                          | `zenith schema variant`                                                                                          |
| Flowcharts / diagrams / labeled boxes & buttons (boxes with centered labels + arrows)                                                                                                                                                                                                                                                                                                                                                                                                                              | `zenith schema node shape` (kind, owned label) · `zenith schema node connector`                                  |
| Architecture/product diagram icons (device, cloud, server, database, network, file/folder, lock/key, search/settings)                                                                                                                                                                                                                                                                                                                                                                                              | `zenith library list` · `zenith library show @zenith/icons-lucide#monitor` · `zenith library add @zenith/icons-lucide#monitor --into <file> --page <id> --at X,Y`             |
| Charts / data visualization (bar, line, area, pie, donut, sparkline; grouped/stacked/horizontal bars; legend; value labels)                                                                                                                                                                                                                                                                                                                                                                                        | `zenith schema node chart` · bind series to data with `render --data <file.json\|csv>` (`series data-ref="col"`) |
| Diagnostic policy (`allow`/`deny`/`warn` codes, CI gating, config files, CLI flags)                                                                                                                                                                                                                                                                                                                                                                                                                                | `references/diagnostics.md` + `zenith schema diagnostics`                                                        |
| Local/system fonts, portability, deterministic rendering, `font.local` advisory                                                                                                                                                                                                                                                                                                                                                                                                                                    | `references/diagnostics.md` · `zenith fonts`                                                                     |
| Reporting a Zenith bug or feature request (the `gh` feedback loop)                                                                                                                                                                                                                                                                                                                                                                                                                                                 | `references/reporting-issues.md`                                                                                 |

## Project configuration (brand / identity / style)

Before authoring, check whether the project pins its own design system:

1. **Brief for the agent** — if `.zenith/brand.md` exists in (or above) the working dir, **read
   it and conform to it** (palette, type, spacing, voice, do/don't). It overrides generic
   defaults. Scaffold one from `templates/brand.md` when the user wants to "set up a brand".
2. **No brand yet** — start from an embedded theme: `zenith new <path> --theme <name>` for a new
   doc, or `zenith theme apply <name|pack-id> <doc>` to re-skin one later. Project library packs
   under `libraries/*.zen` (and the embedded presets) are materializable design systems too —
   list them with `zenith library list <dir>` and pull items in with
   `zenith library add @scope/pack#item --into <file> --page <id>`. A brand kit pack can also
   ship `actions` (typed `tx` bundles) that re-skin a document's tokens in one step. See
   `references/brand.md` and `references/themes.md`.
3. **Canonical design source** — when a project keeps brand, component, or layout source in a canonical `.zen` file, import it with root-level `imports { import id="..." kind="zen" src="..." }` and reference exported pages/components with `source="import-id#page.page-id"` or `source="import-id#component.component-id"` instead of copying geometry by hand. This keeps documents DRY while preserving editable native scene output.
4. **Bespoke palette** — only invent one when the user explicitly wants a look no embedded theme
   or brand pack covers.

Project config is loaded, not assumed: prefer the project's tokens/packs over inventing a new
palette, so output stays on-brand and consistent across documents.

## Reporting

After acting, briefly state: what changed (which ids/tokens), the validate result (clean or
the remaining diagnostics), and the path to the rendered output. Keep it short; the source and
the render are the artifacts.
