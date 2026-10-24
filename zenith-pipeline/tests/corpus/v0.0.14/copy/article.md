# The Case for Plain-Text Design

Design files have always been opaque. You open a `.fig` or `.psd`, drag a few boxes, and export — but the result is a bag of pixels with no history, no diff, and no way to automate reliably. That needs to change.

## Why Text Wins

When design lives in plain text, the same tools that make code trustworthy — version control, pull requests, automated checks — apply to layouts too. A color change becomes a one-line diff. A layout bug has a bisectable history. An agent can open a file, read it, and edit it without guessing.

The key insight is that a design document is structured data, not a painting. Headings, paragraphs, tokens, and geometry are all addressable. Swap a token and every surface that references it updates at once.

## Determinism by Default

Zenith renders the same source to the same pixels on every machine. No timestamps, no iteration-order dependence, no platform quirks. The render is a pure function of the source — which means it is testable, cacheable, and safe to automate at scale.

Long-form copy stays in `.md` files the editor can round-trip. The `.zen` document controls layout, tokens, and block styling. Both are text. Both live in the repo.
