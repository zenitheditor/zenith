// Document tab bar for the hosted WASM editor: Home button, open `.zen` tabs
// with close buttons, and `+` to create a new `.zen` tab. Switching tabs
// snapshots the current tab's source, project files, and file handle so you
// can work across multiple designs without losing edits.

import { byId, clear, h } from "../util/dom.js";
import { icon } from "./icons.js";

let nextTabId = 1;

export class TabBar {
  constructor(app) {
    this.app = app;
    this.bar = byId("tab-bar");
    this.list = byId("tab-list");
    this.tabs = [];
    this.activeId = null;
    if (!this.bar || !this.list || app.host.kind !== "wasm") return;

    this.bar.hidden = false;
    byId("tab-home")?.addEventListener("click", () => {
      app.openHomeOverlay?.();
    });
    byId("tab-new")?.addEventListener("click", () => {
      app.files?.newDocument();
    });
    document.addEventListener("keydown", (e) => {
      if (e.ctrlKey && !e.metaKey && !e.altKey && e.key === "Tab" && this.tabs.length > 1) {
        e.preventDefault();
        const i = this.tabs.findIndex((t) => t.id === this.activeId);
        const next = this.tabs[(i + (e.shiftKey ? -1 : 1) + this.tabs.length) % this.tabs.length];
        if (next) void this.switchTo(next.id);
      }
    });

    this.recordCurrent();
  }

  /** Capture or update the active tab from the currently mounted document. */
  recordCurrent() {
    if (!this.bar || this.bar.hidden) return;
    const app = this.app;
    const snap = this.snapshot();
    if (!this.activeId) {
      const id = `tab-${nextTabId++}`;
      this.tabs.push({ id, ...snap });
      this.activeId = id;
    } else {
      const cur = this.tabs.find((t) => t.id === this.activeId);
      if (cur) Object.assign(cur, snap);
    }
    this.render();
  }

  /** Snapshot the currently open document so it can be restored later. */
  snapshot() {
    const app = this.app;
    const files = app.host?.worker?.files ? { ...app.host.worker.files } : null;
    const handles = app.host?.handles ? new Map(app.host.handles) : new Map();
    return {
      name: app.name,
      path: app.path,
      text: app.code.text(),
      handle: app.host?.handle ?? null,
      project: files && Object.keys(files).length ? { root: app.host?.root ?? "", files, handles } : null,
      dirty: app.dirty(),
      draftSource: app.drafts?.active?.source ?? null,
    };
  }

  /** Save the active tab's live state before another document replaces it. */
  saveCurrent() {
    if (!this.bar || this.bar.hidden || !this.activeId) return;
    const cur = this.tabs.find((t) => t.id === this.activeId);
    if (cur) Object.assign(cur, this.snapshot());
  }

  /** Called right after opening a document into a new tab (or focusing an existing one). */
  openOrFocus(spec) {
    if (!this.bar || this.bar.hidden) return;
    const key = spec.path || spec.name;
    const existing = this.tabs.find((t) => (t.path || t.name) === key && !spec._forceNew);
    if (existing) {
      this.activeId = existing.id;
      Object.assign(existing, {
        name: spec.name,
        path: spec.path ?? spec.name,
        text: spec.text,
        handle: spec.handle ?? existing.handle ?? null,
        project: spec.project ?? existing.project ?? null,
        draftSource: spec.draftSource ?? existing.draftSource ?? null,
      });
    } else {
      const id = `tab-${nextTabId++}`;
      this.tabs.push({
        id,
        name: spec.name,
        path: spec.path ?? spec.name,
        text: spec.text,
        handle: spec.handle ?? null,
        project: spec.project ?? null,
        dirty: false,
        draftSource: spec.draftSource ?? null,
      });
      this.activeId = id;
    }
    this.render();
  }

  async switchTo(id) {
    if (id === this.activeId) return;
    const target = this.tabs.find((t) => t.id === id);
    if (!target || !this.app.files) return;
    const cur = this.tabs.find((t) => t.id === this.activeId);
    if (cur) Object.assign(cur, this.snapshot());
    this.activeId = id;
    this.render();
    await this.app.files.load(
      {
        name: target.name,
        path: target.path,
        text: target.text,
        handle: target.handle,
        project: target.project,
        draftSource: target.draftSource,
      },
      { fromTab: true },
    );
  }

  async closeTab(id) {
    if (this.tabs.length <= 1) return;
    const idx = this.tabs.findIndex((t) => t.id === id);
    if (idx < 0) return;
    this.tabs.splice(idx, 1);
    if (this.activeId === id) {
      const next = this.tabs[Math.max(0, idx - 1)];
      this.activeId = null;
      await this.switchTo(next.id);
    } else {
      this.render();
    }
  }

  syncActiveMeta() {
    const cur = this.tabs.find((t) => t.id === this.activeId);
    if (!cur) return;
    cur.name = this.app.name;
    cur.path = this.app.path;
    cur.dirty = this.app.dirty();
    this.render();
  }

  render() {
    if (!this.list || this.bar?.hidden) return;
    clear(this.list);
    for (const tab of this.tabs) {
      const active = tab.id === this.activeId;
      const closeBtn =
        this.tabs.length > 1
          ? h(
              "button",
              {
                type: "button",
                class: "doc-tab-close",
                "aria-label": `Close ${tab.name}`,
                title: `Close ${tab.name}`,
                onclick: (e) => {
                  e.stopPropagation();
                  void this.closeTab(tab.id);
                },
              },
              icon("close"),
            )
          : null;
      this.list.appendChild(
        h(
          "div",
          {
            role: "tab",
            tabindex: active ? "0" : "-1",
            "aria-selected": String(active),
            class: `doc-tab${active ? " on" : ""}`,
            title: tab.path || tab.name,
            onclick: () => void this.switchTo(tab.id),
            onauxclick: (e) => {
              if (e.button === 1 && this.tabs.length > 1) {
                e.preventDefault();
                void this.closeTab(tab.id);
              }
            },
          },
          icon("page"),
          h("span", { class: "doc-tab-name", text: tab.name }),
          tab.dirty ? h("span", { class: "doc-tab-dot", title: "Unsaved changes" }) : null,
          closeBtn,
        ),
      );
    }
  }
}
