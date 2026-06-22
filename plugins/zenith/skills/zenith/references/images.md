# Images & hybrid (image-model) composition

Zenith is **not** an image generator. When a design needs real-object pixels (a product photo,
a person, an illustration), generate that _asset_ with an image model, then compose it natively
in Zenith — layout, background, text, and variants stay editable deterministic source. Never
let a flattened full-design picture replace the document.

## Declaring and placing an asset

```kdl
assets {
  asset id="asset.hero" kind="image" src="assets/hero.png" sha256="<64-hex>"
}
# on the page:
image id="hero.photo" asset="asset.hero" x=(px)0 y=(px)0 w=(px)1080 h=(px)600 fit="cover"
```

- `src` is relative to the document. Confirmed `fit` values include `cover` and `stretch`
  (`examples/image.zen`, `examples/filter.zen`); check `zenith inspect`/`--help` for the full
  set (e.g. `contain`) before relying on others.
- **Provenance / lockability**: record `sha256` for each asset. Render with
  `zenith render <file> --png out.png --locked` to verify every asset's bytes against its
  declared hash and fail on mismatch — use this for reproducible/production renders.

## Treating an image

Apply a `filter` token (e.g. duotone) so a photo matches the palette without external editing:

```kdl
token id="filter.duo" type="filter" { duotone shadow="#1e1b4b" highlight="#fbbf24" }
image id="hero.photo" asset="asset.hero" x=(px)0 y=(px)0 w=(px)1080 h=(px)600 fit="cover" filter=(token)"filter.duo"
```

Clip an image to a window with a `frame` (`references/layout.md`); add depth with a `shadow`
token (`references/recipes.md`).

## The hybrid workflow

1. Generate only the pixels that genuinely need a model (the product/person/scene).
2. Save it into the project (e.g. `assets/`), compute its `sha256`, declare it as an `asset`.
3. Build the rest — background, headline, CTA, logo — as **native** tokenized nodes around it.
4. Keep text out of the image: real, addressable `text` nodes, never baked into pixels, so copy
   and language variants are one edit.
5. Record generation provenance (model, prompt, seed, license/allowed-reuse) in a `note` so the
   asset's origin is auditable — the engine does not track this for you yet.

This keeps the product image swappable, the text editable, the background recolorable, and the
export deterministic.
