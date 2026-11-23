// Tokens tab in the left panel: displays resolved tokens grouped by category.
// Clicking a token copies/reveals its reference or switches to source.

import { clear, h } from "../util/dom.js";
import { icon } from "../ui/icons.js";

const GROUPS = [
  ["Colors", (t) => t.type === "color"],
  ["Gradients", (t) => t.type === "gradient"],
  ["Type", (t) => t.type === "fontFamily" || t.type === "fontWeight"],
  ["Sizes", (t) => t.type === "dimension" || t.type === "number"],
  ["Effects", (t) => t.type === "shadow" || t.type === "filter" || t.type === "mask"],
];

function valueText(t) {
  if (typeof t.value === "string") return t.value;
  if (typeof t.value === "number") return String(t.value);
  if (t.value && typeof t.value === "object") {
    if ("px" in t.value) return `${t.value.px}px`;
    if ("value" in t.value && "unit" in t.value) return `${t.value.value}${t.value.unit}`;
  }
  return t.type;
}

export class TokensPanel {
  constructor(host, onPick) {
    this.host = host;
    this.onPick = onPick;
    this.tokens = [];
  }

  set(tokens) {
    this.tokens = tokens ?? [];
    this.draw();
  }

  draw() {
    if (!this.host) return;
    clear(this.host);
    if (!this.tokens.length) {
      this.host.appendChild(h("p", { class: "empty", text: "No tokens defined." }));
      return;
    }
    for (const [label, pred] of GROUPS) {
      const items = this.tokens.filter(pred);
      if (!items.length) continue;
      this.host.appendChild(h("div", { class: "tok-group", text: label }));
      for (const t of items) {
        const swatch =
          t.type === "color" && typeof t.value === "string"
            ? h("span", { class: "sw", style: `background:${t.value}` })
            : icon(t.type === "fontFamily" ? "text" : "token", "icon");
        this.host.appendChild(
          h(
            "button",
            {
              type: "button",
              class: "tok",
              title: `${t.id} (${t.type})`,
              onclick: () => this.onPick?.(t),
            },
            swatch,
            h("span", { class: "tok-name", text: t.id }),
            h("span", { class: "v", text: valueText(t) }),
          ),
        );
      }
    }
  }
}
