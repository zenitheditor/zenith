# Pattern node — procedural tiling

> Attribute names, types, and required/optional status: `zenith schema node pattern`. This
> reference covers semantic gotchas, layout rules, and the `detach_pattern` workflow.

The `pattern` node tiles **one motif** (a single child node template) across a bounds box,
producing a repeating layout without manual node authoring. It is deterministic: same inputs →
identical bytes out on any machine. Two kinds are supported: `grid` (row-major, evenly-spaced
cells with optional jitter) and `scatter` (pseudo-random positions driven by a seed).

> **Background panel.** The pattern's own `fill` (solid or gradient), `radius` (uniform rounded
> corners), and `stroke` + `stroke-width` paint a **background panel** behind the motif instances,
> sized to the bounds box. The remaining visual properties (`shadow`, `blur`, `mask`, per-corner
> radii, `blend-mode`) are accepted and validated for token usage but are **inert** for now.

---

## Attributes

Run `zenith schema node pattern` for the full attribute list, types, and required/optional status.
Key semantic rules the schema does not convey:

- **`spacing` must be a `(px)N` literal** — not a `(token)"…"` reference. A token value is
  silently ignored and fires `pattern.grid_missing_spacing`. Tune density by adjusting the
  literal value directly.
- **`jitter` is `0.0..=1.0`** — a fraction of `spacing` per axis (x and y are uncorrelated,
  seed-derived). Out-of-range is a Warning and is clamped; the output still renders.
- **Background panel** — the pattern's own `fill`, `radius`, `stroke`, and `stroke-width` paint a
  background rectangle sized to the bounds box, *behind* all motif instances. The other visual
  props (`shadow`, `blur`, `mask`, per-corner radii, `blend-mode`) are accepted and token-validated
  but are **inert** for now.

---

## The motif — a template, not a real node

The single child of a `pattern` is a **template**. It defines the shape, geometry, and styling
of each instance; it is **not** collected in the document's id registry and cannot be addressed
directly.

- Each rendered instance is a **clone** with a synthetic id `<pattern-id>.<index>` (0-based):
  `bg.dots.0`, `bg.dots.1`, …
- The motif keeps its authored `x`/`y`; each clone is **translated by the instance offset**
  (the offset is added to the motif's authored x/y, not replaced).
- The motif can be any node kind that carries geometry.
- Token references **inside the motif** are collected for token-usage validation, so a token used
  only by the motif does **not** trip `token.unused`.

---

## Layout

### Grid (`kind="grid"`)

Instances are placed row-major at `col * spacing, row * spacing` for every cell whose origin
falls inside the `w × h` bounds. The bounds box clips any instance that overflows.

With `jitter > 0` each instance is displaced by `±jitter * spacing` per axis independently,
using the seed to derive per-instance offsets (x and y are uncorrelated).

### Scatter (`kind="scatter"`)

Exactly `count` instances are placed at seed-derived pseudo-random positions inside the bounds.
`jitter` is ignored.

### Determinism

Both layout functions are fully deterministic. Changing `seed` produces a different-but-stable
arrangement. Default `seed=0` is a valid, reproducible layout. Same document source → same
pixel output on every machine.

---

## Diagnostics

| Code                              | Severity | Trigger                                                       |
| --------------------------------- | -------- | ------------------------------------------------------------- |
| `pattern.unknown_kind`            | Error    | `kind` is not `"grid"` or `"scatter"`.                        |
| `pattern.invalid_spacing`         | Error    | `spacing` resolves to ≤0.                                     |
| `pattern.grid_missing_spacing`    | Error    | `kind="grid"` and no `spacing` attribute.                     |
| `pattern.invalid_count`           | Error    | `count` resolves to ≤0.                                       |
| `pattern.scatter_missing_count`   | Error    | `kind="scatter"` and no `count` attribute.                    |
| `pattern.jitter_out_of_range`     | Warning  | `jitter` is outside `0.0..=1.0` (clamped; output still produced). |

---

## Examples

### Grid — evenly-tiled dots with jitter

```kdl
tokens format="zenith-token-v1" {
  token id="color.dot" type="color" value="#22d3ee55"
}
pattern id="bg.dots" kind="grid" x=(px)0 y=(px)0 w=(px)1080 h=(px)1080 spacing=(px)48 jitter=0.15 seed=7 {
  ellipse id="dot" x=(px)0 y=(px)0 w=(px)8 h=(px)8 fill=(token)"color.dot"
}
```

Tune density by adjusting `spacing` (a `(px)` literal); tune noise by adjusting `jitter`. The
bounds box clips any partially-overlapping instances at the edges.

### Scatter — random confetti

```kdl
tokens format="zenith-token-v1" {
  token id="color.accent" type="color" value="#f59e0b99"
}
pattern id="bg.scatter" kind="scatter" x=(px)0 y=(px)0 w=(px)1080 h=(px)1080 count=25 seed=42 {
  rect id="star" x=(px)0 y=(px)0 w=(px)12 h=(px)12 fill=(token)"color.accent" rotate=(deg)45
}
```

Change `seed` to rotate the arrangement; change `count` to increase/decrease density.

---

## The `detach_pattern` transaction op

`detach_pattern` **materializes** a pattern into an editable native `group`. The pattern is
replaced in place by a group carrying the pattern's `id`, name, role, and bounds, whose children
are the cloned instances (`<id>.0`, `<id>.1`, …), each with its `(x, y)` set to the instance
offset and all other motif props preserved. The detached group renders **byte-identically** to
the live pattern — the same position function drives both.

### JSON op

A transaction file wraps its ops in an `ops` array, so `detach.json` is:

```json
{ "ops": [ { "op": "detach_pattern", "node": "<pattern-id>" } ] }
```

### Workflow

```bash
zenith tx doc.zen detach.json             # dry-run: preview the diff
zenith tx doc.zen detach.json --apply     # materialize the pattern to a group
zenith validate doc.zen --json            # confirm no hard diagnostics
```

After detaching, each instance node is individually editable (`set_fill`, `set_geometry`,
`remove_node`, etc.). The `recipes` provenance block is the right place to record what seed and
params drove the original pattern if you want to reproduce it later.

### Error codes

| Code                              | Trigger                                                     |
| --------------------------------- | ----------------------------------------------------------- |
| `tx.unknown_node`                 | `node` id does not exist in the document.                   |
| `tx.not_a_pattern`                | `node` resolves to a node that is not a `pattern`.          |
| `tx.pattern_unresolved_bounds`    | `w` or `h` is missing or resolves to ≤0 px.                |
| `tx.pattern_not_expandable`       | Layout yields no instances (unknown `kind`, or the required `spacing`/`count` is missing). |
| locked-node rejection             | Pattern or motif is locked — unlock before detaching.       |
