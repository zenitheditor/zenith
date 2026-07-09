# Diagnostic policy

Controls which diagnostic codes are reported, suppressed, or elevated — without changing
rendered pixels. Integrity Errors (structural corruption, referential failures) are immutable;
policy cannot suppress them.

Discover all governable codes with:

```bash
zenith schema diagnostics
```

## Policy sources (highest precedence wins)

1. **CLI flags** — `--allow`, `--warn`, `--deny` on `validate` and `render` (repeatable)
2. **In-file block** — `diagnostics { … }` at the document root
3. **Local config** — `./.zenith.kdl` (walked up from the document directory)
4. **Global config** — `~/.config/zenith/config.kdl`

When the same code appears in more than one source, the highest-precedence source wins.

## In-file diagnostics block

Placed at the document root, alongside `tokens` and `recipes`. It ships inside the `.zen` so
it is visible, diffable, and auditable:

```kdl
diagnostics {
  allow "token.unused"
  deny  "font.local"
  warn  "layout.off_canvas"
}
```

- `allow <code>` — suppress an advisory or warning; it is not reported.
- `deny <code>` — elevate to a blocking Error (CI gate).
- `warn <code>` — force to Warning even if the engine would normally emit an advisory.

## Config files (KDL)

Same `diagnostics { … }` block in `.zenith.kdl` (project-local, walked up) or
`~/.config/zenith/config.kdl` (user-global). Use the local config for project defaults shared
across documents; use the global config for personal preferences:

```kdl
// .zenith.kdl
diagnostics {
  deny "font.local"
}
```

## CLI flags

```bash
zenith validate doc.zen --deny layout.off_canvas --allow token.unused
zenith render   doc.zen --png out.png --deny font.local
```

Flags are repeatable: `--deny font.local --deny layout.off_canvas`. Note `font.local` is raised
while rendering (font resolution happens at render time), so gate it on `render`, not `validate`.

## Local fonts and CI determinism

A `fontFamily` token that names a local/system font (not a Bundled family) resolves on the
current machine but emits a `font.local` advisory: rendering is not deterministic across
machines. For reproducible output:

- Use a Bundled family (`zenith fonts` lists them under "Bundled").
- Or declare the font as a project `font` asset (bundled with the document).
- Or guarantee the target OS has the font installed.

For CI, add `deny "font.local"` to `.zenith.kdl` (or pass `--deny font.local` to `render`)
so a local-font slip becomes a hard error at render time.

## Text legibility (contrast)

Zenith judges a text node against the colour actually painted **behind the glyphs** — the
topmost covering fill resolved by geometry and paint order, not the page background. The metric
is APCA `Lc` (WCAG 3 draft). Three governable codes, all reported by `zenith validate`:

- `contrast.invisible` (warning) — `|Lc| < 15`: the text is effectively the same colour as its
  backdrop. This is a real defect (dark monogram on a navy disc). **Do not ignore it**, and note
  it is a *warning*, so a clean `--json` `"valid": true` does not mean the text is legible.
- `contrast.low` (advisory) — sub-threshold but legible (`Lc` under 60, or 45 for large/bold
  text). Often intentional brand contrast; suppress with `allow "contrast.low"` when deliberate.
- `contrast.indeterminate_backdrop` (advisory) — the backdrop cannot be sampled at validate time
  (an `image`, a `path` fill, or a rotated / masked / blurred / non-normal-blended fill), or an
  anchored text node has no resolvable extent. The validator refuses to guess.

Resolve an indeterminate backdrop by telling the validator what the viewer sees:

```kdl
text id="badge.label" contrast-bg=(token)"color.brand.navy" fill=(token)"color.ink" { span "FS" }
```

`contrast-bg` takes precedence over the detected backdrop (`zenith schema node text`). For CI,
`deny "contrast.invisible"` turns invisible text into a hard failure.

## Policy only changes reporting

Adding `allow` or `deny` does not change the rendered output in any way. The engine compiles
and renders the document identically; policy controls only which diagnostics appear in the
report.
