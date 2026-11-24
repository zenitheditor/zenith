// The page: wires the code pane, canvas, panels, and notices to one
// Engine. Every action goes through an engine command; the page keeps
// only what it shows.

import { CodeEditor } from "../code/editor.js";
import { CanvasView } from "../canvas/view.js";
import { Overlay } from "../canvas/overlay.js";
import { PageCanvas } from "../canvas/paint.js";
import { Renderer } from "../canvas/render.js";
import { InlineTextEditor } from "../canvas/textedit.js";
import { DiagnosticsPanel } from "../panels/diagnostics.js";
import { Inspector } from "../panels/inspector.js";
import { Layers } from "../panels/layers.js";
import { TokensPanel } from "../panels/tokens.js";
import { BufferSync } from "../sync/buffer.js";
import { CommandPalette } from "../ui/cmdk.js";
import { ExportDialog } from "../ui/export.js";
import { HistoryUI } from "../ui/history.js";
import { icon } from "../ui/icons.js";
import { Layout } from "../ui/layout.js";
import { Notices } from "../ui/notify.js";
import { byId, clear, h } from "../util/dom.js";
import { OffsetIndex } from "../util/text.js";
import { Serial } from "../util/serial.js";
import { debounce } from "../util/timing.js";
import { NodeActions } from "./actions.js";
import { EventRouter } from "./events.js";
import { FileTools } from "./files.js";
import { GestureController } from "./gestures.js";
import { normalizeZenName } from "./samples.js";
import { SelectionController } from "./selection.js";
import { Status } from "./status.js";
import { ToolController } from "./tools.js";

const REFRESH_MS = 40;
const SEVERITY = { error: "error", warning: "warning", advisory: "info" };

export class App {
  constructor(engine, summary, { example = null, drafts = null } = {}) {
    this.host = engine;
    this.engine = engine;
    this.path = summary.path;
    this.name = summary.path.split(/[\\/]/).pop() || "document.zen";
    this.page = summary.page || 1;
    this.selectionIds = summary.selection ?? [];
    this.valid = summary.valid;
    this.stale = summary.stale;
    this.serverDirty = summary.dirty;
    /** The file is read-only: Save is off (see `EventRouter.readonly`). */
    this.readonly = false;
    this.needsVersion = new Map();
    this.diagnosticList = [];
    this.index = null;
    this.example = example;
    this.drafts = drafts;

    this.notices = new Notices(byId("banners"), byId("toasts"));
    this.layout = new Layout({ starter: Boolean(this.example) });
    this.events = new EventRouter(this);
    this.engine = this.events.track(engine);
    /**
     * User intents run here one at a time, in input order: selection
     * requests (click, Shift+click, marquee, layers, code cursor), key and
     * inspector edits, undo, redo, and save. Each starts after the one
     * before it got its reply. So replies apply in input order, and a key
     * acts on the selection the clicks before it made. The `zenith edit`
     * host sends each call on its own HTTP request, so two calls in flight
     * at once can reach the server, or come back, in either order.
     */
    this.intents = new Serial();
    this.status = new Status(this);
    this.code = new CodeEditor(byId("code-host"), summary.text, {
      localChange: (changes) => {
        this.drafts?.capture(this.code.text(), this.name);
        this.sync.localChange(changes);
      },
      cursor: (head) => this.selection.cursor(head),
      undo: () => this.history("history.undo"),
      redo: () => this.history("history.redo"),
      save: () => this.save(),
    });
    this.sync = new BufferSync(this.engine, this.code, summary, {
      changed: (info) => this.textChanged(info),
      frame: (text) => this.frame(text),
      dirty: () => this.status.update(),
      error: (env, retry) => this.notices.error("buffer.set", env, { retry }),
      offline: (err) => this.events.offline(err),
    });
    this.paint = new PageCanvas(byId("viewport"), byId("page-canvas"));
    const stages = [byId("stage"), byId("stage-over")];
    this.view = new CanvasView(byId("viewport"), stages, byId("page-frame"), byId("overlay"), this.paint, {
      click: (x, y, e) => {
        if (this.tools.isDrawing()) {
          this.tools.clickDraw({ x, y });
          return;
        }
        this.selection.canvasClick(x, y, e);
      },
      hover: (x, y) => {
        this.selection.hover(x, y);
        this.gestures.pointer(x === null ? null : { x, y });
      },
      grab: (p, e) => this.gestures.grab(p, e),
      hand: () => this.tools.isHand(),
      key: (e) => this.actions.key(e),
      zoomed: () => {
        this.renderer.zoomed();
        this.overlay.draw();
        this.status.zoom();
      },
      // The view can move (a resize) before the renderer exists.
      moved: () => this.renderer?.moved(),
    });
    this.overlay = new Overlay(byId("overlay"), this.view);
    this.renderer = new Renderer(this.engine, this.view, this.paint, {
      page: () => this.page,
      rendered: (r) => this.rendered(r),
      failed: (env) => this.renderFailed(env),
      offline: (err) => this.events.offline(err),
      busy: (on) => this.status.rendering(on),
    });
    this.inspector = new Inspector(byId("inspector"), {
      set: (id, params) => this.actions.set(id, params),
      guideText: (id) => this.example !== null && id === "headline",
      align: (nodeIds, align) =>
        this.actions.run("tx.apply", () => ({
          params: {
            label: `Align ${align}`,
            ops: [{ op: "align_nodes", node_ids: nodeIds, align, anchor: "selection" }],
          },
          done: `Aligned ${align}.`,
        })),
      distribute: (nodeIds, axis) =>
        this.actions.run("tx.apply", () => ({
          params: {
            label: `Distribute ${axis}`,
            ops: [{ op: "distribute_nodes", node_ids: nodeIds, axis }],
          },
          done: `Distributed ${axis}.`,
        })),
      setMany: async (nodeIds, params) => {
        for (const id of nodeIds) {
          await this.actions.set(id, params);
        }
      },
      createColorToken: async (hex) => {
        const clean = hex.replace(/[^0-9a-f]/gi, "").slice(0, 6).toLowerCase() || "custom";
        const id = `color.c${clean}`;
        const ok = await this.actions.run("tx.apply", () => ({
          params: {
            label: `Add token ${id}`,
            ops: [{ op: "create_token", id, type: "color", value: hex }],
          },
          done: `Created ${id}.`,
        }));
        return ok ? id : null;
      },
      setPageSize: async (pageId, w, h) => {
        const ok = await this.actions.run("tx.apply", () => ({
          params: {
            label: `Resize page ${pageId} to ${w}×${h}`,
            ops: [{ op: "set_page_size", page: pageId, w: `(px)${w}`, h: `(px)${h}` }],
          },
          done: `Resized page to ${w} × ${h} px.`,
        }));
        if (ok) requestAnimationFrame(() => this.view.fit());
        return ok;
      },
    });
    this.layers = new Layers(byId("pages"), byId("layers"), {
      page: (n) => this.selection.goToPage(n),
      select: (id) => this.selection.selectIds([id], "layers"),
      selectMany: (ids) => this.selection.selectIds(ids, "layers"),
      toggleVisible: (id, visible) => this.actions.set(id, { visible }),
      toggleLocked: (id, locked) => this.actions.set(id, { locked }),
      addPage: () => {
        const pages = this.outline?.pages ?? [];
        const cur = pages.find((p) => p.page === this.page) ?? pages[0];
        const w = Math.round(cur?.w ?? 600);
        const h = Math.round(cur?.h ?? 720);
        const nextNum = pages.length + 1;
        const id = `page.${nextNum}.${Date.now().toString(36).slice(-3)}`;
        this.actions.run("tx.apply", () => ({
          params: {
            label: `Add page ${nextNum}`,
            ops: [{ op: "add_page", id, w: `(px)${w}`, h: `(px)${h}` }],
          },
          done: `Added page ${nextNum}.`,
        }));
      },
      duplicatePage: () => {
        const cur = this.outline?.pages?.find((p) => p.page === this.page);
        if (!cur) return;
        const stamp = Date.now().toString(36).slice(-3);
        const newId = `${cur.id}-copy-${stamp}`;
        this.actions.run("tx.apply", () => ({
          params: {
            label: `Duplicate page ${cur.id}`,
            ops: [{ op: "duplicate_page", page: cur.id, new_id: newId, id_suffix: `-${stamp}` }],
          },
          done: `Duplicated ${cur.id}.`,
        }));
      },
      deletePage: () => {
        const pages = this.outline?.pages ?? [];
        if (pages.length <= 1) return;
        const cur = pages.find((p) => p.page === this.page);
        if (!cur) return;
        this.actions.run("tx.apply", () => ({
          params: {
            label: `Delete page ${cur.id}`,
            ops: [{ op: "delete_page", page: cur.id }],
          },
          done: `Deleted ${cur.id}.`,
        }));
      },
    });
    this.tokenList = [];
    this.tokensPanel = new TokensPanel(byId("tokens"), {
      onPick: (t) => {
        this.layout.showSource(true);
        const idx = this.code.text().indexOf(`id="${t.id}"`);
        if (idx >= 0) this.code.jumpTo(idx, idx + t.id.length + 5);
      },
      onCreate: ({ id, type, value }) =>
        this.actions.run("tx.apply", () => ({
          params: {
            label: `Add token ${id}`,
            ops: [{ op: "create_token", id, type, value }],
          },
          done: `Added token ${id}.`,
        })),
      onUpdate: (id, value) =>
        this.actions.run("tx.apply", () => ({
          params: {
            label: `Update token ${id}`,
            ops: [{ op: "update_token_value", id, value }],
          },
          done: `Updated ${id}.`,
        })),
    });
    this.diagnostics = new DiagnosticsPanel(byId("diag-list"), byId("diag-counts"), (d) => {
      this.layout.closePopovers();
      this.layout.closeSheet();
      this.jumpToDiagnostic(d);
    });
    this.selection = new SelectionController(this);
    this.gestures = new GestureController(this, byId("gesture-hint"));
    this.actions = new NodeActions(this);
    this.tools = new ToolController(this);
    this.textEdit = new InlineTextEditor(this, byId("viewport"));
    this.historyUI = new HistoryUI(this);
    this.cmdk = new CommandPalette(this);
    this.exportDlg = new ExportDialog(this);
    this.layout.historyClose = () => this.historyUI.close();
    this.bindPhoneSheets();
    this.refreshLater = debounce(() => this.refreshDocument(), REFRESH_MS);
    this.lastView = (this.layout.starter ?? this.layout.state).view;
    this.layout.onChange(() => {
      const curView = (this.layout.starter ?? this.layout.state).view;
      if (curView !== this.lastView) {
        this.lastView = curView;
        if (curView === "split" || curView === "design") {
          requestAnimationFrame(() => this.view.fit());
        }
      }
      this.overlay.draw();
    });
    // The static host opens files itself; `zenith edit` serves one file.
    this.files = engine.kind === "wasm" ? new FileTools(this, engine) : null;
  }

  async start() {
    this.bindControls();
    this.status.update();
    this.layers.setPage(this.page);
    const list = await this.engine.run("commands.list");
    if (list.ok) {
      for (const c of list.result.commands) this.needsVersion.set(c.id, c.needs_version);
    } else {
      this.notices.error("commands.list", list, { retry: () => this.start() });
    }
    await this.diagnose();
    this.renderer.request({ fit: true });
    await this.refreshDocument();
    await this.historyUI.refresh();
    if (this.example !== null) await this.selection.selectIds(["headline"], "starter");
    this.events.subscribe();
    const summary = await this.engine.state({ text: true });
    this.events.summary(summary);
  }

  bindPhoneSheets() {
    const titleEl = byId("sheet-title");
    const subEl = byId("sheet-sub");
    const bodyEl = byId("sheet-body");
    const origins = {
      layers: { el: byId("layers"), home: byId("layers")?.parentElement },
      edit: { el: byId("inspector"), home: byId("inspector")?.parentElement },
      code: { el: byId("code-host"), home: byId("code-host")?.parentElement },
      problems: { el: byId("diag-list"), home: byId("diag-list")?.parentElement },
    };
    const restoreAll = () => {
      for (const o of Object.values(origins)) {
        if (o.el && o.home && o.el.parentElement !== o.home) o.home.appendChild(o.el);
      }
    };
    this.layout.sheetHandlers = {
      render: (kind) => {
        restoreAll();
        clear(bodyEl);
        if (subEl) subEl.textContent = "";
        if (kind === "layers") {
          titleEl.textContent = "Layers";
          if (origins.layers.el) bodyEl.appendChild(origins.layers.el);
        } else if (kind === "edit") {
          titleEl.textContent = this.selectionIds.length === 1 ? this.selectionIds[0] : "Page";
          if (subEl) subEl.textContent = this.selectionIds.length > 1 ? `${this.selectionIds.length} selected` : "";
          if (origins.edit.el) bodyEl.appendChild(origins.edit.el);
        } else if (kind === "code") {
          titleEl.textContent = "Source";
          if (origins.code.el) bodyEl.appendChild(origins.code.el);
        } else if (kind === "problems") {
          titleEl.textContent = "Problems";
          if (origins.problems.el) bodyEl.appendChild(origins.problems.el);
        } else if (kind === "insert") {
          titleEl.textContent = "Insert";
          const items = [
            ["text", "Text"],
            ["rect", "Rectangle"],
            ["ellipse", "Ellipse"],
            ["line", "Line"],
          ];
          bodyEl.appendChild(
            h(
              "div",
              { class: "insert-grid" },
              items.map(([k, label]) =>
                h(
                  "button",
                  {
                    type: "button",
                    onclick: () => {
                      this.layout.closeSheet();
                      this.tools.set(k);
                    },
                  },
                  icon(k, "icon"),
                  h("span", { text: label }),
                ),
              ),
            ),
          );
        }
      },
      closed: () => restoreAll(),
    };
  }

  bindControls() {
    byId("save")?.addEventListener("click", () => this.save());
    byId("zoom-in")?.addEventListener("click", () => {
      this.layout.closePopovers();
      this.view.zoomTo(this.view.zoom * 1.25);
    });
    byId("zoom-out")?.addEventListener("click", () => {
      this.layout.closePopovers();
      this.view.zoomTo(this.view.zoom / 1.25);
    });
    byId("zoom-100")?.addEventListener("click", () => {
      this.layout.closePopovers();
      this.view.actualSize();
    });
    byId("zoom-fit")?.addEventListener("click", () => {
      this.layout.closePopovers();
      this.view.fit();
    });
    byId("cmdk-btn")?.addEventListener("click", () => this.cmdk.open());
    byId("menu-cmdk")?.addEventListener("click", () => {
      this.layout.closePopovers();
      this.cmdk.open();
    });
    byId("stale-badge")?.addEventListener("click", () => this.layout.openProblems());
    const snap = byId("snap-toggle");
    if (snap) {
      snap.setAttribute("aria-pressed", String(this.gestures.snapOn));
      snap.classList.toggle("on", this.gestures.snapOn);
      snap.addEventListener("click", () => {
        const on = !this.gestures.snapOn;
        this.gestures.setSnap(on);
        snap.setAttribute("aria-pressed", String(on));
        snap.classList.toggle("on", on);
      });
    }
    document.addEventListener("keydown", (e) => {
      if (this.files && (this.files.switching || document.querySelector("dialog[open]"))) return;
      const mod = e.ctrlKey || e.metaKey;
      const key = e.key.toLowerCase();
      if (mod && !e.altKey && !e.shiftKey && key === "k") {
        e.preventDefault();
        this.cmdk.open();
        return;
      }
      if (e.target instanceof Element && e.target.closest(".cm-editor")) return;
      const field = e.target instanceof Element && e.target.closest("input, textarea, select");
      if (mod && !e.altKey) {
        if (key === "s") this.save();
        else if (key === "e" && e.shiftKey) this.exportDlg.open();
        else if (key === "1") this.layout.setView("design");
        else if (key === "2") this.layout.setView("split");
        else if (key === "3") this.layout.setView("code");
        else if (e.key === "\\" && !e.shiftKey) this.layout.toggle("left");
        else if (e.key === "\\" && e.shiftKey) this.layout.toggle("right");
        else if (key === "d" && !e.shiftKey && !field) this.actions.duplicate();
        else if (key === "g" && !e.shiftKey && !field) this.actions.group();
        else if (key === "g" && e.shiftKey && !field) this.actions.ungroup();
        else if (e.key === "]" && !field) this.actions.reorder(e.shiftKey ? "front" : "forward");
        else if (e.key === "[" && !field) this.actions.reorder(e.shiftKey ? "back" : "backward");
        else if (key === "c" && !e.shiftKey && !field && this.selectionIds.length && !window.getSelection()?.toString()) this.actions.copy();
        else if (key === "x" && !e.shiftKey && !field && this.selectionIds.length && !window.getSelection()?.toString()) this.actions.cut();
        else if (key === "v" && !e.shiftKey && !field && this.actions.clipboard) this.actions.paste();
        else if (key === "z" && !e.shiftKey) this.history("history.undo");
        else if ((key === "z" && e.shiftKey) || key === "y") this.history("history.redo");
        else return;
        e.preventDefault();
        return;
      }
      if (field || e.altKey) return;
      if (e.shiftKey && e.key === "!") {
        this.view.fit();
        e.preventDefault();
        return;
      }
      if (e.shiftKey && e.key === ")") {
        this.view.actualSize();
        e.preventDefault();
        return;
      }
      if (e.key === "Enter" && !e.shiftKey && this.selectionIds.length === 1 && this.inspector.lastInfo?.kind === "text") {
        if (this.openInlineText()) e.preventDefault();
        return;
      }
      const toolKeys = { v: "move", t: "text", r: "rect", o: "ellipse", l: "line", h: "hand" };
      if (toolKeys[key]) {
        this.tools.set(toolKeys[key]);
        e.preventDefault();
      } else if (e.key === "Escape" && this.tools.mode !== "move") {
        this.tools.set("move");
        e.preventDefault();
      }
    });
    this.bindCanvasExtras();
    window.addEventListener("beforeunload", (e) => {
      if (!this.atRisk()) return;
      e.preventDefault();
      e.returnValue = "";
    });
  }

  /** Rename the open in-browser `.zen` document. */
  renameDocument(rawName) {
    const next = normalizeZenName(rawName, this.name);
    if (!next || next === this.name) return;
    this.name = next;
    this.path = next;
    if (this.host.kind === "wasm") {
      this.host.name = next;
      this.host.path = next;
    }
    const nameEl = byId("doc-name");
    if (nameEl) {
      nameEl.textContent = next;
      nameEl.title = `${next} — click to rename`;
    }
    if (this.drafts?.active) this.drafts.active.name = next;
    this.status.update();
    this.notices.toast(`Renamed to ${next}.`);
  }

  /** Open inline canvas text editing for the currently selected text node. */
  openInlineText({ selectAll = false } = {}) {
    const info = this.inspector.lastInfo;
    if (info && info.kind === "text" && this.selectionIds.length === 1 && this.selectionIds[0] === info.id) {
      return this.textEdit.open(info, { selectAll });
    }
    return false;
  }

  bindCanvasExtras() {
    const vp = byId("viewport");
    const ctx = byId("ctx-menu");
    const closeCtx = () => {
      if (ctx) ctx.hidden = true;
    };
    vp?.addEventListener("dblclick", (e) => {
      if (this.tools.isDrawing() || this.tools.isHand()) return;
      if (this.openInlineText()) e.preventDefault();
    });
    const imgBtn = byId("tool-image");
    if (imgBtn && this.files) {
      imgBtn.hidden = false;
      imgBtn.addEventListener("click", () => this.files.pickInsertImage());
    }
    vp?.addEventListener("dragover", (e) => {
      if (this.files && e.dataTransfer?.types?.includes("Files")) {
        e.preventDefault();
        e.dataTransfer.dropEffect = "copy";
      }
    });
    vp?.addEventListener("drop", async (e) => {
      if (!this.files) return;
      const file = [...(e.dataTransfer?.files ?? [])].find((f) =>
        /^image\/(png|jpeg|webp|gif|svg\+xml)$/i.test(f.type) || /\.(png|jpe?g|webp|gif|svg)$/i.test(f.name),
      );
      if (!file) return;
      e.preventDefault();
      const pt = this.view.toPage(e.clientX, e.clientY);
      await this.files.insertImageFile(file, pt);
    });
    vp?.addEventListener("contextmenu", (e) => {
      if (!ctx) return;
      e.preventDefault();
      this.layout.closePopovers();
      const hasSel = this.selectionIds.length > 0;
      const single = this.selectionIds.length === 1;
      const multi = this.selectionIds.length >= 2;
      const isText = single && this.inspector.lastInfo?.kind === "text";
      const isGroup = single && this.inspector.lastInfo?.kind === "group";
      byId("ctx-edit-text").hidden = !isText;
      byId("ctx-duplicate").hidden = !hasSel;
      byId("ctx-group").hidden = !multi;
      byId("ctx-ungroup").hidden = !isGroup;
      byId("ctx-forward").hidden = !single;
      byId("ctx-backward").hidden = !single;
      byId("ctx-delete").hidden = !hasSel;
      byId("ctx-reveal").hidden = !single;
      byId("ctx-sep").hidden = !hasSel;
      ctx.hidden = false;
      const w = ctx.offsetWidth || 180;
      const h = ctx.offsetHeight || 220;
      ctx.style.left = `${Math.min(e.clientX, window.innerWidth - w - 8)}px`;
      ctx.style.top = `${Math.min(e.clientY, window.innerHeight - h - 8)}px`;
    });
    document.addEventListener("pointerdown", (e) => {
      if (ctx && !ctx.hidden && !ctx.contains(e.target)) closeCtx();
    });
    document.addEventListener("keydown", (e) => {
      if (e.key === "Escape" && ctx && !ctx.hidden) {
        closeCtx();
        e.preventDefault();
      }
    });
    byId("ctx-edit-text")?.addEventListener("click", () => {
      closeCtx();
      this.openInlineText();
    });
    byId("ctx-duplicate")?.addEventListener("click", () => {
      closeCtx();
      this.actions.duplicate();
    });
    byId("ctx-group")?.addEventListener("click", () => {
      closeCtx();
      this.actions.group();
    });
    byId("ctx-ungroup")?.addEventListener("click", () => {
      closeCtx();
      this.actions.ungroup();
    });
    byId("ctx-forward")?.addEventListener("click", () => {
      closeCtx();
      this.actions.reorder("forward");
    });
    byId("ctx-backward")?.addEventListener("click", () => {
      closeCtx();
      this.actions.reorder("backward");
    });
    byId("ctx-delete")?.addEventListener("click", () => {
      closeCtx();
      this.actions.remove();
    });
    byId("ctx-reveal")?.addEventListener("click", () => {
      closeCtx();
      const id = this.selectionIds[0];
      if (!id) return;
      this.layout.showSource(true);
      const idx = this.code.text().indexOf(`id="${id}"`);
      if (idx >= 0) this.code.jumpTo(idx, idx + id.length + 5);
    });
    byId("ctx-fit")?.addEventListener("click", () => {
      closeCtx();
      this.view.fit();
    });
    byId("ctx-100")?.addEventListener("click", () => {
      closeCtx();
      this.view.actualSize();
    });
  }

  /** `true` while the document has edits not on disk. */
  dirty() {
    return this.serverDirty || this.sync.pending();
  }

  atRisk() {
    if (!this.drafts) return this.dirty();
    const text = this.code.text();
    return (this.dirty() || this.drafts.needsCopy(text)) && !this.drafts.confirmed(text);
  }

  /** Byte/UTF-16 index of the engine text, rebuilt when it changes. */
  offsets() {
    if (!this.index || this.index.text !== this.sync.base) this.index = new OffsetIndex(this.sync.base);
    return this.index;
  }

  /** Pane position of engine byte offset `byte`. */
  paneAt(byte) {
    return this.sync.toPane(this.offsets().toUtf16(byte));
  }

  /** The engine text moved: refresh what depends on it. */
  textChanged(info) {
    if (typeof info.dirty === "boolean") this.serverDirty = info.dirty;
    if (!["open", "typing", "disk"].includes(info.source)) {
      this.drafts?.capture(this.code.text(), this.name);
    }
    const reply = info.reply;
    if (reply && Array.isArray(reply.diagnostics)) {
      this.setDiagnostics(reply.diagnostics, reply.valid, reply.stale);
    } else {
      this.diagnose();
    }
    if (reply?.notes?.length) {
      this.notices.show("notes", {
        level: "info",
        message: reply.notes.map((n) => n.message).join(" "),
      });
    }
    if (reply?.removed_comments?.length) {
      this.notices.show("comments", {
        level: "warning",
        title: "Comments removed.",
        message: `The edit removed ${reply.removed_comments.length} comment(s).`,
        detail: reply.removed_comments.join("\n"),
      });
    }
    if (reply?.reformatted) {
      this.notices.show("reformatted", {
        level: "warning",
        message: "The edit rewrote the document in canonical form. Comments are gone. Undo restores them.",
      });
    }
    if (Array.isArray(reply?.selection)) this.selectionIds = reply.selection;
    this.selection.textChanged();
    this.status.update();
    if (info.source === "typing") {
      const ub = byId("undo");
      if (ub) ub.disabled = false;
    } else {
      this.historyUI.refresh();
    }
    if (info.frame) this.applyFrame(info.frame);
    else this.refreshLater();
  }

  /**
   * The commands to send in one batch with a `buffer.set` of pane text
   * `text`: what `refreshDocument` and the cursor would ask next (render,
   * outline, tokens, the node under the cursor, its handles and
   * inspection).
   */
  frame(text) {
    const head = this.code.head();
    this.index = new OffsetIndex(text);
    const render = this.renderer.frameStep();
    const steps = [
      { command: "doc.outline", params: {} },
      { command: "doc.tokens", params: {} },
      { command: "select.at_offset", params: { offset: this.index.toByte(head) } },
      { command: "node.handles", params: {} },
      { command: "node.inspect", params: {} },
    ];
    if (render) steps.unshift(render);
    return { steps, head, render };
  }

  /** Show what a keystroke batch read (see `frame`). */
  async applyFrame({ steps, envs, head, render }) {
    this.refreshLater.cancel();
    const env = (command) => envs[steps.findIndex((s) => s.command === command)];
    const shown = render ? this.renderer.acceptFrame(env("doc.render"), render) : this.renderer.request();
    const outline = env("doc.outline");
    if (outline.ok) {
      this.outline = outline.result;
      this.layers.setOutline(outline.result);
      this.inspector.setPageInfo(outline.result, this.page);
      this.status.stale(outline.result.stale);
    }
    const tokens = env("doc.tokens");
    if (tokens.ok) {
      this.tokenList = tokens.result.tokens ?? [];
      this.inspector.setTokens(this.tokenList);
      this.tokensPanel.set(this.tokenList);
    }
    await this.selection.applyFrame({
      head,
      version: this.sync.version,
      cursor: env("select.at_offset"),
      handles: env("node.handles"),
      inspect: env("node.inspect"),
    });
    await shown;
  }

  /** Outline, render, tokens, and selection after a text change. */
  async refreshDocument() {
    this.renderer.request();
    const outline = await this.engine.run("doc.outline");
    if (outline.ok) {
      this.outline = outline.result;
      this.layers.setOutline(outline.result);
      this.inspector.setPageInfo(outline.result, this.page);
      this.status.stale(outline.result.stale);
    }
    const tokens = await this.engine.run("doc.tokens", {});
    if (tokens.ok) {
      this.tokenList = tokens.result.tokens ?? [];
      this.inspector.setTokens(this.tokenList);
      this.tokensPanel.set(this.tokenList);
    }
    await this.selection.refresh("document");
  }

  /** Tell screen readers what an action did. */
  announce(text) {
    const el = byId("announce");
    el.textContent = "";
    // A new text node after a clear is announced even when the text repeats.
    requestAnimationFrame(() => {
      el.textContent = text;
    });
  }

  async diagnose() {
    const env = await this.engine.run("doc.diagnose");
    if (env.ok) this.setDiagnostics(env.result.diagnostics, env.result.valid, env.result.stale);
    else this.notices.error("doc.diagnose", env, { retry: () => this.diagnose() });
  }

  setDiagnostics(list, valid, stale) {
    this.diagnosticList = list ?? [];
    if (typeof valid === "boolean") this.valid = valid;
    if (typeof stale === "boolean") this.stale = stale;
    this.diagnostics.set(this.diagnosticList);
    const marks = [];
    for (const d of this.diagnosticList) {
      if (typeof d.start !== "number" || d.import) continue;
      const from = this.paneAt(d.start);
      const to = this.paneAt(typeof d.end === "number" ? d.end : d.start);
      marks.push({
        from,
        to: Math.max(from, to),
        severity: SEVERITY[d.severity] ?? "info",
        source: d.code,
        message: d.message,
      });
    }
    this.code.setDiagnostics(marks);
    this.status.update();
    if (this.files) {
      const missing = (list ?? []).filter((d) =>
        d.code === "asset.missing" || d.code === "import.missing" || d.code === "font.missing",
      );
      if (missing.length) {
        const srcText = this.code.text();
        const declared = [];
        for (const m of srcText.matchAll(/\bsrc="([^"]+)"/g)) declared.push(m[1]);
        for (const m of srcText.matchAll(/\bimport\s+"([^"]+)"/g)) declared.push(m[1]);
        const loaded = this.host.worker.files ?? {};
        const paths = [];
        for (const rel of declared) {
          if (!loaded[rel] && !paths.includes(rel)) paths.push(rel);
        }
        if (!paths.length) {
          for (const d of missing) {
            const m = String(d.message ?? "").match(/['"]([^'"]+\.[a-z0-9]{2,6})['"]/i);
            if (m && !paths.includes(m[1])) paths.push(m[1]);
          }
        }
        const preview = paths.length ? ` (${paths.slice(0, 3).join(", ")}${paths.length > 3 ? "…" : ""})` : "";
        this.notices.show("missing-assets", {
          level: "warning",
          title: `${missing.length} missing external file${missing.length === 1 ? "" : "s"}${preview}.`,
          message: `Auto-load from ${this.name}'s folder, or pick the missing file(s) directly.`,
          actions: [
            { label: "Auto-load folder…", run: () => this.files.attachFolderAssets(paths) },
            { label: "Import files…", run: () => this.files.importMissingAssets(paths) },
          ],
        });
      } else {
        this.notices.hide("missing-assets");
      }
    }
  }

  jumpToDiagnostic(d) {
    const from = this.paneAt(d.start);
    const to = this.paneAt(typeof d.end === "number" ? d.end : d.start);
    this.layout.showSource(true);
    this.code.jumpTo(from, to);
  }

  rendered(r) {
    byId("canvas-empty").hidden = true;
    this.notices.hide("error:doc.render");
    if (r.page !== this.page) {
      this.page = r.page;
      this.layers.setPage(r.page);
    }
    this.status.stale(r.stale);
    this.status.zoom();
    this.overlay.draw();
  }

  renderFailed(env) {
    const e = env.error ?? {};
    const empty = byId("canvas-empty");
    if (e.code === "editor.no_valid_render" || e.code === "render.blocked") {
      empty.hidden = false;
      empty.replaceChildren();
      const p = document.createElement("p");
      p.textContent =
        e.code === "editor.no_valid_render"
          ? "No preview yet. The document has errors and no earlier text was valid. Fix the errors in Diagnostics."
          : `No preview: ${e.message}`;
      empty.appendChild(p);
      return;
    }
    this.notices.error("doc.render", env, { retry: () => this.renderer.request() });
  }

  /** Undo or redo through the engine history (an intent, see `intents`). */
  history(command) {
    return this.intents.run(async () => {
      const env = await this.sync.edit((version) => this.engine.run(command, {}, { version }), "history");
      if (env.ok) {
        await this.historyUI.refresh();
        return;
      }
      const code = env.error?.code;
      if (code === "editor.nothing_to_undo" || code === "editor.nothing_to_redo") {
        await this.historyUI.refresh();
        return;
      }
      this.notices.error(command, env, { runOffer: (o) => this.runOffer(o) });
    });
  }

  /** Jump directly to a step in the engine history timeline (`history.goto`). */
  historyGoto(index) {
    return this.intents.run(async () => {
      const env = await this.sync.edit((version) => this.engine.run("history.goto", { index }, { version }), "history");
      if (env.ok) {
        await this.historyUI.refresh();
        return;
      }
      this.notices.error("history.goto", env, { runOffer: (o) => this.runOffer(o) });
    });
  }

  /** Save to disk (an intent, see `intents`). A conflict shows the conflict notice. */
  async save({ overwrite = false } = {}) {
    if (this.saving) return this.saving;
    this.saving = this.intents.run(async () => {
      this.status.saving(true);
      try {
        const env = await this.sync.edit(
          (version) => this.engine.save({ version, overwrite }),
          "save",
        );
        if (env.ok) {
          this.serverDirty = env.dirty;
          this.events.cleared();
          const r = env.result;
          if (r.warning) this.notices.show("save-warning", { level: "warning", message: r.warning });
          else this.notices.toast(r.stamped ? `Saved ${this.name} and added its doc-id.` : `Saved ${this.name}.`);
        } else if (env.offline) {
          this.notices.show("error:file.save", {
            level: "error",
            code: env.error.code,
            title: "file.save failed.",
            message: "The save did not reach zenith edit. Retry when the connection is back.",
            actions: [{ label: "Retry", kind: "primary", run: () => this.save({ overwrite }) }],
          });
        } else if (env.error?.code === "edit.conflict") {
          await this.events.conflictFromSave(env);
        } else {
          this.notices.error("file.save", env, {
            runOffer: (o) => this.runOffer(o),
            retry: () => this.save({ overwrite }),
          });
        }
      } catch (err) {
        this.notices.error("file.save", { code: err.code ?? "edit.unsynced", message: err.message }, {
          retry: () => this.save({ overwrite }),
        });
      } finally {
        this.status.saving(false);
        this.status.update();
        this.saving = null;
      }
    });
    return this.saving;
  }

  /** Static host: write the text to a new file the user picks (`file.save_as`). */
  async saveAs() {
    const env = await this.sync.edit((version) => this.engine.run("file.save_as", {}, { version }), "save");
    if (env.ok) {
      this.serverDirty = env.dirty;
      if (!env.result.saved) return;
      this.events.cleared();
      this.notices.hide("error:file.save");
      if (env.result.path && env.result.path !== this.path) {
        this.path = env.result.path;
        this.name = this.path.split(/[\\/]/).pop() || this.name;
        byId("doc-name").textContent = this.name;
        byId("doc-name").title = this.path;
      }
      this.notices.toast(env.result.warning ?? `Saved ${this.name}.`);
      this.status.update();
    } else {
      this.notices.error("file.save_as", env, { retry: () => this.saveAs() });
    }
  }

  /**
   * The static host opened another document (`opened` event): show its
   * text and start from page 1 with nothing selected.
   */
  documentOpened(data) {
    this.example = null;
    this.layout.resetDocument();
    this.path = data.path;
    this.name = data.name;
    byId("doc-name").textContent = this.name;
    byId("doc-name").title = this.path;
    this.serverDirty = false;
    this.notices.clear();
    this.sync.remote({
      delta: data.delta,
      baseVersion: data.base_version,
      version: data.version,
      source: "open",
      extra: { dirty: false },
    });
    this.page = 1;
    this.layers.setPage(1);
    this.selectionIds = [];
    this.renderer.reset();
    this.status.update();
  }

  /** Resend an engine offer as it came, at the current version. */
  async runOffer(offer) {
    this.notices.hide(`error:${offer.command}`);
    if (offer.command === "file.save_as") {
      this.notices.hide("error:file.save");
      return this.saveAs();
    }
    const params = offer.params ?? {};
    const env = this.needsVersion.get(offer.command) !== false || offer.command.startsWith("file.")
      ? await this.sync.edit((version) => this.engine.run(offer.command, params, { version }), "offer")
      : await this.engine.run(offer.command, params);
    if (env.ok) {
      this.notices.toast(`${offer.label}: done.`);
      if (offer.command === "file.save") {
        this.serverDirty = env.dirty;
        this.events.cleared();
      }
      if (offer.command === "file.reload") this.events.cleared();
      this.status.update();
      await this.selection.refresh("offer");
    } else {
      this.notices.error(offer.command, env, { runOffer: (o) => this.runOffer(o) });
    }
  }
}
