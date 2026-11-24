// File tools of the static host: open a file, open a project folder, and
// download a copy. The Save button reaches the same engine (`file.save`
// writes back to the picked file, or downloads where the browser cannot).

import { blobToBase64, download, downloadBlob } from "../engine/wasm/files.js";
import { openNewDocDialog } from "../ui/newdoc.js";
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
    this.menu.hidden = false;
    const saveLbl = byId("save-label");
    if (saveLbl) saveLbl.textContent = "Save to file";
    const saveBtn = byId("save");
    if (saveBtn) {
      saveBtn.classList.remove("primary");
      saveBtn.classList.add("ghost");
      saveBtn.title = "Save to file (Ctrl+S)";
      saveBtn.setAttribute("aria-label", "Save to file");
    }
    for (const id of ["menu-new", "menu-home", "menu-drafts", "menu-home-sep", "menu-open-file", "menu-open-folder", "menu-file-sep"]) {
      const el = byId(id);
      if (el) el.hidden = false;
    }
    byId("menu-new")?.addEventListener("click", () => {
      app.layout.closePopovers();
      this.newDocument();
    });
    byId("menu-home")?.addEventListener("click", async () => {
      app.layout.closePopovers();
      if (app.atRisk() && !(await this.confirmDiscard("Home"))) return;
      const home = new URL("./", location.href);
      location.href = home.href;
    });
    byId("menu-drafts")?.addEventListener("click", () => {
      app.layout.closePopovers();
      app.drafts?.show();
    });
    byId("menu-open-file")?.addEventListener("click", () => {
      app.layout.closePopovers();
      this.openFile();
    });
    byId("menu-open-folder")?.addEventListener("click", () => {
      app.layout.closePopovers();
      this.openFolder();
    });
    byId("menu-copy-example")?.addEventListener("click", () => {
      app.layout.closePopovers();
      this.copyExample();
    });
    byId("export-dialog-btn")?.addEventListener("click", () => {
      this.menu.open = false;
      app.exportDlg?.open();
    });
    byId("download")?.addEventListener("click", () => this.downloadCopy());
    byId("export-png")?.addEventListener("click", () => this.exportPng());
    byId("copy-example")?.addEventListener("click", () => this.copyExample());
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
      } else if (e.altKey && !e.ctrlKey && !e.metaKey && !e.shiftKey && e.key.toLowerCase() === "n") {
        e.preventDefault();
        this.newDocument();
      }
    });
    this.bindRename();
  }

  /** Open the New Document modal and load the created `.zen` file. */
  newDocument() {
    openNewDocDialog((spec) => {
      void this.load(spec);
    }, { preset: "custom" });
  }

  /** Click the filename in the topbar to rename the in-browser `.zen` document. */
  bindRename() {
    const el = byId("doc-name");
    if (!el) return;
    el.classList.add("editable");
    el.title = "Click to rename file";
    el.addEventListener("click", () => {
      if (el.dataset.renaming === "true") return;
      el.dataset.renaming = "true";
      const prev = this.app.name;
      const input = h("input", {
        type: "text",
        class: "doc-name-input",
        value: prev,
        "aria-label": "Document file name",
      });
      const finish = (commit) => {
        if (el.dataset.renaming !== "true") return;
        delete el.dataset.renaming;
        const next = input.value;
        input.replaceWith(el);
        if (commit) this.app.renameDocument(next);
      };
      input.addEventListener("keydown", (e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          finish(true);
        } else if (e.key === "Escape") {
          e.preventDefault();
          finish(false);
        }
      });
      input.addEventListener("blur", () => finish(true));
      el.replaceWith(input);
      input.focus();
      const dot = prev.lastIndexOf(".");
      input.setSelectionRange(0, dot > 0 ? dot : prev.length);
    });
  }

  updateActions() {
    const blocked = !this.app.valid || this.app.stale;
    const png = byId("export-png");
    if (png) {
      png.disabled = blocked || this.exporting;
      const svg = png.querySelector("svg");
      png.replaceChildren(...(svg ? [svg] : []), this.exporting ? "Exporting PNG…" : "Export PNG");
      if (blocked) png.setAttribute("aria-describedby", "png-unavailable");
      else png.removeAttribute("aria-describedby");
    }
    const unavail = byId("png-unavailable");
    if (unavail) unavail.hidden = !blocked;
    const copyEx = byId("copy-example");
    if (copyEx) copyEx.hidden = !this.app.example;
    const menuCopyEx = byId("menu-copy-example");
    if (menuCopyEx) menuCopyEx.hidden = !this.app.example;
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
  async load(spec, { fromTab = false } = {}) {
    const app = this.app;
    if (this.switching) return false;
    let approved = null;
    if (!fromTab) {
      while (app.atRisk() && approved !== app.code.text()) {
        approved = app.code.text();
        if (!(await this.confirmDiscard(spec.name))) return false;
      }
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
        if (!fromTab && app.atRisk() && approved !== app.code.text()) {
          throw new Error("The document changed while opening. Try again.");
        }
        if (!fromTab) app.tabs?.saveCurrent();
        await this.host.openDocument(spec);
      }));
      app.drafts?.bind(app.name, app.code.text(), spec.draftSource ?? null, spec.draftRecord);
      if (!fromTab) app.tabs?.openOrFocus(spec);
      app.assetsPanel?.refresh();
      this.showDraftWarning(spec.draftWarning);
      if (!fromTab) app.notices.toast(`Opened ${spec.name}.`);
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
   * Prompt the user to pick one or more local files (opening directly in the
   * `.zen` file's folder when Chromium provides a handle) and attach them into
   * the running WASM worker so missing assets/fonts/imports resolve immediately.
   */
  async importMissingAssets(missingPaths) {
    const files = await this.attempt(() => this.host.pickAssetFiles({ multiple: true }));
    if (!files?.length) return;

    const map = { ...(this.host.worker.files ?? {}) };
    let matched = 0;
    for (const file of files) {
      const b64 = await blobToBase64(file);
      const base = file.name.toLowerCase();
      const targets = missingPaths.filter((p) => p.split("/").pop().toLowerCase() === base);
      if (targets.length) {
        for (const t of targets) map[t] = b64;
        matched += targets.length;
      } else if (missingPaths.length === 1 && files.length === 1) {
        map[missingPaths[0]] = b64;
        matched += 1;
      } else {
        map[file.name] = b64;
        map[`assets/${file.name}`] = b64;
        matched += 1;
      }
    }
    await this.host.worker.project({ files: map });
    this.app.notices.toast(`Attached ${matched} asset file${matched === 1 ? "" : "s"}.`);
    await this.app.diagnose();
    await this.app.refreshDocument();
  }

  /**
   * Pick a folder (opening directly at the `.zen` file's folder via `startIn`)
   * and auto-attach all matching files into the running WASM worker without
   * reloading or discarding edits in the open document.
   */
  async attachFolderAssets(missingPaths) {
    const project = await this.attempt(() => this.host.pickProject());
    if (!project) return;
    showSkippedFiles(this.app.notices, project);
    const map = { ...(this.host.worker.files ?? {}), ...project.files };
    // Also map basename matches when the picked folder is a subfolder (e.g. `assets/`).
    const byBase = new Map();
    for (const [rel, b64] of Object.entries(project.files)) {
      const base = rel.split("/").pop().toLowerCase();
      if (!byBase.has(base)) byBase.set(base, b64);
    }
    let resolved = 0;
    for (const req of missingPaths) {
      if (map[req]) {
        resolved += 1;
      } else {
        const hit = byBase.get(req.split("/").pop().toLowerCase());
        if (hit) {
          map[req] = hit;
          resolved += 1;
        }
      }
    }
    await this.host.worker.project({ files: map });
    this.app.notices.toast(
      resolved
        ? `Auto-loaded ${resolved} missing file${resolved === 1 ? "" : "s"} from ${project.root}.`
        : `Attached folder ${project.root} (${Object.keys(project.files).length} files).`,
    );
    await this.app.diagnose();
    await this.app.refreshDocument();
  }

  /** Open a file picker to insert an image/SVG onto the current page. */
  async pickInsertImage() {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = "image/png,image/jpeg,image/webp,image/gif,image/svg+xml,.png,.jpg,.jpeg,.webp,.svg";
    input.hidden = true;
    document.body.appendChild(input);
    const file = await new Promise((resolve) => {
      input.addEventListener("change", () => resolve(input.files?.[0] ?? null), { once: true });
      input.addEventListener("cancel", () => resolve(null), { once: true });
      input.click();
    });
    input.remove();
    if (file) await this.insertImageFile(file);
  }

  /** Attach an image/SVG file to the WASM project and insert an `image` node. */
  async insertImageFile(file, dropPoint = null) {
    if (!file) return false;
    const pg = this.app.outline?.pages?.find((p) => p.page === this.app.page);
    if (!pg) return false;
    const safeName = file.name.replace(/[^a-zA-Z0-9._-]/g, "_");
    const rel = `assets/${safeName}`;
    const isSvg = /\.svg$/i.test(safeName) || file.type === "image/svg+xml";
    const b64 = await blobToBase64(file);
    await this.host.worker.project({ files: { ...(this.host.worker.files ?? {}), [rel]: b64 } });

    let nw = 320;
    let nh = 240;
    if (!isSvg && typeof createImageBitmap === "function") {
      try {
        const bmp = await createImageBitmap(file);
        nw = bmp.width || nw;
        nh = bmp.height || nh;
        bmp.close();
      } catch {
        // Keep fallback dimensions when browser cannot decode bitmap.
      }
    }
    const maxW = Math.max(80, pg.w * 0.6);
    const maxH = Math.max(80, pg.h * 0.6);
    const ratio = Math.min(1, maxW / nw, maxH / nh);
    const w = Math.max(24, Math.round(nw * ratio));
    const h = Math.max(24, Math.round(nh * ratio));
    const x = dropPoint ? Math.round(dropPoint.x - w / 2) : Math.round((pg.w - w) / 2);
    const y = dropPoint ? Math.round(dropPoint.y - h / 2) : Math.round((pg.h - h) / 2);

    const slug = safeName.replace(/\.[^.]+$/, "").replace(/[^a-zA-Z0-9_-]/g, "").slice(0, 18) || "image";
    const stamp = Date.now().toString(36).slice(-4);
    const assetId = `asset.${slug}.${stamp}`;
    const nodeId = `img.${slug}.${stamp}`;
    const source = `image id="${nodeId}" asset="${assetId}" x=(px)${x} y=(px)${y} w=(px)${w} h=(px)${h} fit="contain"`;

    return this.app.actions.run("tx.apply", () => ({
      params: {
        label: `Insert ${safeName}`,
        select: [nodeId],
        ops: [
          { op: "add_asset", id: assetId, kind: isSvg ? "svg" : "image", src: rel },
          { op: "add_node", parent: pg.id, source },
        ],
      },
      done: `Inserted ${safeName}.`,
    }));
  }

  /** Insert an already-loaded project image/SVG (from the Assets panel) onto the current page. */
  async insertProjectAsset(rel) {
    const pg = this.app.outline?.pages?.find((p) => p.page === this.app.page);
    if (!pg) return false;
    const name = rel.split("/").pop() || "asset";
    const isSvg = /\.svg$/i.test(name);
    const slug = name.replace(/\.[^.]+$/, "").replace(/[^a-zA-Z0-9_-]/g, "").slice(0, 18) || "image";
    const stamp = Date.now().toString(36).slice(-4);
    const assetId = `asset.${slug}.${stamp}`;
    const nodeId = `img.${slug}.${stamp}`;
    const w = Math.min(280, Math.round(pg.w * 0.45));
    const h = Math.min(220, Math.round(pg.h * 0.45));
    const x = Math.round((pg.w - w) / 2);
    const y = Math.round((pg.h - h) / 2);
    const source = `image id="${nodeId}" asset="${assetId}" x=(px)${x} y=(px)${y} w=(px)${w} h=(px)${h} fit="contain"`;
    return this.app.actions.run("tx.apply", () => ({
      params: {
        label: `Insert ${name}`,
        select: [nodeId],
        ops: [
          { op: "add_asset", id: assetId, kind: isSvg ? "svg" : "image", src: rel },
          { op: "add_node", parent: pg.id, source },
        ],
      },
      done: `Inserted ${name}.`,
    }));
  }

  /** Open a sibling `.zen` document from the currently loaded project folder. */
  async openProjectDoc(rel) {
    const files = this.host.worker.files ?? {};
    const b64 = files[rel];
    if (!b64) return false;
    const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
    const text = new TextDecoder().decode(bytes);
    const project = { root: this.host.root ?? "", files: { ...files }, handles: new Map(this.host.handles ?? []) };
    return this.load({
      name: rel.split("/").pop(),
      path: rel,
      text,
      project,
      draftSource: { kind: "device" },
    });
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
