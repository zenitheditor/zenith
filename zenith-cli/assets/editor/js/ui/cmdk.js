// Command palette (Cmd/Ctrl+K): commands, plus layers and tokens when typing.

import { byId, clear, h } from "../util/dom.js";
import { icon, kindIcon } from "./icons.js";

export class CommandPalette {
  constructor(app) {
    this.app = app;
    this.dlg = byId("cmdk");
    this.input = byId("cmdk-q");
    this.list = byId("cmdk-list");
    this.items = [];
    this.sel = 0;
    this.bind();
  }

  bind() {
    if (!this.dlg || !this.input || !this.list) return;
    this.dlg.addEventListener("click", (e) => {
      if (e.target === this.dlg) this.dlg.close();
    });
    this.input.addEventListener("input", () => {
      this.sel = 0;
      this.render();
    });
    this.input.addEventListener("keydown", (e) => {
      if (e.key === "ArrowDown") {
        this.sel = Math.min(this.items.length - 1, this.sel + 1);
        this.render();
        e.preventDefault();
      } else if (e.key === "ArrowUp") {
        this.sel = Math.max(0, this.sel - 1);
        this.render();
        e.preventDefault();
      } else if (e.key === "Enter") {
        e.preventDefault();
        this.run(this.sel);
      }
    });
  }

  open() {
    if (!this.dlg) return;
    this.input.value = "";
    this.sel = 0;
    this.render();
    this.dlg.showModal();
    this.input.focus();
  }

  baseCommands() {
    const app = this.app;
    const cmds = [
      ["Create", "Move tool", "pointer", "V", () => app.tools?.set("move")],
      ["Create", "Text", "text", "T", () => app.tools?.set("text")],
      ["Create", "Rectangle", "rect", "R", () => app.tools?.set("rect")],
      ["Create", "Ellipse", "ellipse", "O", () => app.tools?.set("ellipse")],
      ["Create", "Line", "line", "L", () => app.tools?.set("line")],
      ["Create", "Hand pan", "hand", "H", () => app.tools?.set("hand")],
      ["View", "Design view", "layout", "⌘1", () => app.layout.setView("design")],
      ["View", "Split view", "columns", "⌘2", () => app.layout.setView("split")],
      ["View", "Code view", "code", "⌘3", () => app.layout.setView("code")],
      ["View", "Toggle left panel", "panelLeft", "⌘\\", () => app.layout.toggle("left")],
      ["View", "Fit page", "fit", "⇧1", () => app.view.fit()],
      ["View", "Zoom 100%", "zoomIn", "⇧0", () => app.view.actualSize()],
      ["View", "Toggle theme", "theme", "", () => app.layout.cycleTheme()],
      ["Edit", "Undo", "undo", "⌘Z", () => app.history("history.undo")],
      ["Edit", "Redo", "redo", "⇧⌘Z", () => app.history("history.redo")],
      ["Edit", "Duplicate", "copy", "⌘D", () => app.actions.duplicate()],
      ["Edit", "Format document", "code", "", () => app.formatDocument?.()],
      ["File", "Save", "save", "⌘S", () => app.save()],
    ];
    if (app.files) {
      cmds.push(
        ["File", "Open .zen file…", "page", "⌘O", () => app.files.openFile()],
        ["File", "Open project folder…", "folder", "", () => app.files.openFolder()],
        ["File", "Download .zen", "download", "", () => app.files.downloadCopy()],
        ["File", "Export PNG", "image", "⇧⌘E", () => app.files.exportPng()],
      );
    }
    return cmds;
  }

  layerCommands() {
    const out = [];
    const walk = (list) => {
      for (const n of list ?? []) {
        if (n.id) {
          const name = n.name || n.id;
          out.push(["Layers", name, kindIcon(n.kind), "", () => this.app.selection.selectIds([n.id], "cmdk")]);
        }
        if (n.children?.length) walk(n.children);
      }
    };
    const page = this.app.outline?.pages?.find((p) => p.page === this.app.page);
    if (page) walk(page.children);
    return out;
  }

  tokenCommands() {
    return (this.app.tokenList ?? []).map((t) => [
      "Tokens",
      t.id,
      "token",
      t.type,
      () => this.app.layout.selectLeftTab("tokens"),
    ]);
  }

  render() {
    const q = this.input.value.trim().toLowerCase();
    const pool = q ? [...this.baseCommands(), ...this.layerCommands(), ...this.tokenCommands()] : this.baseCommands();
    this.items = pool.filter((c) => c[1].toLowerCase().includes(q) || c[0].toLowerCase().includes(q));
    this.sel = Math.min(this.sel, Math.max(0, this.items.length - 1));
    clear(this.list);
    if (this.items.length === 0) {
      this.list.appendChild(h("div", { class: "empty-pop", text: "No matches" }));
      return;
    }
    let group = "";
    this.items.forEach((c, i) => {
      if (c[0] !== group) {
        group = c[0];
        this.list.appendChild(h("div", { class: "cmdk-group", text: group }));
      }
      this.list.appendChild(
        h(
          "button",
          {
            type: "button",
            class: `cmdk-item${i === this.sel ? " on" : ""}`,
            onclick: () => this.run(i),
          },
          icon(c[2], "icon"),
          h("span", { text: c[1] }),
          c[3] ? h("kbd", { text: c[3] }) : null,
        ),
      );
    });
  }

  run(index) {
    const item = this.items[index];
    if (!item) return;
    this.dlg.close();
    item[4]();
  }
}
