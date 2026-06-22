---
description: Set up a project brand kit, or apply the brand to a document.
argument-hint: "[setup | apply <file>]"
allowed-tools:
  - Bash(zenith:*)
  - Read
  - Write
  - Glob
---

Brand task: **$ARGUMENTS**

Read `references/brand.md` from the zenith skill, then:

- **setup** → create `.zenith/brand.md` (scaffold from `templates/brand.md`) filled from the
  user's brand (palette roles, fonts, spacing, voice, do/don't), and a `libraries/<brand>.zen`
  pack holding the brand tokens (+ an `apply-<brand>` action). Confirm both with the user.
- **apply <file>** → ensure the document's colors/fonts/sizes are tokens, then apply the brand
  action with a transaction: `zenith tx <file> brand.tx.json` (dry-run) → review the diff →
  `--apply`. Then `zenith validate` and `zenith render` and confirm it reads on-brand.

Prefer the project's existing tokens/packs over inventing a palette. Verify ops with
`zenith tx --help` and `zenith library --help`.
