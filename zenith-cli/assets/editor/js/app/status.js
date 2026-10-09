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
    document.title = `${dirty ? "● " : ""}${app.name} · Zenith`;
    byId("dirty-mark").hidden = !dirty;
    const t = app.diagnostics.tally();
    const valid = byId("status-valid");
    clear(valid);
    if (t.error > 0) {
      valid.append(icon("error"), `${plural(t.error, "error")}`);
      valid.style.color = "var(--danger)";
    } else {
      valid.append(icon("check"), t.warning ? `Valid, ${plural(t.warning, "warning")}` : "Valid");
      valid.style.color = "var(--ok)";
    }
    byId("status-save").textContent = this.isSaving
      ? "Saving"
      : dirty
        ? app.sync.pending()
          ? "Unsaved, syncing"
          : "Unsaved changes"
        : "Saved";
    byId("status-version").textContent = `v${app.sync.version}`;
    byId("status-conn").textContent = this.conn;
    this.cursor();
  }

  saving(on) {
    this.isSaving = on;
    byId("save").disabled = on;
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
