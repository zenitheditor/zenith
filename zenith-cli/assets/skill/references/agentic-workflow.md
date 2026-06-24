# Agentic visual workflow

How an agent takes a vague brief to a finished, auditable design without polluting the final
file. Most of this loop is now **first-class** in the engine: page workflow metadata
(`workspace-role`/`candidate-status`/…), the `promote_candidate`/`finalize_run` tx ops, and the
`agent-runs` + `previews` provenance blocks. Prefer those over ad-hoc conventions; the few
remaining conventions are called out as such.

> Exact transaction op names + fields: `zenith schema op <name>` and `zenith tx --help`
> (+ `examples/*.tx.json`). Authorable attributes: `zenith schema node <kind>` / `schema page`.
> Verify before relying on a specific op or field.

## 1. Capture the brief and plan (traceability)

Record intent _in the document_ with the first-class `agent-runs` provenance block, so the result
traces back to the brief and every step is auditable (`zenith inspect` surfaces it):

```kdl
agent-runs {
  run id="run.hero" brief="Launch hero: dark, energetic, product-forward" {
    step id="s1" action="generate-bg" action-version="1" {
      param name="palette" value="brand"
      param name="seed" value="7"
    }
    // steps carry action + params; the engine also attaches affected node ids + diagnostics.
  }
}
```

- Name the layer groups you will create and give each a **`semantic-role`** (plus optional
  `layer-priority` / `intensity`) so the plan and the final source line up and layers stay
  addressable — e.g. `group id="bg.grunge" semantic-role="background" layer-priority=0`.
  (`zenith schema node group` lists these.)
- List measurable acceptance criteria (e.g. "title contrast ≥ Lc 60 (APCA)", "product safe area
  kept clear"); check them with `zenith validate` and by inspecting the render.

## 2. Scratch experiments (don't pollute the final)

`workspace-role` is the **first-class scratchpad marker** on a page — set `workspace-role="scratch"`
on every experiment page (the engine validates + carries it; `finalize_run` later acts on it). A
naming convention reinforces it but is no longer the mechanism:

- Final pages: `page.<name>` (e.g. `page.hero`).
- Experiments: `workspace-role="scratch"` (optionally named `page.scratch.<topic>.<NN>`).
- Give each experiment a `candidate-status` and a `cleanup-policy` up front (step 3) so the run can
  be finalized automatically (step 6) — don't hand-delete losers.
- Nothing in a scratch page reaches the deliverable unless you deliberately promote it (step 5).
  (`zenith schema page` lists `workspace-role`, `candidate-status`, `cleanup-policy`, …)

## 3. Generate multiple candidates from one plan

Explore directions instead of committing to the first idea:

- Create several candidate pages (`page.scratch.bg.01/02/03`), each a different take on the
  _same_ plan and palette tokens.
- Track each candidate's lifecycle on the page itself: **`candidate-status`** (`"draft"` →
  `"selected"`/`"rejected"`; other values warn via `page.invalid_candidate_status`), point the
  winner at its destination with **`promotion-target`** (the final page id/label), and record the
  variant intent, seed, or cleanup intent in **`notes`** / **`cleanup-policy`** so the choice is
  replayable. (`zenith schema page` lists these fields.)
- Keep all candidates referencing the **same tokens** so a later palette change is one edit.

## 4. Render-preview and self-critique

Inspect output before trusting it:

```bash
zenith validate doc.zen --json                 # hard diagnostics must be empty
zenith render doc.zen --all-pages preview/      # contact sheet: one PNG per page
```

- Validation already catches many issues: text fit/overflow, contrast, off-canvas nodes,
  missing assets, token problems. Treat every **Error** as blocking.
- Then _look at_ the PNGs: is the headline legible over the motif? Is the product safe area
  clear? Is the texture too noisy? Revise the offending nodes (by id) and re-render.
- Do not finalize while hard diagnostics remain.
- For an audit trail, record each preview in the document-level `previews` block — a `preview`
  entry per candidate captures its `candidate` page id, the `source-hash` it was rendered from, the
  `output` path + `output-hash`, and the `parent-revision`. It is pure provenance (never affects
  render); `zenith inspect` surfaces it.

## 5. Promote the chosen candidate into the final page

Mark the winner with `candidate-status="selected"` (and `promotion-target` = the final page id),
then use the **`promote_candidate`** tx op — it deep-copies the selected candidate page's content
into the target export page, appending an `id_suffix` so ids stay unique:

```bash
# promote.json: {"ops":[{"op":"promote_candidate","source_page":"page.scratch.hero.02","target_page":"page.hero","id_suffix":".final"}]}
zenith tx doc.zen promote.json --apply
```

- The source must have `candidate-status="selected"`; the target page's content is replaced.
- See `zenith schema op promote_candidate` for the exact fields.
- `validate` and `render` again after promotion.

## 6. Finalize and clean up

Use the **`finalize_run`** tx op: for each page in the run whose `candidate-status="rejected"`, it
applies that page's `cleanup-policy` — `"delete"` removes the page; `"archive"` (or absent policy)
sets its `workspace-role` to `"archived"`:

```bash
# finalize.json: {"ops":[{"op":"finalize_run","run_pages":["page.scratch.hero.01","page.scratch.hero.02"]}]}
zenith tx doc.zen finalize.json --apply
```

- See `zenith schema op finalize_run` for the fields.
- Then check `zenith tokens <file>` / validation for now-unused-token advisories.
- Final source must `validate` with no hard diagnostics and `render` cleanly.

## 7. Durable history and undo

Zenith has real local history — use it instead of ad-hoc backups:

```bash
zenith history doc.zen          # list versions
zenith version doc.zen "v1-pre-promote"   # name a checkpoint
zenith undo doc.zen / zenith redo doc.zen
zenith restore doc.zen <rev>    # restore a past version
zenith sync doc.zen             # capture an external/hand edit into history
```

Name a checkpoint before risky steps (e.g. before promotion) so you can restore precisely.

## 8. Later semantic edits

Because you used stable ids, tokens, and semantic groups, later edits are precise transactions:

- "Reduce the grunge" → `set_opacity` on `bg.grunge`.
- "Stronger neuron glow" → update the shadow/token the glow references.
- "Remove honeycomb near the headline" → delete/clip only the intended nodes in `bg.honeycomb`.

If instead the background were a flattened image or anonymous nodes, none of this is possible —
which is why steps 1–3 insist on ids, tokens, and groups.

## Known gaps (do not pretend these exist)

Most of the loop is first-class now — prefer these over ad-hoc conventions: page workflow metadata
(`workspace-role`, `candidate-status`, `promotion-target`, `notes`, `cleanup-policy`); group
`semantic-role`/`layer-priority`/`intensity` + `protected-region`/`editable-param` children; the
`promote_candidate`/`finalize_run` tx ops (steps 5–6); and the document-level `agent-runs` and
`previews` provenance blocks. Still **not** implemented; do not generate source that assumes them:
brush/stamp definitions and a built-in automated critique report (you self-critique by reading the
render — step 4). Use today's primitives until the engine ships these.
