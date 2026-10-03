# Recipes by document kind

Each recipe: scaffold → structure → primitives. Syntax comes from `zenith schema node <kind>`.

## Shared rules

- Scaffold with `zenith new <doc> --theme <name>` plus the canvas flags below.
- Use only ids from `zenith tokens <doc>`, plus any you add.
- Build structure from layout frames (`references/layout.md`). The outer frame takes `x`/`y` or `anchor`.
- Leave font, fill, and radius to theme defaults. Set `style="ui.*"` for a role (`references/themes.md`).
- Run the loop: `validate --json` → `fix --apply` → `render --contact-sheet <png> --scale 0.5`.
- Add a pack item only when you apply it. Leftover filter or mask tokens raise advisories.
- Library items land on a page. `reparent` an item into a layout frame.

## 1. Social square

- Scaffold: default 1080×1080.
- Structure: one column frame with accent bar, headline, one subline, CTA.
- Primitives: `text style="ui.h1"`, `text`, `shape style="ui.button"`. Optional Lucide icon for a concrete object.
- Margins: 64-96 px. One focal point. One accent color.
- Bespoke-token starting point: `templates/social-square.zen`.

## 2. Story / reel (9:16)

- Scaffold: `--width 1080 --height 1920`.
- Structure: as the square, inside a `safe-zone`. Keep ~120 px top and ~200 px bottom clear for app chrome.
- Anchor the content frame with `anchor-zone`. Edge bars outside the zone are chrome (advisory only).
- Text over grain or photo: set `contrast-bg` (`references/diagnostics.md`).

## 3. Banner / header

- Scaffold: `--width 1600 --height 400`.
- Structure: one row frame, `justify="space-between"`, `align="center"`: mark, message, CTA.
- Anchor the row so it survives size variants.

## 4. Poster / flyer (print)

- Scaffold: `--format a4` (or `a3`, `tabloid`, `--landscape`).
- Structure: one column frame: title, supporting line, body or bullets, venue/date/CTA.
- A larger title needs a new size token, never a raw px font size.
- One depth motif: `pattern`, gradient token, or `light`.
- Print proof: `zenith render <doc> --pdf <out.pdf>`.

## 5. Deck / slides

- Scaffold: `--format letter --landscape --pages N`. Pages are `page.1` … `page.N`.
- Shared chrome (running head, page number, brand bar) goes on a master.
- Master ops in one tx file: `create_master`, `add_node` with the master as `parent`, `set_page_master` per page.
- Master children project under each page as `page.N/<id>`.
- One idea per page: heading + bullets, or heading + one diagram.
- A title slide on 64 px `size.h1` can need a larger `size.display` token.
- Page numbers and footers: `color.base.content` plus `opacity`.
- Check every page in one image with `--contact-sheet`.

## 6. Flowchart / process

- Scaffold: any size, e.g. `--width 960 --height 720`.
- Nodes: `shape` kinds `terminator`, `process`, `decision`, `ellipse`. Labels pick up the `shape` default.
- Or add `@zenith/flowchart#process` / `#decision` / `#terminator` with `zenith library add`.
- Edges: always a `connector` (`route="orthogonal"`, `marker-end="arrow"`). The pack draws no edges.
- Branch labels: `span` children on the connector. Nudge with `label-offset-x` / `label-offset-y`.
- A straight chain of steps can sit in a row or column frame. Connectors still draw the arrows.
- Never use bare `line` for arrows. Never add icons for abstract steps.
- Run `zenith inspect <doc>` for instance ids before wiring connectors.

## 7. Architecture / product map

- Scaffold: any size, e.g. `--width 1280 --height 800`.
- Real things are Lucide icons. Find names with `zenith library search <term>`.
- Service groups are `frame`s with a label. Relationships are `connector`s.
- Multi-edge cards: a `ports` block (`zenith schema ports`), nine-point anchors, or divided `i/N` anchors.
- Dark themes: recolor icons before the first render (`references/icons.md`).
- Optional depth: `mesh` + one `light`.

## 8. Charts / KPIs

- Scaffold: any size.
- Data is a `chart` (`bar`, `line`, `area`, `sparkline`, `pie`, `donut`). Never hand-drawn bars.
- Grouped or stacked: `bar-mode`. Horizontal: `orientation`. Data binding: `data-ref` + `render --data`.
- Two series: if `secondary` ≈ `primary` in `zenith tokens`, use `color.primary` + `color.accent`.
- Chart text: the chart style sets `font-family`, `font-size`, and `fill`. `stroke` colours the axes.
- Title draws at 1.25 × `font-size`. Value labels draw at 0.875 × `font-size`.
- No style: text scales with the chart box and contrasts with the fill under it.
- A theme row `chart style="ui.chart"` styles every chart. Text on a dark frame takes its `.content` colour.
- KPI tile: column frame of big number, `sparkline` chart, caption.

## 9. Table / schedule

- Scaffold: `--format a4`.
- Rows of data are a `table`, never free `text` cells.
- Keep cell type at `size.body` or `size.caption` so rows fit.

## 10. Long-form article / report

- Scaffold: `--format a4 --pages N`.
- Short copy: `text` with `span`s.
- Structured prose: `format="markdown"`, styled with `block role="…"` (`zenith schema block`).
- External copy: `src="copy/body.md"` (project-relative).
- Overflow: add boxes with the same `chain` id. Only the first box holds content.
- Book parts: `footnote`, `toc`, `code` nodes.
- Never shrink a long text into one undersized box.

## 11. Photo + type

- Scaffold: any size, e.g. `--width 1080 --height 1350`.
- Import: `zenith asset import <file> --into <doc> --id <asset-id> --src <rel-path> --kind image`.
- Place an `image`. Clip it with an absolute `frame`.
- Grade or vignette: `@zenith/filters` and `@zenith/masks` tokens on the image (`filter=`, `mask=`).
- Put type on a solid or scrim band, or set `contrast-bg`.

## 12. Premium background

Build structure first. Then add one depth system:

| Tool | Use |
| --- | --- |
| `pattern` | dots, confetti, motif tiles (`references/pattern.md`) |
| `mesh` | technical grid, perspective plane |
| `light` | glow, ambient wash |
| gradient token | hero wash (`zenith schema token gradient`) |
| `noise` filter token | grain (`zenith schema token filter`) |

- Set `role="background"` on the depth layer. It then leaves overlap lint.
- Keep text on solid or low-noise bands.

## 13. Multi-size campaign

1. Build one master page. Anchor logo, CTA, and legal.
2. Declare a `variants` block (`zenith schema variant`).
3. Run `zenith variant <doc> --out-dir <dir> --manifest run.json`.

Details: `references/variants.md`.

## 14. Mail-merge

1. Set `role="data.<column>"` on variable `text` and `image` nodes.
2. Validate the template once.
3. Run `zenith merge <template> <data.csv> --out-dir <dir> --name-by <col> --manifest run.json`.
4. Check the longest row for overflow.

## 15. Brand from hex

- Run `zenith theme new <name> --scheme light --primary '<hex>'` (see `zenith theme new --help`).
- Then run `zenith theme apply <pack> <doc> --apply`.
- For a full kit: `references/brand.md`, `templates/brand.md`, `templates/brand-kit.zen`.

## Anti-patterns

| Instead of | Use |
| --- | --- |
| Hand-computed x/y for stacked siblings | layout `frame` |
| `rect` + centered `text` button | `shape style="ui.button"` |
| Font/fill/radius on every node of a themed doc | theme defaults and `ui.*` styles |
| Fixed `h` on text inside a layout frame | omitted `h` |
| Hand-drawn bar chart | `chart` |
| `line` arrows | `connector` |
| Generic boxes for servers, databases, clouds | Lucide icons |
| Guessed icon names | `zenith library search` |
| `color.base.300` as text on dark themes | `color.base.content` + `opacity` |
| Hand-tokenizing raw literals | `zenith fix --apply` |
| Shrinking type to clear overflow | bigger box, reflow, or `chain` |
| Five background effects | one motif |
