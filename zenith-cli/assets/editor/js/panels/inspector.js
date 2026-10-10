// The inspector panel: `node.inspect` for the one selected node, with an
// edit form for the `node.set` fields the engine lists (see editform.js).

import { cssColor } from "../util/color.js";
import { clear, h, num, plural } from "../util/dom.js";
import { icon, kindIcon } from "../ui/icons.js";
import { editForm } from "./editform.js";

export class Inspector {
  /** `hooks.set(id, params)`: send `node.set`; resolves `true` on success. */
  constructor(root, hooks) {
    this.root = root;
    this.hooks = hooks;
    this.tokens = [];
    this.shownId = null;
    this.showEmpty("Nothing selected. Click a node on the canvas, in the layers, or in the source.");
  }

  /** The color tokens the fill and stroke pickers list: `[{id, type, value}]`. */
  setTokens(tokens) {
    this.tokens = tokens ?? [];
  }

  showEmpty(text) {
    this.shownId = null;
    clear(this.root);
    this.root.appendChild(h("p", { class: "empty", text }));
  }

  /** Show several selected nodes by id. */
  showMany(ids) {
    this.shownId = null;
    clear(this.root);
    this.root.append(
      h("p", { class: "inspector-sub", text: `${plural(ids.length, "node")} selected.` }),
      h("ul", { class: "page-list" }, ids.map((id) => h("li", { class: "row", text: id }))),
    );
  }

  /** Show the reply of `node.inspect`. */
  show(info) {
    // Keep the focused edit field focused across a refresh of the same node.
    const active = document.activeElement;
    const focused = this.shownId === info.id && active instanceof HTMLElement && this.root.contains(active)
      ? active.dataset.field
      : null;
    this.shownId = info.id;
    clear(this.root);
    const where = info.page ? `Page ${info.page}` : info.master ? `Master ${info.master}` : "";
    this.root.append(
      h(
        "div",
        { class: "inspector-head" },
        icon(kindIcon(info.kind)),
        h("span", { class: "inspector-id", text: info.id }),
      ),
      h("p", {
        class: "inspector-sub",
        text: [info.kind, info.name ? `"${info.name}"` : null, where, info.parent ? `in ${info.parent}` : null]
          .filter(Boolean)
          .join(" · "),
      }),
    );
    this.flags(info);
    if (info.edit) {
      const id = info.id;
      this.root.append(...editForm(info, this.tokens, (params) => this.hooks.set(id, params)).flat());
    }
    if (info.box) this.box(info.box);
    this.attributes(info.attributes ?? []);
    if (info.style) this.style(info.style);
    if (focused) this.root.querySelector(`[data-field="${focused}"]`)?.focus();
  }

  flags(info) {
    const lock = info.lock ?? {};
    const lines = [];
    if (info.stale) lines.push(["warning", "From the last valid text. The source has errors."]);
    if (info.master) lines.push(["link", `Master content: an edit changes every page that uses ${info.master}.`]);
    if (lock.locked_by) {
      lines.push(["lock", lock.locked_by === info.id ? "Locked." : `Locked by ${lock.locked_by}.`]);
    }
    if (lock.hidden_by) {
      lines.push(["hidden", lock.hidden_by === info.id ? "Hidden." : `Hidden by ${lock.hidden_by}.`]);
    }
    if (lock.in_flow) lines.push(["frame", `Placed by ${lock.in_flow.mode} layout of ${lock.in_flow.frame}.`]);
    if (lock.anchored) lines.push(["link", "Placed by an anchor."]);
    for (const [name, text] of lines) {
      this.root.appendChild(h("p", { class: "flag" }, icon(name), h("span", { text })));
    }
  }

  box(b) {
    const cells = [
      ["X", b.x],
      ["Y", b.y],
      ["W", b.w],
      ["H", b.h],
    ];
    if (b.rotate !== undefined && b.rotate !== null) cells.push(["R", `${num(b.rotate)}°`]);
    if (Math.abs(b.angle ?? 0) > 1e-9) cells.push(["∠", `${num(b.angle)}°`]);
    this.root.append(
      h("h3", { text: "Drawn box (px)" }),
      h(
        "div",
        { class: "box-grid" },
        cells.map(([k, v]) =>
          h("div", {}, h("span", { text: k }), h("span", { text: typeof v === "number" ? num(v) : v })),
        ),
      ),
    );
  }

  attributes(list) {
    this.root.appendChild(h("h3", { text: "Attributes" }));
    if (list.length === 0) {
      this.root.appendChild(h("p", { class: "empty", text: "No attributes." }));
      return;
    }
    const dl = h("dl", { class: "kv" });
    for (const a of list) {
      dl.append(h("dt", { text: a.name }), h("dd", {}, value(a)));
    }
    this.root.appendChild(dl);
  }

  style(s) {
    const label = s.source === "style" ? `Style ${s.id}` : `Defaults for ${s.id}`;
    this.root.appendChild(h("h3", { text: label }));
    if (!s.defined) {
      this.root.appendChild(h("p", { class: "empty", text: `${s.id} is not defined.` }));
      return;
    }
    const entries = Object.entries(s.properties ?? {});
    if (entries.length === 0) {
      this.root.appendChild(h("p", { class: "empty", text: "No properties." }));
      return;
    }
    const dl = h("dl", { class: "kv" });
    for (const [name, p] of entries) {
      const text = p.token ? null : String(p.value ?? p.data ?? "");
      dl.append(
        h("dt", { text: name }),
        h("dd", {}, p.token ? tokenChip(p.token, p.resolved) : text),
      );
    }
    this.root.appendChild(dl);
  }
}

/** The value cell of one attribute: value, unit, token binding, px. */
function value(a) {
  const parts = [];
  if (a.token) {
    parts.push(tokenChip(a.token.id, a.token.value, a.token.type));
  } else {
    const shown = typeof a.value === "string" ? a.value : JSON.stringify(a.value);
    parts.push(h("span", { text: shown }));
    if (a.unit) parts.push(h("span", { class: "unit", text: ` ${a.unit}` }));
  }
  const resolvedPx = a.token && format(a.token.value ?? "") === `${num(a.px)}px`;
  if (typeof a.px === "number" && !(a.unit === "px" && !a.token) && !resolvedPx) {
    parts.push(h("span", { class: "px", text: ` = ${num(a.px)} px` }));
  }
  return parts;
}

/** A token binding: the token id, then its resolved value (with a swatch for colors). */
function tokenChip(id, resolved, type) {
  const chip = h(
    "span",
    { class: "chip", title: type ? `${type} token` : "token" },
    icon("link"),
    h("span", { text: id }),
  );
  if (resolved === undefined || resolved === null) return chip;
  const shown = format(resolved);
  return [chip, h("span", { class: "resolved" }, swatch(shown), h("span", { text: shown }))];
}

/** A color swatch of `value`, or `null` when `value` is not a CSS color. */
export function swatch(value) {
  const color = cssColor(value);
  if (color === null) return null;
  const el = h("span", { class: "swatch" });
  el.style.setProperty("background-color", color);
  return el;
}

function format(v) {
  if (typeof v === "string") {
    // `(px)10` reads as `10px`.
    const m = /^\(([a-z%]+)\)(-?[0-9.]+)$/.exec(v);
    return m ? `${m[2]}${m[1]}` : v;
  }
  if (typeof v === "number") return num(v);
  if (v && typeof v === "object" && "value" in v && "unit" in v) return `${num(v.value)}${v.unit}`;
  return JSON.stringify(v);
}
