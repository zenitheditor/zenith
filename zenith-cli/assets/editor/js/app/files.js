// File tools of the static host: open a file, open a project folder, and
// download a copy. The Save button reaches the same engine (`file.save`
// writes back to the picked file, or downloads where the browser cannot).

import { download } from "../engine/wasm/files.js";
import { byId } from "../util/dom.js";
import { projectDocument, showSkippedFiles } from "./project-picker.js";

const CONFIRM = "open-confirm";

export class FileTools {
  constructor(app, host) {
    this.app = app;
    this.host = host;
    /** The `finish(answer)` of the discard question on screen, or `null`. */
    this.confirming = null;
    byId("file-tools").hidden = false;
    byId("open-file").addEventListener("click", () => this.openFile());
    byId("open-folder").addEventListener("click", () => this.openFolder());
    byId("download").addEventListener("click", () => this.downloadCopy());
    document.addEventListener("keydown", (e) => {
      if ((e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey && e.key.toLowerCase() === "o") {
        e.preventDefault();
        this.openFile();
      }
    });
  }

  async openFile() {
    const picked = await this.attempt(() => this.host.pickDocument());
    if (picked) await this.load(picked);
  }

  async openFolder() {
    const project = await this.attempt(() => this.host.pickProject());
    if (!project) return;
    const notices = this.app.notices;
    showSkippedFiles(notices, project);
    if (!project.documents.length) {
      notices.show("project-empty", {
        level: "error",
        code: "static.no_document",
        message: `${project.root} holds no .zen file. Pick a folder with a document in it.`,
      });
      return;
    }
    const spec = await this.attempt(() => projectDocument(project));
    if (spec) await this.load(spec);
  }

  /** Replace the document with `spec` (see `WasmEngine.openDocument`). */
  async load(spec) {
    const app = this.app;
    if (app.dirty() && !(await this.confirmDiscard(spec.name))) return;
    app.notices.hide(CONFIRM);
    if (!(await app.sync.tryFlush())) {
      app.notices.toast("The editor text has not reached the engine yet. Try again.", "error");
      return;
    }
    try {
      await this.host.openDocument(spec);
      app.notices.toast(`Opened ${spec.name}.`);
    } catch (err) {
      app.notices.error("file.open", { code: err.code ?? "static.open_failed", message: err.message });
    }
  }

  /**
   * Ask before unsaved edits go. Resolves `true` to go on. A second ask
   * replaces the first, which resolves `false`, so no caller waits forever.
   */
  confirmDiscard(next) {
    this.confirming?.(false);
    return new Promise((resolve) => {
      const finish = (answer) => {
        if (this.confirming !== finish) return;
        this.confirming = null;
        this.app.notices.hide(CONFIRM);
        resolve(answer);
      };
      this.confirming = finish;
      this.app.notices.show(CONFIRM, {
        level: "warning",
        title: `${this.app.name} has unsaved edits.`,
        message: `Opening ${next} drops them. Save first to keep them.`,
        dismiss: false,
        actions: [
          { label: `Open ${next}`, kind: "danger", run: () => finish(true) },
          { label: "Cancel", run: () => finish(false) },
        ],
      });
    });
  }

  async downloadCopy() {
    const app = this.app;
    await app.sync.tryFlush();
    download(app.name, app.code.text());
  }

  /** Run `pick`; a failed picker shows a notice and resolves `null`. */
  async attempt(pick) {
    try {
      return await pick();
    } catch (err) {
      this.app.notices.error("file.open", { code: err.code ?? "static.pick_failed", message: err.message });
      return null;
    }
  }
}
