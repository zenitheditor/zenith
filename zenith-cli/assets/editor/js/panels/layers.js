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
    this.query = "";
    // Tree items by node id, rebuilt with the tree.
    this.items = new Map();
    treeEl.addEventListener("keydown", (e) => this.onKey(e));
    const filterEl = document.getElementById("layers-filter");
    filterEl?.addEventListener("input", () => {
      this.query = filterEl.value.trim().toLowerCase();
      this.drawTree();
    });
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
    const bar = this.pagesEl.previousElementSibling?.id === "pages-bar" ? this.pagesEl.previousElementSibling : null;
    if (bar && this.handlers.addPage) {
      clear(bar);
      bar.append(
        h(
          "button",
          { type: "button", class: "page-act-btn", title: "Add page", onclick: () => this.handlers.addPage() },
          icon("plus"),
          h("span", { text: "Add" }),
        ),
        h(
          "button",
          {
            type: "button",
            class: "page-act-btn",
            title: "Duplicate current page",
            disabled: !pages.length,
            onclick: () => this.handlers.duplicatePage?.(),
          },
          icon("copy"),
          h("span", { text: "Dup" }),
        ),
        h(
          "button",
          {
            type: "button",
            class: "page-act-btn",
            title: "Delete current page",
            disabled: pages.length <= 1,
            onclick: () => this.handlers.deletePage?.(),
          },
          icon("trash"),
          h("span", { text: "Del" }),
        ),
      );
    }
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
    const matchesQuery = (node) => {
      if (!this.query) return true;
      const hay = `${node.id ?? ""} ${node.name ?? ""} ${node.kind ?? ""}`.toLowerCase();
      if (hay.includes(this.query)) return true;
      return (node.children ?? []).some(matchesQuery);
    };
    const filteredChildren = this.query ? (page.children ?? []).filter(matchesQuery) : (page.children ?? []);
    const items = [];
    this.buildChildren(filteredChildren, 1, page.id, items);
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
    const quickBtns =
      o.id !== undefined && (this.handlers.toggleVisible || this.handlers.toggleLocked)
        ? h(
            "span",
            { class: `row-flags${o.locked || o.hidden ? " pinned" : ""}` },
            this.handlers.toggleLocked
              ? h(
                  "button",
                  {
                    type: "button",
                    class: `layer-quick-btn${o.locked ? " on" : ""}`,
                    title: o.locked ? "Unlock layer" : "Lock layer",
                    "aria-label": `${o.locked ? "Unlock" : "Lock"} ${o.label}`,
                    onclick: (e) => {
                      e.stopPropagation();
                      this.handlers.toggleLocked(o.id, !o.locked);
                    },
                  },
                  icon(o.locked ? "lock" : "unlock"),
                )
              : null,
            this.handlers.toggleVisible
              ? h(
                  "button",
                  {
                    type: "button",
                    class: `layer-quick-btn${o.hidden ? " on" : ""}`,
                    title: o.hidden ? "Show layer" : "Hide layer",
                    "aria-label": `${o.hidden ? "Show" : "Hide"} ${o.label}`,
                    onclick: (e) => {
                      e.stopPropagation();
                      this.handlers.toggleVisible(o.id, Boolean(o.hidden));
                    },
                  },
                  icon(o.hidden ? "hidden" : "eye"),
                )
              : null,
          )
        : h(
            "span",
            { class: "row-flags" },
            o.locked ? icon("lock") : null,
            o.hidden ? icon("hidden") : null,
          );
    const row = h(
      "div",
      {
        class: `row${o.id === undefined && !o.key.startsWith("master:") ? " dim" : ""}${o.hidden ? " dim" : ""}`,
        style: `--depth:${o.depth - 1}`,
        title: o.kind ? `${o.kind}${o.id ? ` ${o.id}` : ""}` : o.label,
        draggable: o.id !== undefined && this.handlers.reorderLayer ? "true" : undefined,
        ondragstart: (e) => {
          if (o.id === undefined) return;
          e.dataTransfer?.setData("application/x-zenith-layer", o.id);
          e.dataTransfer.effectAllowed = "move";
        },
        ondragover: (e) => {
          if (o.id !== undefined && this.handlers.reorderLayer && e.dataTransfer?.types?.includes("application/x-zenith-layer")) {
            e.preventDefault();
            e.currentTarget.classList.add("drop-before");
          }
        },
        ondragleave: (e) => {
          e.currentTarget.classList.remove("drop-before");
        },
        ondrop: (e) => {
          e.currentTarget.classList.remove("drop-before");
          const srcId = e.dataTransfer?.getData("application/x-zenith-layer");
          if (srcId && o.id && srcId !== o.id && this.handlers.reorderLayer) {
            e.preventDefault();
            this.handlers.reorderLayer(srcId, o.id);
          }
        },
        onclick: (e) => {
          if (e.target.closest(".twisty") && !o.leaf) this.toggle(o.key);
          else this.activate(o, e);
        },
        oncontextmenu: (e) => {
          if (o.id === undefined || !this.handlers.contextMenu) return;
          e.preventDefault();
          if (!this.selection.includes(o.id)) this.handlers.select(o.id);
          this.handlers.contextMenu(o.id, e);
        },
      },
      h("span", { class: `twisty${o.leaf ? " leaf" : ""}` }, icon("chevron")),
      icon(o.iconName),
      h("span", { class: "row-label", text: o.label }),
      o.sub ? h("span", { class: "row-meta", text: o.sub }) : null,
      quickBtns,
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

  /** Collapse runs of >= 4 consecutive same-kind siblings sharing a `prefix.NN` id. */
  buildChildren(children, depth, prefix, out) {
    const runPrefix = (node) => {
      if (!node?.id || node.children?.length) return null;
      const m = /^([a-z][\w-]*)\.\d+$/i.exec(node.id);
      return m ? `${node.kind}:${m[1]}` : null;
    };
    let i = 0;
    while (i < children.length) {
      const rp = runPrefix(children[i]);
      if (rp) {
        let j = i + 1;
        while (j < children.length && runPrefix(children[j]) === rp) j++;
        if (j - i >= 4) {
          const run = children.slice(i, j);
          const name = rp.split(":")[1];
          const key = `run:${prefix}/${rp}/${i}`;
          // Collapsed by default unless explicitly expanded.
          const open = this.expandedRuns?.has(key) ?? false;
          const memberIds = run.map((c) => c.id).filter(Boolean);
          const label = name[0].toUpperCase() + name.slice(1);
          out.push(
            this.wrap(
              {
                key,
                runIds: memberIds,
                label,
                sub: String(run.length),
                iconName: "group",
                kind: "group",
                depth,
                open,
                leaf: false,
              },
              open ? run.map((c, k) => this.item(c, depth + 1, `${key}/${k}`)) : [],
            ),
          );
          i = j;
          continue;
        }
      }
      out.push(this.item(children[i], depth, `${prefix}/${i}`));
      i++;
    }
  }

  activate(o, e = null) {
    this.focus(o.key);
    if (o.runIds?.length && this.handlers.selectMany) this.handlers.selectMany(o.runIds);
    else if (o.id !== undefined) {
      if (e && (e.shiftKey || e.metaKey || e.ctrlKey) && this.handlers.selectMany) {
        const next = this.selection.includes(o.id)
          ? this.selection.filter((x) => x !== o.id)
          : [...this.selection, o.id];
        this.handlers.selectMany(next);
      } else {
        this.handlers.select(o.id);
      }
    } else if (!o.leaf) this.toggle(o.key);
  }

  toggle(key) {
    if (key.startsWith("run:")) {
      this.expandedRuns ??= new Set();
      if (this.expandedRuns.has(key)) this.expandedRuns.delete(key);
      else this.expandedRuns.add(key);
    } else if (this.collapsed.has(key)) this.collapsed.delete(key);
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
