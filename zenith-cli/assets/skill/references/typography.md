# Typography & text

## Text nodes and spans

A `text` node has a box (`x y w h`) and contains one or more `span`s. Per-span overrides let
you mix styles in one line.

```kdl
tokens format="zenith-token-v1" {
  token id="font.body" type="fontFamily" value="Noto Sans"
  token id="size.h1"   type="dimension"  value=(px)64
  token id="color.ink" type="color"      value="#111827"
  token id="color.link" type="color"     value="#2563eb"
}
text id="hero.title" x=(px)80 y=(px)120 w=(px)920 h=(px)200
     fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.h1" {
  span "Design that works like "
  span "code" fill=(token)"color.link" underline=#true
  span "."
}
```

Confirmed span decorations: `underline=#true`, `strikethrough=#true`, per-span `fill`
(`examples/decorations.zen`). For bold/italic and other run attributes, check
`examples/bold.zen`, `examples/italic.zen`, and `examples/highlight.zen`.

## Fonts

- Bundled, always available: **Noto Sans** (regular/bold/italic/bold-italic) and
  **Noto Sans Mono** (regular/bold). Use `Noto Sans Mono` for code via the `code` node
  (`examples/code.zen`).
- A glyph the font can't render raises a `font.glyph_missing` diagnostic — `validate` catches
  it; pick a covered font or different glyph.

## Layout behaviour

- Text wraps within its box; Zenith does real shaping (rustybuzz) and Knuth–Liang hyphenation.
- If text overflows its box, validation reports it — enlarge the box or reduce the size rather
  than letting it clip. Never finalize with a text-fit Error.
- Size and family are tokens so a type-scale or font change is one edit.

## Quality checks

- **Contrast**: Zenith's `contrast.low` check uses **APCA (WCAG 3)** — keep body text at
  **Lc ≥ 60** against its background (large/bold text ≥ 45). Over busy or
  textured backgrounds add a scrim (translucent rect, see `references/recipes.md`) behind the
  text rather than hoping it reads.
- **Hierarchy**: distinct size tokens for `size.h1` / `size.h2` / `size.body`; don't fake
  hierarchy with color alone.
- Always `zenith render` and _look_ — shaping/wrapping is exact but your box math may be off.
