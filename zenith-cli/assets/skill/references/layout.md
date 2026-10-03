# Layout — auto-layout, anchors, frames, pages

Attributes and types: `zenith schema node frame`, `zenith schema page`. This file covers choice and gotchas.

## Pick the placement tool

| Content | Tool |
| --- | --- |
| Stack, row, list, card, chip, button row, tile grid | layout `frame` |
| Logo, CTA, page number, badge pinned to an edge | `anchor` |
| Top-level block on the page | `x`/`y` on the outer frame, or `anchor` |
| Free decoration (glow, blob, motif) | `x`/`y` plus `role="decoration"` |

Never compute sibling coordinates by hand where a layout frame fits. Hand math drifts 1-3 px and breaks on copy edits.

## Layout frames

`frame layout="row|column|grid"` places and sizes its children.

- **Container** — `gap`, `padding*`, `justify`, `align`, `wrap=#true` + `wrap-gap`. Grid takes `columns` / `rows`.
- **Child size** — per axis: fixed, `"hug"` (content), or `"fill"` (share of free space). `min-*` / `max-*` clamp it.
- **Omitted size** — main axis hugs. Cross axis fills under `align="stretch"`.
- **Frame size** — a layout frame without `w`/`h` hugs its children. Give the outer frame a `w` so text wraps.
- **Text** — omit `h` on text inside a layout frame. The compile measures it.
- **Out of flow** — `position="absolute"` places a child by `x`/`y` from the frame's top-left.
- **Clipping** — row and column frames clip only with `clip=#true`. `layout="absolute"` (default) clips.
- **Anchored layout frame** — measures first, then anchors at its measured size.

A card is a column frame. A chip row is a wrapping row frame of `w="hug"` row frames.

Editing:

- `set_layout` sets or clears container and child fields (`zenith schema op set_layout`).
- `tx` rejects `x`/`y` on an in-flow child with `tx.layout_managed`.
- To move an in-flow child, reorder it. Or set `position="absolute"` plus `x`/`y` in one tx.
- `zenith inspect <file> --json` reports each node's final `box` after layout and anchors.
- `inspect` adds `rotate` and the painted `bounds` when they differ from `box`.

## Anchors

`anchor` takes a nine-point name: `top-left` … `bottom-right`. It resolves to plain geometry at compile time.

- Explicit `x` or `y` wins over the anchor per axis.
- Without a layout frame, the node needs `w`/`h` in px.
- Reference box precedence: `anchor-zone` > `anchor-sibling` > `anchor-parent=#true` > page.
- `anchor-zone="<id>"` anchors inside a page `safe-zone`. Keep copy and CTAs there.
- `anchor-sibling="<id>"` anchors to a sibling's box. Cycles error with `anchor.cycle`.

`anchor-edge` with `anchor-sibling` places the node outside the sibling, `anchor-gap` px away:

| `anchor-edge` | Position | Cross-axis default |
| --- | --- | --- |
| `below` / `above` | under / over the sibling | left edge. The `anchor` horizontal part overrides it. |
| `after` / `before` | right / left of the sibling | top edge. The `anchor` vertical part overrides it. |

Thin brand bars flush to the page edge emit `safe_zone.violation`. That advisory is expected for chrome. Do not move copy out of the safe zone to clear it.

## Frames and groups

- Children of every frame count `x`/`y` from the frame's top-left. A child at `x=(px)0 y=(px)0` sits on the frame's corner.
- Moving a frame moves its children. Never offset child coordinates by the frame's position.
- `frame` with the default `layout="absolute"` clips its children. Use it for image windows.
- `group` bundles nodes without clipping. A motif then moves, dims, or deletes in one op.
- Opacity and transforms cascade through groups and frames.
- `protected-region` children of a group mark text-free areas. They do not render.
- `line` draws rules and dividers.

## Pages and sizes

- A document holds many pages. Render one with `--page N`, or all with `--contact-sheet`.
- One design at many sizes: a `variants` block plus `zenith variant` (`references/variants.md`).
- Anchored nodes reflow per size. Free-coordinate decoration needs per-variant overrides.

## Check

Run `zenith validate <file> --json` for overflow and off-canvas nodes. Then render `--contact-sheet <png> --scale 0.5` and look.
