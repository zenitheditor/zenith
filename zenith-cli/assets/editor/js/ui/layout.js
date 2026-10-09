// Layout: the split divider, fullscreen modes, collapsible panels, and the
// theme override. The state persists per viewer in localStorage; a page
// without storage starts from the defaults.

import { byId } from "../util/dom.js";
import { load, save } from "../util/store.js";
import { icon } from "./icons.js";

const DEFAULTS = { split: 0.5, left: true, right: true, bottom: true, theme: "system" };
const MIN_PANE_PX = 220;
const MIN_PANE_PX_STACKED = 140;
const STEP = 0.02;
const BIG_STEP = 0.1;
const THEMES = ["system", "light", "dark"];

export class Layout {
  constructor() {
    this.app = byId("app");
    this.center = byId("center");
    this.divider = byId("divider");
    this.stackedQuery = window.matchMedia("(max-width: 720px)");
    const stored = load("layout", {});
    this.state = { ...DEFAULTS };
    for (const key of Object.keys(DEFAULTS)) {
      if (typeof stored[key] === typeof DEFAULTS[key]) this.state[key] = stored[key];
    }
    if (!THEMES.includes(this.state.theme)) this.state.theme = "system";
    this.full = "none";
    this.listeners = [];
    this.bind();
    this.apply();
  }

  /** Run `fn` after every layout change (resize, panels, fullscreen). */
  onChange(fn) {
    this.listeners.push(fn);
  }

  stacked() {
    return this.stackedQuery.matches;
  }

  bind() {
    const toggles = { left: "toggle-left", right: "toggle-right", bottom: "toggle-bottom" };
    for (const [panel, id] of Object.entries(toggles)) {
      byId(id).addEventListener("click", () => this.toggle(panel));
    }
    byId("full-code").addEventListener("click", () => this.setFull(this.full === "code" ? "none" : "code"));
    byId("full-canvas").addEventListener("click", () =>
      this.setFull(this.full === "canvas" ? "none" : "canvas"),
    );
    byId("theme").addEventListener("click", () => this.cycleTheme());
    document.addEventListener("keydown", (e) => {
      if (e.key === "Escape" && this.full !== "none" && !e.defaultPrevented) {
        this.setFull("none");
        e.preventDefault();
      }
    });
    this.stackedQuery.addEventListener("change", () => this.apply());
    this.bindDivider();
  }

  bindDivider() {
    const d = this.divider;
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
      const f = this.stacked() ? (e.clientY - r.top) / r.height : (e.clientX - r.left) / r.width;
      this.setSplit(f, { persist: false });
    });
    const end = (e) => {
      if (!dragging) return;
      dragging = false;
      d.classList.remove("dragging");
      if (d.hasPointerCapture(e.pointerId)) d.releasePointerCapture(e.pointerId);
      this.persist();
    };
    d.addEventListener("pointerup", end);
    d.addEventListener("pointercancel", end);
    d.addEventListener("dblclick", () => this.setSplit(DEFAULTS.split));
    d.addEventListener("keydown", (e) => {
      const step = e.shiftKey ? BIG_STEP : STEP;
      const [less, more] = this.stacked() ? ["ArrowUp", "ArrowDown"] : ["ArrowLeft", "ArrowRight"];
      const [min, max] = this.bounds();
      let next = null;
      if (e.key === less) next = this.state.split - step;
      else if (e.key === more) next = this.state.split + step;
      else if (e.key === "Home") next = min;
      else if (e.key === "End") next = max;
      else if (e.key === "Enter") next = DEFAULTS.split;
      if (next === null) return;
      e.preventDefault();
      this.setSplit(next);
    });
  }

  /** The smallest and largest split that keep both panes usable. */
  bounds() {
    const r = this.center.getBoundingClientRect();
    const size = this.stacked() ? r.height : r.width;
    const minPx = this.stacked() ? MIN_PANE_PX_STACKED : MIN_PANE_PX;
    if (size <= 0) return [0.2, 0.8];
    const min = Math.min(0.5, minPx / size);
    return [min, 1 - min];
  }

  setSplit(fraction, { persist = true } = {}) {
    const [min, max] = this.bounds();
    this.state.split = Math.min(max, Math.max(min, fraction));
    this.applySplit();
    if (persist) this.persist();
    this.changed();
  }

  toggle(panel) {
    this.state[panel] = !this.state[panel];
    this.apply();
    this.persist();
  }

  /** `"none"`, `"code"`, or `"canvas"`. */
  setFull(mode) {
    const was = this.full;
    this.full = mode;
    this.apply();
    if (mode === "code") byId("code-host").querySelector(".cm-content")?.focus();
    else if (mode === "canvas") byId("viewport").focus();
    else if (was === "code") byId("full-code").focus();
    else if (was === "canvas") byId("full-canvas").focus();
  }

  cycleTheme() {
    const i = THEMES.indexOf(this.state.theme);
    this.state.theme = THEMES[(i + 1) % THEMES.length];
    this.apply();
    this.persist();
  }

  apply() {
    const s = this.state;
    this.app.dataset.left = s.left ? "open" : "closed";
    this.app.dataset.right = s.right ? "open" : "closed";
    this.app.dataset.bottom = s.bottom ? "open" : "closed";
    this.app.dataset.full = this.full;
    byId("toggle-left").setAttribute("aria-pressed", String(s.left));
    byId("toggle-right").setAttribute("aria-pressed", String(s.right));
    byId("toggle-bottom").setAttribute("aria-pressed", String(s.bottom));
    for (const [id, mode, label] of [
      ["full-code", "code", "source"],
      ["full-canvas", "canvas", "canvas"],
    ]) {
      const on = this.full === mode;
      const button = byId(id);
      button.setAttribute("aria-pressed", String(on));
      button.title = on ? "Restore layout (Esc)" : `Fullscreen ${label} (Esc restores)`;
      button.querySelector(".sr-only").textContent = on ? "Restore layout" : `Fullscreen ${label}`;
      const svg = button.querySelector("svg");
      const want = on ? "collapse" : "expand";
      if (svg && svg.dataset.icon !== want) {
        const next = icon(want);
        next.dataset.icon = want;
        svg.replaceWith(next);
      }
    }
    if (s.theme === "system") document.documentElement.removeAttribute("data-theme");
    else document.documentElement.dataset.theme = s.theme;
    const themeLabel = `Theme: ${s.theme}`;
    byId("theme").title = themeLabel;
    byId("theme-label").textContent = themeLabel;
    this.divider.setAttribute("aria-orientation", this.stacked() ? "horizontal" : "vertical");
    this.applySplit();
    this.changed();
  }

  applySplit() {
    this.center.style.setProperty("--split", String(this.state.split));
    this.divider.setAttribute("aria-valuenow", String(Math.round(this.state.split * 100)));
    this.divider.setAttribute("aria-valuetext", `Source ${Math.round(this.state.split * 100)} percent`);
  }

  changed() {
    for (const fn of this.listeners) fn();
  }

  persist() {
    save("layout", this.state);
  }
}
