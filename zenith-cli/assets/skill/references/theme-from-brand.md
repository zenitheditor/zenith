# Make a theme from a brand

When the user gives you a brand — a logo, a website, brand docs, screenshots, or
just a couple of colors — turn it into a complete Zenith theme with the native
**`zenith theme new`** command. The engine derives the full contract (surfaces,
every role's readable `.content` foreground, and status colors) and picks the
foregrounds by **APCA (WCAG 3)** contrast, so text is legible by construction.
You only supply the brand hues.

## 1. Extract the brand palette

Get the key colors as `#rrggbb` hex:

- **Logo / image / screenshot** — look at it and read off the dominant brand
  color (→ primary), a secondary, and an accent. Sample exact pixels rather than
  guessing when you can.
- **Website** — `WebFetch` the page and pull the CSS custom properties / brand
  colors, or inspect a screenshot. Note whether the brand reads light or dark.
- **Brand doc / style guide** — read the stated palette and type/spacing rules.

Decide the **scheme** (light or dark) and, optionally, the shape language
(corner radius, border width, raised vs flat, grain).

## 2. Synthesize the theme

```bash
zenith theme new acme --scheme light --primary "#7c3aed" \
    --secondary "#06b6d4" --accent "#f43f5e" \
    --radius-box 16 --radius-field 8 --depth \
    --out themes/acme.zen
```

- Only `--scheme` and `--primary` are required; unset roles are derived
  (secondary→primary, accent→secondary, neutral→tinted grey, status→universal
  hues). Override any of `--secondary/--accent/--neutral/--info/--success/--warning/--error`.
- Shape: `--radius-box/--radius-field/--radius-selector`, `--border` (px),
  `--depth` (adds a `shadow.depth` elevation token), `--noise` (records a grain
  flag — apply the grain recipe from `references/recipes.md`).
- Omit `--out` to print the theme to stdout for review.

The result is canonical `.zen` under the theme contract (`references/themes.md`),
already validated by the engine on the way out.

## 3. Verify and name

```bash
zenith validate themes/acme.zen --json     # should be clean
zenith render themes/acme.zen --png themes/acme.png   # eyeball the swatches
```

Name the theme by its character if the brand has no name (see the naming notes in
`references/themes.md`). Then use it: copy its tokens into new documents, or apply
it to existing ones via a brand action (`references/brand.md`).

## Why native (not a script)

`zenith theme new` is a built-in command — no Python or external tooling. The
colour math (APCA contrast, palette derivation) lives in the engine, so the same
deterministic result is produced everywhere, and the agent just calls the CLI.

> Pasting a daisyUI/Tailwind `oklch(...)` theme directly is not yet supported —
> the color parser takes sRGB hex / CMYK. Convert the brand's key colors to hex
> first (or pull the hex equivalents), then feed them to `zenith theme new`.
