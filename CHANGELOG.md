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
