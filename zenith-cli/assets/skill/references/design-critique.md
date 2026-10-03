# Design critique — judging a rendered `.zen`

`validate` reports defects. It cannot judge quality. This file is the judgment layer. It enforces no style.

## Facts vs judgment

| Source | Tells you |
| --- | --- |
| `zenith validate <doc> --json` | Defects: overflow, contrast, overlap, near-miss alignment, uneven gaps (`references/diagnostics.md`) |
| `zenith inspect <doc> --json` | Each node's final `box` after layout and anchors, its `role`, and painted `bounds` |
| The contact sheet | Everything the numbers miss |
| You | Whether it is good for this intent |

- Clear every Error and Warning before judging aesthetics.
- Style is an input, never a fix. Flat, minimal, and maximal can all succeed.
- Never add gradients, shadows, or icons as a reflex. Name the violated principle and fix that.

## Order: coarse → fine

1. **Intent and primitives** — labeled box is a `shape`, data is a `chart`, a comparison is no arrow.
2. **Composition and balance** — one dominant element, no large dead zones.
3. **Consistency** — same-role elements share treatment across pages.
4. **Noise** — every mark earns its place.
5. **Semantic accuracy** — every visual is a claim. An arrow asserts direction or cause.

Reversing the order polishes things you later move or delete.

## Questions per page

- **Hierarchy** — is there one focal point?
- **Balance** — does content sit with intent, or drift beside a dead band?
- **Alignment** — do edges that belong together match exactly?
- **Rhythm** — are gaps in a series even?
- **Consistency** — does one element break the system?
- **Noise** — what can be deleted?
- **Legibility** — does text read on its actual backdrop?

"Valid but flat, hollow, cramped, half-empty, or misaligned" fails. Re-render until it passes.

## Using `inspect --json`

Deciding which nodes form a set is your call. Compute aggregates from `box`:

- **Alignment / rhythm** — `align.near_miss` and `spacing.uneven_gap` flag drift. Move drifting siblings into a layout frame.
- **Margins** — per page, compare min/max content edges with page `w`/`h`. Keep them equal across pages.
- **Balance** — an area-weighted center far from the page center beside empty space means rebalance.
- **Consistency** — group nodes by `role` across pages. Every `role="heading"` shares font, size, and position.

Tag nodes with `role` (`heading`, `accent-rule`, `footer`, `card`). Cross-page checks then become mechanical.

## Report

State the weak principle, the changed node ids, the render path, and the validate result.
