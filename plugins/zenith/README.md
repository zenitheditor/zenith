# Zenith plugin for Claude Code

Teaches the agent to author, validate, and render deterministic `.zen` design documents with
the `zenith` CLI — posters, decks, social graphics, flyers, books, diagrams — as editable,
version-controllable source instead of flat AI images.

## Install

```
/plugin marketplace add farhan-syah/zenith
/plugin install zenith
```

Requires the `zenith` CLI on `PATH`:

```
curl -fsSL https://raw.githubusercontent.com/farhan-syah/zenith/main/scripts/install.sh | sh
```

## What's inside

- **Skill `zenith`** — a router that encodes the agentic loop (author → validate → render →
  inspect → edit) and best practices (stable ids, tokenize everything, validate before
  finalize), and routes to on-demand reference packs:
  - `references/agentic-workflow.md` — scratch → candidates → promote → clean → provenance
  - `references/recipes.md` — gradients, aurora, glass, topographic, motifs, grain, shadows
  - `references/color.md` · `typography.md` · `layout.md` · `images.md` · `brand.md`
  - `references/themes.md` · `theme-from-brand.md` — ready-made themes + `zenith theme new`
  - `templates/` — starter `.zen`, project brand brief, brand-kit pack
- **Commands** — `/zenith-new`, `/zenith-check`, `/zenith-render`, `/zenith-brand`

The skill drives the `zenith` CLI directly (cross-platform); it ships no helper scripts. A
"contact sheet" is `zenith render <file> --all-pages <dir>`; scaffolding is a copy of a
`templates/` file + `zenith validate`.

## Project configuration

Drop a `.zenith/brand.md` (palette, type, spacing, voice) in your project and the skill
conforms to it; pair it with a `libraries/<brand>.zen` pack the CLI can apply. See
`skills/zenith/references/brand.md`.

## Portability

The skill is plain markdown and drives the CLI, so the same `skills/zenith/` works in other
skill-aware agents (Codex, OpenCode). MCP and an opt-in validate-on-edit hook are possible
future surfaces; this plugin is the agent-driven baseline.
