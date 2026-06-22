# Brand, identity & per-project style

Two layers work together: a **human/agent-facing brief** (`.zenith/brand.md`) that tells the
agent how to design on-brand, and a **machine brand kit** (a `.zen` library pack of tokens +
re-skin actions) the CLI can apply.

## 1. The project brief — `.zenith/brand.md`

A markdown file in (or above) the project that the skill loads before authoring. It captures
what generic defaults can't: palette roles, type system, spacing rhythm, voice, and do/don't.
Scaffold it from `templates/brand.md`. **When it exists, conform to it and prefer its tokens
over inventing new ones.**

Keep it concrete and token-oriented, e.g.:

```markdown
# Acme brand

- Palette (roles): color.brand=#e11d48, color.accent=#3b82f6, color.ink=#0f172a, color.bg=#ffffff
- Type: headings Noto Sans bold; body Noto Sans; scale h1=64 h2=40 body=28 (px)
- Spacing: multiples of 8px; page safe margin 64px
- Voice: confident, plain, no exclamation marks
- Do: generous whitespace, one accent per view. Don't: gradients on text, more than 2 fonts.
```

## 2. The machine brand kit — a `.zen` library pack

A pack declares its identity, the brand tokens, and optional `actions` (typed `tx` bundles)
that re-skin a document's tokens in one step. Pattern (from the bundled `@zenith/brand-kit`):

```kdl
zenith version=1 {
  project id="@acme/brand" name="Acme Brand"
  libraries { library id="@acme/brand" version="1.0.0" }
  actions {
    action id="apply-acme" label="Apply Acme palette" version="1.0.0" {
      tx "{\"ops\":[{\"op\":\"update_token_value\",\"id\":\"color.brand\",\"value\":\"#e11d48\"},{\"op\":\"update_token_value\",\"id\":\"color.accent\",\"value\":\"#3b82f6\"}]}"
    }
  }
  styles {}
  document id="pack.preview" title="Acme Brand" { page id="pack.pg" w=(px)100 h=(px)100 {} }
}
```

Put project packs under `libraries/*.zen` next to your documents.

## 3. Discover and apply

```bash
zenith library list <project-dir>                 # project packs + embedded presets
zenith library add @acme/brand#<item> --into doc.zen --page page.hero   # materialize an item
```

- A document whose colors/fonts/sizes are all tokens can be re-skinned by applying a brand
  action (a `tx` that does `update_token_value` on `color.brand`, etc.) — preview with
  `zenith tx doc.zen brand.tx.json`, then `--apply`. No geometry changes; the palette swaps
  consistently everywhere (see `references/recipes.md` → "re-skin").
- This is why the engine insists on tokens: brand application is a token-value diff, not a
  redraw.

## 4. Workflow for "set up / apply our brand"

1. If `.zenith/brand.md` is missing, scaffold it from `templates/brand.md` and fill it from the
   user's brand (palette, fonts, spacing, voice).
2. Create/locate the `libraries/<brand>.zen` pack with the brand tokens (+ an apply action).
3. New documents: start from the brand tokens (or `templates/`), keep every fill/size a token.
4. Existing documents: apply the brand action via `tx` (dry-run, then `--apply`), `validate`,
   `render`, and confirm it reads on-brand.

Verify op names/flags with `zenith tx --help` and `zenith library --help`; don't assume an op
exists without checking.
