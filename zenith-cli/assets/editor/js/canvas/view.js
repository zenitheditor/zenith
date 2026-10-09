// The canvas viewport: zoom and pan over one page, in page px.
//
// Screen point = viewport origin + pan + page point × zoom. The page frame
// and the overlay move with a CSS transform and are sized to `page × zoom`;
// the overlay draws in page coordinates through its viewBox. The page raster
// is drawn by `PageCanvas`. `panX`/`panY` hold the exact pan; the view shows
// `px`/`py`, the pan snapped to whole device pixels (after the viewport's own
// fractional offset), so a render at `zoom × dpr` lands 1:1 on device pixels.
// Snapping only what is shown keeps drags free of rounding drift.
//
// Input: Ctrl/Cmd + wheel and trackpad pinch zoom about the pointer. Wheel
// and two-finger scroll pan. Space + drag and middle-button drag pan. A
// left drag goes to the gesture layer: on a node it moves it, on empty
// canvas it draws a marquee band, so a plain left drag never pans.
// Touch: one finger taps, drags the selection, or pans elsewhere; two
// fingers pinch and pan. Keys on the focused
// viewport: + and - zoom, 0 shows 100%, 1 fits, arrows pan (when the `key`
// handler does not take them).
//
// A left press asks `grab(point, event)` for a drag. A drag gets
// `move(point, event)` once the pointer passes the click slop, then
// `end(point, event)` on release or `cancel()` (pointer cancel, a second
// finger, `cancelDrag()`). A press that never moves is a click: its drag
// gets `cancel()` and `click` runs.

const MIN_ZOOM = 0.02;
const MAX_ZOOM = 64;
const FIT_PAD = 32;
const CLICK_SLOP = 4;
const KEY_ZOOM = 1.25;

export class CanvasView {
  /**
   * `stages`: the transformed layers (page frame below the raster, overlay
   * above it). `handlers`: `click(x, y, event)`, `hover(x, y, event)`
   * (`null`s on leave), `zoomed()` after every zoom or device pixel ratio
   * change, `moved()` after every view change, `grab(point, event)` a drag
   * or `null`, `key(event)` `true` when it took the key.
   */
  constructor(viewport, stages, frame, overlay, paint, handlers) {
    this.viewport = viewport;
    this.stages = stages;
    this.frame = frame;
    this.overlay = overlay;
    this.paint = paint;
    this.handlers = handlers;
    this.vw = 0;
    this.vh = 0;
    this.zoom = 1;
    this.panX = 0;
    this.panY = 0;
    this.px = 0;
    this.py = 0;
    this.pageW = 0;
    this.pageH = 0;
    this.fitted = true;
    this.space = false;
    this.pointers = new Map();
    this.gesture = null;
    this.bind();
    new ResizeObserver(() => {
      if (this.fitted) this.fit();
      else this.apply();
    }).observe(viewport);
    this.dpr = window.devicePixelRatio || 1;
    this.watchDpr();
    window.addEventListener("resize", () => this.dprChanged());
  }

  /** Watch the device pixel ratio (browser zoom, moving between displays). */
  watchDpr() {
    const query = matchMedia(`(resolution: ${this.dpr}dppx)`);
    const changed = () => {
      query.removeEventListener("change", changed);
      this.dprChanged();
    };
    query.addEventListener("change", changed);
  }

  /** Re-apply and re-render after a device pixel ratio change. */
  dprChanged() {
    const dpr = window.devicePixelRatio || 1;
    if (dpr === this.dpr) return;
    this.dpr = dpr;
    this.watchDpr();
    this.apply();
    this.handlers.zoomed();
  }

  /** The viewport in page px: `{x, y, w, h}`. */
  visible() {
    const z = this.zoom || 1;
    return { x: -this.px / z, y: -this.py / z, w: this.vw / z, h: this.vh / z };
  }

  /** Set the page size in page px. `fit` frames the whole page. */
  setPage(w, h, { fit = false } = {}) {
    const changed = w !== this.pageW || h !== this.pageH;
    this.pageW = w;
    this.pageH = h;
    if (fit || (changed && this.fitted)) this.fit();
    else this.apply();
  }

  /** The page point under client point `(cx, cy)`. */
  toPage(cx, cy) {
    const r = this.viewport.getBoundingClientRect();
    return { x: (cx - r.left - this.px) / this.zoom, y: (cy - r.top - this.py) / this.zoom };
  }

  fit() {
    const r = this.viewport.getBoundingClientRect();
    if (!this.pageW || !this.pageH || r.width <= 0 || r.height <= 0) return;
    const pad = Math.min(FIT_PAD, r.width / 8, r.height / 8);
    const z = Math.min((r.width - 2 * pad) / this.pageW, (r.height - 2 * pad) / this.pageH);
    this.zoom = clampZoom(z);
    this.panX = (r.width - this.pageW * this.zoom) / 2;
    this.panY = (r.height - this.pageH * this.zoom) / 2;
    this.fitted = true;
    this.apply();
    this.handlers.zoomed();
  }

  /** Zoom to `z` keeping the page point under client `(cx, cy)` fixed. */
  zoomTo(z, cx, cy) {
    const r = this.viewport.getBoundingClientRect();
    const ax = cx ?? r.left + r.width / 2;
    const ay = cy ?? r.top + r.height / 2;
    const p = this.toPage(ax, ay);
    this.zoom = clampZoom(z);
    this.panX = ax - r.left - p.x * this.zoom;
    this.panY = ay - r.top - p.y * this.zoom;
    this.fitted = false;
    this.apply();
    this.handlers.zoomed();
  }

  /** 100%: one page px per CSS px, centered. */
  actualSize() {
    const r = this.viewport.getBoundingClientRect();
    this.zoom = 1;
    this.panX = (r.width - this.pageW) / 2;
    this.panY = (r.height - this.pageH) / 2;
    this.fitted = false;
    this.apply();
    this.handlers.zoomed();
  }

  panBy(dx, dy) {
    this.panX += dx;
    this.panY += dy;
    this.fitted = false;
    this.apply();
  }

  apply() {
    const { fx, fy, width, height } = this.paint.layout();
    this.vw = width;
    this.vh = height;
    // Whole device pixels: (fractional viewport offset + pan) × dpr.
    const dpr = window.devicePixelRatio || 1;
    this.px = Math.round((fx + this.panX) * dpr) / dpr - fx;
    this.py = Math.round((fy + this.panY) * dpr) / dpr - fy;
    const w = this.pageW * this.zoom;
    const h = this.pageH * this.zoom;
    for (const stage of this.stages) stage.style.transform = `translate(${this.px}px, ${this.py}px)`;
    this.frame.style.width = `${w}px`;
    this.frame.style.height = `${h}px`;
    this.frame.hidden = !(this.pageW && this.pageH);
    this.overlay.setAttribute("width", String(w));
    this.overlay.setAttribute("height", String(h));
    this.overlay.setAttribute("viewBox", `0 0 ${this.pageW || 1} ${this.pageH || 1}`);
    this.handlers.moved();
  }

  bind() {
    const v = this.viewport;
    v.addEventListener("wheel", (e) => this.onWheel(e), { passive: false });
    v.addEventListener("pointerdown", (e) => this.onDown(e));
    v.addEventListener("pointermove", (e) => this.onMove(e));
    v.addEventListener("pointerup", (e) => this.onUp(e));
    v.addEventListener("pointercancel", (e) => this.onUp(e, true));
    v.addEventListener("pointerleave", (e) => {
      if (e.pointerType !== "touch") this.handlers.hover(null, null);
    });
    // Safari sends a trackpad pinch as gesture events, not as Ctrl + wheel.
    // Other browsers never fire them.
    let pinchBase = 1;
    v.addEventListener("gesturestart", (e) => {
      e.preventDefault();
      pinchBase = this.zoom;
    });
    v.addEventListener("gesturechange", (e) => {
      e.preventDefault();
      this.zoomTo(pinchBase * e.scale, e.clientX, e.clientY);
    });
    v.addEventListener("gestureend", (e) => e.preventDefault());
    v.addEventListener("auxclick", (e) => e.button === 1 && e.preventDefault());
    v.addEventListener("keydown", (e) => this.onKey(e));
    v.addEventListener("keyup", (e) => {
      if (e.key === " ") this.setSpace(false);
    });
    v.addEventListener("blur", () => this.setSpace(false));
    // Space pans while the pointer is over the canvas, focused or not.
    window.addEventListener("keydown", (e) => {
      if (e.key !== " " || e.repeat || !v.matches(":hover")) return;
      if (e.target instanceof Element && e.target.closest(".cm-editor, input, textarea, button")) return;
      this.setSpace(true);
      e.preventDefault();
    });
    window.addEventListener("keyup", (e) => e.key === " " && this.setSpace(false));
  }

  setSpace(on) {
    this.space = on;
    this.viewport.classList.toggle("space", on);
  }

  onWheel(e) {
    e.preventDefault();
    const unit = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? 400 : 1;
    if (e.ctrlKey || e.metaKey) {
      const factor = Math.exp((-e.deltaY * unit) / 400);
      this.zoomTo(this.zoom * factor, e.clientX, e.clientY);
      return;
    }
    const dx = e.shiftKey && !e.deltaX ? e.deltaY : e.deltaX;
    const dy = e.shiftKey && !e.deltaX ? 0 : e.deltaY;
    this.panBy(-dx * unit, -dy * unit);
  }

  onDown(e) {
    this.viewport.setPointerCapture(e.pointerId);
    this.pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
    if (this.pointers.size === 2) {
      this.gesture?.drag?.cancel();
      this.gesture = { kind: "pinch", ...this.pinchState() };
      return;
    }
    if (this.pointers.size > 2) return;
    const pan = e.button === 1 || (e.button === 0 && this.space);
    const drag = !pan && e.button === 0 ? (this.handlers.grab?.(this.toPage(e.clientX, e.clientY), e) ?? null) : null;
    this.gesture = {
      kind: pan ? "pan" : "press",
      startX: e.clientX,
      startY: e.clientY,
      lastX: e.clientX,
      lastY: e.clientY,
      moved: false,
      drag,
    };
    if (pan) {
      this.viewport.classList.add("panning");
      e.preventDefault();
    } else if (e.pointerType === "mouse") {
      // Keys work after a click; no focus ring for a pointer focus.
      this.viewport.focus({ preventScroll: true, focusVisible: false });
    }
  }

  onMove(e) {
    if (this.pointers.has(e.pointerId)) this.pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
    const g = this.gesture;
    if (!g) {
      if (e.pointerType !== "touch") {
        const p = this.toPage(e.clientX, e.clientY);
        this.handlers.hover(p.x, p.y, e);
      }
      return;
    }
    if (g.kind === "pinch") {
      const now = this.pinchState();
      if (g.dist > 0) this.zoomTo(this.zoom * (now.dist / g.dist), now.cx, now.cy);
      this.panBy(now.cx - g.cx, now.cy - g.cy);
      Object.assign(g, now);
      return;
    }
    const far = Math.hypot(e.clientX - g.startX, e.clientY - g.startY) > CLICK_SLOP;
    if (far) g.moved = true;
    if (g.kind === "press" && g.drag && far) g.kind = "drag";
    if (g.kind === "drag") {
      g.drag.move(this.toPage(e.clientX, e.clientY), e);
      return;
    }
    // A one-finger drag on touch pans the canvas.
    if (g.kind === "press" && e.pointerType === "touch" && far) g.kind = "pan";
    if (g.kind === "pan") {
      this.panBy(e.clientX - g.lastX, e.clientY - g.lastY);
      g.lastX = e.clientX;
      g.lastY = e.clientY;
    }
  }

  onUp(e, cancel = false) {
    this.pointers.delete(e.pointerId);
    if (this.viewport.hasPointerCapture(e.pointerId)) this.viewport.releasePointerCapture(e.pointerId);
    const g = this.gesture;
    if (this.pointers.size > 0) {
      if (g?.kind === "pinch") this.gesture = { kind: "done" };
      return;
    }
    this.gesture = null;
    this.viewport.classList.remove("panning");
    if (g?.kind === "drag") {
      if (cancel) g.drag.cancel();
      else g.drag.end(this.toPage(e.clientX, e.clientY), e);
      return;
    }
    g?.drag?.cancel();
    if (!cancel && g?.kind === "press" && !g.moved && e.button === 0) {
      const p = this.toPage(e.clientX, e.clientY);
      this.handlers.click(p.x, p.y, e);
    }
  }

  /** Stop the drag in progress (Escape): its drag gets `cancel()`. */
  cancelDrag() {
    const g = this.gesture;
    if (!g?.drag) return false;
    const was = g.kind === "drag";
    g.drag.cancel();
    this.gesture = { kind: "done" };
    return was;
  }

  /** Set the viewport cursor: a CSS cursor value, or `""` for the default. */
  setCursor(cursor) {
    if (this.viewport.style.cursor !== cursor) this.viewport.style.cursor = cursor;
  }

  pinchState() {
    const [a, b] = [...this.pointers.values()];
    return { dist: Math.hypot(a.x - b.x, a.y - b.y), cx: (a.x + b.x) / 2, cy: (a.y + b.y) / 2 };
  }

  onKey(e) {
    if (this.handlers.key?.(e)) {
      e.preventDefault();
      return;
    }
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    const step = e.shiftKey ? 200 : 40;
    const keys = {
      "+": () => this.zoomTo(this.zoom * KEY_ZOOM),
      "=": () => this.zoomTo(this.zoom * KEY_ZOOM),
      "-": () => this.zoomTo(this.zoom / KEY_ZOOM),
      _: () => this.zoomTo(this.zoom / KEY_ZOOM),
      0: () => this.actualSize(),
      1: () => this.fit(),
      ArrowLeft: () => this.panBy(step, 0),
      ArrowRight: () => this.panBy(-step, 0),
      ArrowUp: () => this.panBy(0, step),
      ArrowDown: () => this.panBy(0, -step),
      " ": () => this.setSpace(true),
    };
    const run = keys[e.key];
    if (!run) return;
    e.preventDefault();
    run();
  }
}

function clampZoom(z) {
  return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, z));
}
