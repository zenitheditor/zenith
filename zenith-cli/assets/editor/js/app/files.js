// File tools of the static host: open a file, open a project folder, and
// download a copy. The Save button reaches the same engine (`file.save`
// writes back to the picked file, or downloads where the browser cannot).

import { download, downloadBlob } from "../engine/wasm/files.js";
import { byId, h } from "../util/dom.js";
import { projectDocument, showSkippedFiles } from "./project-picker.js";

const CONFIRM = "open-confirm";

export class FileTools {
  constructor(app, host) {
    this.app = app;
    this.host = host;
    this.exporting = false;
    this.menu = byId("export-tools");
    /** The `finish(answer)` of the discard question on screen, or `null`. */
    this.confirming = null;
    byId("file-tools").hidden = false;
    this.menu.hidden = false;
    byId("save-label").textContent = "Save to file";
    byId("save").title = "Save to file (Ctrl+S)";
    byId("save").setAttribute("aria-label", "Save to file");
    byId("open-file").addEventListener("click", () => this.openFile());
    byId("open-folder").addEventListener("click", () => this.openFolder());
    byId("download").addEventListener("click", () => this.downloadCopy());
    byId("export-png").addEventListener("click", () => this.exportPng());
    byId("copy-example").addEventListener("click", () => this.copyExample());
    document.addEventListener("pointerdown", (e) => {
      if (this.menu.open && !this.menu.contains(e.target)) this.menu.open = false;
    });
    document.addEventListener("keydown", (e) => {
      if (this.switching || document.querySelector("dialog[open]")) return;
      if (this.menu.open && e.key === "Escape") {
        this.closeMenu();
        e.preventDefault();
      } else if ((e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey && e.key.toLowerCase() === "o") {
        e.preventDefault();
        this.openFile();
      }
    });
  }

  updateActions() {
    const blocked = !this.app.valid || this.app.stale;
    const png = byId("export-png");
    png.disabled = blocked || this.exporting;
    png.textContent = this.exporting ? "Exporting PNG…" : "Export PNG";
    if (blocked) png.setAttribute("aria-describedby", "png-unavailable");
    else png.removeAttribute("aria-describedby");
    byId("png-unavailable").hidden = !blocked;
    byId("copy-example").hidden = !this.app.example;
  }

  closeMenu() {
    this.menu.open = false;
    byId("export-menu-toggle").focus();
  }

  async openFile() {
    const picked = await this.attempt(() => this.host.pickDocument());
    if (picked) await this.load({ ...picked, draftSource: { kind: "device" } });
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
    if (spec) await this.load({ ...spec, draftSource: { kind: "device" } });
  }

  /** Replace the document with `spec` (see `WasmEngine.openDocument`). */
  async load(spec) {
    const app = this.app;
    if (this.switching) return false;
    let approved = null;
    while (app.atRisk() && approved !== app.code.text()) {
      approved = app.code.text();
      if (!(await this.confirmDiscard(spec.name))) return false;
    }
    if (this.switching) return false;
    this.switching = true;
    const root = byId("app");
    const focused = document.activeElement;
    root.inert = true;
    root.setAttribute("aria-busy", "true");
    try {
      await app.intents.run(() => app.sync.exclusive(async () => {
        if (!(await app.sync.flushNow())) throw new Error("Sync the editor text and try again.");
        if (app.atRisk() && approved !== app.code.text()) {
          throw new Error("The document changed while opening. Try again.");
        }
        await this.host.openDocument(spec);
      }));
      app.drafts?.bind(app.name, app.code.text(), spec.draftSource ?? null, spec.draftRecord);
      this.showDraftWarning(spec.draftWarning);
      app.notices.toast(`Opened ${spec.name}.`);
      return true;
    } catch (err) {
      app.notices.error("file.open", { code: err.code ?? "static.open_failed", message: err.message });
      return false;
    } finally {
      root.inert = false;
      root.removeAttribute("aria-busy");
      this.switching = false;
      if (root.contains(focused) && !focused.closest("[hidden]")) focused.focus();
      else byId("viewport").focus();
    }
  }

  showDraftWarning(message) {
    if (message) this.app.notices.show("draft-assets", { level: "warning", message });
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
        message: `Save to a file or download .zen to keep these edits before opening ${next}.`,
        dismiss: false,
        actions: [
          { label: `Open ${next}`, run: () => finish(true) },
          { label: "Cancel", run: () => finish(false) },
        ],
      });
    });
  }

  downloadCopy() {
    const app = this.app;
    this.closeMenu();
    download(app.name, app.code.text());
  }

  async exportPng() {
    if (this.exporting) return;
    this.closeMenu();
    this.exporting = true;
    this.updateActions();
    const app = this.app;
    try {
      await app.intents.run(() => app.sync.exclusive(async () => {
        if (!(await app.sync.flushNow())) throw new Error("Sync the editor text and try again.");
        if (!app.valid || app.stale) throw new Error("Fix document errors and try again.");
        const { name, page } = app;
        const text = app.code.text();
        const version = app.sync.version;
        const env = await app.engine.run("doc.render", { page, scale: 1 });
        if (!env.ok) throw new Error(env.error?.message ?? "Could not render the page. Try again.");
        if (env.result?.stale || !env.image) throw new Error("Fix document errors and try again.");
        const changed = () => env.version !== version || env.result.page !== page || app.sync.version !== version
          || app.sync.queued.some((event) => event.version > version) || app.sync.pending()
          || app.code.text() !== text || app.page !== page || app.name !== name;
        if (changed()) throw new Error("The document changed during export. Try again.");
        const png = await app.engine.image(env.image);
        if (changed()) throw new Error("The document changed during export. Try again.");
        const suffix = env.result.page_count > 1 ? `-page-${page}` : "";
        downloadBlob(`${name.replace(/\.zen$/i, "")}${suffix}.png`, png);
      }));
      app.notices.hide("export-png");
      app.notices.toast("Exported PNG.");
    } catch (error) {
      app.notices.show("export-png", { level: "error", title: "PNG export failed.", message: error.message });
    } finally {
      this.exporting = false;
      this.updateActions();
    }
  }

  async copyExample() {
    const app = this.app;
    const example = app.example;
    if (!example) return;
    this.closeMenu();
    const url = new URL(location.href);
    url.search = "";
    url.hash = "";
    url.searchParams.set("doc", example);
    try {
      await navigator.clipboard.writeText(url.href);
      if (app.example === example) app.notices.toast("Copied example link.");
    } catch {
      if (app.example !== example) return;
      const notice = app.notices.show("example-link", { level: "info", message: "Copy the example link." });
      const input = h("input", {
        class: "example-link", type: "url", readonly: true, "aria-label": "Example link", value: url.href,
      });
      notice.querySelector(".notice-text").appendChild(input);
      input.select();
      input.focus();
    }
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
