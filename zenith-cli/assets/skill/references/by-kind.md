# Recipes by document kind

**Intent → scaffold → primitives → polish.** Syntax always from `zenith schema node <kind>`.
After scaffold: `zenith tokens <doc>` and use those ids only.

## Shared ergonomics (every kind)

| Topic | Rule |
| --- | --- |
| **Text box height** | Theme type tokens are font size, not box height. `size.h1` = 64 → text `h` ≈ **90+**; `size.h2` = 40 → `h` ≈ **56+**; `size.body` = 28 → `h` ≈ **40+**. Multi-line = more. Overflow → grow box first. |
| **Muted on dark** | On `pine`/`ember`/`harbor`/`sunset`, captions & page numbers: `fill=(token)"color.base.content"` + optional `opacity=0.6` — **not** `color.base.300` (that is a surface, often invisible on `base.100`). |
| **Styles** | Prefer `create_style` / `set_style_property` (`zenith schema style`). Or hand-author `styles { }` then `zenith sync`. |
| **Packs** | Only `library add` what you apply. Leftover filter/mask tokens → `token.unused` advisories; remove them. |
| **Series colors** | Some themes map `secondary` ≈ `primary`. For two chart series use `color.primary` + `color.accent` (or `info`), not primary+secondary, until `zenith tokens` shows distinct hexes. |
| **Chrome vs safe-zone** | Full-bleed accent bars / edge marks may trip `safe_zone.violation` — expected for decorative chrome. Keep **copy and CTAs** inside the safe area; suppress the advisory only if intentional (`allow` / diagnostics policy). |

```bash
zenith validate <doc> --json          # fix every Error
zenith render <doc> --png <doc>.png   # then OPEN the PNG
# siblings look uneven?
#   zenith schema op align_nodes | distribute_nodes | align_to_edge
```

---

## 1. Social square (feed post)

```bash
zenith new post.zen --name "Launch" --theme sunset
# default canvas 1080×1080
```

**Build:** one dominant headline · short subcopy · brand accent bar or mark · one CTA `shape` ·
optional Lucide icon for a concrete object. Margins ≥ ~64–96 px. Hierarchy: one focal point.

**Primitives:** `text` (title/body) · `rect` (accent only) · `shape` (CTA with `text-style`) ·
`instance` (icon via `library add @zenith/icons-lucide#…`).

**CTA pattern (theme dialect):**

```kdl
styles {
  style id="cta.label" {
    font-family (token)"font.body"
    font-size (token)"size.body"
    fill (token)"color.primary.content"
  }
}
shape id="cta" kind="process" x=(px)96 y=(px)880 w=(px)280 h=(px)72 \
    fill=(token)"color.primary" radius=(token)"radius.field" \
    text-style="cta.label" h-align="center" v-align="middle" {
  span "Get started"
}
```

Avoid: raw hex · `rect`+floating `text` for buttons · more than one accent color fighting the title.

---

## 2. Story / reel (9:16)

```bash
zenith new story.zen --name "Story" --theme ember --width 1080 --height 1920
```

**Build:** same as square but vertical rhythm — top safe (~120 px), bottom safe for UI chrome
(~200 px). Large type; short lines; CTA in the lower third but above the chrome band.
`safe-zone` + `anchor-zone` help (`references/layout.md`). Thin top brand bars outside the
safe-zone are fine chrome (advisory only).

**Primitives:** same as square. Optional `pattern` / `light` / grain for depth. If type sits
over a filtered/noisy full-bleed layer, set `contrast-bg=(token)"color.base.100"` (or the
solid band color) so validate can judge legibility — see `references/diagnostics.md`.

---

## 3. Banner / header / ad strip

```bash
zenith new banner.zen --name "Banner" --theme cobalt --width 1600 --height 400
```

**Build:** horizontal lockup — logo/mark left or right, message center/left, CTA opposite.
Use `anchor` so logo/CTA survive size variants later.

**Primitives:** `text` · `shape` CTA · `image` or icon · optional `rect` brand bar full-bleed edge.

---

## 4. Poster / flyer (print)

```bash
zenith new poster.zen --name "Poster" --theme poppy --format a4
# tabloid / a3 for large format; --landscape if needed
```

**Build:** title → supporting line → body or bullets → venue/date/CTA. Generous margins
(print safe). Type scale from theme is a starting point; posters can go larger on the title only
(add a token, don't raw-px font sizes).

**Primitives:** `text` · `shape` · optional `image` · depth via `pattern` / gradient token /
`light` — one motif. Icons only for concrete info (location, time), not decoration spam.

Render proof: `zenith render poster.zen --pdf poster.pdf` when print-bound.

---

## 5. Deck / slides

```bash
zenith new deck.zen --name "Pitch" --theme harbor --format letter --landscape --pages 8
# pages are page.1 … page.N
```

**Build:** one idea per page. Shared chrome (running head, page number, brand bar) via a
**master** — do not copy chrome nodes per page:

```bash
# after zenith new deck.zen …
zenith tx deck.zen --ops '[
  {"op":"create_master","id":"m.deck"},
  {"op":"add_node","parent":"m.deck","source":"field id=\"folio\" type=\"page-number\" x=(px)700 y=(px)540 w=(px)80 h=(px)24 fill=(token)\"color.base.content\" font-family=(token)\"font.body\" font-size=(token)\"size.caption\""},
  {"op":"set_page_master","page":"page.1","master":"m.deck"},
  {"op":"set_page_master","page":"page.2","master":"m.deck"}
]'
# discover: zenith schema op create_master|set_page_master
```

Master children project under each page at compile time (local ids become `page.N/folio`).
Title slide: big claim + sub (add a `size.display` token if 64px h1 feels small on landscape).
Content: heading + bullets **or** diagram, not both dense.

**Muted chrome:** page numbers / footers → `color.base.content` + opacity, not `base.300`.

**Primitives:** `text` · `shape` · `chart` · icons · `connector` for simple flows.
**Not:** packing a whole poster onto every slide.

```bash
zenith render deck.zen --all-pages preview/    # contact sheet — look at every page
```

---

## 6. Flowchart / process

```bash
zenith new flow.zen --name "Onboard" --theme lagoon --width 960 --height 720
```

**Pick a path:**

| | **Path A — native** | **Path B — pack** |
| --- | --- | --- |
| Use when | Yes/No/Retry labels, custom fills, help-doc control | Fast scaffold of process/decision/terminator |
| Nodes | `shape` kinds + `connector` | `library add @zenith/flowchart#…` then `connector` |
| Edges | Always hand-author `connector` (pack does not wire arrows) | Same — pack is only the boxes |

**Path A:** `shape` kinds `terminator` | `process` | `decision` | `ellipse` + `connector`
(`route="orthogonal"`, `marker-end="arrow"`). Branch labels = `span` on the connector +
`text-style`. Node labels = `text-style` + hand-authored `styles { }`.

**Path B:**

```bash
zenith library add @zenith/flowchart#terminator --into flow.zen --page page.1 --at 80,40
zenith library add @zenith/flowchart#decision --into flow.zen --page page.1 --at 90,160
zenith library add @zenith/flowchart#process --into flow.zen --page page.1 --at 80,330
zenith inspect flow.zen   # resolve instance ids before connecting
```

**Do not:** bare `line` for arrows; decorative Lucide icons for abstract steps ("validate",
"synergy"). Connector midpoint labels can sit on the stroke — keep labels short.

Schema: `zenith schema node shape` · `zenith schema node connector`.

---

## 7. Architecture / product diagram

```bash
zenith new arch.zen --name "System" --theme pine --width 1280 --height 800
```

**Build:** real things → Lucide icons; relationships → `connector`; groups of services → `frame`
or `group` with a label. Search, don't guess names:

```bash
zenith library search database --limit 5
zenith library add @zenith/icons-lucide#database --into arch.zen --page page.1 --at 120,200 --id icon.db
# then: set instance w/h in source; recolor for dark theme BEFORE render — see icons.md
```

On dark themes, update `lib.icons.stroke` or override **every** path immediately
(`references/icons.md`). Multi-edge cards: `ports { … }`, nine-point anchors, or divided
`i/N` on ellipse/rect/process/polygon/polyline/path (exact outlines for divided anchors;
named/auto stay bounds-based). Connector labels: nudge with
`label-offset-x` / `label-offset-y` (px) off the midpoint.

Optional polish: `mesh` + one `light` for technical depth.

**Do not:** labeled generic boxes for AWS/GCP/device metaphors when an icon exists.

---

## 8. Charts / KPI cards

```bash
zenith new stats.zen --name "Metrics" --theme volt --width 1080 --height 1080
```

**Build:** `chart` node — never a row of hand-sized `rect`s for bars.

```kdl
chart id="c.rev" kind="bar" x=(px)64 y=(px)120 w=(px)640 h=(px)400 \
    legend=#true legend-position="bottom" value-labels="top" title="Revenue" {
  categories "Q1" "Q2" "Q3" "Q4"
  series label="Revenue" color=(token)"color.primary" 120.0 185.0 143.0 210.0
}
```

Kinds: `bar` | `line` | `area` | `sparkline` | `pie` | `donut`.
Grouped/stacked: `bar-mode`; horizontal: `orientation`. Schema: `zenith schema node chart`.
Data-bound series: `data-ref` + `zenith render --data file.json|csv` (`zenith render --help`).

Two series: confirm distinct hexes in `zenith tokens` — if `secondary` matches `primary`, use
`color.primary` + `color.accent` (or `info`). Title/text boxes still need height headroom
(shared ergonomics). Not every theme scaffolds `shadow.depth`; flat cards are fine.

KPI strip: big number `text` + sparkline `chart` + caption — tokenize number color from theme.

---

## 9. Table / schedule

```bash
zenith new grid.zen --name "Schedule" --theme cobalt --format a4
```

**Build:** `table` with columns/rows/cells — not aligned free `text` cells.
Schema: `zenith schema node table`. Header row styling via cell fills/tokens; keep type at
`size.body` / `size.caption` so rows fit.

---

## 10. Long-form article / report

```bash
zenith new article.zen --name "Report" --theme sorbet --format a4 --pages 4
```

**Build:**

- Short copy: `text` with `span`s.
- Structured prose: `format="markdown"` (headings, lists, quotes, fences) + optional
  `block role="…"` styling (`zenith schema block`).
- External file: `src="copy/body.md"` (project-relative).
- Overflow: add chained boxes — same `chain="main"` id; first box holds content, later boxes empty
  (`zenith schema node text` for chain rules). Prefer more geometry over smaller type.
- Book bits: `footnote`, `toc`, `code` nodes as needed.

**Do not:** paste a novel into one undersized box and shrink to "fit."

---

## 11. Photo + type (campaign, feature)

```bash
zenith new feature.zen --name "Feature" --theme prism --width 1080 --height 1350
zenith asset import photo.jpg --into feature.zen --id asset.hero \
  --src assets/hero.jpg --kind image --apply
```

**Build:** `image` for the photo · type in safe regions · polish with packs:

```bash
# add ONLY the packs you will apply (unused → token.unused noise)
zenith library add @zenith/masks#vignette --into feature.zen
zenith library add @zenith/filters#warm --into feature.zen
# on the image: mask=(token)"vignette" filter=(token)"warm"
# ids confirmed via: zenith tokens feature.zen
```

Clip windows: `frame` around `image`. Prefer a solid/scrim content band under type, or set
`contrast-bg` when type sits on photo/filter (`references/diagnostics.md`). CTA = `shape` +
`styles` (hand-authored).

---

## 12. Premium / abstract background

Structure first (headline still legible), then **one** depth system:

| Tool | When |
| --- | --- |
| `pattern` grid/scatter | dots, confetti, motif tiles — `references/pattern.md` |
| `mesh` | technical grid / perspective plane |
| `light` | soft glow / ambient wash (`kind` ambient\|glow\|key\|rim) |
| gradient token | hero washes — `zenith schema token gradient` |
| noise filter | grain on a surface — `zenith schema token filter` |
| `@zenith/filters` / masks | photo grade / vignette |

Schema: `zenith schema node pattern|mesh|light`. Keep text on solid or low-noise bands;
run validate for `contrast.*`.

---

## 13. Multi-size campaign

1. Design **one** master page with `anchor` / `anchor-zone` for logo, CTA, legal.
2. Declare `variants { … }` (`zenith schema variant`).
3. `zenith variant doc.zen --out-dir out/ --manifest run.json`

Details: `references/variants.md` · `references/layout.md`.

---

## 14. Mail-merge / personalization

1. Template with `role="data.<column>"` on variable `text` / `image` nodes.
2. `zenith validate` the template once.
3. `zenith merge template.zen data.csv --out-dir out/ --name-by <col> --manifest run.json`

Details: `references/variants.md`. Spot-check longest row for overflow.

---

## 15. Brand-from-hex (no kit yet)

```bash
zenith theme new acme --scheme light --primary '#e11d48' --accent '#3b82f6'
# follow theme new --help for output path; then:
zenith theme apply <pack-or-file> doc.zen --apply
```

Or scaffold brand files: `references/brand.md` · `templates/brand.md` · `templates/brand-kit.zen`.

---

## Quick anti-patterns

| Instead of… | Use… |
| --- | --- |
| `rect` + centered `text` button | `shape` + `text-style` |
| Hand-drawn bar chart | `chart` |
| Generic boxes for servers/db/cloud | Lucide `library search` + `instance` |
| Dark theme + un-recolored Lucide | recolor `lib.icons.stroke` / every path first |
| Untouched icon size after `library add` | `set_geometry` on the instance (`w`/`h`) |
| `color.base.300` as page-number fill on dark | `color.base.content` + opacity |
| Text `h` equal to font size only | box taller than type (h1 ≈ 90px for 64px type) |
| `line` arrows between steps | `connector` |
| Guessing icon names | `library search` |
| Raw `#hex` / `font-size=(px)N` on visuals | tokens |
| `library add` packs you never apply | add only what you set on nodes |
| Five background effects | one motif + clear type |
| Eyeball alignment only | `align_nodes` / `distribute_nodes` |
| Shrinking type to clear overflow | larger box / reflow / `chain` |
