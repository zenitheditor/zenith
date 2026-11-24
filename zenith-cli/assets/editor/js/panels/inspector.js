// The inspector panel: `node.inspect` for the one selected node, with an
// edit form for the `node.set` fields the engine lists (see editform.js).

import { cssColor } from "../util/color.js";
import { clear, h, num, plural } from "../util/dom.js";
import { icon, kindIcon } from "../ui/icons.js";
import { editForm, textField } from "./editform.js";

export class Inspector {
  /** `hooks.set(id, params)`: send `node.set`; resolves `true` on success. */
  constructor(root, hooks) {
    this.root = root;
    this.hooks = hooks;
    this.tokens = [];
    this.outline = null;
    this.page = 1;
    this.shownId = null;
    this.lastInfo = null;
    this.showEmpty();
  }

  /** The color tokens the fill and stroke pickers list: `[{id, type, value}]`. */
  setTokens(tokens) {
    this.tokens = tokens ?? [];
    if (!this.shownId) this.showEmpty();
  }

  setPageInfo(outline, page) {
    this.outline = outline;
    this.page = page || 1;
    if (!this.shownId) this.showEmpty();
  }

  showEmpty(text = "Select something on the canvas to edit it, or press ⌘K.") {
    this.shownId = null;
    this.lastInfo = null;
    clear(this.root);
    const pg = this.outline?.pages?.find((p) => p.page === this.page);
    if (!pg) {
      this.root.appendChild(h("p", { class: "empty", text }));
      return;
    }
    const countNodes = (list) => (list ?? []).reduce((n, c) => n + 1 + countNodes(c.children), 0);
    const layerCount = countNodes(pg.children);
    this.root.append(
      h(
        "div",
        { class: "insp-head-wrap" },
        h(
          "div",
          { class: "inspector-head page-head" },
          icon("page"),
          h("span", { class: "inspector-id", text: pg.name || pg.id }),
          h("span", { class: "muted", text: "Page" }),
        ),
      ),
      h(
        "section",
        { class: "insp-sec" },
        h("h3", { text: "Size" }),
        h(
          "div",
          { class: "box-grid" },
          h("div", {}, h("span", { text: "W" }), h("span", { text: `${Math.round(pg.w)} px` })),
          h("div", {}, h("span", { text: "H" }), h("span", { text: `${Math.round(pg.h)} px` })),
        ),
      ),
      h(
        "section",
        { class: "insp-sec" },
        h("h3", { text: "Document" }),
        h(
          "dl",
          { class: "kv" },
          h("dt", { text: "Tokens" }),
          h("dd", { text: String(this.tokens.length) }),
          h("dt", { text: "Layers" }),
          h("dd", { text: String(layerCount) }),
          h("dt", { text: "Pages" }),
          h("dd", { text: String(this.outline?.pages?.length ?? 1) }),
        ),
        h("p", { class: "insp-note", text }),
      ),
    );
  }

  /** Show several selected nodes by id with Align & Distribute controls. */
  showMany(ids) {
    this.shownId = null;
    this.lastInfo = null;
    clear(this.root);
    const aligns = [
      ["left", "Left"],
      ["hcenter", "Center H"],
      ["right", "Right"],
      ["top", "Top"],
      ["vcenter", "Center V"],
      ["bottom", "Bottom"],
    ];
    const alignSec = this.hooks.align
      ? h(
          "section",
          { class: "insp-sec" },
          h("h3", { text: "Align" }),
          h(
            "div",
            { class: "align-grid" },
            aligns.map(([key, label]) =>
              h("button", {
                type: "button",
                class: "align-btn",
                text: label,
                title: `Align ${label}`,
                onclick: () => this.hooks.align(ids, key),
              }),
            ),
          ),
          ids.length >= 3 && this.hooks.distribute
            ? h(
                "div",
                { class: "align-grid dist-grid" },
                h("button", {
                  type: "button",
                  class: "align-btn",
                  text: "Distribute H",
                  onclick: () => this.hooks.distribute(ids, "horizontal"),
                }),
                h("button", {
                  type: "button",
                  class: "align-btn",
                  text: "Distribute V",
                  onclick: () => this.hooks.distribute(ids, "vertical"),
                }),
              )
            : null,
        )
      : null;
    this.root.append(
      h(
        "div",
        { class: "insp-head-wrap" },
        h("p", { class: "inspector-sub", text: `${plural(ids.length, "node")} selected.` }),
      ),
      alignSec,
      h(
        "section",
        { class: "insp-sec" },
        h("h3", { text: "Selection" }),
        h("ul", { class: "page-list" }, ids.map((id) => h("li", { class: "row", text: id }))),
      ),
    );
  }

  /** Show the reply of `node.inspect`. */
  show(info) {
    // Keep the focused edit field focused across a refresh of the same node.
    const active = document.activeElement;
    const focused = this.shownId === info.id && active instanceof HTMLElement && this.root.contains(active)
      ? active.dataset.field
      : null;
    const expanded = this.shownId === info.id && this.root.querySelector(".inspector-advanced")?.open;
    this.shownId = info.id;
    this.lastInfo = info;
    clear(this.root);
    const guided = this.hooks.guideText?.(info.id) && info.edit?.fields?.includes("text");
    const set = (params) => this.hooks.set(info.id, params);
    const details = guided
      ? h("details", { class: "inspector-advanced", open: expanded },
          h("summary", { class: "insp-sec", text: "More settings", dataset: { field: "advanced" } }))
      : this.root;
    if (guided) {
      this.root.append(
        h("section", { class: "insp-sec" }, ...textField(info.edit.text ?? "", set, "Headline")),
        details,
      );
    }
    const where = info.page ? `Page ${info.page}` : info.master ? `Master ${info.master}` : "";
    details.append(
      h(
        "div",
        { class: "insp-head-wrap" },
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
      ),
    );
    this.flags(info);
    if (info.edit) {
      details.append(...editForm(info, this.tokens, set, { omitText: guided }).flat());
    }
    if (info.box) this.box(info.box, details);
    this.attributes(info.attributes ?? [], details);
    if (info.style) this.style(info.style, details);
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

  box(b, target) {
    const cells = [
      ["X", b.x],
      ["Y", b.y],
      ["W", b.w],
      ["H", b.h],
    ];
    if (b.rotate !== undefined && b.rotate !== null) cells.push(["R", `${num(b.rotate)}°`]);
    if (Math.abs(b.angle ?? 0) > 1e-9) cells.push(["∠", `${num(b.angle)}°`]);
    target.append(
      h(
        "details",
        { class: "insp-sec insp-details", open: true },
        h("summary", { text: "Drawn box (px)" }),
        h(
          "div",
          { class: "box-grid" },
          cells.map(([k, v]) =>
            h("div", {}, h("span", { text: k }), h("span", { text: typeof v === "number" ? num(v) : v })),
          ),
        ),
      ),
    );
  }

  attributes(list, target) {
    const sec = h("details", { class: "insp-sec insp-details", open: true }, h("summary", { text: "Attributes" }));
    if (list.length === 0) {
      sec.appendChild(h("p", { class: "empty", text: "No attributes." }));
    } else {
      const dl = h("dl", { class: "kv" });
      for (const a of list) {
        dl.append(h("dt", { text: a.name }), h("dd", {}, value(a)));
      }
      sec.appendChild(dl);
    }
    target.appendChild(sec);
  }

  style(s, target) {
    const label = s.source === "style" ? `Style ${s.id}` : `Defaults for ${s.id}`;
    const sec = h("details", { class: "insp-sec insp-details", open: true }, h("summary", { text: label }));
    target.appendChild(sec);
    if (!s.defined) {
      sec.appendChild(h("p", { class: "empty", text: `${s.id} is not defined.` }));
      return;
    }
    const entries = Object.entries(s.properties ?? {});
    if (entries.length === 0) {
      sec.appendChild(h("p", { class: "empty", text: "No properties." }));
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
    sec.appendChild(dl);
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
