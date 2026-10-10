# Repository Guidelines

This file is the **source of truth** for working in the Zenith repository — for human
contributors and AI agents alike. Read it before making changes.

## Project Structure & Module Organization

Zenith is a Rust workspace (`edition = "2024"`). Each crate owns one concern and exposes a
stable contract; `zenith-geometry` depends on no other Zenith crate; `zenith-core` may depend only on pure `zenith-geometry` for path math (validation / fill queries).

- `zenith-core/` — KDL parser adapter, semantic AST, canonical formatter, tokens, validation, the full diagnostic set. May depend on pure `zenith-geometry` only among Zenith crates.
- `zenith-geometry/` — pure geometry (paths, contours, booleans). C-free; depends on no other Zenith crate; used by core, layout, scene, and tx.
- `zenith-layout/` — text shaping & font metrics (`rustybuzz` + `ttf-parser`); all third-party shaping types confined here.
- `zenith-raster/` — raster surface, blend modes, adjustments used by perception and render.
- `zenith-perception/` — visual QA / perception reports over geometry + raster.
- `zenith-scene/` — backend-neutral scene IR and compilation (geometry, text wrap, anchors, opacity/clip, text outlines).
- `zenith-render/` — CPU PNG backend (tiny-skia) and vector PDF backend; raster-time determinism rules.
- `zenith-tx/` — transaction op set, apply/dry-run engine, diffs, audit-record contract. Pure AST; no scene/layout dependency.
- `zenith-session/` — local doc identity (ULID), ephemeral session DAG, durable content-addressed version store. Never on the render path.
- `zenith-zpx/` — packaged design export (manifest + bake).
- `zenith-producers/` — higher-level produce/export helpers (SVG native, ZPX bake).
- `zenith-pipeline/` — config policy, imports, fonts, assets, validate, compile, and render over host I/O traits (`SourceFs`, `ConfigSource`, `LocalFontSource`, `PageRunner`). Shared by the CLI and the wasm editor. NEVER `std::fs` / `std::thread` / time in it; the CLI's native host lives in `zenith-cli/src/native.rs`.
- `zenith-editor/` — the editor engine: stateless `execute(env, session, request)` over a serializable `Session` (selection, gestures to tx ops, text-delta history, render). Shared by the browser module, `zenith edit`, and MCP; wasm-safe like `zenith-pipeline`.
- `zenith-editor-wasm/` — `wasm32-wasip1` command module of the editor engine: one JSON request on stdin, one JSON response on stdout. Backs the static browser editor. `publish = false`.
- `zenith-editor/benches/` — `zenith-editor-bench`: fluxbench benches of the editor loops. `publish = false`.
- `zenith-cli/` — the `zenith` binary (crate package name `zenith-tool`): command dispatch, clap, JSON/human output, MCP; render/validate logic goes in `zenith-pipeline`, not here. `src/main.rs` is thin; logic lives in `src/lib.rs` and `src/commands/`.
- `examples/` — runnable `.zen` documents (keep them valid against the current parser).
- `assets/` — bundled fonts (`zenith-core/assets/fonts/`) and embedded library presets (`assets/libraries/`). **Committed.**
- `conformance/` — regenerable rendered proof of simulation scenarios. **Gitignored**; never stage it.
- `resource/`, `.agents/`, `.claude/`, `.codex/` — local planning/research/session scratch. **Gitignored.**

## Build, Test, and Development Commands

```bash
cargo build --release                                      # build the workspace + `zenith` CLI
cargo test --workspace                                     # run all unit + integration tests
cargo clippy --workspace --all-targets -- -D warnings      # lint; warnings are errors
cargo fmt --all                                            # canonical formatting
cargo run -p zenith-cli -- <command> [args]                # run the CLI locally
```

The full green bar is: `cargo build` clean, `cargo test --workspace` green, `cargo clippy
--workspace --all-targets -- -D warnings` clean, and `cargo fmt --all` applied.

## Coding Style & Naming Conventions

- Rust 2024 edition; format with `cargo fmt`, keep `cargo clippy -D warnings` clean.
- Modules and functions use `snake_case`; types and traits use `CamelCase`.
- Transaction ops are `CamelCase` variants of `Op`; diagnostic codes follow `<namespace>.<snake_event>` (e.g. `token.cyclic_reference`, `font.glyph_missing`).
- Builder/setter methods follow `with_*` naming where applicable.

## Hard Rules (Invariants)

These are non-negotiable and enforced in review. They exist to keep the engine deterministic,
auditable, and dependency-light.

- **No `unwrap` / `expect` / `panic!` / `unreachable!` / `todo!` / unchecked indexing in library code.** Return `Result`. (Tests may use them.)
- **`unsafe` is forbidden** workspace-wide (`unsafe_code = "forbid"`). No FFI.
- **The dependency graph stays C-free** — no `zstd-sys` / `libz-sys` / `zlib-sys` or other C deps in `Cargo.lock`. (This is why history compression uses DEFLATE, not zstd.)
- **Determinism on the render path** — use `BTreeMap` / `BTreeSet`, never `HashMap` / `HashSet`. No time, no randomness, no iteration-order dependence. Same bytes in → same bytes out, on any machine.
- **Exhaustive matches on our own enums** — no `_` wildcard arm over a Zenith enum; a new node kind or op must force a compile error at every site.
- **No `#[allow(...)]`.** Fix the cause. Over-long argument lists are bundled into a `Copy` context struct, not suppressed.
- **Module-root files are wiring only.** `mod.rs` / `lib.rs` contain imports, re-exports, and the dispatcher edge — no business logic.
- **No file over ~1000 lines.** Prefer a directory module (`foo/mod.rs` + submodules) over one large file; split early rather than late.
- **Additive changes are byte-identical when the new feature is absent.** A document that doesn't use a new property must render exactly as before.
- **Fix issues at the source immediately.** Don't accumulate tracked debt; multiple focused commits per unit are fine.

## Testing Guidelines

- **Unit tests** live in the implementation file under `#[cfg(test)] mod tests { use super::*; }` and may exercise private functions.
- **Integration tests** live in each crate's `tests/` directory, compiled as separate crates, and exercise only the public API. Name files after the feature (e.g. `tests/recipe.rs`). Group with subdirectories that contain a `mod.rs`; shared helpers go in `tests/common/mod.rs`.
- Add coverage for every new node kind, diagnostic, transaction op, and serialization path.
- Render proof of conformance scenarios into `conformance/<area>/` (not `/tmp`); it is regenerable and not committed.

## Commit & Pull Request Guidelines

- Commit messages follow Conventional Commits with a crate scope: `feat(zenith-scene): …`, `fix(zenith-core): …`, `test(zenith-cli): …`. List all touched crates in the scope when a change spans several.
- Keep commits focused — one coherent, green, bisect-safe unit per commit.
- Do **not** add co-author trailers or tool-attribution lines.
- PRs should describe the change, the commands run to verify it (build/test/clippy/fmt), and any rendered output or diagnostic changes. Link related issues.
- Never stage `conformance/`, `resource/`, `zenith-showcase/`, or any local-scratch directory.
  </content>
