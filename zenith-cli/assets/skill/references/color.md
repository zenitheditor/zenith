# Color & tokens

Color is never a raw literal on a node — it is a `token` referenced with `(token)"id"`, so a
palette/brand change is one token edit. **This is enforced**: putting raw hex directly on a
node's visual property (`fill`, `stroke`, `background`, …) fails validation with
`token.raw_visual_literal`. Hex lives only in a token's `value`; nodes reference the token.

## Color tokens

```kdl
tokens format="zenith-token-v1" {
  token id="color.bg"     type="color" value="#0b1020"     # sRGB hex
  token id="color.ink"    type="color" value="#111827"
  token id="color.scrim"  type="color" value="#0f172a55"   # 8-digit hex = #rrggbbaa (alpha)
}
# use it:
rect id="bg" x=(px)0 y=(px)0 w=(px)1080 h=(px)1080 fill=(token)"color.bg"
```

- **Alpha**: append two hex digits (`55` ≈ 33%, `aa` ≈ 67%, `00` = transparent). Confirmed in
  `examples/shadow.zen`. Use this for translucent scrims, glass panels, and soft motifs.
- **Print / CMYK**: Zenith supports print-oriented color and PDF CMYK output. The exact token
  syntax for native CMYK is not shown in the basic examples — before using it, check
  `zenith tokens <file>`, the repo `examples/`, and `zenith render --help` (PDF flags). Don't
  invent CMYK syntax.

## Gradients

```kdl
token id="grad.sky" type="gradient" angle=(deg)90 {        # linear, angle in degrees
  stop offset=0.0 color=(token)"color.sky.top"
  stop offset=1.0 color=(token)"color.sky.bottom"
}
token id="grad.sun" type="gradient" radial=#true center-x=0.5 center-y=0.45 radius=0.55 {
  stop offset=0.0 color="#fde68a"
  stop offset=1.0 color="#f9731600"
}
```

- Stops use `offset` 0–1 and a `color` (token or hex; hex may carry alpha).
- A gradient token fills the same slots a color token does: `page background=(token)"grad.sky"`,
  `rect fill=(token)"grad.sun"`, `ellipse fill=…`. Confirmed in `examples/gradient.zen`.

## Discipline

- Name tokens semantically and by role: `color.brand`, `color.accent`, `color.ink`,
  `color.bg`, `color.scrim` — not `color.blue1`. Roles survive a rebrand; hues don't.
- Keep a small palette; reuse tokens across nodes so a swap is consistent everywhere.
- Validation flags unused tokens and (where it can) low-contrast text — run
  `zenith validate` and `zenith tokens <file>` to audit.
