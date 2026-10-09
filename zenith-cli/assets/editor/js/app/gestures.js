// Canvas gestures: drag the selected node (or the whole selection) or its
// handles, see a live preview, and commit on release; drag on empty canvas
// to select with a marquee band. Every edit is an engine command.
//
// While dragging, `gesture.preview` renders the page with the gesture
// applied, in the window and at the scale of the image on screen; one
// preview is in flight at a time and the newest pointer position wins. A
// rejected preview turns the ghost outline red and the hint says why and
// what goes ahead (Alt to detach, release for the offers). With snapping
// on, the preview's `snap.guides` show what lined up.
//
// On release, `gesture.commit` runs against the text version the drag
// started on. When the text moved on meanwhile (typing, another client, a
// disk reload) the gesture is dropped and a notice says so. A rejection
// shows its diagnostics and offers. The code pane gets the engine delta.
//
// A press on empty canvas (or on a locked node) and a drag draws a marquee
// band; release runs `select.marquee` (Alt: only nodes wholly inside;
// Shift: add to the selection). Pan stays on Space + drag, the middle
// button, the wheel, and two fingers.

import { handleCursor, inside } from "../canvas/overlay.js";
import { load, save } from "../util/store.js";
import { Drag, SNAP_PX } from "./drag.js";

/** Handle hit radius, screen px: mouse and pen, touch. */
const GRAB_PX = 7;
const GRAB_TOUCH_PX = 16;
/** Click slop for hits, screen px. */
const HIT_PX = 4;
/** A marquee smaller than this (screen px) on both axes selects nothing. */
const BAND_MIN_PX = 3;

/** What a rejected preview tells the user, by the first error code. */
const TIPS = {
  "tx.token_bound": "Bound to a token. Hold Alt to detach it, or release to see the offers.",
  "tx.anchored": "Placed by an anchor. Hold Alt to detach it, or release to see the offers.",
  "tx.computed_size": "The size is computed (hug, fill, or content). Release to set a fixed size.",
  "tx.layout_managed": "The layout places this node. Release to reorder it or take it out of the layout.",
  "tx.value_unresolved": "This value has no px form. Release to replace it with px.",
  "tx.derived_geometry": "A connector follows its targets. Move the targets instead.",
  "editor.locked": "Locked. Release to unlock it.",
  "editor.hidden": "Hidden. Release to show it.",
};

export class GestureController {
  constructor(app, hintEl) {
    this.app = app;
    this.hintEl = hintEl;
    /** The drag in progress, or `null`. */
    this.drag = null;
    this.flight = null;
    /** Params waiting for the preview in flight to end. */
    this.want = null;
    /** Params of the preview on screen (JSON), to know whether a commit matches it. */
    this.previewed = null;
    this.pointerAt = null;
    /** Snap moves and resizes to other nodes and the page. */
    this.snapOn = load("snap", true);
    for (const type of ["keydown", "keyup"]) {
      window.addEventListener(type, (e) => {
        if (this.drag && (e.key === "Shift" || e.key === "Alt" || e.key === "Control" || e.key === "Meta")) {
          this.drag.modifiers(e);
          if (e.key === "Alt") e.preventDefault();
        }
      });
    }
  }

  /** Turn snapping on or off (the canvas Snap toggle). */
  setSnap(on) {
    this.snapOn = on;
    save("snap", on);
  }

  /** The snap reach in page px, or `0` with snapping off. */
  snapReach() {
    return this.snapOn ? SNAP_PX / (this.app.view.zoom || 1) : 0;
  }

  /** `true` when canvas edits can run: the text is valid. */
  enabled() {
    return this.app.valid && !this.app.stale;
  }

  /** CanvasView `grab`: the drag a left press at page point `p` starts. */
  grab(p, e) {
    const app = this.app;
    const version = app.sync.version;
    const blocked = this.enabled() ? null : "The source has errors. Fix them to edit on the canvas.";
    const item = app.overlay.handleItem();
    if (item && !blocked) {
      const h = app.overlay.handleAt(p, e.pointerType === "touch" ? GRAB_TOUCH_PX : GRAB_PX);
      if (h) return this.begin(new Drag(this, { start: p, version, ...this.members(item), item, handle: h }));
    }
    // One finger on touch drags only the selection; elsewhere it pans.
    if (e.pointerType === "touch") {
      if (!item || blocked || !this.onSelection(p)) return null;
      return this.begin(new Drag(this, { start: p, version, ...this.members(item), item }));
    }
    return this.begin(new Drag(this, { start: p, version, blocked }));
  }

  /** `{node}` or `{nodes}` for the handle owner `item`. */
  members(item) {
    const ids = this.app.selectionIds;
    return item === this.app.overlay.group && ids.length > 1 ? { nodes: [...ids] } : { node: item.id };
  }

  /** `true` when page point `p` lies on a selected node or in the selection box. */
  onSelection(p) {
    const o = this.app.overlay;
    if (o.group?.corners && inside(o.group.corners, p)) return true;
    return o.selection.some((s) => s.corners && inside(s.corners, p));
  }

  begin(drag) {
    this.drag = drag;
    this.previewed = null;
    return drag;
  }

  /**
   * What a body drag at page point `p` moves: the selection when a selected
   * node is under the point (or, with several selected, the point is in the
   * selection box), else the topmost node there (which becomes the
   * selection). `null` (a marquee) when nothing, or only a locked node, is
   * there.
   */
  async target(p) {
    const app = this.app;
    const env = await app.engine.run("select.hit", { x: p.x, y: p.y, tolerance: HIT_PX / app.view.zoom, select: false });
    if (!env.ok) return null;
    const hits = env.result.hits;
    const sel = app.selectionIds;
    const overlay = app.overlay;
    if (sel.length > 1 && overlay.group && (hits.some((h) => sel.includes(h.id)) || inside(overlay.group.corners, p))) {
      return { nodes: [...sel], item: overlay.group };
    }
    if (sel.length === 1 && hits.some((h) => h.id === sel[0])) {
      const item = overlay.single();
      return { node: sel[0], item: item?.id === sel[0] ? item : null };
    }
    const top = hits[0];
    if (!top || top.locked) return null;
    await app.selection.selectIds([top.id], "canvas");
    const item = overlay.single();
    return { node: top.id, item: item?.id === top.id ? item : null };
  }

  /** The marquee band moved: draw it and say what release does. */
  band(drag) {
    const app = this.app;
    app.overlay.setMarquee(drag.band());
    const how = drag.mods.alt ? "Select nodes wholly inside" : "Select nodes the band touches";
    this.hint(drag.mods.shift ? `${how}, added to the selection` : how, drag.point);
  }

  /** The pointer moved: show the ghost and hint, ask for a preview. */
  update(drag) {
    const params = drag.params();
    const overlay = this.app.overlay;
    if (drag.action === "move" && drag.item?.corners && !drag.mods.shift) {
      const shift = (corners) => corners.map(([x, y]) => [x + params.dx, y + params.dy]);
      const members = params.nodes ? overlay.selection.filter((s) => s.corners).map((s) => ({ corners: shift(s.corners) })) : undefined;
      overlay.setGhost({
        corners: shift(drag.item.corners),
        members,
        blocked: overlay.ghost?.blocked ?? false,
      });
    }
    if (!overlay.ghost?.blocked) this.hint(this.describe(drag, params), drag.point);
    this.want = { drag, params };
    if (!this.flight) this.flight = this.previewLoop();
  }

  /** Run previews until no newer pointer position waits. */
  async previewLoop() {
    try {
      while (this.want) {
        const { drag, params } = this.want;
        this.want = null;
        if (drag.done || drag !== this.drag) continue;
        await this.preview(drag, params);
      }
    } finally {
      this.flight = null;
    }
  }

  async preview(drag, params) {
    const app = this.app;
    const win = app.renderer.previewRequest(app.page);
    const env = await app.engine.run("gesture.preview", win ? { ...params, ...win } : { ...params, render: false });
    if (drag.done || drag !== this.drag) return;
    if (env.offline) return;
    if (!env.ok) {
      const e = env.error ?? {};
      const code = e.diagnostics?.find((d) => d.severity === "error")?.code ?? e.code;
      app.renderer.clearPreview();
      this.previewed = null;
      const ghost = app.overlay.ghost;
      app.overlay.setGhost({ corners: ghost?.corners ?? drag.item?.corners ?? null, members: ghost?.members, blocked: true });
      this.hint(TIPS[code] ?? `${e.code}: ${e.message}`, drag.point, "blocked");
      return;
    }
    const r = env.result;
    app.overlay.setGhost({ corners: r.corners ?? null, members: r.members, blocked: false });
    app.overlay.setGuides(r.snap?.guides ?? []);
    this.hint(this.describe(drag, params, r), drag.point);
    if (!env.image || !r.rect) return;
    let bitmap;
    try {
      bitmap = await createImageBitmap(await app.engine.image(env.image));
    } catch {
      return;
    }
    if (drag.done || drag !== this.drag) {
      bitmap.close();
      return;
    }
    app.renderer.showPreview({ bitmap, rect: r.rect, scale: win.scale });
    this.previewed = JSON.stringify(params);
  }

  /** The drag ended (`commit`) or was cancelled. */
  async finish(drag, commit) {
    const app = this.app;
    this.want = null;
    this.hint(null);
    if (drag.resolving) await drag.resolving;
    if (drag.marquee) {
      app.overlay.setMarquee(null);
      if (drag === this.drag) this.drag = null;
      if (commit) await this.select(drag);
      return;
    }
    if (!commit || drag.blocked || drag.inert || !drag.bound || drag.still()) {
      this.clear(drag);
      return;
    }
    const params = drag.params();
    // A preview reply that lands while the commit runs must not show.
    if (this.flight) await this.flight;
    const matches = this.previewed === JSON.stringify(params);
    const env = await app.sync.editAt(
      drag.version,
      (version) => app.engine.run("gesture.commit", params, { version }),
      "gesture",
    );
    if (drag === this.drag) this.drag = null;
    app.overlay.setGuides([]);
    if (env.ok) {
      app.notices.hide("error:gesture.commit");
      app.notices.hide("gesture-stale");
      if (env.result.changed && matches) app.renderer.holdPreview();
      else app.renderer.clearPreview();
      if (!env.result.changed) app.overlay.setGhost(null);
      const what = params.nodes ? `${params.nodes.length} nodes` : params.node;
      app.announce(`${verb(drag.action)} ${what}.`);
      return;
    }
    app.renderer.clearPreview();
    app.overlay.setGhost(null);
    if (env.error?.code === "editor.stale_version") {
      app.notices.show("gesture-stale", {
        level: "warning",
        code: "editor.stale_version",
        title: "Gesture dropped.",
        message: "The text changed while you dragged, so nothing was applied. Drag again.",
      });
      return;
    }
    app.notices.error("gesture.commit", env, { runOffer: (o) => app.runOffer(o) });
  }

  /** Release of a marquee band: select what it touches (or holds). */
  async select(drag) {
    const app = this.app;
    const b = drag.band();
    const zoom = app.view.zoom || 1;
    if (Math.abs(b.x1 - b.x0) * zoom < BAND_MIN_PX && Math.abs(b.y1 - b.y0) * zoom < BAND_MIN_PX) return;
    const env = await app.engine.run("select.marquee", {
      x: b.x0,
      y: b.y0,
      w: b.x1 - b.x0,
      h: b.y1 - b.y0,
      contain: drag.mods.alt,
      extend: drag.mods.shift,
    });
    if (!env.ok) {
      app.notices.error("select.marquee", env);
      return;
    }
    app.selectionIds = env.result.selection;
    await app.selection.refresh("canvas");
    const n = env.result.selection.length;
    app.announce(n ? `${n} node${n === 1 ? "" : "s"} selected.` : "Nothing selected.");
  }

  clear(drag) {
    if (drag === this.drag) this.drag = null;
    this.previewed = null;
    this.app.renderer.clearPreview();
    this.app.overlay.setGhost(null);
    this.app.overlay.setMarquee(null);
  }

  /** Escape: stop the drag in progress. `true` when one stopped. */
  cancel() {
    return this.app.view.cancelDrag();
  }

  /** The hint text for `params` of `drag`, with the preview reply `r` when known. */
  describe(drag, params, r) {
    let text;
    if (drag.action === "rotate") {
      // The preview's op names the rotation the engine writes (snapped).
      const op = r?.ops?.find((o) => o.op === "set_geometry" && "rotate" in o);
      text = op && !params.nodes ? `Rotation ${fmt(op.rotate ?? 0)}°` : `Turn ${fmt(params.angle)}°`;
    } else if (drag.action === "resize" && r?.corners) {
      const [a, b, c] = r.corners;
      text = `${fmt(Math.hypot(b[0] - a[0], b[1] - a[1]))} × ${fmt(Math.hypot(c[0] - b[0], c[1] - b[1]))}`;
    } else {
      const dx = r?.snap?.dx ?? params.dx;
      const dy = r?.snap?.dy ?? params.dy;
      text = `Δx ${fmt(dx)}  Δy ${fmt(dy)}`;
    }
    if (params.nodes) text += ` · ${params.nodes.length} nodes`;
    if (r?.snap?.guides?.length) text += " · snapped";
    const master = drag.item?.master;
    if (master) text += ` · master ${master}: every page that uses it changes`;
    for (const n of r?.notes ?? []) text += ` · ${n.message}`;
    return text;
  }

  /** Show `text` next to page point `p` (`null` hides the hint). */
  hint(text, p, kind = "") {
    const el = this.hintEl;
    if (!text) {
      el.hidden = true;
      return;
    }
    const v = this.app.view;
    el.textContent = text;
    el.className = `gesture-hint ${kind}`.trim();
    el.hidden = false;
    const x = v.px + p.x * v.zoom + 16;
    const y = v.py + p.y * v.zoom + 18;
    const maxX = Math.max(8, v.vw - el.offsetWidth - 8);
    const maxY = Math.max(8, v.vh - el.offsetHeight - 8);
    el.style.left = `${Math.min(Math.max(8, x), maxX)}px`;
    el.style.top = `${Math.min(Math.max(8, y), maxY)}px`;
  }

  /** The pointer is over page point `p` (no button): set the cursor. */
  pointer(p) {
    this.pointerAt = p;
    this.cursor();
  }

  /** Set the cursor for the last pointer point. */
  cursor() {
    const app = this.app;
    const p = this.pointerAt;
    if (!p || !this.enabled()) {
      app.view.setCursor("");
      return;
    }
    const item = app.overlay.handleItem();
    const h = item ? app.overlay.handleAt(p, GRAB_PX) : null;
    if (h) app.view.setCursor(handleCursor(h, item));
    else if (item && app.selectionIds.includes(app.selection.hoverId)) app.view.setCursor("move");
    else app.view.setCursor("");
  }
}

function fmt(n) {
  return String(Math.round(n * 10) / 10);
}

function verb(action) {
  return { move: "Moved", resize: "Resized", rotate: "Rotated", point: "Edited" }[action] ?? "Edited";
}
