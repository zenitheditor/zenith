// Layout: Design | Split | Code view switch, collapsible left rail (with
// Layers/Pages/Tokens tabs and 721-860px tablet drawer), popovers, phone
// bottom sheets, split divider, and light-first theme toggle.

import { byId } from "../util/dom.js";
import { load, save } from "../util/store.js";

const DEFAULTS = { split: 0.5, left: true, right: true, theme: "system" };
const MIN_PANE_PX = 220;
const STEP = 0.02;
const BIG_STEP = 0.1;
const THEMES = ["system", "light", "dark"];

export class Layout {
  constructor({ starter = false } = {}) {
    this.app = byId("app");
    this.center = byId("center");
    this.divider = byId("divider");
    this.phoneQuery = window.matchMedia("(max-width: 720px)");
    this.tabletQuery = window.matchMedia("(max-width: 860px)");
    const stored = load("layout", {});
    this.state = { ...DEFAULTS, view: "design" };
    for (const key of Object.keys(DEFAULTS)) {
      if (typeof stored[key] === typeof DEFAULTS[key]) this.state[key] = stored[key];
    }
    if (!THEMES.includes(this.state.theme)) this.state.theme = "light";
    this.starter = starter ? { ...this.state, left: true, right: true, view: "design" } : null;
    this.full = "none";
    this.sheet = null;
    this.leftDrawer = false;
    this.listeners = [];
    this.sheetHandlers = null;
    this.historyClose = null;
    this.popovers = [
      ["main-menu-btn", "main-menu"],
      ["zoom-btn", "zoom-menu"],
      ["problems-chip", "problems-pop"],
    ];
    this.bind();
    this.apply();
  }

  /** Run `fn` after every layout change (resize, panels, view). */
  onChange(fn) {
    this.listeners.push(fn);
  }

  stacked() {
    return this.phoneQuery.matches;
  }

  bind() {
    byId("toggle-left")?.addEventListener("click", () => this.toggle("left"));
    byId("reopen-left")?.addEventListener("click", () => this.toggle("left"));
    byId("toggle-right")?.addEventListener("click", () => this.toggle("right"));
    byId("reopen-right")?.addEventListener("click", () => this.toggle("right"));
    byId("theme")?.addEventListener("click", () => {
      this.closePopovers();
      this.cycleTheme();
    });

    for (const btn of document.querySelectorAll(".seg [data-view]")) {
      btn.addEventListener("click", () => this.setView(btn.dataset.view));
    }
    for (const tab of document.querySelectorAll("[data-ltab]")) {
      tab.addEventListener("click", () => this.selectLeftTab(tab.dataset.ltab));
    }

    for (const [btnId, popId] of this.popovers) {
      const btn = byId(btnId);
      const pop = byId(popId);
      if (!btn || !pop) continue;
      btn.addEventListener("click", (e) => {
        e.stopPropagation();
        const opening = pop.hidden;
        this.closePopovers();
        pop.hidden = !opening;
        btn.setAttribute("aria-expanded", String(opening));
      });
    }
    document.addEventListener("click", (e) => {
      if (!e.target.closest(".menu, .popover")) this.closePopovers();
    });
    document.addEventListener("keydown", (e) => {
      if (e.key === "Escape" && !e.defaultPrevented) {
        if (this.anyPopoverOpen()) {
          this.closePopovers();
          e.preventDefault();
          return;
        }
        if (this.sheet) {
          this.closeSheet();
          e.preventDefault();
          return;
        }
        if (this.leftDrawer) {
          this.leftDrawer = false;
          this.apply();
          e.preventDefault();
        }
      }
    });

    // Phone bottom bar & sheet close
    for (const btn of document.querySelectorAll("#phone-bar [data-sheet]")) {
      btn.addEventListener("click", () => this.openSheet(btn.dataset.sheet));
    }
    byId("sheet-close")?.addEventListener("click", () => this.closeSheet());

    this.phoneQuery.addEventListener("change", () => {
      if (!this.phoneQuery.matches) this.closeSheet();
      this.apply();
    });
    this.tabletQuery.addEventListener("change", () => {
      if (!this.tabletQuery.matches) this.leftDrawer = false;
      this.apply();
    });
    this.bindDivider();
  }

  anyPopoverOpen() {
    for (const [, popId] of this.popovers) {
      const pop = byId(popId);
      if (pop && !pop.hidden) return true;
    }
    const hist = byId("history");
    return Boolean(hist && !hist.hidden);
  }

  closePopovers(except = null) {
    for (const [btnId, popId] of this.popovers) {
      const pop = byId(popId);
      const btn = byId(btnId);
      if (pop && pop !== except) pop.hidden = true;
      if (btn && pop !== except) btn.setAttribute("aria-expanded", "false");
    }
    const hist = byId("history");
    if (hist !== except) this.historyClose?.();
  }

  openProblems() {
    if (this.phoneQuery.matches) {
      this.openSheet("problems");
      return;
    }
    const pop = byId("problems-pop");
    const btn = byId("problems-chip");
    if (!pop || !btn) return;
    this.closePopovers(pop);
    pop.hidden = false;
    btn.setAttribute("aria-expanded", "true");
  }

  selectLeftTab(tab) {
    for (const btn of document.querySelectorAll("[data-ltab]")) {
      btn.setAttribute("aria-selected", String(btn.dataset.ltab === tab));
    }
    for (const body of document.querySelectorAll("[data-lbody]")) {
      body.hidden = body.dataset.lbody !== tab;
    }
    const s = this.starter ?? this.state;
    if (!s.left && !this.phoneQuery.matches) {
      s.left = true;
      this.apply();
    }
  }

  openSheet(kind) {
    const sheet = byId("sheet");
    if (!sheet) return;
    if (this.sheet === kind && !sheet.hidden) {
      this.closeSheet();
      return;
    }
    this.sheet = kind;
    sheet.hidden = false;
    for (const btn of document.querySelectorAll("#phone-bar [data-sheet]")) {
      btn.classList.toggle("on", btn.dataset.sheet === kind);
    }
    this.sheetHandlers?.render(kind);
  }

  closeSheet() {
    const sheet = byId("sheet");
    if (!sheet || sheet.hidden) return;
    sheet.hidden = true;
    this.sheet = null;
    for (const btn of document.querySelectorAll("#phone-bar [data-sheet]")) {
      btn.classList.remove("on");
    }
    this.sheetHandlers?.closed?.();
  }

  bindDivider() {
    const d = this.divider;
    if (!d) return;
    let dragging = false;
    d.addEventListener("pointerdown", (e) => {
      if (e.button !== 0) return;
      dragging = true;
      d.setPointerCapture(e.pointerId);
      d.classList.add("dragging");
      e.preventDefault();
    });
    d.addEventListener("pointermove", (e) => {
      if (!dragging) return;
      const r = this.center.getBoundingClientRect();
      const f = (e.clientX - r.left) / r.width;
      this.setSplit(f, { persist: false });
    });
    const end = (e) => {
      if (!dragging) return;
      dragging = false;
      d.classList.remove("dragging");
      if (d.hasPointerCapture(e.pointerId)) d.releasePointerCapture(e.pointerId);
      if (!this.starter) this.persist();
    };
    d.addEventListener("pointerup", end);
    d.addEventListener("pointercancel", end);
    d.addEventListener("dblclick", () => this.setSplit(DEFAULTS.split));
    d.addEventListener("keydown", (e) => {
      const step = e.shiftKey ? BIG_STEP : STEP;
      const [min, max] = this.bounds();
      let next = null;
      const split = (this.starter ?? this.state).split;
      if (e.key === "ArrowLeft") next = split - step;
      else if (e.key === "ArrowRight") next = split + step;
      else if (e.key === "Home") next = min;
      else if (e.key === "End") next = max;
      else if (e.key === "Enter") next = DEFAULTS.split;
      if (next === null) return;
      e.preventDefault();
      this.setSplit(next);
    });
  }

  bounds() {
    const r = this.center.getBoundingClientRect();
    const size = r.width;
    if (size <= 0) return [0.2, 0.8];
    const min = Math.min(0.5, MIN_PANE_PX / size);
    return [min, 1 - min];
  }

  setSplit(fraction, { persist = true } = {}) {
    const [min, max] = this.bounds();
    (this.starter ?? this.state).split = Math.min(max, Math.max(min, fraction));
    this.applySplit();
    if (persist && !this.starter) this.persist();
    this.changed();
  }

  setView(view) {
    if (!["design", "split", "code"].includes(view)) return;
    const s = this.starter ?? this.state;
    s.view = view;
    this.full = view === "code" ? "code" : "none";
    this.apply();
    if (view === "code") byId("code-host")?.querySelector(".cm-content")?.focus();
  }

  toggle(panel) {
    if (panel === "left" && this.tabletQuery.matches && !this.phoneQuery.matches) {
      this.leftDrawer = !this.leftDrawer;
      this.apply();
      return;
    }
    const state = this.starter ?? this.state;
    state[panel] = !state[panel];
    this.apply();
    if (!this.starter) this.persist();
  }

  showSource(on) {
    const s = this.starter ?? this.state;
    if (on) {
      if (s.view === "design") this.setView("split");
    } else if (s.view !== "design") {
      this.setView("design");
    }
  }

  resetDocument() {
    this.starter = null;
    this.state.view = "design";
    this.full = "none";
    this.leftDrawer = false;
    this.closeSheet();
    this.apply();
  }

  setFull(mode) {
    if (mode === "code") this.setView("code");
    else this.setView("design");
  }

  cycleTheme() {
    const i = THEMES.indexOf(this.state.theme);
    this.state.theme = THEMES[(i + 1) % THEMES.length];
    if (this.starter) this.starter.theme = this.state.theme;
    this.apply();
    this.persist();
  }

  apply() {
    const s = this.starter ?? this.state;
    const view = s.view || "design";
    const source = view === "split" || view === "code";
    this.app.dataset.view = view;
    this.app.dataset.left = s.left ? "open" : "closed";
    this.app.dataset.right = s.right ? "open" : "closed";
    this.app.dataset.source = source ? "open" : "closed";
    this.app.dataset.full = view === "code" ? "code" : "none";
    this.app.dataset.starter = String(Boolean(this.starter));
    this.app.classList.toggle("left-drawer", this.leftDrawer);

    const skip = byId("skip-source");
    if (skip) skip.hidden = !source;
    for (const btn of document.querySelectorAll(".seg [data-view]")) {
      btn.setAttribute("aria-checked", String(btn.dataset.view === view));
    }
    byId("toggle-left")?.setAttribute("aria-pressed", String(s.left));
    byId("toggle-right")?.setAttribute("aria-pressed", String(s.right));

    if (s.theme === "system") document.documentElement.removeAttribute("data-theme");
    else document.documentElement.dataset.theme = s.theme;
    const themeLabel = `Theme: ${s.theme}`;
    const themeBtn = byId("theme");
    if (themeBtn) themeBtn.title = themeLabel;
    const lbl = byId("theme-label");
    if (lbl) lbl.textContent = themeLabel;

    this.applySplit();
    this.changed();
  }

  applySplit() {
    const split = (this.starter ?? this.state).split;
    this.center.style.setProperty("--split", String(split));
    if (this.divider) {
      this.divider.setAttribute("aria-valuenow", String(Math.round(split * 100)));
      this.divider.setAttribute("aria-valuetext", `Source ${Math.round(split * 100)} percent`);
    }
  }

  changed() {
    for (const fn of this.listeners) fn();
  }

  persist() {
    const { split, left, right, theme } = this.state;
    save("layout", { split, left, right, theme });
  }
}
