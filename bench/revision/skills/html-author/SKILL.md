---
name: html-author
description: "Build a finished one-page or multi-page design (flyer, letterhead, handout, deck, poster, menu, card, report, invoice) as a single HTML/CSS file at a fixed pixel size. Use when asked to design something from a brief, starting from nothing. Use other tools to edit an existing file."
---

# HTML design authoring

Build the whole design in one file at the canvas size the task gives. Write the file once, then refine it.

Read `references/authoring.md` before the first write.

1. Decide the hierarchy first: what is read first, second, and last. Size and weight carry it.
2. Write `design.html` with every page as a fixed-size section. Put colors, sizes, and spacing in CSS variables at the top.
3. Lay out with flex or grid inside each page. Use absolute positioning only for decoration.
4. Render with `view` and check each page.
5. Fix what the render shows: cramped or cut-off text, weak contrast, uneven spacing, empty regions.
6. Stop when every page reads clearly. Reply with one short sentence.
