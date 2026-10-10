// File tools of the static host: open a file, open a project folder, and
// download a copy. The Save button reaches the same engine (`file.save`
// writes back to the picked file, or downloads where the browser cannot).

import { download } from "../engine/wasm/files.js";
import { byId, h } from "../util/dom.js";

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
    byId("doc-dialog-cancel").addEventListener("click", () => byId("doc-dialog").close());
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
    if (project.skipped.length) {
      notices.show("project-skipped", {
        level: "warning",
        title: `${project.root}:`,
        message: `${project.skipped.length} file(s) were too large to load and are missing from the project.`,
        detail: project.skipped.join("\n"),
      });
    }
    if (!project.documents.length) {
      notices.show("project-empty", {
        level: "error",
        code: "static.no_document",
        message: `${project.root} holds no .zen file. Pick a folder with a document in it.`,
      });
      return;
    }
    const path = project.documents.length === 1 ? project.documents[0] : await this.choose(project);
    if (!path) return;
    await this.load({ name: path.split("/").pop(), text: await this.textOf(project, path), path, project });
  }

  /** The text of `path` in `project` (its files hold base64). */
  async textOf(project, path) {
    const bytes = Uint8Array.from(atob(project.files[path]), (c) => c.charCodeAt(0));
    return new TextDecoder().decode(bytes);
  }

  /** Let the user pick one of the project's documents. Resolves the path or `null`. */
  choose(project) {
    const dialog = byId("doc-dialog");
    const list = byId("doc-dialog-list");
    byId("doc-dialog-note").textContent = `${project.root} holds ${project.documents.length} documents.`;
    return new Promise((resolve) => {
      let chosen = null;
      list.replaceChildren(
        ...project.documents.map((path) =>
          h(
            "li",
            {},
            h("button", {
              type: "button",
              class: "button",
              text: path,
              onclick: () => {
                chosen = path;
                dialog.close();
              },
            }),
          ),
        ),
      );
      dialog.addEventListener("close", () => resolve(chosen), { once: true });
      dialog.showModal();
      list.querySelector("button")?.focus();
    });
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
