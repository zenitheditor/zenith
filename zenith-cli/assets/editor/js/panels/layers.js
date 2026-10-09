// The pages list and the layers tree, from `doc.outline`.
//
// The tree follows the ARIA tree pattern: one tab stop, arrows move,
// Right/Left expand and collapse, Enter or Space selects. Nodes without an
// id show dimmed and cannot be selected.

import { clear, h } from "../util/dom.js";
import { icon, kindIcon } from "../ui/icons.js";

export class Layers {
  /** `handlers`: `page(n)`, `select(id)`. */
  constructor(pagesEl, treeEl, handlers) {
    this.pagesEl = pagesEl;
    this.treeEl = treeEl;
    this.handlers = handlers;
    this.outline = null;
    this.page = 1;
    this.selection = [];
    this.collapsed = new Set();
    this.focusKey = null;
    this.outlineKey = null;
    // Tree items by node id, rebuilt with the tree.
    this.items = new Map();
    treeEl.addEventListener("keydown", (e) => this.onKey(e));
  }

  setOutline(outline) {
    // Typing refreshes the outline often; skip the redraw when it is the same.
    const key = JSON.stringify(outline);
    if (key === this.outlineKey) return;
    this.outlineKey = key;
    this.outline = outline;
    this.draw();
  }

  setPage(page) {
    if (page === this.page) return;
    this.page = page;
    this.draw();
  }

  /** Mark `ids` selected without rebuilding the tree. */
  setSelection(ids) {
    this.selection = ids;
    for (const el of this.treeEl.querySelectorAll('[aria-selected="true"]')) {
      el.setAttribute("aria-selected", "false");
    }
    let first = null;
    for (const id of ids) {
      const el = this.items.get(id);
      if (!el) continue;
      el.setAttribute("aria-selected", "true");
      first ??= el;
    }
    first?.querySelector(":scope > .row")?.scrollIntoView({ block: "nearest" });
  }

  draw() {
    this.drawPages();
    this.drawTree();
  }

  drawPages() {
    clear(this.pagesEl);
    const pages = this.outline?.pages ?? [];
    for (const p of pages) {
      const current = p.page === this.page;
      this.pagesEl.appendChild(
        h(
          "li",
          {},
          h(
            "button",
            {
              type: "button",
              class: "row",
              "aria-current": current ? "page" : false,
              onclick: () => this.handlers.page(p.page),
            },
            icon("page"),
            h("span", { class: "row-label", text: p.name || p.id }),
            h("span", { class: "row-meta", text: `${Math.round(p.w)}×${Math.round(p.h)}` }),
          ),
        ),
      );
    }
  }

  drawTree() {
    const hadFocus = this.treeEl.contains(document.activeElement);
    clear(this.treeEl);
    this.items.clear();
    const page = this.outline?.pages?.find((p) => p.page === this.page);
    if (!page) {
      this.treeEl.appendChild(h("p", { class: "empty", text: "No page to show." }));
      return;
    }
    const items = [];
    (page.children ?? []).forEach((child, i) => items.push(this.item(child, 1, `${page.id}/${i}`)));
    const master = page.master ? this.outline.masters?.find((m) => m.id === page.master) : null;
    if (master) {
      const key = `master:${master.id}`;
      const open = !this.collapsed.has(key);
      items.push(
        this.wrap(
          { key, label: `Master ${master.id}`, iconName: "layers", depth: 1, open, leaf: false },
          open ? master.children.map((c, i) => this.item(c, 2, `${key}/${i}`)) : [],
        ),
      );
    }
    if (items.length === 0) {
      this.treeEl.appendChild(h("p", { class: "empty", text: "This page has no nodes." }));
      return;
    }
    this.treeEl.append(...items);
    const focusable = this.treeEl.querySelectorAll('[role="treeitem"]');
    const target =
      [...focusable].find((n) => n.dataset.key === this.focusKey) ??
      this.treeEl.querySelector('[aria-selected="true"]') ??
      focusable[0];
    if (target) {
      target.tabIndex = 0;
      if (hadFocus) target.focus({ preventScroll: true });
    }
  }

  item(layer, depth, path) {
    const key = layer.id ?? path;
    const leaf = !(layer.children?.length > 0);
    const open = !this.collapsed.has(key);
    const children = !leaf && open ? layer.children.map((c, i) => this.item(c, depth + 1, `${key}/${i}`)) : [];
    return this.wrap(
      {
        key,
        id: layer.id,
        label: layer.name ? `${layer.name}` : (layer.id ?? `(${layer.kind} without id)`),
        sub: layer.name && layer.id ? layer.id : null,
        iconName: kindIcon(layer.kind),
        kind: layer.kind,
        depth,
        open,
        leaf,
        hidden: !layer.visible,
        locked: layer.locked,
        guide: layer.guide,
      },
      children,
    );
  }

  wrap(o, children) {
    const selected = o.id !== undefined && this.selection.includes(o.id);
    const row = h(
      "div",
      {
        class: `row${o.id === undefined && !o.key.startsWith("master:") ? " dim" : ""}${o.hidden ? " dim" : ""}`,
        style: `--depth:${o.depth - 1}`,
        title: o.kind ? `${o.kind}${o.id ? ` ${o.id}` : ""}` : o.label,
        onclick: (e) => {
          if (e.target.closest(".twisty") && !o.leaf) this.toggle(o.key);
          else this.activate(o);
        },
      },
      h("span", { class: `twisty${o.leaf ? " leaf" : ""}` }, icon("chevron")),
      icon(o.iconName),
      h("span", { class: "row-label", text: o.label }),
      o.sub ? h("span", { class: "row-meta", text: o.sub }) : null,
      h(
        "span",
        { class: "row-flags" },
        o.locked ? icon("lock") : null,
        o.hidden ? icon("hidden") : null,
      ),
    );
    const flags = [o.locked ? "locked" : null, o.hidden ? "hidden" : null, o.guide ? "guide" : null]
      .filter(Boolean)
      .join(", ");
    const item = h(
      "div",
      {
        role: "treeitem",
        tabindex: "-1",
        "aria-level": String(o.depth),
        "aria-selected": o.id !== undefined ? String(selected) : false,
        "aria-expanded": o.leaf ? false : String(o.open),
        "aria-label": `${o.kind ?? ""} ${o.label}${flags ? `, ${flags}` : ""}`.trim(),
        dataset: { key: o.key, id: o.id ?? "" },
      },
      row,
      children.length ? h("div", { role: "group" }, children) : null,
    );
    if (o.id !== undefined) this.items.set(o.id, item);
    return item;
  }

  activate(o) {
    this.focus(o.key);
    if (o.id !== undefined) this.handlers.select(o.id);
    else if (!o.leaf) this.toggle(o.key);
  }

  toggle(key) {
    if (this.collapsed.has(key)) this.collapsed.delete(key);
    else this.collapsed.add(key);
    this.focusKey = key;
    this.drawTree();
    this.focus(key);
  }

  focus(key) {
    const el = [...this.treeEl.querySelectorAll('[role="treeitem"]')].find((n) => n.dataset.key === key);
    if (!el) return;
    for (const n of this.treeEl.querySelectorAll('[role="treeitem"]')) n.tabIndex = -1;
    el.tabIndex = 0;
    el.focus();
    this.focusKey = key;
  }

  onKey(e) {
    const current = e.target.closest('[role="treeitem"]');
    if (!current) return;
    const items = [...this.treeEl.querySelectorAll('[role="treeitem"]')];
    const i = items.indexOf(current);
    const key = current.dataset.key;
    const expanded = current.getAttribute("aria-expanded");
    const move = (el) => el && this.focus(el.dataset.key);
    switch (e.key) {
      case "ArrowDown":
        move(items[i + 1]);
        break;
      case "ArrowUp":
        move(items[i - 1]);
        break;
      case "Home":
        move(items[0]);
        break;
      case "End":
        move(items[items.length - 1]);
        break;
      case "ArrowRight":
        if (expanded === "false") this.toggle(key);
        else if (expanded === "true") move(items[i + 1]);
        break;
      case "ArrowLeft":
        if (expanded === "true") this.toggle(key);
        else move(current.parentElement?.closest('[role="treeitem"]'));
        break;
      case "Enter":
      case " ": {
        const id = current.dataset.id;
        if (id) this.handlers.select(id);
        else if (expanded !== null) this.toggle(key);
        this.focusKey = key;
        break;
      }
      default:
        return;
    }
    e.preventDefault();
  }
}
