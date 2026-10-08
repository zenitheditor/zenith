# Agentic workflow — candidates, history, MCP

Takes a vague brief to a finished, auditable design. The `.zen` holds only final content.

Process state (candidates, lifecycle, history) lives in the workspace store, keyed by `doc-id`. Never hand-edit it. Flags: `zenith workspace --help`.

## 1. Plan for addressability

- Start with `zenith new <doc> --theme <name>`. Identity and the store attach on the first edit.
- Give every node a stable `id`. Each edit is then one precise `tx` op.
- Give each layer group a `semantic-role`, plus optional `layer-priority` / `intensity`.
- Write acceptance criteria (e.g. title contrast `Lc` ≥ 60). Check them with `validate` and the render.
- Keep the brief off the deliverable page. A short `role="guide"` note is fine.

## 2. Generate candidates

Edit the document to one take. Then snapshot it with `zenith workspace scratch new <doc>`.

- `--page <id>` limits the snapshot to one page (default `*`).
- `--status`, `--notes`, `--workspace-role`, `--cleanup-policy`, `--promotion-target` record intent.
- Keep all takes on the same tokens. A palette change is then one edit.

## 3. Check each take

1. `zenith validate <doc> --json`. With no Errors this also reports overflow, contrast, and lint.
2. `zenith fix <doc> --apply` for machine-fixable diagnostics.
3. `zenith render <doc> --contact-sheet <png> --scale 0.5 --json`. Open the PNG.
4. Revise nodes by id with `tx`. Re-snapshot.

Critique method: `references/design-critique.md`.

## 4. Select and promote

| Step | Command |
| --- | --- |
| List | `zenith workspace scratch list <doc>` |
| Detail | `zenith workspace scratch show <doc> <cand>` |
| Set status | `zenith workspace candidate <doc> <cand> selected` (or `rejected`) |
| Promote | `zenith workspace promote <doc> <cand> --into <page-id>` |
| Clean up | `zenith workspace finalize <doc>` |

- Only a `selected` candidate promotes.
- Promote deep-copies the candidate page into the target page. Ids get a suffix (default `.promoted`, or `--id-suffix`).
- Promote validates, writes in place, and records a version.
- `finalize` removes candidates that are `rejected` with cleanup policy `delete`.
- After promote, run `validate` and `render` on the deliverable.

## 5. History and portability

| Need | Command |
| --- | --- |
| Versions | `zenith history <doc>` |
| Checkpoint | `zenith version <doc> "<name>"` |
| Step back / forward | `zenith undo <doc>` · `zenith redo <doc>` |
| Restore | `zenith restore <doc> <rev>` |
| Capture a hand edit | `zenith sync <doc>` |
| Move the store | `zenith workspace bundle <doc> --out <file>` · `zenith workspace unbundle <file>` |

Name a checkpoint before risky steps such as promotion.

## 6. MCP

Prefer the CLI when it can run. Use `zenith mcp` for remote, CI, sandboxed, or hosted agents.

- Every tool takes `doc` as a path or the 26-char `doc-id`.
- Large and binary results come back as resource links. Read them with `resources/read`.
- Transport is stdio. `zenith mcp --http <ADDR>` serves Streamable-HTTP (needs the `http` build feature).
- History commands (`history`, `undo`, `redo`, `version`, `restore`, `sync`) are CLI-only.

| CLI | MCP tool |
| --- | --- |
| `schema` | `zenith_schema` |
| `tx` | `zenith_tx` |
| `validate` | `zenith_validate` |
| `fix` | `zenith_fix` (`apply`) |
| `render` | `zenith_render` (`format`, `page`, `raster_scale`, `out`, `diagnostics`, PNG `scale`/`contact_sheet`) |
| `inspect` / `tokens` / `fmt` / `fonts` | `zenith_inspect` / `zenith_tokens` / `zenith_fmt` / `zenith_fonts` |
| `workspace scratch` / `candidate` / `promote` | `zenith_workspace_scratch` / `zenith_workspace_candidate` / `zenith_workspace_promote` |
| `workspace finalize` / `bundle` / `unbundle` | `zenith_workspace_finalize` |
| `merge` / `theme new` | `zenith_merge` / `zenith_theme_new` |

Render parameters and fallback contracts: `export.md`. PDF defaults to every page. SVG defaults to page 1. Fallback diagnostics remain visible with `diagnostics=false`. MCP merge exports PNG by default or SVG with `format: svg`. SVG accepts `raster_scale`. Batch policy and partial output contracts match CLI merge. Structured reports retain committed paths after row or manifest errors. There is no MCP variant tool.

## Not implemented

- Brush and stamp definitions.
- An automated critique report. Critique by reading the render.
- A command that writes agent-run or preview logs.
