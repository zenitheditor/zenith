---
name: html-design-revision
description: "Revise an existing HTML/CSS design file with small, safe edits. Use when asked to change a color, text, size, position or add an element to a design that already exists. Use other tools for building a design from scratch."
---

# HTML design revision

Revise the design file in place. Change only what the request names. Every other element keeps its position, size, color and text.

Read `references/revising.md` before the first edit.

1. Read the whole file first. Find the rule or element the request names.
2. Make the smallest edit that does it. Prefer `edit_file` over rewriting the file.
3. Reuse existing classes, variables and values. Never rename or reorder unrelated rules.
4. Keep the layout model of the file. If the file uses flex, stay on flex. Never mix in absolute positioning to fix one element.
5. Render with `view` after the edit and compare against the request.
6. Check that nothing else moved: text did not rewrap, boxes did not shift, nothing overflows the canvas.
7. Reply with one short sentence when done.
