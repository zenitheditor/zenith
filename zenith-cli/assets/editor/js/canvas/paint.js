// The page raster on screen: one <canvas> over the viewport whose backing
// store maps 1:1 to device pixels.
//
// An <img> scaled by CSS is resampled by the browser at most device pixel
// ratios, even at the "right" size. A canvas drawn at whole device pixels is
// not, when the canvas box itself starts on a whole CSS pixel. So the canvas
// is shifted left/up by the fractional part of the viewport position, and
// sized in whole CSS px; its backing store is that size × devicePixelRatio.
// At a render's own scale the image is drawn at integer device coordinates
// without smoothing: one image pixel per device pixel. At any other scale
// (between a zoom and its render) it is drawn scaled and smoothed.

export class PageCanvas {
  constructor(viewport, canvas) {
    this.viewport = viewport;
    this.canvas = canvas;
    this.ctx = canvas.getContext("2d");
    /** Viewport position: fractional CSS px of its left/top edge. */
    this.fx = 0;
    this.fy = 0;
    this.dpr = 1;
    /** Where the last draw put the image (see `placement`), for tests. */
    this.last = null;
  }

  /**
   * Size the canvas to the viewport. Returns the fractional offsets
   * `{fx, fy}` (CSS px) the view needs to keep the page on whole device
   * pixels, and the viewport size `{width, height}` (CSS px).
   */
  layout() {
    const r = this.viewport.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    const fx = r.left - Math.floor(r.left);
    const fy = r.top - Math.floor(r.top);
    const cw = Math.max(1, Math.ceil(r.width + fx));
    const ch = Math.max(1, Math.ceil(r.height + fy));
    const bw = Math.round(cw * dpr);
    const bh = Math.round(ch * dpr);
    const c = this.canvas;
    c.style.left = `${-fx}px`;
    c.style.top = `${-fy}px`;
    c.style.width = `${cw}px`;
    c.style.height = `${ch}px`;
    if (c.width !== bw) c.width = bw;
    if (c.height !== bh) c.height = bh;
    this.fx = fx;
    this.fy = fy;
    this.dpr = dpr;
    return { fx, fy, width: r.width, height: r.height };
  }

  /**
   * Draw `shown` (`{bitmap, rect, scale}`, rect in device px of the page at
   * `scale`) for the view `{zoom, px, py}` (`px`/`py`: the shown pan, on
   * whole device pixels). Clears first; draws nothing without a bitmap.
   */
  draw(shown, view) {
    const g = this.ctx;
    const c = this.canvas;
    g.setTransform(1, 0, 0, 1, 0, 0);
    g.clearRect(0, 0, c.width, c.height);
    this.last = null;
    if (!shown?.bitmap) return null;
    const place = placement(shown, view, this.fx, this.fy, this.dpr);
    this.last = place;
    g.imageSmoothingEnabled = !place.exact;
    g.imageSmoothingQuality = "high";
    if (place.exact) g.drawImage(shown.bitmap, place.x, place.y);
    else g.drawImage(shown.bitmap, place.x, place.y, place.w, place.h);
    return place;
  }
}

/**
 * Where `shown` lands in canvas backing pixels: `{x, y, w, h, exact}`.
 * `exact` means 1:1 at whole device pixels.
 */
export function placement(shown, view, fx, fy, dpr) {
  const { rect, scale } = shown;
  const ox = (fx + view.px) * dpr;
  const oy = (fy + view.py) * dpr;
  const want = view.zoom * dpr;
  const exact = Math.abs(scale - want) <= 1e-9 * Math.max(scale, want);
  if (exact) {
    return { x: Math.round(ox) + rect.x, y: Math.round(oy) + rect.y, w: rect.w, h: rect.h, exact };
  }
  const k = want / scale;
  return { x: ox + rect.x * k, y: oy + rect.y * k, w: rect.w * k, h: rect.h * k, exact };
}
