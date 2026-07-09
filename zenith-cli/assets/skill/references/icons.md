# Icons

Zenith embeds the full Lucide set (~1745 icons) as `@zenith/icons-lucide`. Icons
materialize as **editable native `path` nodes** — not images — so they take tokens,
overrides, and transactions like any other geometry.

This file is judgment and recipes. For attributes and valid values, ask the CLI:
`zenith schema node instance`, `zenith schema node text`. For which icons exist:
`zenith library search`.

## Find the icon — never guess, never enumerate

Assume the icon exists and search for it. Do not hard-code icon-name lists; do not
assume a name from another icon set (`sync` is `refresh-cw` here; `home` is `house`).

```bash
zenith library search sync                          # → refresh-cw (matched on its alias)
zenith library search arrow --category navigation   # narrow by category
zenith library search cloud --kind component --limit 5
zenith library show @zenith/icons-lucide#lock-keyhole
```

Search is **ranked**: an icon *named* for your query beats one *aliased* to it, which
beats one merely *tagged* with it. The top hit is usually right. Every query term must
match, so a two-word query that returns nothing means one of the words is wrong.

If nothing sensible matches after a couple of tries, that is a signal the concept is
abstract — **use text, not a decorative icon**.

## Use an icon where a real thing is being named

Use icons for concrete referents: devices, clouds, servers, databases, files, folders,
locks, keys, networks, users, actions (search, settings, upload). Do not draw a generic
labeled box when the thing itself has an icon.

Do **not** reach for an icon when:

- The concept is abstract and the icon would be arbitrary decoration ("synergy", "Q3",
  "strategy"). An arbitrary icon is worse than none: it asserts a meaning that isn't there.
- The item is a paragraph rather than a label. Icons pair with short phrases; beside a
  three-line block they read as bullets pretending to be meaningful.
- The design is dense with icons already. Past roughly 7 varied icons in one region,
  they stop being landmarks and become texture.

## Lists: icons by default, bullets on purpose

A list of items that each name a **distinct thing or capability** should carry an icon
per row — the icon is the fastest way to scan it. Reach for a plain bullet deliberately,
not by default.

Use a **plain bullet** (`text bullet="•" bullet-gap=(px)12`, or `format="markdown"` with
`- item`) when:

- **The same icon would repeat down every row.** A repeated icon is a bullet in costume;
  use a real bullet. Icons must differentiate rows, or they carry no information.
- **Items are long or prose-like** — more than about one line each.
- **The list is ordered or sequential** — use numbers (`format="markdown"` with `1.`),
  because an icon cannot express position.
- **You cannot name the icon in one word tied to that item.** If choosing it takes
  argument, the reader will not decode it either.

Mixed lists are fine: give icons to the rows that have real referents and leave the rest
plain — but keep the left edge of the labels aligned across all rows.

## The icon-row recipe

Icons are 24×24 at natural size. Give the instance a `w`/`h` box and it scales into it;
`fit` defaults to `contain` (uniform, centered). Without `w`/`h` it renders at 24px.

An icon reads best at roughly the text's line height, with the label's left edge on a
consistent gutter. This renders correctly (16px text on a 28px line, 20px icon, 12px gap):

```kdl
instance id="row.icon" component="lib.zenith.icons-lucide.shield-check" \
    x=(px)40 y=(px)44 w=(px)20 h=(px)20 {
  override ref="icon.0" stroke=(token)"color.accent"
  override ref="icon.1" stroke=(token)"color.accent"
}
text id="row.label" x=(px)72 y=(px)40 w=(px)400 h=(px)28 \
    font-size=(token)"size.body" fill=(token)"color.fg" { span "Signed, reproducible builds" }
```

Vertically center the icon against the text box: `icon.y = text.y + (line_height - icon_h) / 2`.

## Three gotchas that will bite you

1. **Recoloring means overriding EVERY path.** An icon is `icon.0 … icon.N-1`, and `N`
   varies per icon: `zap` has 1 path, `lock-keyhole` has 3. An `override ref="icon.0"`
   alone recolors part of the icon and leaves the rest at its default ink — it renders,
   validates clean, and looks broken. Get the count from the CLI before writing overrides:

   ```bash
   zenith library show @zenith/icons-lucide#lock-keyhole   # → nodes : path(3)
   ```

2. **`stroke-width` scales with the box.** A 2px stroke in a 24px icon becomes 8px at
   96px. Blow an icon up and it turns into a fat cartoon. Override `stroke-width` on each
   path to hold the optical weight you want.

3. **Visual properties must be tokens.** `font-size=(px)16` is an Error
   (`token.raw_visual_literal`). Geometry (`x`/`y`/`w`/`h`) may be raw px; color, size,
   and stroke must be `(token)"…"`. Tokenize the icon's ink and size once
   (`color.accent`, `size.icon`) and a palette swap moves every icon.

## Bring your own icon set

A directory of `*.svg` under `<project>/libraries/<name>/` is a pack — nothing to author.
Each file is one icon, id = file stem, addressed `@local/<name>#<stem>`, converted to
native paths on demand exactly like the bundled set. Add an optional `library.kdl` beside
the icons to declare `id`/`version`/`license` and per-icon `aliases`, `tags`, and
`categories` so search can find them by more than filename.

```bash
zenith library list <project>              # your set appears alongside the presets
zenith library search rocket <project>
```

## Always look

Icons are the easiest thing to get subtly wrong — misaligned by 2px, one path left
unrecolored, an icon whose meaning does not survive contact with its label. Render the
page and **open the PNG**. A clean `zenith validate` cannot see any of it.
