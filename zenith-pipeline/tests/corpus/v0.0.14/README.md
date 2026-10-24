# Frozen stability corpus

This directory is frozen. Never edit a document here to make a test pass.

- `*.zen`, `assets/`, `copy/`: the top-level `examples/` documents and their assets, copied when the workspace version was 0.0.14.
- `coverage.zen`, `brand/brand.zen`: node kinds and features that `examples/` lacks (master, field, footnote, toc, path, light, mesh, composition import).
- `FROZEN_AT`: the version of the freeze, one line.
- `pins.json`: the canonical documents and the PNG SHA-256 of every page.

`cargo test -p zenith-pipeline --test stability` checks the corpus. README.md `## Stability` states the promise.

Refresh the corpus only for a declared break:

1. Add `### Breaking` to the release section in CHANGELOG.md, and list the break.
2. Replace this directory with `tests/corpus/v<X.Y.Z>/` for the release, and set `FROZEN_AT` to `X.Y.Z`.
3. Run `ZENITH_UPDATE_HASHES=1 cargo test -p zenith-pipeline --test stability` and commit `pins.json` in the same commit.

A pin change with no document change (pixels after a font or shaping change) needs step 3 only, plus a CHANGELOG entry.
