// The status bar, the title, the dirty mark, and the canvas badges.

import { byId, clear, plural } from "../util/dom.js";
import { icon } from "../ui/icons.js";

export class Status {
  constructor(app) {
    this.app = app;
    this.isSaving = false;
    this.conn = "Connecting";
    byId("doc-name").textContent = app.name;
    byId("doc-name").title = app.path;
  }

  update() {
    const app = this.app;
    const dirty = app.dirty();
    const hosted = app.host.kind === "wasm";
    const t = app.diagnostics.tally();
    byId("app").toggleAttribute("data-has-diagnostics", !!(t.error || t.warning || t.advisory));
    const valid = byId("status-valid");
    const checking = hosted && app.sync.pending();
    byId("app").toggleAttribute("data-checking", checking);
    clear(valid);
    if (checking) {
      valid.textContent = "Checking…";
      valid.style.color = "";
    } else if (t.error > 0) {
      valid.append(icon("error"), `${plural(t.error, "error")}`);
      valid.style.color = "var(--danger)";
    } else {
      valid.append(icon("check"), t.warning ? `Valid, ${plural(t.warning, "warning")}` : "Valid");
      valid.style.color = "var(--ok)";
    }
    this.fileSave(dirty);
    const save = byId("save");
    save.disabled = this.isSaving || app.readonly === true;
    if (hosted) save.hidden = !app.host.handle && typeof window.showSaveFilePicker !== "function";
    const revision = byId("status-version");
    revision.hidden = hosted;
    revision.textContent = `v${app.sync.version}`;
    byId("status-conn").textContent = this.conn;
    app.files?.updateActions();
    this.cursor();
  }

  fileSave(dirty) {
    const app = this.app;
    const hosted = app.host.kind === "wasm";
    const mark = byId("dirty-mark");
    const label = hosted && app.host.handle ? "File not saved" : "Unsaved changes";
    mark.title = label;
    mark.setAttribute("aria-label", label);
    const atRisk = hosted ? app.atRisk() : dirty;
    mark.hidden = hosted && !app.host.handle ? !atRisk : !dirty;
    document.title = `${mark.hidden ? "" : "● "}${app.name} · Zenith`;
    if (hosted) {
      byId("status-save").textContent = this.isSaving ? "Saving to file…"
        : app.host.handle ? dirty ? "File not saved" : "File up to date"
        : atRisk ? "Unsaved changes" : "";
      return;
    }
    byId("status-save").textContent = this.isSaving
      ? "Saving"
      : dirty
        ? app.sync.pending()
          ? "Unsaved, syncing"
          : "Unsaved changes"
        : "Saved";
  }

  saving(on) {
    this.isSaving = on;
    this.update();
  }

  connection(text) {
    this.conn = text;
    byId("status-conn").textContent = text;
  }

  /** Show or hide "Showing last valid preview". */
  stale(on) {
    byId("stale-badge").hidden = !on;
  }

  zoom() {
    byId("zoom-value").textContent = `${Math.round(this.app.view.zoom * 100)}%`;
  }

  /** Show or hide "Rendering" (a render that takes long). */
  rendering(on) {
    byId("render-badge").hidden = !on;
  }

  cursor() {
    const { line, col } = this.app.code.lineCol();
    byId("cursor-pos").textContent = `Ln ${line}, Col ${col}`;
  }
}
