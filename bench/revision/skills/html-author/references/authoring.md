# Authoring a design in HTML

## File shape

- One `<section class="page">` per page, exactly the canvas width and height in px, `overflow: hidden`.
- `body { margin: 0 }`. Sections stack with no gap.
- Declare the palette, type scale, and spacing scale as `:root` variables. Reference them everywhere.

## Layout

- Give each page one outer padding equal to the margin the task needs. Keep all content inside it.
- Use flex or grid for rows, columns, and card grids. Equal columns use `grid-template-columns: repeat(n, 1fr)` with one `gap`.
- Let text boxes size to their content. Set a `max-width` for readable line length. Never fix a height that text could outgrow.
- Align number columns right. Use `font-variant-numeric: tabular-nums`.

## Type

- Use at most two weights and two sizes for body text. Make the headline at least three times the body size.
- Body text is at least 1.4% of the page width. Line height is 1.3 to 1.6.
- Keep light text on dark and dark text on light with a contrast ratio of at least 4.5.

## Spacing

- Pick one base unit and use its multiples. Group related items closer than unrelated ones.
- Use a few large spaces and many small ones. Do not leave a large empty region unless it is deliberate.

## Shapes and effects

- Gradients: `linear-gradient`. Shadows: `box-shadow`. Patterns: repeating gradients or small repeated elements.
- Charts and diagrams: draw with inline SVG or sized `div`s. Compute sizes from the data.

## Check

- After each render, check that no text is cut off, no box overlaps another, and every page matches the canvas size.
