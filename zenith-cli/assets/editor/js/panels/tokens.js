// Tokens tab in the left panel: displays resolved tokens grouped by category,
// with visual `+ Add token` creation and inline editing (color picker swatch,
// dimension/font/weight inputs) backed by `tx.apply` (`create_token`,
// `update_token_value`) so users never need to hand-edit KDL.

import { clear, h } from "../util/dom.js";
import { icon } from "../ui/icons.js";

const GROUPS = [
  ["Colors", (t) => t.type === "color"],
  ["Gradients", (t) => t.type === "gradient"],
  ["Type", (t) => t.type === "fontFamily" || t.type === "fontWeight"],
  ["Sizes", (t) => t.type === "dimension" || t.type === "number"],
  ["Effects", (t) => t.type === "shadow" || t.type === "filter" || t.type === "mask"],
];

const BUNDLED_FONTS = ["Noto Sans", "Noto Serif", "Noto Sans Mono"];
const WEIGHTS = ["300", "400", "500", "600", "700", "800"];

function valueText(t) {
  if (typeof t.value === "string") return t.value;
  if (typeof t.value === "number") return String(t.value);
  if (t.value && typeof t.value === "object") {
    if ("px" in t.value) return `${t.value.px}px`;
    if ("value" in t.value && "unit" in t.value) return `${t.value.value}${t.value.unit}`;
  }
  return t.type;
}

function hex6(v) {
  if (typeof v !== "string") return "#0870a6";
  const m = v.trim().match(/^#([0-9a-f]{6})/i);
  return m ? `#${m[1]}` : "#0870a6";
}

export class TokensPanel {
  /** `hooks`: `{ onPick(token), onCreate({id, type, value}), onUpdate(id, value) }`. */
  constructor(host, hooks) {
    this.host = host;
    this.hooks = typeof hooks === "function" ? { onPick: hooks } : hooks ?? {};
    this.tokens = [];
    this.adding = false;
    this.addType = "color";
    this.query = "";
  }

  set(tokens) {
    this.tokens = tokens ?? [];
    this.draw();
  }

  draw() {
    if (!this.host) return;
    clear(this.host);

    const search = h("input", {
      type: "search",
      class: "edit-input asset-search",
      placeholder: "Filter…",
      value: this.query,
      "aria-label": "Filter tokens",
      oninput: (e) => {
        this.query = e.target.value;
        this.draw();
        const next = this.host.querySelector(".asset-search");
        if (next) {
          next.focus();
          next.setSelectionRange(this.query.length, this.query.length);
        }
      },
    });
    const addBtn = this.hooks.onCreate
      ? h(
          "button",
          {
            type: "button",
            class: "page-act-btn asset-upload-btn",
            id: "tok-add-btn",
            onclick: () => {
              this.adding = !this.adding;
              this.draw();
            },
          },
          icon("plus"),
          h("span", { text: this.adding ? "Cancel" : "Add" }),
        )
      : null;
    this.host.appendChild(h("div", { class: "asset-bar" }, search, addBtn));
    if (this.adding && this.hooks.onCreate) this.host.appendChild(this.renderAddForm());

    const q = this.query.trim().toLowerCase();
    const visible = q
      ? this.tokens.filter((t) => `${t.id} ${valueText(t)} ${t.type}`.toLowerCase().includes(q))
      : this.tokens;
    if (!visible.length) {
      this.host.appendChild(h("p", { class: "empty", text: q ? "No matching tokens." : "No tokens defined." }));
      return;
    }
    for (const [label, pred] of GROUPS) {
      const items = visible.filter(pred);
      if (!items.length) continue;
      this.host.appendChild(h("div", { class: "tok-group", text: label }));
      for (const t of items) {
        this.host.appendChild(this.renderRow(t));
      }
    }
  }

  renderAddForm() {
    const typeSel = h(
      "select",
      { class: "edit-input", "aria-label": "Token type" },
      [
        ["color", "Color"],
        ["dimension", "Size (px)"],
        ["fontFamily", "Font family"],
        ["fontWeight", "Font weight"],
      ].map(([v, lbl]) => h("option", { value: v, text: lbl, selected: v === this.addType })),
    );
    const defaultId = {
      color: "color.custom",
      dimension: "size.custom",
      fontFamily: "font.custom",
      fontWeight: "weight.500",
    }[this.addType];
    const idInput = h("input", {
      class: "edit-input",
      type: "text",
      placeholder: defaultId,
      value: defaultId,
      "aria-label": "Token ID",
    });

    let valControl;
    const readVal = () => {
      if (this.addType === "dimension") {
        const n = Number(valControl.value);
        return Number.isFinite(n) ? `${n}px` : "16px";
      }
      return valControl.value.trim();
    };

    if (this.addType === "color") {
      valControl = h("input", { class: "tok-color-pick", type: "color", value: "#0870a6", "aria-label": "Token color" });
    } else if (this.addType === "fontFamily") {
      valControl = h("select", { class: "edit-input", "aria-label": "Font family" },
        BUNDLED_FONTS.map((f) => h("option", { value: f, text: f })));
    } else if (this.addType === "fontWeight") {
      valControl = h("select", { class: "edit-input", "aria-label": "Font weight" },
        WEIGHTS.map((w) => h("option", { value: w, text: w, selected: w === "600" })));
    } else {
      valControl = h("input", { class: "edit-input", type: "number", value: "16", "aria-label": "Size in px" });
    }

    typeSel.addEventListener("change", () => {
      this.addType = typeSel.value;
      this.draw();
    });

    const submit = async (e) => {
      e.preventDefault();
      const id = idInput.value.trim();
      const value = readVal();
      if (!id || !value) return;
      const ok = await this.hooks.onCreate({ id, type: this.addType, value });
      if (ok) {
        this.adding = false;
        this.draw();
      }
    };

    return h(
      "form",
      { class: "tok-form", onsubmit: submit },
      typeSel,
      idInput,
      h("div", { class: "tok-form-row" }, valControl, h("button", { type: "submit", class: "button primary small", text: "Add" })),
    );
  }

  renderRow(t) {
    let leading;
    if (t.type === "color" && typeof t.value === "string" && this.hooks.onUpdate) {
      const picker = h("input", {
        type: "color",
        class: "tok-sw-input",
        value: hex6(t.value),
        title: `Change ${t.id} color`,
        "aria-label": `Change ${t.id} color`,
      });
      picker.addEventListener("click", (e) => e.stopPropagation());
      picker.addEventListener("change", (e) => {
        e.stopPropagation();
        this.hooks.onUpdate(t.id, picker.value);
      });
      leading = picker;
    } else if (t.type === "color" && typeof t.value === "string") {
      leading = h("span", { class: "sw", style: `background:${t.value}` });
    } else {
      leading = icon(t.type === "fontFamily" ? "text" : "token", "icon");
    }

    const row = h(
      "div",
      { class: "tok", title: `${t.id} (${t.type})` },
      leading,
      h(
        "button",
        {
          type: "button",
          class: "tok-btn",
          onclick: () => this.hooks.onPick?.(t),
        },
        h("span", { class: "tok-name", text: t.id }),
        h("span", { class: "v", text: valueText(t) }),
      ),
    );
    return row;
  }
}
