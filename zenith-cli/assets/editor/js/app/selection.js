// Selection: canvas click and hover, the code cursor, the layers panel,
// and page switches. The engine session holds the selection; this module
// asks for it and shows it (overlay, inspector, source highlight, layers).

import { debounce, throttle } from "../util/timing.js";

const HOVER_MS = 70;
const CURSOR_MS = 120;
const CLICK_SLOP_PX = 4;
const MAX_OUTLINES = 24;

export class SelectionController {
  constructor(app) {
    this.app = app;
    this.seq = 0;
    this.hoverSeq = 0;
    this.hoverId = null;
    this.corners = new Map();
    this.cursorWaiting = false;
    /** `{head, version}` a keystroke batch already resolved. */
    this.frameCursor = null;
    this.cursorLater = debounce(() => this.cursorNow(), CURSOR_MS);
    this.hoverLater = throttle((x, y) => this.hoverNow(x, y), HOVER_MS);
  }

  /** Text changed: cached outlines are stale; a waiting cursor can run. */
  textChanged() {
    this.corners.clear();
    this.hoverId = null;
    if (this.cursorWaiting && this.app.sync.synced()) {
      this.cursorWaiting = false;
      this.cursorLater();
    }
  }

  async canvasClick(x, y, event) {
    const app = this.app;
    const tolerance = CLICK_SLOP_PX / app.view.zoom;
    const env = await app.engine.run("select.hit", {
      x,
      y,
      tolerance,
      extend: event.shiftKey,
    });
    if (!env.ok) {
      app.notices.error("select.hit", env, { runOffer: (o) => app.runOffer(o) });
      return;
    }
    app.selectionIds = env.result.selection;
    await this.refresh("canvas");
  }

  hover(x, y) {
    if (x === null || x < 0 || y < 0 || x > this.app.view.pageW || y > this.app.view.pageH) {
      this.hoverLater.cancel();
      this.hoverSeq++;
      if (this.hoverId !== null) {
        this.hoverId = null;
        this.app.overlay.setHover(null);
      }
      return;
    }
    this.hoverLater(x, y);
  }

  async hoverNow(x, y) {
    const app = this.app;
    const seq = ++this.hoverSeq;
    const tolerance = CLICK_SLOP_PX / app.view.zoom;
    let env;
    try {
      env = await app.engine.run("select.hit", { x, y, tolerance, select: false });
    } catch {
      return;
    }
    if (seq !== this.hoverSeq || !env.ok) return;
    const top = env.result.hits[0]?.id ?? null;
    if (top === this.hoverId) return;
    this.hoverId = top;
    app.gestures.cursor();
    const corners = top ? await this.outline(top) : null;
    if (seq !== this.hoverSeq) return;
    app.overlay.setHover(corners?.corners ?? null);
  }

  /** `node.handles` of `id`, cached until the text changes. */
  async outline(id) {
    if (this.corners.has(id)) return this.corners.get(id);
    let env;
    try {
      env = await this.app.engine.run("node.handles", { id });
    } catch {
      return null;
    }
    const value = env.ok && env.result.corners ? env.result : null;
    this.corners.set(id, value);
    return value;
  }

  /**
   * `node.handles {ids}` of a selection of several nodes (the selection box
   * and its grips), cached until the text changes. `null` when the engine
   * has none (nodes on several pages, a node with no box).
   */
  async groupOutline(ids) {
    const key = `\u0000${JSON.stringify(ids)}`;
    if (this.corners.has(key)) return this.corners.get(key);
    let env;
    try {
      env = await this.app.engine.run("node.handles", { ids });
    } catch {
      return null;
    }
    const value = env.ok && env.result.corners ? env.result : null;
    this.corners.set(key, value);
    return value;
  }

  /** The code cursor moved to UTF-16 offset `head`. */
  cursor(head) {
    this.app.status.cursor();
    this.cursorHead = head;
    this.cursorLater();
  }

  async cursorNow() {
    const app = this.app;
    if (!app.sync.synced()) {
      // Byte offsets name the engine text: wait until the pane text is there.
      this.cursorWaiting = true;
      return;
    }
    const head = app.code.head();
    // The keystroke batch already asked for this cursor in this text.
    const f = this.frameCursor;
    if (f && f.head === head && f.version === app.sync.version) return;
    const offset = app.offsets().toByte(head);
    const env = await app.engine.run("select.at_offset", { offset });
    if (!env.ok) {
      app.notices.error("select.at_offset", env);
      return;
    }
    if (this.applyCursor(env.result)) await this.refresh("code");
  }

  /**
   * Take a `select.at_offset` reply: the selection, and the page that holds
   * the cursor. `true` when the selection or the page changed.
   */
  applyCursor(r) {
    const app = this.app;
    if (!r.parsed) return false;
    const pageChanged = typeof r.page === "number" && r.page !== app.page;
    const same =
      r.selection.length === app.selectionIds.length &&
      r.selection.every((id, i) => id === app.selectionIds[i]);
    app.selectionIds = r.selection;
    if (pageChanged) {
      app.page = r.page;
      app.layers.setPage(r.page);
      app.renderer.request({ fit: true });
    }
    return !same || pageChanged;
  }

  /**
   * A keystroke batch asked for the cursor at pane offset `head` in the
   * text at `version`, and for the selected node's handles (`handles`
   * envelope) and inspection (`inspect` envelope). Shows the selection
   * without asking again.
   */
  async applyFrame({ head, version, cursor, handles, inspect }) {
    this.frameCursor = { head, version };
    this.cursorWaiting = false;
    if (cursor?.ok) this.applyCursor(cursor.result);
    const h = handles?.ok ? handles.result : null;
    if (h?.id && h.corners) this.corners.set(h.id, h);
    await this.refresh("document", { inspect });
  }

  /** Select `ids` (layers panel). Switches to the page that holds them. */
  async selectIds(ids, source) {
    const app = this.app;
    const env = await app.engine.run("select.set", { ids });
    if (!env.ok) {
      app.notices.error("select.set", env);
      return;
    }
    app.selectionIds = env.result.selection;
    const page = this.pageOf(ids[0]);
    if (page && page !== app.page) await this.goToPage(page, { refresh: false });
    await this.refresh(source);
  }

  /** The 1-based page whose layers hold `id`, if the outline has it. */
  pageOf(id) {
    const has = (layers) => layers.some((l) => l.id === id || has(l.children ?? []));
    return this.app.outline?.pages?.find((p) => has(p.children ?? []))?.page ?? null;
  }

  async goToPage(page, { refresh = true } = {}) {
    const app = this.app;
    const env = await app.engine.run("view.set", { page });
    if (!env.ok) {
      app.notices.error("view.set", env);
      return;
    }
    app.page = env.result.page;
    app.layers.setPage(app.page);
    app.renderer.request({ fit: true });
    if (refresh) await this.refresh("page");
  }

  /**
   * Show the selection: outlines on the canvas, the inspector, the layers
   * highlight, and (unless the cursor chose it) the source range.
   * `inspect`: a `node.inspect` envelope of the selected node to use
   * instead of asking.
   */
  async refresh(source, { inspect = null } = {}) {
    const app = this.app;
    const seq = ++this.seq;
    const ids = app.selectionIds;
    app.layers.setSelection(ids);
    if (ids.length === 0) {
      app.overlay.setSelection([]);
      app.inspector.showEmpty("Nothing selected. Click a node on the canvas, in the layers, or in the source.");
      app.code.clearNode();
      return;
    }
    const outlines = await Promise.all(ids.slice(0, MAX_OUTLINES).map((id) => this.outline(id)));
    const group = ids.length > 1 ? await this.groupOutline(ids) : null;
    if (seq !== this.seq) return;
    app.overlay.setSelection(
      outlines
        .filter((o) => o && o.page === app.page)
        .map((o) => ({
          id: o.id,
          kind: o.kind,
          corners: o.corners,
          center: o.center,
          handles: o.handles,
          disabled: o.disabled,
          master: o.master,
          stale: o.stale,
        })),
      group && group.page === app.page ? group : null,
    );
    if (ids.length > 1) {
      app.inspector.showMany(ids);
      app.code.clearNode();
      return;
    }
    // A keystroke batch may have inspected this node already.
    let env = inspect?.ok && inspect.result?.id === ids[0] ? inspect : null;
    try {
      env ??= await app.engine.run("node.inspect", { id: ids[0] });
    } catch {
      return;
    }
    if (seq !== this.seq) return;
    if (!env.ok) {
      const e = env.error ?? {};
      app.inspector.showEmpty(`${e.code}: ${e.message}`);
      app.code.clearNode();
      return;
    }
    const info = env.result;
    app.inspector.show(info);
    if (info.span && app.sync.synced()) {
      const from = app.paneAt(info.span.start);
      const to = app.paneAt(info.span.end);
      app.code.highlightNode(from, to, { scroll: source !== "code" && source !== "document" });
    } else if (!info.span) {
      app.code.clearNode();
    }
  }
}
