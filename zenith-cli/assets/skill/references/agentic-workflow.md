# Agentic visual workflow

Take a vague brief to a finished, auditable design without polluting the final file.

The `.zen` holds ONLY final content. Process state — scratch candidates, their lifecycle, history
— lives in the app-managed store keyed by the document's `doc-id`, reachable through the
`zenith workspace` commands. You never hand-edit it.

> Op fields: `zenith schema op <name>`, `zenith tx --help`.
> Attributes: `zenith schema node <kind>` / `zenith schema page`.
> Store commands: `zenith workspace --help`.

## 1. Plan for addressability

Start a fresh document with `zenith new doc.zen --name "…"` (a minimal valid scaffold with a
`doc-id`), or open any existing `.zen` — identity and the workspace store attach transparently on
the first edit, so `zenith workspace scratch new` works immediately with no prior `tx`. There is no
in-file brief block; keep the plan in the work itself.

- Give every node a stable `id` so each edit is a precise transaction.
- Give each layer group a `semantic-role` (+ optional `layer-priority`/`intensity`) so layers stay
  addressable: `group id="bg.grunge" semantic-role="background" layer-priority=0`.
- Write down acceptance criteria (e.g. "title contrast ≥ Lc 60 APCA") and check them with
  `zenith validate` and the render — these are your gate, not decoration.

## 2. Generate candidates from one plan

Edit the document to a take, then snapshot it into the store as a scratch candidate. Each
candidate is a content-addressed `.zen` snapshot — it never lives in the deliverable.

```bash
zenith workspace scratch new doc.zen --page page.hero --status draft \
  --notes "take A: dark, product-forward" --workspace-role scratch --cleanup-policy delete
```

- `--page <id>` is the page this candidate captures (default `*` = whole document).
- Repeat for each take. `--promotion-target` records where it is meant to land.
- Keep all takes on the same tokens so a palette change is one edit.

## 3. Render-preview and self-critique

```bash
zenith validate doc.zen --json              # hard diagnostics must be empty
zenith render doc.zen --all-pages preview/  # one PNG per page
```

- Treat every Error as blocking.
- Look at the PNGs: headline legible over the motif? product safe area clear? texture too noisy?
  Revise nodes by id, re-snapshot.

## 4. Review and set lifecycle

```bash
zenith workspace scratch list doc.zen            # enumerate candidates (cand0, cand1, …)
zenith workspace scratch show doc.zen cand0      # detail for one (add --json)
zenith workspace candidate doc.zen cand0 selected   # or: rejected
```

Lifecycle is `draft → selected | rejected`. Only a `selected` candidate can be promoted.

## 5. Promote the chosen candidate

```bash
zenith workspace promote doc.zen cand0 --into page.export
# keep cloned ids unique with a custom suffix:
zenith workspace promote doc.zen cand0 --into page.export --id-suffix .v2
```

Fetches the candidate's stored snapshot, deep-copies its source page into the target page
(suffixing all ids, default `.promoted`), validates, writes the document back in place, and records
the promote in version history. Then `validate` + `render` the deliverable.

## 6. Finalize and clean up

```bash
zenith workspace finalize doc.zen            # add --json for a machine-readable report
```

Removes candidates with `status = rejected` and `cleanup-policy = delete` from the scratch index
(snapshot objects are left for a future GC pass); all other candidates are preserved. Then check
`zenith tokens doc.zen` for unused-token advisories; final source must validate + render clean.

## 7. History and portability

```bash
zenith history doc.zen                      # list versions
zenith version doc.zen "v1-pre-promote"     # name a checkpoint
zenith undo doc.zen  /  zenith redo doc.zen
zenith restore doc.zen <rev>                # <rev> grammar: zenith restore --help
zenith sync doc.zen                         # capture an external/hand edit
zenith workspace bundle doc.zen --out doc.zenithbundle   # pack the whole store (history + scratch)
zenith workspace unbundle doc.zenithbundle               # restore it on another machine/clone
```

Name a checkpoint before risky steps (e.g. promotion). The `doc-id` in the `.zen` is the key that
reattaches a bundle to its file.

## 8. Later semantic edits

Stable ids + tokens + `semantic-role` groups make edits precise transactions:

- "Reduce the grunge" → `set_opacity` on `bg.grunge`.
- "Stronger glow" → update the shadow token it references.
- "Remove honeycomb near the headline" → delete/clip nodes in `bg.honeycomb`.

## Running this loop over MCP (when the CLI isn't available)

Prefer the `zenith` CLI whenever your environment can run it — it is the primary, fastest surface.
When a local binary is not suitable (remote, CI, sandboxed, hosted agents), the same loop runs over
the **`zenith mcp`** server, which exposes the command surface as MCP tools. It is a first-class
surface, not a thin wrapper: results are trimmed structured JSON, schema detail is fetched on demand,
and large/binary artifacts come back as **resource links** (read them with `resources/read`).

Every tool takes a `doc` argument that is either a path **or** the 26-char `doc-id` — so after the
first call (which attaches identity) you can address the document by id and stop passing paths.

| CLI step                                            | MCP tool                                                                 |
| --------------------------------------------------- | ------------------------------------------------------------------------ |
| `zenith schema node/op …` (learn syntax on demand)  | `zenith_schema` `{surface, name}`                                        |
| `zenith tx` (typed edit, dry-run by default)        | `zenith_tx` `{doc, transaction, apply?, diff?}`                          |
| `zenith validate`                                   | `zenith_validate` `{doc, severity?}` → `{valid, error_count, …}`         |
| `zenith render`                                     | `zenith_render` `{doc, format}` → a resource link (never raw bytes)      |
| `zenith inspect` / `tokens` / `fmt`                 | `zenith_inspect` / `zenith_tokens` / `zenith_fmt`                        |
| `zenith workspace scratch new/list/show`            | `zenith_workspace_scratch` `{doc, op:"new"\|"list"\|"show", …}`          |
| `zenith workspace candidate`                        | `zenith_workspace_candidate` `{doc, candidate_id, status}`               |
| `zenith workspace promote`                          | `zenith_workspace_promote` `{doc, candidate_id, target_page}`            |
| `zenith workspace finalize` / `bundle` / `unbundle` | `zenith_workspace_finalize` `{doc, op:"finalize"\|"bundle"\|"unbundle"}` |
| `zenith merge` / `theme new`                        | `zenith_merge` / `zenith_theme_new`                                      |

Transport: stdio by default; `zenith mcp --http <ADDR>` serves native Streamable-HTTP (binary built
with the `http` feature). History navigation (`history`/`undo`/`redo`/`version`/`restore`/`sync`) is
CLI-only for now — use the CLI when you need it.

## Not implemented (don't assume these)

Brush/stamp definitions; an automated critique report (self-critique by reading the render, step 3);
recording an agent-run/preview log via the CLI (the store has the schema, but no command writes to
it yet).
