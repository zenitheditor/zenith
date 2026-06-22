---
description: Scaffold a new .zen design document from a brief (validates + renders a first preview).
argument-hint: "[brief, e.g. 'square instagram promo for a coffee launch']"
allowed-tools:
  - Bash(zenith:*)
  - Read
  - Write
  - Glob
---

Create a new Zenith design document for: **$ARGUMENTS**

Follow the `zenith` skill. Steps:

1. If `.zenith/brand.md` (or a `libraries/*.zen` brand pack) exists in or above the working
   dir, read it and use its tokens. Otherwise pick a sensible small token palette.
2. Choose a canvas size from the brief (e.g. 1080×1080 social square, 1080×1920 story,
   poster). Start from a `templates/` starter if one fits.
3. Write the `.zen` source: tokens first (color/font/size as tokens), then a `document` with a
   `page` and semantically-id'd nodes. Capture the brief in a `note`.
4. `zenith validate <file> --json` — fix every hard diagnostic at the source.
5. `zenith render <file> --png <file>.png`, then describe what you produced and the output path.

Do not invent syntax — mirror `examples/*.zen` and verify with `zenith <cmd> --help`.
