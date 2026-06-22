---
name: zenith
description: "Author, edit, and render deterministic .zen design documents (posters, decks/slides, social graphics, flyers, books, magazines, diagrams, ads) with the zenith CLI. Use when the task is to create or change a visual design as structured, editable, version-controllable source — not a flat AI image. Covers: design tokens & color (sRGB + CMYK), gradients, typography, layout & anchors, frames/groups, images, visual recipes & procedural backgrounds, transactions (typed edits), variants/mail-merge, PNG/PDF rendering, validation, brand kits, and the agentic author->validate->render->inspect->edit loop. Triggers: design, poster, deck, slide, social graphic, flyer, brochure, banner, diagram, flowchart, .zen, zenith, brand kit, render to PNG/PDF."
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
for that, then compose the resulting asset into a `.zen` document — see
`references/images.md`); pure backend/code tasks; or editing existing raster files.

## First: confirm the tool is available

```bash
zenith --version        # if missing, see https://github.com/farhan-syah/zenith#install
zenith --help           # top-level command list
zenith <command> --help # exact flags for any command
```

If `zenith` is not installed, tell the user the one-line installer
(`curl -fsSL https://raw.githubusercontent.com/farhan-syah/zenith/main/scripts/install.sh | sh`)
rather than guessing — do not fabricate a workflow you can't run.

## The core loop (always follow this)

Zenith is deterministic and auditable; lean into that instead of guessing.

1. **Plan in source.** Capture the brief, palette, and layer plan as `note`/`role="guide"`
   content or a sidecar, so the design can be traced back to intent.
2. **Author / edit.** Write `.zen` source (new work) or apply a typed transaction with
   `zenith tx` (changes to existing nodes). Prefer `tx` for edits — it is dry-run by default,
   shows a source + scene diff, and enforces id-uniqueness and referential integrity.
3. **Validate.** Run `zenith validate <file> --json` after every change. **Never finalize
   while hard (Error) diagnostics remain.** Fix at the source.
4. **Render to inspect.** `zenith render <file> --png out.png` (or `--all-pages <dir>` for a
   contact sheet, `--pdf` for print). Actually look at the output before declaring success.
5. **Iterate**, then report what changed and where the output is.

Run `zenith fmt <file>` to canonicalize source; it is idempotent.

## Non-negotiable best practices

These make designs editable, on-brand, and reproducible — and keep the agentic loop sound.

- **Stable, meaningful ids.** Every node carries an `id`. Use semantic, hierarchical ids
  (`bg.grunge`, `hero.title`, `cta.button`) so later edits and transactions target exactly
  the right node. Anonymous node soup cannot be edited later.
- **Tokenize everything.** Colors, fonts, sizes, gradients, shadows go through `token`s and
  are referenced with `(token)"id"`. Never embed raw hex/sizes in nodes — a palette/brand
  swap must be a token-value change, not a geometry rewrite.
- **Group semantically.** Put related layers in `group`/`frame` with a stable id so a whole
  motif can be moved, dimmed (`set_opacity`), or removed in one operation.
- **Validate before render, render before finalize.** Hard diagnostics block finalization.
- **Keep real-object pixels external.** Photos/illustrations from an image model are declared
  as `assets` (with `sha256` for lockable provenance) and placed as `image` nodes — never bake
  text or layout into a flattened picture. See `references/images.md`.
- **Determinism.** Same source + backend → same bytes. No reliance on time/randomness. If you
  generate many nodes procedurally, record the parameters/seed in a note so it is replayable.
- **Verify syntax against reality, not memory.** Exact node/attribute syntax lives in the
  repo's `examples/*.zen` and `zenith <command> --help`. When unsure of a property, read an
  example or run `zenith inspect <file>` — then validate. Do not invent syntax.

## Command surface

Every command supports `--json` for machine-readable output. Run `zenith <cmd> --help` for flags.

| Group        | Commands                                                                         |
| ------------ | -------------------------------------------------------------------------------- |
| **Author**   | `validate` · `fmt` · `tokens` · `inspect`                                        |
| **Render**   | `render` (`--png` · `--pdf` · `--scene` · `--all-pages` · `--spread` · `--page`) |
| **Edit**     | `tx` (typed transactions, dry-run by default; add `--apply` to write)            |
| **Variants** | `merge` (CSV mail-merge → one render per row)                                    |
| **Library**  | `library list` · `library add` (materialize reusable component/token packs)      |
| **Theme**    | `theme new` (synthesize a theme pack from brand colours; APCA-safe `.content`)   |
| **History**  | `history` · `undo` · `redo` · `version` · `restore` · `sync`                     |

## Routing — load a reference pack on demand

Read only the pack you need for the current sub-task (progressive disclosure). Each lives in
`references/` next to this file.

| The task involves…                                                                                  | Read                                                       |
| --------------------------------------------------------------------------------------------------- | ---------------------------------------------------------- |
| The full agent run: scratch experiments, multiple candidates, select, promote, clean up, provenance | `references/agentic-workflow.md`                           |
| Backgrounds, gradients, glows, patterns, motifs, texture/grain, "make it look premium"              | `references/recipes.md`                                    |
| Picking or applying a ready-made visual theme (palette + shape language)                            | `references/themes.md` + `themes/*.zen`                    |
| Generating a theme from a brand (logo, website, brand colours)                                      | `references/theme-from-brand.md` (uses `zenith theme new`) |
| Color systems, palettes, sRGB vs CMYK, gradient tokens                                              | `references/color.md`                                      |
| Text, fonts, spans, wrapping, hyphenation, contrast                                                 | `references/typography.md`                                 |
| Page setup, anchors, safe zones, frames, grids, spreads                                             | `references/layout.md`                                     |
| Bringing in a photo/illustration asset and composing around it                                      | `references/images.md`                                     |
| Defining or applying a brand/identity, or per-project style                                         | `references/brand.md`                                      |

## Project configuration (brand / identity / style)

Before authoring, check whether the project pins its own design system:

1. **Brief for the agent** — if `.zenith/brand.md` exists in (or above) the working dir, **read
   it and conform to it** (palette, type, spacing, voice, do/don't). It overrides generic
   defaults. Scaffold one from `templates/brand.md` when the user wants to "set up a brand".
2. **Machine brand kit** — project library packs under `libraries/*.zen` (and the embedded
   presets) are materializable design systems. List them with `zenith library list <dir>` and
   pull items in with `zenith library add @scope/pack#item --into <file> --page <id>`. A brand
   kit pack can ship `actions` (typed `tx` bundles) that re-skin a document's tokens in one
   step. See `references/brand.md`.

Project config is loaded, not assumed: prefer the project's tokens/packs over inventing a new
palette, so output stays on-brand and consistent across documents.

## Reporting

After acting, briefly state: what changed (which ids/tokens), the validate result (clean or
the remaining diagnostics), and the path to the rendered output. Keep it short; the source and
the render are the artifacts.
