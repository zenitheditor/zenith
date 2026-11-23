// Engine-driven Undo / Redo with a Photoshop-style History popover:
// - Click Undo (Cmd/Ctrl+Z) = 1 step back (`history.undo`).
// - Press and hold Undo (~450 ms), right-click, or ArrowDown = History popover
//   populated from `history.list`; clicking any step jumps via `history.goto`.
// - Redo (Shift+Cmd/Ctrl+Z / Ctrl+Y) = 1 step forward (`history.redo`), enabled
//   only while behind the newest step. No toasts for edits.

import { byId, clear, h } from "../util/dom.js";
import { icon } from "./icons.js";

const HOLD_MS = 450;

const LABELS = {
  "doc.open": ["Opened", "page"],
  "buffer.set": ["Edit source", "code"],
  "gesture.commit": ["Transform", "pointer"],
  "node.set": ["Edit property", "sliders"],
  "node.add": ["Add layer", "plus"],
  "node.remove": ["Delete", "trash"],
  "node.duplicate": ["Duplicate", "copy"],
  "node.reorder": ["Arrange", "layers"],
  "node.group": ["Group", "group"],
  "node.ungroup": ["Ungroup", "group"],
  "doc.format": ["Format", "code"],
  "tx.apply": ["Apply edit", "edit"],
};

function labelFor(rawLabel, docName) {
  if (rawLabel === "doc.open") return { text: "Opened", detail: docName, ic: "page" };
  const mapped = LABELS[rawLabel];
  if (mapped) return { text: mapped[0], detail: "", ic: mapped[1] };
  return { text: rawLabel || "Edit", detail: "", ic: "edit" };
}

export class HistoryUI {
  constructor(app) {
    this.app = app;
    this.undoBtn = byId("undo");
    this.redoBtn = byId("redo");
    this.pop = byId("history");
    this.listEl = byId("history-list");
    this.countEl = byId("history-count");
    this.data = { index: 0, count: 0, can_undo: false, can_redo: false, entries: [] };
    this.holdTimer = null;
    this.held = false;
    this.bind();
  }

  bind() {
    const ub = this.undoBtn;
    const rb = this.redoBtn;
    if (!ub || !rb) return;

    ub.addEventListener("pointerdown", (e) => {
      if (e.button !== 0) return;
      this.held = false;
      ub.classList.add("pressing");
      this.holdTimer = setTimeout(() => {
        this.held = true;
        ub.classList.remove("pressing");
        this.open(false);
      }, HOLD_MS);
    });
    const release = (fire) => {
      clearTimeout(this.holdTimer);
      ub.classList.remove("pressing");
      if (fire && !this.held && this.pop.hidden && !ub.disabled) {
        this.app.history("history.undo");
      }
    };
    ub.addEventListener("pointerup", () => release(true));
    ub.addEventListener("pointerleave", () => release(false));
    ub.addEventListener("pointercancel", () => release(false));
    ub.addEventListener("contextmenu", (e) => {
      e.preventDefault();
      clearTimeout(this.holdTimer);
      ub.classList.remove("pressing");
      this.open(false);
    });
    ub.addEventListener("keydown", (e) => {
      if (e.key === "ArrowDown" || (e.key === "Enter" && e.altKey)) {
        e.preventDefault();
        this.open(true);
      }
    });
    ub.addEventListener("click", (e) => {
      if (e.detail === 0 && !ub.disabled) this.app.history("history.undo");
    });
    rb.addEventListener("click", () => {
      if (!rb.disabled) this.app.history("history.redo");
    });
    document.addEventListener("pointerdown", (e) => {
      if (!this.pop.hidden && !e.target.closest("#history, #undo")) this.close();
    });
  }

  isOpen() {
    return this.pop && !this.pop.hidden;
  }

  async open(focusCurrent = false) {
    if (!this.pop) return;
    this.app.layout?.closePopovers?.(this.pop);
    await this.refresh();
    this.pop.hidden = false;
    this.undoBtn?.setAttribute("aria-expanded", "true");
    this.render();
    if (focusCurrent) this.listEl?.querySelector(".h-step.cur")?.focus();
  }

  close() {
    if (!this.pop || this.pop.hidden) return;
    this.pop.hidden = true;
    this.undoBtn?.setAttribute("aria-expanded", "false");
  }

  async refresh() {
    const env = await this.app.engine.run("history.list", {});
    if (!env.ok) return;
    this.data = env.result;
    this.syncButtons();
    if (!this.pop?.hidden) this.render();
  }

  syncButtons() {
    const d = this.data;
    if (this.undoBtn) {
      this.undoBtn.disabled = !d.can_undo;
      const cur = d.entries?.[d.index];
      const lbl = cur && d.index > 0 ? labelFor(cur.label, this.app.name).text.toLowerCase() : null;
      this.undoBtn.title = lbl ? `Undo ${lbl} (Ctrl+Z) · Hold for history` : "Nothing to undo · Hold for history";
    }
    if (this.redoBtn) {
      this.redoBtn.disabled = !d.can_redo;
      const next = d.entries?.[d.index + 1];
      const lbl = next ? labelFor(next.label, this.app.name).text.toLowerCase() : null;
      this.redoBtn.title = lbl ? `Redo ${lbl} (Ctrl+Shift+Z)` : "Nothing to redo";
    }
  }

  render() {
    if (!this.listEl) return;
    clear(this.listEl);
    const d = this.data;
    if (this.countEl) this.countEl.textContent = `${d.index} of ${d.count}`;
    for (const item of d.entries ?? []) {
      const info = labelFor(item.label, this.app.name);
      const detail = info.detail || (item.selection?.length ? item.selection.join(", ") : "");
      const cls = ["h-step", item.current && "cur", item.ahead && "ahead"].filter(Boolean).join(" ");
      this.listEl.appendChild(
        h(
          "button",
          {
            type: "button",
            class: cls,
            onclick: () => this.goto(item.index),
          },
          icon(info.ic, "icon"),
          h("span", { class: "h-label", text: info.text }),
          detail ? h("span", { class: "h-detail", text: detail }) : null,
        ),
      );
    }
    this.listEl.querySelector(".cur")?.scrollIntoView({ block: "nearest" });
  }

  async goto(index) {
    if (index === this.data.index) return;
    await this.app.historyGoto(index);
    await this.refresh();
  }
}
