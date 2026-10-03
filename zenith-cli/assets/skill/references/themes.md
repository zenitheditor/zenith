# Themes — token contract, styles, defaults

A theme is a token contract, the `ui.*` styles, and a `defaults` block under fixed ids. Swapping the theme re-skins any document built on it.

- Start: `zenith new <doc> --theme <name>`.
- Switch: `zenith theme apply <name> <doc>` (dry-run), then add `--apply`.
- From brand hexes: `zenith theme new --help`.
- Current set: `zenith library list`.

A theme picks no layout. Pick canvas and primitives in `references/by-kind.md` first.

## Token contract

| Group | Ids |
| --- | --- |
| Surfaces | `color.base.100` (page), `color.base.200`, `color.base.300`, `color.base.content` |
| Roles | `color.primary`, `color.secondary`, `color.accent`, `color.neutral`, each with `.content` |
| Status | `color.info`, `color.success`, `color.warning`, `color.error`, each with `.content` |
| Shape | `radius.box`, `radius.field`, `radius.selector`, `border.width`, `space.unit`, `shadow.depth` (depth themes only) |
| Type | `font.heading`, `font.body`, `size.display` 112, `size.h1` 64, `size.h2` 40, `size.body` 28, `size.caption` 18, `font.weight.heading` |

- Each `X.content` is the readable ink on fill `X`.
- `color.base.200` and `color.base.300` are surfaces, never text ink.
- Two chart series: if `secondary` ≈ `primary`, pair `primary` with `accent` or `info`.

## Styles and defaults

| Style | Use |
| --- | --- |
| `ui.body` | default `text` |
| `ui.display` | hero title (`size.display`) |
| `ui.h1`, `ui.h2`, `ui.caption` | headings, captions |
| `ui.label` | default shape label |
| `ui.control` | default `shape` |
| `ui.button` | primary button shape |
| `ui.card` | card frame or rect |
| `ui.connector` | default `connector` (label `ui.caption`) |

- `rect`, `frame`, and `ellipse` take no default. Set `style="ui.card"` to opt in.
- Content pairing: an unset text or label fill takes the `.content` token of the fill under it.
- `X` pairs with `X.content`. A step `P.N` pairs with `P.content`.
- Text on a `ui.card` frame gets `color.base.content`. A `ui.button` label gets `color.primary.content`.
- So author bare `text`, `shape`, and `connector` with no font, fill, or radius attributes.
- Set a node attribute only to override one key.
- Cascade and placement rules: `zenith schema defaults`.

`templates/social-square.zen` shows the same styles, defaults, and pairing on bespoke tokens.

## Catalog

| Theme | Scheme | Character (raised = `shadow.depth`) | Box radius |
| --- | --- | --- | --- |
| `prism` | light | bright cyan/violet, raised, grain | 8px |
| `sorbet` | light | soft warm pastel, flat | 16px |
| `cobalt` | light | crisp corporate indigo, grain | 32px |
| `volt` | light | electric lime + black, raised | 32px |
| `poppy` | light | vivid scarlet + magenta, raised, grain | 16px |
| `lagoon` | light | teal + blue, technical, raised, grain | 4px |
| `pine` | dark | emerald/teal, flat | 8px |
| `ember` | dark | amber-gold + green, raised, grain | 32px |
| `harbor` | dark | navy, amber + sky, 2px border, grain | 16px |
| `sunset` | dark | navy, orange + indigo, grain | 4px |

- Light/dark pairs: `sorbet`↔`pine`, `cobalt`↔`harbor`, `prism`↔`sunset`.
- Dark themes: captions use `color.base.content` plus `opacity`. Recolor Lucide icons before the first render (`references/icons.md`).
- "Grain" is a header flag, not a token. Add a `noise` filter token yourself (`zenith schema token filter`).

## Rules

- `theme apply` adds missing styles and `defaults` rows. It keeps an existing style id or kind and reports it as skipped.
- Theme tokens carry `set="@zenith/theme.<name>"`. Theme sets never raise `token.set_partially_used`. Other token sets still do.
- Keep the contract whole. Trimming it breaks the one-command re-skin.
- One dialect per document. Never mix theme ids with brand ids (`references/brand.md`).
- To make a theme the project default, record it in `.zenith/brand.md`.
