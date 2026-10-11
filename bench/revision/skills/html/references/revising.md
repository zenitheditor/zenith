# Revising an HTML design

## Finding what to change

- Match the request to an element by its visible text, class name, or color. Search the file for the exact value (`#2563eb`, `Get started`).
- A color can live in one rule or in several. Search for every occurrence before editing, then change only the ones that belong to the element in the request.
- If a value is set through a CSS variable, edit the variable only when every user of the variable should change. Otherwise edit the one rule.

## Moving things

- In a flex or grid container, move an element by changing `gap`, `margin`, `padding` or `order` on that element or its container. Prefer `margin-top` on the element itself, because it affects nothing else in the container.
- A change to a container's `gap` or `padding` moves every child. Use it only when that is the request.
- In an absolutely positioned layout, change `top`/`left` of that element only.
- Moving an element inside a column must not change the size of its siblings. Fixed heights stay fixed.

## Changing size and text

- A longer headline can wrap to another line and push content down. Check the render. If the box has a fixed height, keep the height and shorten or shrink the text only when asked.
- Changing `font-size` changes the line box. Check the elements below it.
- Never change an element's `width` or `height` to make new text fit unless the request says so. Report the problem instead.

## Adding an element

- Add it next to its siblings, with the same unit and naming style as the file.
- Give it its own class. Do not extend an existing rule that other elements use.
- Place it with the layout model the container already uses.
- Check contrast against the background and keep text above 12px.

## Colors

- Use the exact hex value from the request. Keep the file's notation (`#rrggbb`).
- Check that text on the changed surface is still readable.

## Before replying

- Render the final file and look at it.
- Confirm the requested change is visible and that every other element is where it was.
