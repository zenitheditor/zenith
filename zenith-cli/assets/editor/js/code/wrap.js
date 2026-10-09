// Hanging indent for wrapped lines: a wrapped line continues two columns
// right of its own indentation, so long Zenith nodes keep their shape.

import { Decoration, RangeSetBuilder, ViewPlugin } from "../../vendor/codemirror.js";

const EXTRA_COLUMNS = 2;
const cache = new Map();

/** The line decoration for `columns` of leading whitespace. */
function lineDeco(columns) {
  let deco = cache.get(columns);
  if (!deco) {
    const n = columns + EXTRA_COLUMNS;
    deco = Decoration.line({
      attributes: { style: `padding-left: calc(6px + ${n}ch); text-indent: -${n}ch` },
    });
    cache.set(columns, deco);
  }
  return deco;
}

function build(view) {
  const builder = new RangeSetBuilder();
  const tab = view.state.tabSize;
  for (const { from, to } of view.visibleRanges) {
    for (let pos = from; pos <= to; ) {
      const line = view.state.doc.lineAt(pos);
      let columns = 0;
      for (const ch of line.text) {
        if (ch === " ") columns++;
        else if (ch === "\t") columns += tab - (columns % tab);
        else break;
      }
      builder.add(line.from, line.from, lineDeco(columns));
      pos = line.to + 1;
    }
  }
  return builder.finish();
}

export const hangingIndent = ViewPlugin.fromClass(
  class {
    constructor(view) {
      this.decorations = build(view);
    }

    update(update) {
      if (update.docChanged || update.viewportChanged) this.decorations = build(update.view);
    }
  },
  { decorations: (v) => v.decorations },
);
