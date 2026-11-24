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
    const totalProblems = t.error + t.warning;

    const valid = byId("status-valid");
    const checking = hosted && app.sync.pending();
    byId("app").toggleAttribute("data-checking", checking);
    clear(valid);
    if (checking) {
      valid.className = "chip status-item";
      valid.textContent = "Checking…";
    } else if (t.error > 0) {
      valid.className = "chip status-item err";
      valid.append(icon("error"), "Showing last valid render");
    } else {
      valid.className = "chip status-item ok";
      valid.append(icon("check"), "Valid");
    }

    const pChip = byId("problems-chip");
    const pLabel = byId("problems-label");
    if (pChip && pLabel) {
      pChip.className = t.error > 0 ? "chip err" : "chip";
      pLabel.textContent =
        t.error > 0
          ? plural(t.error, "problem")
          : totalProblems > 0
            ? plural(totalProblems, "warning")
            : "No problems";
    }

    byId("app").classList.toggle("has-error", t.error > 0);
    this.fileSave(dirty);
    const save = byId("save");
    if (save) {
      save.disabled = this.isSaving || app.readonly === true;
      if (hosted) save.hidden = !app.host.handle && typeof window.showSaveFilePicker !== "function";
    }
    const revision = byId("status-version");
    if (revision) {
      revision.hidden = hosted;
      revision.textContent = `v${app.sync.version}`;
    }
    const conn = byId("status-conn");
    if (conn) {
      conn.hidden = hosted;
      conn.textContent = this.conn;
    }
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

    const docState = byId("doc-state");
    const statusSave = byId("status-save");
    if (hosted) {
      const diskText = this.isSaving
        ? "Saving to file…"
        : app.host.handle
          ? dirty
            ? "File not saved"
            : "File up to date"
          : atRisk
            ? "Unsaved changes"
            : "";
      if (statusSave) statusSave.textContent = diskText;
      if (docState) {
        docState.classList.toggle("dirty", dirty || this.isSaving);
        docState.textContent = diskText || "Saved in browser";
      }
      return;
    }
    const text = this.isSaving
      ? "Saving"
      : dirty
        ? app.sync.pending()
          ? "Unsaved, syncing"
          : "Unsaved changes"
        : "Saved";
    if (docState) {
      docState.classList.toggle("dirty", dirty || this.isSaving);
      docState.textContent = text;
    }
    if (statusSave) statusSave.textContent = text;
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
    const pct = Math.round(this.app.view.zoom * 100);
    byId("zoom-value").textContent = `${pct}%`;
    const zi = byId("zoom-input");
    if (zi && document.activeElement !== zi) zi.value = String(pct);
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
