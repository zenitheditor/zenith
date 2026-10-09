// Page renders: only the visible part of the page, at the exact device scale.
//
// Each render asks `doc.render` for the visible part of the page plus a
// margin of half a viewport on each side (`region.js`), at
// `scale = zoom × devicePixelRatio`. The engine snaps that window out to
// whole device pixels and names it in `rect`. `PageCanvas` draws the image
// 1:1 on device pixels; between renders it draws the last image moved and
// scaled with the view, so pan and zoom feel instant.
//
// A render starts when the view leaves the rendered window, when the zoom or
// the device pixel ratio changes, and after a document change. One render is
// in flight at a time; a newer need is checked when it ends. A page switch
// or a refit drops the reply of the render in flight (request counter). A
// failed render is reported once and not retried until the document or the
// page changes. "Rendering" shows only when a render takes longer than
// `SLOW_MS`. Images are `ImageBitmap`s, closed when replaced.
//
// After a keystroke the page sends the render in the same batch as the
// text (`frameStep`, `acceptFrame`), so typing costs one engine call.
//
// A gesture preview (`gesture.preview` with the shown window and scale)
// draws over the shown image until `clearPreview`, or, after a commit, until
// the first render of the changed document (`holdPreview`).

import { EngineUnavailable } from "../engine/index.js";
import { debounce } from "../util/timing.js";
import { contains, plan, sameScale } from "./region.js";

/** Wait after a zoom or a document change before rendering. */
const SETTLE_MS = 90;
/** Show "Rendering" after this long. */
const SLOW_MS = 250;

export class Renderer {
  /**
   * `hooks`: `page()` the page the engine renders, `rendered(reply)` after a
   * new image shows, `failed(envelope)` when the engine cannot render,
   * `offline(err)` when the engine is out of reach, `busy(on)` to show or
   * hide "Rendering".
   */
  constructor(engine, view, paint, hooks) {
    this.engine = engine;
    this.view = view;
    this.paint = paint;
    this.hooks = hooks;
    /** Bumped to drop the reply of the render in flight. */
    this.seq = 0;
    /** Bumped by every document change; the shown image has `shown.gen`. */
    this.gen = 0;
    this.flight = null;
    /** `{gen, page}` of a failed render: no retry until either changes. */
    this.halted = null;
    this.fitNext = true;
    /** Page sizes (page px) by page number, from replies. */
    this.sizes = new Map();
    /** The image on screen: reply fields plus `bitmap`, `gen`, `request`. */
    this.shown = null;
    /** Images shown so far. */
    this.renders = 0;
    /** A gesture preview drawn instead of `shown`: `{bitmap, rect, scale, until?}`. */
    this.preview = null;
    this.slow = null;
    this.later = debounce(() => this.kick(), SETTLE_MS);
  }

  /** The scale the view needs now. */
  wantScale() {
    return this.view.zoom * (window.devicePixelRatio || 1);
  }

  /** The device scale of the image on screen, `0` before the first. */
  get scale() {
    return this.shown ? this.shown.scale : 0;
  }

  /** `true` when no render is waiting or running. */
  idle() {
    return !this.later.pending() && !this.flight;
  }

  /** The document changed (or a page switch): render soon. */
  request({ fit = false } = {}) {
    this.gen++;
    this.halted = null;
    if (fit) {
      this.clearPreview();
      this.fitNext = true;
      this.seq++;
      this.flight = null;
    }
    this.later();
  }

  /** A different document: forget the page sizes and fit the first render. */
  reset() {
    this.sizes.clear();
    this.request({ fit: true });
  }

  /** The zoom or the device pixel ratio changed. */
  zoomed() {
    this.draw();
    if (this.needs()) this.later();
  }

  /** The view moved (pan, zoom, resize): redraw, render when uncovered. */
  moved() {
    this.draw();
    const s = this.shown;
    // A zoom waits for `zoomed`'s debounce; a pan past the window renders now.
    if (s && sameScale(s.scale, this.target().scale) && this.needs()) this.kick();
  }

  /** The window and scale the view needs. */
  target() {
    const page = this.sizes.get(this.hooks.page());
    if (!page) return { scale: this.wantScale(), viewport: null, visible: null };
    return plan({ visible: this.view.visible(), page, scale: this.wantScale() }) ?? {
      scale: this.wantScale(),
      viewport: null,
      visible: null,
    };
  }

  /** `true` when the image on screen does not serve the current view. */
  needs() {
    const page = this.hooks.page();
    if (this.halted && this.halted.gen === this.gen && this.halted.page === page) return false;
    if (!this.sizes.has(page)) return true;
    const t = this.target();
    // The page is off-screen: nothing to render.
    if (!t.visible) return false;
    const s = this.shown;
    if (!s || s.gen !== this.gen || s.page !== page) return true;
    return !sameScale(s.scale, t.scale) || !contains(s.view, t.visible, 1 / s.scale);
  }

  draw() {
    this.paint.draw(this.preview ?? this.shown, this.view);
  }

  /**
   * The window and scale a gesture preview renders: the shown image's, so
   * the preview lands on the same device pixels. `null` before the first
   * image or on another page.
   */
  previewRequest(page) {
    const s = this.shown;
    if (!s || s.page !== page) return null;
    return { scale: s.request.scale, viewport: { ...s.request.viewport } };
  }

  /** Draw preview `{bitmap, rect, scale}` over the shown image. */
  showPreview(preview) {
    if (this.preview && this.preview.bitmap !== preview.bitmap) this.preview.bitmap.close();
    this.preview = preview;
    this.draw();
  }

  /** Drop the preview and draw the shown image again. */
  clearPreview() {
    if (!this.preview) return;
    this.preview.bitmap.close();
    this.preview = null;
    this.draw();
  }

  /**
   * Keep the preview until an image of the document after the next change
   * shows: a commit's result looks the same, so the canvas does not flash
   * back to the old image while the new one renders.
   */
  holdPreview() {
    if (this.preview) this.preview.until = this.gen + 1;
  }

  /** Start a render now; while one is in flight, check again when it ends. */
  kick() {
    this.later.cancel();
    if (this.flight) return;
    const flight = this.render().finally(() => {
      // A refit dropped this render and started another: leave that one be.
      if (this.flight !== flight) return;
      this.flight = null;
      if (this.needs()) this.kick();
    });
    this.flight = flight;
  }

  /** Report a failed render and stop retrying until the document or page changes. */
  fail(env) {
    this.halted = { gen: this.gen, page: this.hooks.page() };
    this.clearPreview();
    this.hooks.failed(env);
  }

  async render() {
    const seq = this.seq;
    const gen = this.gen;
    this.slow = setTimeout(() => this.hooks.busy(true), SLOW_MS);
    try {
      await this.renderOnce(seq, gen);
    } finally {
      clearTimeout(this.slow);
      this.hooks.busy(false);
    }
  }

  async renderOnce(seq, gen) {
    const page = this.hooks.page();
    if (!this.sizes.has(page) && !(await this.probe(seq))) return;
    if (seq !== this.seq) return;
    const size = this.sizes.get(this.hooks.page());
    if (this.fitNext && size) {
      this.fitNext = false;
      this.view.setPage(size.w, size.h, { fit: true });
    }
    const t = this.target();
    if (!t.viewport) return;
    const v = t.viewport;
    const request = { scale: t.scale, viewport: { x: v.x, y: v.y, w: v.w, h: v.h } };
    const env = await this.engine.run("doc.render", request);
    await this.show(env, seq, gen, request);
  }

  /**
   * The `doc.render` step for a keystroke batch: the window and scale the
   * view needs now. `null` when the renderer cannot say yet (page size
   * unknown, first fit pending, page off-screen): the page then renders
   * the usual way.
   */
  frameStep() {
    if (this.fitNext || !this.sizes.has(this.hooks.page())) return null;
    const t = this.target();
    if (!t.viewport) return null;
    const v = t.viewport;
    return { command: "doc.render", params: { scale: t.scale, viewport: { x: v.x, y: v.y, w: v.w, h: v.h } } };
  }

  /**
   * The document changed and a batch already rendered it: `env` is the
   * envelope of the batch's `doc.render` step (`step` its request). Shows
   * it as a render of the new document; a render in flight for the old one
   * is dropped.
   */
  async acceptFrame(env, step) {
    this.gen++;
    this.halted = null;
    this.seq++;
    this.later.cancel();
    await this.show(env, this.seq, this.gen, step.params);
    if (this.needs()) this.later();
  }

  /** Show the envelope `env` of a `doc.render` of `request` (seq/gen as sent). */
  async show(env, seq, gen, request) {
    if (seq !== this.seq || env.offline) return;
    if (!env.ok || !env.image) {
      this.fail(env);
      return;
    }
    let bitmap;
    try {
      bitmap = await createImageBitmap(await this.engine.image(env.image));
    } catch (err) {
      if (err instanceof EngineUnavailable) this.hooks.offline(err);
      else this.fail({ ok: false, error: { code: err.code ?? "edit.image_failed", message: err.message } });
      return;
    }
    if (seq !== this.seq) {
      bitmap.close();
      return;
    }
    const r = env.result;
    this.sizes.set(r.page, r.page_size);
    const old = this.shown;
    this.shown = { ...r, bitmap, gen, request };
    old?.bitmap?.close();
    if (this.preview && this.preview.until !== undefined && gen >= this.preview.until) {
      this.preview.bitmap.close();
      this.preview = null;
    }
    this.renders++;
    if (r.page_size.w !== this.view.pageW || r.page_size.h !== this.view.pageH) {
      this.view.setPage(r.page_size.w, r.page_size.h);
    }
    this.draw();
    this.hooks.rendered(r);
  }

  /**
   * Learn the size of the session page: a 1 × 1 device px render at its
   * origin. `false` when it failed (reported).
   */
  async probe(seq) {
    const env = await this.engine.run("doc.render", { scale: 1, viewport: { x: 0, y: 0, w: 1, h: 1 } });
    if (seq !== this.seq || env.offline) return false;
    if (!env.ok) {
      this.fail(env);
      return false;
    }
    this.sizes.set(env.result.page, env.result.page_size);
    return true;
  }
}
