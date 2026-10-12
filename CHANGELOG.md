# Changelog

All notable changes to Zenith will be documented in this file.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Zenith uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

The 0.0.11 entry is the first entry. It covers everything up to and including
0.0.11. Tags `v0.0.1` through `v0.0.10` predate this file and are folded into
it rather than reconstructed, so nothing in that entry is stated as a delta
against an earlier version. Every later entry is a delta against the one below
it.

---

## [Unreleased]

### Added

- **Bundled fallback fonts for Arabic, Hebrew, Chinese, Japanese, Korean, and emoji.** The new `zenith-fonts-fallback` crate embeds Noto Sans Arabic, Noto Sans Hebrew, a Noto Sans CJK SC subset, and monochrome Noto Emoji, and the CLI tries them in that order for a glyph the requested face lacks. A document with no missing glyph renders the same bytes as before. The browser module does not embed them.
- **`zenith schema diagnostics <FILTER>` lists only the codes that match.** The match is case-insensitive on the code and summary, and it includes Error codes. The full list is 15 KB as text and 63 KB as JSON. In a merge benchmark, agents called the full list up to 14 times in one run because a tool-output cap cut it off before the codes they needed.

### Fixed

- **The bundled Noto font notices name the SIL Open Font License 1.1.** They said Apache-2.0, but each face's name table states OFL 1.1.

### Changed

- **`zenith merge` reports each row's warnings and applies the config policy, for PNG as well as SVG.** A `deny`ed code fails the row. A malformed config now fails the merge with exit code 2.

---

## [0.0.14] — 2026-10-12

### Added

- **`zenith schema write`, `zenith tx --help`, and the starter output list `duplicate_node` and `duplicate_page`.** The `schema write` rules also say to check many pages in one call with `zenith render --contact-sheet`. Before, agents retyped each copy of a repeated row or page and looked at every page on its own. In a 3-brief build run they used `duplicate_node` and `duplicate_page` where they used neither before.
- **`zenith tx --apply --png OUT` renders the edited document in the same call.** An edit and a look at it are one step instead of two. `--page N` picks the page (default 1). The page renders before anything is written, so a page that does not render (`tx.png_failed`) leaves the document and the PNG as they were. A rejected edit writes neither file. `--png` may not name the document or the tx file, and its directory must exist, as for `zenith render --png`. If the PNG cannot be written after the edit, the result says the edit is written. The result adds a `png` object (`path`, `width`, `height`, `page`, `pages`) or a `png_error` string to `--json`, always as one object.
- **`content.placeholder` warns about sample text left in a document.** A text or shape node with `role="placeholder"`, or a span that is exactly the fill-command text `<new text>`, still holds sample content. `zenith validate` and `zenith tx` give one document-level warning that names the first six node ids and a count. It has no subject, so `allow "content.placeholder"` silences it. The text ops of `tx` (`set_span_text`, `replace_text`, `find_replace_text`) clear the mark on any node whose text they change, even when only part of the text changed. Other ops and other roles keep it. `role` holds one value, so a node that needs another role cannot also carry the mark. Every text node of every `zenith new --starter` layout carries the mark. In the benchmark, two runs delivered a starter with its sample text in place (1 of 7 and 1 of 17 required strings).

---

## [0.0.13] — 2026-10-12

### Added

- **`zenith schema write` and the `zenith new --starter` output list the common `tx` ops.** The ops are runnable JSON, from the same text as `zenith tx --help`, and a test keeps them identical. In a 4-brief build run with the skill on, `schema op` calls fell from 30 to 10 and the cost per run from $0.078 to $0.044.

### Changed

- **The skill leads with a starter or `zenith schema write`.** It no longer sends an agent to read `authoring-workflow.md` first or to discover syntax one `schema node` call at a time.

---

## [0.0.12] — 2026-10-12

### Added

- **KDL parse errors name the faulty line.** An unclosed block used to point at its opening `{`, often far above the mistake. The error now adds the line before which a `}` is likely missing, or the first line that does not parse on its own (a stray `)` or `"`).
- **`zenith_schema` over MCP has every CLI surface.** It adds `write`, `ports`, `brand`, `block`, `style`, and `defaults`.
- **`zenith new --starter` titles the document like `zenith new`.** Without `--name` the title is `Untitled` and the ids come from the file name, not the starter's sample person or event. `--landscape` makes the wider side the width, so `deck` and `card` stay wide. A starter's own tokens and styles replace a theme pack's with the same id. Printed commands quote a path with spaces.
- **`zenith schema write`** — one page with a working document, the layout rules, and the common attributes of `page`, `frame`, `text`, `rect`, `ellipse`, `line`, and `image`. Types and defaults come from the live schema, and a test validates and renders the example. `--help`, bare `zenith schema`, and the `zenith new --starter` output point to it. In a 4-brief build run it cut `schema node` calls from 27 to 3 and the first write from turn 8–12 to turn 6–9.
- **`zenith new --starter` prints a ready `tx --ops` command** with every single-span text slot id, and `zenith --help` lists the starter first.
- **`zenith perceive page`** — renders pages as `zenith render --png` does, then measures deterministic signals: luminance histogram, value zones, density map, edge map, ink bounds, optical balance, clearspace, and grid conformance. These are signals, not a judgement of design quality.
  - `--page N` or `--all-pages`. `--json` prints a `zenith-perceive-page-v1` envelope with every number rounded to 4 decimal places and no tool version.
  - `clearspace` reads the page margins and `grid_conformance` reads the construction guides. A metric reports `not_applicable` when the page declares no such input.
  - `zenith-cli/tests/perceive_hashes.json` pins the output of every example on every CI target.
- **`zenith_perceive` MCP tool** — the same envelope over MCP, read-only and confined to the server root.
- **Per-child `margin` in auto layout** — `margin`, `margin-x`, `margin-y`, `margin-top`, `margin-right`, `margin-bottom`, and `margin-left` on a child of a `row` or `column` frame. Margins add outer space around that child and never collapse, so one child moves without changing the frame's `gap`.
  - Precedence is side, then axis, then `margin`. A negative value is `node.invalid_value`.
  - Cross axis: `align` start, center, and end place the margin box. `align=stretch` shrinks the stretched size by the cross margins. A hugging frame includes its children's margins.
  - Margin on a grid child, an absolute child, or a child of an absolute frame has no effect and reports `layout.inert_attribute`.
  - `tx` `set_layout` takes the seven fields, and `null` clears one. The `tx.layout_managed` rejection names `margin_top` as the way to offset a child in the flow.
  - A document without margins renders byte-identically.
- **`zenith tx --ops`** — takes the transaction inline, as a `{"ops":[...]}` object or a bare array, so an edit needs no separate file. A file path or `--ops` is required, and not both.
- **`zenith schema op <unknown>` suggests ops.** It lists up to 12 ops whose names or summaries share a word with the name you typed, in place of the list of all 69. It prints the full list only when nothing matches.
- **`zenith new --starter <kind>`** — writes a finished layout to read and adapt: `flyer`, `letterhead`, `handout`, `deck`, `poster`, `menu`, `card`, or `report`. Each validates with no diagnostics at its own size and default theme. The command prints every text node's id and current text with the one `set_span_text` transaction that fills them, so a design is adapted in one call. A blank `zenith new` and `zenith schema` point at the starters.
  - The layout uses the theme contract (`color.*`, `size.*`, `radius.*`, `ui.*` styles, `defaults`), so `--theme` restyles it.
  - `--format`, `--width`, `--height`, and `--landscape` resize it. Type, spacing, margins, and the page box scale to fit the new page. `--pages` conflicts, because the page count comes from the starter.
  - The file gets a minted `doc-id` and a first history version. `--name` sets the project name, the title, and the ids.
  - `zenith-cli/tests/starter_hashes.json` pins the PNG, SVG, and PDF output of every starter on every CI target.

### Changed

- **`zenith inspect` prints a layout outline.** The human output is one line per node: kind, id, final box rounded to 0.1 px, `layout=` for a row, column or grid frame, and the text of a `text` node cut at 48 characters. It shows the final box of every compiled node, not only those that differ from the authored one, and no longer prints authored geometry or text ink bounds, which stay in `--json`. The JSON and the MCP `zenith_inspect` summary gain `text`, `spans`, and `layout` on the nodes that have them. `zenith new --starter` reads the same data for its text-slot list.
- **`set_fill`, `set_stroke`, and `set_stroke_width` accept a `frame`.** They rejected it with `tx.unsupported_property` although a frame paints a fill and stroke, so a button built as a frame could not be recolored by `tx`.
- **`zenith tx` reports what an edit introduces.** The result lists each Error and Warning that `zenith validate` shows on the result and did not show on the input, such as `text.overflow` or `layout.block_overlap`. A diagnostic counts as new by code and node, so an edit that only changes a measured value reports nothing. Before, a text edit that overflowed its box printed `diagnostics: (none)`. A new Warning gives `accepted (with warnings)`. A new Error, such as `text.fit_failed`, rejects the edit and writes nothing. A document that already has an error gives no baseline, so such an edit is never rejected for compile errors.
- **`text.overflow` in a row or column frame names the flow.** When the fix resizes an in-flow child along the frame's main axis (`h` in a column, `w` in a row), the message adds that a taller or wider box moves the siblings after it and a smaller `font-size` does not. The advice to set `h` reflowed the whole stack. A fix on the other axis gets no note.
- **`zenith schema node <kind>` prints a brief by default.** Attribute families such as `padding-x` and `margin-top` fold into their base row, and prose past about 600 characters is cut at a sentence, with a line naming `--notes`. `--notes` prints every row and the full text. `--json` is unchanged. `schema node text` drops from 9.0 KB to 4.4 KB and `frame` from 6.8 KB to 5.3 KB.
- **`zenith tx --help` lists common edits.** Eight runnable one-line ops cover text, fill, token, geometry, margin, add, remove, and visibility. A test checks that every op in the list exists. `--op` works as an alias of `--ops`.
- **`zenith tx` says whether it wrote the file.** The output gains `applied`. A dry run prints `applied: false (dry run, nothing written; add --apply to write the file)`, and `--json` carries `"applied": false`. Before, a dry run printed `changed: true` and a diff, which agents read as done. The MCP `zenith_tx` result carries `applied` too.
- **`zenith inspect` rounds boxes to 0.01 px.** The `box=` and `bounds=` values no longer print float noise such as `52.829999923706055`.

---

## [0.0.11] — 2026-10-10

A plain-text design format and a deterministic engine that agents and people edit, check, and render to PNG, SVG, and PDF.

### Added

- **`.zen` format** — KDL v2 text with `project`, `tokens`, `styles`, `document`, and `page` blocks. Every node carries a stable id.
  - `zenith new` scaffolds a valid document. The first edit stamps a ULID `doc-id` into the file.
  - Unknown nodes pass through unchanged, for forward compatibility.
  - `zenith fmt` writes the canonical form.
- **Design tokens** — `color` (sRGB and native CMYK), `dimension`, `number`, `fontFamily`, `fontWeight`, `gradient`, `shadow`, `filter`, and `mask`. Aliases chain, and a cycle is a diagnostic.
- **Node set** — `rect`, `ellipse`, `line`, `polygon`, `polyline`, `text`, `code`, `image`, `frame`, `group`, `pattern`, `shape`, `connector`, `chart`, `instance`, `field`, `footnote`, `toc`, and `table`.
- **Typography** — `rustybuzz` shaping with bundled Noto Sans and Noto Sans Mono, and font fallback.
  - Knuth–Liang hyphenation, drop caps, tab leaders, text runaround, and threaded text across `chain`ed boxes.
  - Inline spans: bold, italic, underline, strikethrough, highlight, inline code, and links.
  - `text src=` reads body copy from a `.md` or `.txt` file. `format="markdown"` parses inline marks and block structure.
  - `block role=` sets per-role typography at document, page, or text scope.
  - A missing glyph reports `font.glyph_missing`.
- **Visual effects** — linear and radial gradients, layered shadows, Gaussian blur, feathered masks, and 12 Porter-Duff blend modes. Also opacity cascade, per-corner radius, and image fit, clip shape, and object position.
- **Anchors** — 9-point placement against the page, a safe zone, the parent, or a sibling. Precedence is zone, then sibling, then parent, then page. Anchors resolve to explicit geometry. A node without an anchor renders the same as hand-placed coordinates.
- **Recipes** — a `recipes` block records how a generated motif was made: kind, seed, generator, params, palette tokens, and expanded node ids. Typed recipe transactions edit it.
- **Transaction engine** (`zenith tx`) — a typed op set for fill, stroke, geometry, structure, alignment, pages, tokens, find-replace, and more.
  - Dry-run is the default. `--apply` writes the file.
  - It enforces referential integrity and id uniqueness.
  - Each run reports a source diff, moved and resized boxes, affected ids, and an audit record.
  - `zenith fix` applies machine fixes for diagnostics, also dry-run by default.
- **Deterministic rendering** (`zenith render`) — the same bytes in give the same bytes out on any machine. The render path has no time, no randomness, no `HashMap`, no `unsafe`, and no C dependencies.
  - PNG through tiny-skia: one page, all pages, or facing-page spreads.
  - Self-contained SVG with outlined text and embedded assets.
  - Vector PDF with native DeviceRGB and CMYK, MediaBox, TrimBox, BleedBox, and no timestamps. Text is selectable and searchable, and links are clickable. Effects the PDF model lacks rasterize their enclosing scope, and a report lists each fallback.
  - `--scene` dumps the scene IR as JSON.
- **Validation** (`zenith validate`) — 70+ checks, each with a stable code, a message, the node id, and the source location.
  - Three severities: Error blocks rendering, Warning still renders, and Advisory is informational.
  - Checks cover references, raw visual literals instead of tokens, overflow, margins and safe zones, APCA contrast, colorspace and bleed, and the `variants`, `recipes`, and `provenance` blocks.
  - `--json` gives agents a machine-readable result.
- **Variants and data merge** — one design gives many outputs, deterministically.
  - `zenith variant` expands a `variants` block into named sizes, with per-target overrides. Output is PNG or SVG plus a `.zen` companion and a `zenith-variant-manifest-v1` manifest.
  - `zenith merge` renders one design per CSV row from `role="data.<column>"` bindings, for text and image columns. It writes a per-row report and a byte-reproducible manifest.
- **Connectors** — arrows whose endpoints derive from their `from` and `to` nodes, so they reroute when a node moves.
  - Named and divided outline anchors, and page `ports` that project through instances.
  - `straight`, `orthogonal`, and obstacle-avoiding `avoid` routes.
  - Self-loops, arrowheads, and `line-jumps="arc"` or `"gap"` at crossings.
- **Libraries** — packs export `<package>#<item>` items of kind `component`, `token`, or `action`.
  - Embedded packs: `@zenith/flowchart`, `@zenith/filters`, `@zenith/masks`, `@zenith/brand-kit`, and `@zenith/icons-lucide` (1745 icons).
  - A pack is a `.zen` file or a directory of `*.svg`. SVG icons convert to editable `path` nodes.
  - `zenith library search` ranks name over alias over tag. `library add` records `libraries` and `provenance`.
- **History and versions** — per-document history off the render path.
  - `undo` and `redo` walk an ephemeral session timeline of full snapshots.
  - `version` saves a named, content-addressed checkpoint (SHA-256 and DEFLATE). `restore` takes a name or a revspec.
  - `sync` records an edit made outside the engine.
  - Scratch candidates (`scratch`, `candidate`, `promote`, `finalize`) and portable `.zenithbundle` files (`bundle`, `unbundle`).
- **CLI** — the `zenith` binary. Every command takes `--json`, and `zenith schema` describes the authorable surface.
  - Author: `new`, `validate`, `fmt`, `tokens`, `inspect`.
  - Render: `render`. Edit: `tx`, `fix`, `edit`.
  - Variants: `variant`, `merge`. Library: `library list`, `search`, `show`, `add`. Theme: `theme new`.
  - History: `history`, `undo`, `redo`, `version`, `restore`, `sync`. Workspace: `scratch`, `candidate`, `promote`, `finalize`, `bundle`, `unbundle`.
  - Agent: `plugin install`, `plugin uninstall`, `plugin list`, `mcp`. `plugin install` writes a skill or rule file for Claude Code, Codex, OpenCode, Cursor, and other agents.
- **MCP server** (`zenith mcp`) — the full author loop as MCP tools, from `zenith_schema` to `zenith_render` and `zenith_merge`.
  - Results are trimmed JSON. Large and binary outputs return as resource links into a content-addressed store.
  - Six `zenith_editor_*` tools drive editor sessions: open, command, render, sessions, close, and attach to a running `zenith edit`.
  - Stdio is the default transport. The `http` feature adds Streamable HTTP at `/mcp` with a bearer token, loopback bind, Host and Origin checks, an 8 MiB body cap, and `--root` confinement.
  - The npm package `@zenitheditor/zenith-mcp` launches it through `npx`. `server.json` lists it in the MCP registry.
- **Browser editor** (`zenith edit`) — source and rendered canvas side by side. The source text is the source of truth.
  - Security: a 64-hex token from the OS RNG, new on every run, travels in the URL fragment and as a `Bearer` header only. The server binds to loopback unless `--allow-remote`. It checks Host and Origin.
  - Files: the engine reads only under the document's directory or `--root`. `..` and escaping symlinks are errors.
  - Ctrl-C, `SIGTERM`, and `SIGHUP` stop a clean session with exit code 0. With unsaved edits, the first signal warns, and a second discards them and exits with 130.
  - Panels: pages and layers, inspector, and diagnostics. A draggable divider and fullscreen buttons resize the split view.
  - Canvas: click, Shift-click, and band selection. Move, resize, and rotate one node or several as one selection, with snapping to nodes and the page.
  - Every canvas and inspector edit is a transaction. It patches the text in place and keeps comments and layout.
  - The canvas renders only the visible region at device resolution.
  - Undo and redo cover typing and canvas edits in one history. Agents drive the same session over `POST /api/cmd`.
- **Static editor site** — the same page with the engine as a `wasm32-wasip1` module in a Worker, and no server. It opens and saves files on the user's disk. `build-static-editor.mjs` builds the site and writes a CSP `_headers` file.
- **Wasm parity** — the wasm engine and native `zenith edit` give equal results on every example: the PNG SHA-256, diagnostics, a viewport render, and the text after each scripted gesture. An end-to-end test checks this.
- **Crates** — 14 crates on crates.io: 13 libraries and `zenith-tool`, which installs the `zenith` binary.
  - Libraries: `zenith-geometry`, `zenith-core`, `zenith-layout`, `zenith-raster`, `zenith-perception`, `zenith-scene`, `zenith-render`, `zenith-tx`, `zenith-session`, `zenith-zpx`, `zenith-producers`, `zenith-pipeline`, and `zenith-editor`.
  - `zenith-editor-wasm` is not published.
