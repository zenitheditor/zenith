# Layout — pages, anchors, frames, spreads

## Pages

```kdl
document id="doc.social" title="Social" {
  page id="page.sq" w=(px)1080 h=(px)1080 background=(token)"color.bg" {
    # nodes…
  }
}
```

- `w` / `h` set the canvas in px; `background` takes a color **or** gradient token.
- A document can hold multiple pages (deck slides, book pages, size variants). Render one with
  `--page N`, all with `--all-pages <dir>`, or a facing-page `--spread A-B`.

## Coordinates vs anchors

Nodes take explicit `x y w h`. For placement relative to the page, use `anchor` instead of
hand-computing coordinates — it resolves to deterministic geometry (identical bytes to the
hand-placed version).

```kdl
rect id="logo" w=(px)160 h=(px)60 fill=(token)"color.brand" anchor="top-left"
text id="cta"  w=(px)300 h=(px)80  anchor="bottom-right" font-size=(token)"size.body" { span "Buy now" }
```

Nine-point anchors (confirmed in `examples/anchors.zen`):
`top-left top-center top-right center-left center center-right bottom-left bottom-center bottom-right`.

Use anchors for logos, page numbers, captions, and CTAs so they stay correctly placed across
size variants. Keep a **safe area**: don't let content-critical nodes touch the page edge.

## Frames (clipping) and groups

- `frame id x y w h { … }` clips its children to its box — use it for image windows, cards,
  and any "nothing escapes this region" layout (`examples/frame.zen`).
- `group id { … }` bundles nodes logically (no clip) so a whole motif moves/dims/deletes as a
  unit (`examples/group.zen`). Opacity and transforms cascade through groups/frames.

## Dividers and rules

`line x1 y1 x2 y2 stroke=(token) stroke-width=(token)` for separators/rules
(`examples/line.zen`).

## Multi-size variants

For square/story/banner from one design: duplicate the page, change `w`/`h`, and let anchored
nodes reflow; reposition free-coordinate decorative nodes as needed. Keep all variants on the
same tokens. (A first-class responsive/recipe-aware regeneration model is roadmap, not
shipped — don't assume it.)

## Always verify

Anchors, frames, and clipping interact; render (`--all-pages` for a contact sheet) and look
before finalizing, and `zenith validate` to catch off-canvas / overflow.
