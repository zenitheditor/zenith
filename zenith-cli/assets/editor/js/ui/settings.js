// Sidebar-tabbed Settings modal (General, Editor, Storage).

import { byId } from "../util/dom.js";
import { hydrateIcons } from "./icons.js";

export class SettingsDialog {
  constructor(app) {
    this.app = app;
    this.dlg = byId("settings-dlg");
    this.tab = "general";
    if (!this.dlg) return;
    this.bind();
  }

  bind() {
    byId("settings-close")?.addEventListener("click", () => this.dlg.close());
    this.dlg.addEventListener("click", (e) => {
      if (e.target === this.dlg) this.dlg.close();
    });
    for (const btn of this.dlg.querySelectorAll("[data-set-tab]")) {
      btn.addEventListener("click", () => {
        this.tab = btn.dataset.setTab;
        this.render();
      });
    }
    byId("set-theme")?.addEventListener("change", (e) => {
      const val = e.target.value;
      if (!["system", "light", "dark"].includes(val)) return;
      this.app.layout.state.theme = val;
      if (this.app.layout.starter) this.app.layout.starter.theme = val;
      this.app.layout.apply();
      this.app.layout.persist();
    });
    byId("set-snap")?.addEventListener("change", (e) => {
      const on = e.target.checked;
      this.app.gestures.setSnap(on);
      const snap = byId("snap-toggle");
      if (snap) {
        snap.setAttribute("aria-pressed", String(on));
        snap.classList.toggle("on", on);
      }
    });
    byId("set-left")?.addEventListener("change", (e) => {
      const s = this.app.layout.starter ?? this.app.layout.state;
      s.left = e.target.checked;
      this.app.layout.apply();
      if (!this.app.layout.starter) this.app.layout.persist();
    });
    byId("set-right")?.addEventListener("change", (e) => {
      const s = this.app.layout.starter ?? this.app.layout.state;
      s.right = e.target.checked;
      this.app.layout.apply();
      if (!this.app.layout.starter) this.app.layout.persist();
    });
    byId("set-open-drafts")?.addEventListener("click", () => {
      this.dlg.close();
      this.app.drafts?.show();
    });
  }

  async open(tab = "general") {
    if (!this.dlg) return;
    this.tab = tab;
    hydrateIcons(this.dlg);
    await this.render();
    if (!this.dlg.open) this.dlg.showModal();
  }

  async render() {
    for (const btn of this.dlg.querySelectorAll("[data-set-tab]")) {
      const on = btn.dataset.setTab === this.tab;
      btn.classList.toggle("on", on);
      btn.setAttribute("aria-selected", String(on));
    }
    for (const pane of this.dlg.querySelectorAll("[data-set-pane]")) {
      pane.hidden = pane.dataset.setPane !== this.tab;
    }

    const s = this.app.layout.starter ?? this.app.layout.state;
    const themeSel = byId("set-theme");
    if (themeSel) themeSel.value = s.theme || "system";
    const snapChk = byId("set-snap");
    if (snapChk) snapChk.checked = Boolean(this.app.gestures?.snapOn);
    const leftChk = byId("set-left");
    if (leftChk) leftChk.checked = Boolean(s.left);
    const rightChk = byId("set-right");
    if (rightChk) rightChk.checked = Boolean(s.right);

    const summary = byId("set-storage-summary");
    if (summary && this.app.drafts?.store) {
      try {
        const { records } = await this.app.drafts.store.list();
        const bytes = records.reduce((n, r) => n + (r.text?.length ?? 0), 0);
        const kb = (bytes / 1024).toFixed(1);
        summary.textContent = `${records.length} browser draft${records.length === 1 ? "" : "s"} (${kb} KB) stored locally in IndexedDB.`;
      } catch {
        summary.textContent = "Browser draft storage is currently unavailable.";
      }
    }
  }
}
