// The page: wires the code pane, canvas, panels, and notices to one
// Engine. Every action goes through an engine command; the page keeps
// only what it shows.

import { CodeEditor } from "../code/editor.js";
import { CanvasView } from "../canvas/view.js";
import { Overlay } from "../canvas/overlay.js";
import { PageCanvas } from "../canvas/paint.js";
import { Renderer } from "../canvas/render.js";
import { DiagnosticsPanel } from "../panels/diagnostics.js";
import { Inspector } from "../panels/inspector.js";
import { Layers } from "../panels/layers.js";
import { BufferSync } from "../sync/buffer.js";
import { Layout } from "../ui/layout.js";
import { Notices } from "../ui/notify.js";
import { byId } from "../util/dom.js";
import { OffsetIndex } from "../util/text.js";
import { Serial } from "../util/serial.js";
import { debounce } from "../util/timing.js";
import { NodeActions } from "./actions.js";
import { EventRouter } from "./events.js";
import { FileTools } from "./files.js";
import { GestureController } from "./gestures.js";
import { SelectionController } from "./selection.js";
import { Status } from "./status.js";

const REFRESH_MS = 40;
const SEVERITY = { error: "error", warning: "warning", advisory: "info" };

export class App {
  constructor(engine, summary) {
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

    this.notices = new Notices(byId("banners"), byId("toasts"));
    this.layout = new Layout();
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
      localChange: (changes) => this.sync.localChange(changes),
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
      click: (x, y, e) => this.selection.canvasClick(x, y, e),
      hover: (x, y) => {
        this.selection.hover(x, y);
        this.gestures.pointer(x === null ? null : { x, y });
      },
      grab: (p, e) => this.gestures.grab(p, e),
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
    });
    this.layers = new Layers(byId("pages"), byId("layers"), {
      page: (n) => this.selection.goToPage(n),
      select: (id) => this.selection.selectIds([id], "layers"),
    });
    this.diagnostics = new DiagnosticsPanel(byId("diag-list"), byId("diag-counts"), (d) =>
      this.jumpToDiagnostic(d),
    );
    this.selection = new SelectionController(this);
    this.gestures = new GestureController(this, byId("gesture-hint"));
    this.actions = new NodeActions(this);
    this.refreshLater = debounce(() => this.refreshDocument(), REFRESH_MS);
    this.layout.onChange(() => this.overlay.draw());
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
    this.events.subscribe();
    const summary = await this.engine.state({ text: true });
    this.events.summary(summary);
  }

  bindControls() {
    byId("save").addEventListener("click", () => this.save());
    byId("undo").addEventListener("click", () => this.history("history.undo"));
    byId("redo").addEventListener("click", () => this.history("history.redo"));
    byId("zoom-in").addEventListener("click", () => this.view.zoomTo(this.view.zoom * 1.25));
    byId("zoom-out").addEventListener("click", () => this.view.zoomTo(this.view.zoom / 1.25));
    byId("zoom-100").addEventListener("click", () => this.view.actualSize());
    byId("zoom-fit").addEventListener("click", () => this.view.fit());
    const snap = byId("snap-toggle");
    snap.setAttribute("aria-pressed", String(this.gestures.snapOn));
    snap.addEventListener("click", () => {
      const on = !this.gestures.snapOn;
      this.gestures.setSnap(on);
      snap.setAttribute("aria-pressed", String(on));
      this.notices.toast(on ? "Snapping on." : "Snapping off.", "info");
    });
    document.addEventListener("keydown", (e) => {
      const mod = e.ctrlKey || e.metaKey;
      if (!mod || e.altKey) return;
      if (e.target instanceof Element && e.target.closest(".cm-editor")) return;
      const key = e.key.toLowerCase();
      const field = e.target instanceof Element && e.target.closest("input, textarea, select");
      if (key === "s") this.save();
      else if (key === "d" && !e.shiftKey && !field) this.actions.duplicate();
      else if (key === "z" && !e.shiftKey) this.history("history.undo");
      else if ((key === "z" && e.shiftKey) || key === "y") this.history("history.redo");
      else return;
      e.preventDefault();
    });
    window.addEventListener("beforeunload", (e) => {
      if (!this.dirty()) return;
      e.preventDefault();
      e.returnValue = "";
    });
  }

  /** `true` while the document has edits not on disk. */
  dirty() {
    return this.serverDirty || this.sync.pending();
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
      this.status.stale(outline.result.stale);
    }
    const tokens = env("doc.tokens");
    if (tokens.ok) this.inspector.setTokens(tokens.result.tokens);
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
      this.status.stale(outline.result.stale);
    }
    const tokens = await this.engine.run("doc.tokens", {});
    if (tokens.ok) this.inspector.setTokens(tokens.result.tokens);
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
  }

  jumpToDiagnostic(d) {
    const from = this.paneAt(d.start);
    const to = this.paneAt(typeof d.end === "number" ? d.end : d.start);
    if (this.layout.full === "canvas") this.layout.setFull("none");
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
      if (env.ok) return;
      const code = env.error?.code;
      if (code === "editor.nothing_to_undo" || code === "editor.nothing_to_redo") {
        this.notices.toast(code === "editor.nothing_to_undo" ? "Nothing to undo." : "Nothing to redo.", "info");
        return;
      }
      this.notices.error(command, env, { runOffer: (o) => this.runOffer(o) });
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
