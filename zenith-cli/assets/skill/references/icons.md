# Icons

`@zenith/icons-lucide` embeds the Lucide set (~1745 icons). An icon materializes as editable native `path` nodes inside an `instance`. Attributes: `zenith schema node instance`.

## Find the icon

- Search, never guess: `zenith library search <term>` (`--category`, `--kind`, `--limit`).
- Names differ from other sets: `sync` is `refresh-cw`, `home` is `house`.
- Ranking: name match beats alias match beats tag match. The top hit is usually right.
- Every query term must match. An empty two-word result means one word is wrong.
- Detail and path count: `zenith library show @zenith/icons-lucide#<name>`.
- No sensible hit after two tries means the concept is abstract. Use text.

## When to use an icon

Use an icon for a concrete thing: device, cloud, server, database, file, lock, user, action.

Skip the icon when:

- The concept is abstract ("synergy", "Q3"). An arbitrary icon asserts a false meaning.
- The item is a paragraph. Icons pair with short phrases.
- The region already holds about 7 varied icons. More become texture.

## Lists

Give each row naming a distinct thing its own icon. Use a plain bullet instead when:

- The same icon repeats down every row.
- Items run longer than one line.
- Order matters. Use `format="markdown"` with `1.`.
- No one-word icon fits the item.

Mixed lists are fine. Keep the label edges aligned (a column frame does this).

## After `library add`

`library add` places the instance on a page at `--at X,Y`. Then:

1. **Move** — add `--parent <frame-or-group-id>` to place it into a container directly. A layout frame puts it in flow and ignores `--at`. Or `reparent` it later, or keep it absolute.
2. **Size** — `set_geometry` with `w`/`h` (raw px). Without them it draws at 24 px.
3. **Recolor** — before the first render on dark themes (below).

`rotate` does not apply to instances.

## Icon rows

- Put the icon and its label in a row frame with `align="center"` and a `gap`.
- Size the icon near the label's line height (e.g. 20 px for 16 px text).
- `fit` defaults to `contain`.
- A list of icon rows is a column frame of those row frames.

## Recolor

Lucide strokes default to `lib.icons.stroke` ≈ `#111827`. That is invisible on `pine`, `ember`, `harbor`, and `sunset`.

| Path | When | How |
| --- | --- | --- |
| Shared token | One ink for every icon | `update_token_value` on `lib.icons.stroke` |
| Per path | Multi-color or per-icon ink | `override` every path `icon.0` … `icon.N-1` with a color token |

## Gotchas

- **Every path** — `N` varies (`zap` 1, `lock-keyhole` 3, some 10+). A partial override validates clean and looks broken.
- **Stroke scales with the box** — a 2 px stroke becomes 8 px at 96 px. Override `stroke-width` per path.
- **Tokens vs px** — stroke and fill take tokens. Instance `w`/`h` take raw px.

## Your own icon set

- A directory of `*.svg` under `<project>/libraries/<name>/` is a pack. Each file is one icon.
- Address an icon as `@local/<name>#<stem>`.
- An optional `library.kdl` declares `id`, `version`, `license`, and per-icon `aliases`, `tags`, `categories`.
- `zenith library list <project>` and `zenith library search <term> <project>` include it.

## Look

Validate cannot see a 2 px misalignment, an unrecolored path, or a misleading icon. Open the PNG.
