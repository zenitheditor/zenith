# Agentic visual workflow

How an agent takes a vague brief to a finished, auditable design without polluting the final
file. This encodes the full loop using **today's** Zenith primitives. Where a step is a
_convention_ rather than a first-class engine feature, that is called out — follow the
convention; don't claim the engine enforces it.

> Exact transaction op names and flags: `zenith tx --help` and `examples/*.tx.json`
> (e.g. `examples/center.tx.json`). Verify before you rely on a specific op.

## 1. Capture the brief and plan (traceability)

Before generating anything, write the brief down _in the document_ so the result can be traced
back to intent:

- Put the goal, palette, mood, and layer plan in `note` / `role="guide"` content, or a sidecar
  `*.brief.md` next to the `.zen`. These do not render.
- Reference intended layer groups by the **stable ids** you will create (`bg.*`, `hero.*`,
  `cta.*`), so the plan and the final source line up.
- List measurable acceptance criteria (e.g. "title contrast ≥ Lc 60 (APCA)", "product safe area kept
  clear"). You will check these with `zenith validate` and by inspecting the render.

## 2. Scratch experiments (don't pollute the final)

Zenith has no first-class "scratch" page role yet, so use a **naming + page convention**:

- Final pages: `page.<name>` (e.g. `page.hero`).
- Experiments: `page.scratch.<topic>.<NN>` (e.g. `page.scratch.bg.01`).
- Keep experiments in a separate working copy or clearly-named pages, render them, and delete
  the losers before final export. Nothing in a `scratch.*` page should reach the deliverable
  unless you deliberately promote it (step 5).

## 3. Generate multiple candidates from one plan

Explore directions instead of committing to the first idea:

- Create several candidate pages (`page.scratch.bg.01/02/03`), each a different take on the
  _same_ plan and palette tokens.
- Because there is no candidate-set metadata yet, record the variant intent and any
  generation parameters/seed in a `note` on each page so the choice is replayable.
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

## 5. Promote the chosen candidate into the final page

Promotion is not yet a single primitive; compose it from structural ops:

- Copy the selected groups from the scratch page into the final page (`add_node` /
  reparent ops — see `zenith tx --help`), regenerating ids if they would collide.
- Ensure z-order: decorative background groups sit **below** the foreground product/title
  groups.
- Record what came from which candidate in a `note` (source candidate id + id mapping), since
  the engine does not track this for you.
- `validate` and `render` again after promotion.

## 6. Finalize and clean up

Produce a clean deliverable:

- Delete unpromoted `scratch.*` pages and any now-unused generated assets/tokens. (Check
  `zenith tokens <file>` and validation for unused-token advisories.)
- If an audit trail is wanted, keep the rejected candidates in a separate archived copy rather
  than in the deliverable.
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

These are on the roadmap but **not** implemented; do not generate source that assumes them:
first-class scratch/candidate roles, a `promote_candidate` op, a source-level recipe/seed
model, brush/stamp definitions, a built-in critique report, and structured run-log provenance.
Use the conventions above with today's primitives until the engine ships these.
