// The window a canvas render asks for: the visible part of the page plus a
// margin, at the exact device scale, inside the engine's region limits.
// Pure math; `unit.js` tests it.

/** The engine's region limits (see the zenith-editor "Viewport render" docs). */
export const LIMITS = { side: 8191, pixels: 16777216, extent: 1048576 };

/** Margin on each side, as a fraction of the visible size. */
export const MARGIN = 0.5;

/** Device px the engine adds when it snaps a window out to whole pixels. */
const SNAP = 2;

/** The overlap of page rects `a` and `b` (`{x, y, w, h}`), or `null`. */
export function intersect(a, b) {
  const x = Math.max(a.x, b.x);
  const y = Math.max(a.y, b.y);
  const w = Math.min(a.x + a.w, b.x + b.w) - x;
  const h = Math.min(a.y + a.h, b.y + b.h) - y;
  return w > 0 && h > 0 ? { x, y, w, h } : null;
}

/**
 * `true` when page rect `outer` holds page rect `inner`, give or take `eps`
 * page px. The engine clamps a window to the page's rounded device size, so
 * a window can stop up to half a device px short of the page edge: callers
 * pass one device px (`1 / scale`).
 */
export function contains(outer, inner, eps = 1e-6) {
  return (
    inner.x >= outer.x - eps &&
    inner.y >= outer.y - eps &&
    inner.x + inner.w <= outer.x + outer.w + eps &&
    inner.y + inner.h <= outer.y + outer.h + eps
  );
}

/** `true` when `rect` (page px) fits the region limits at `scale`. */
export function fits(rect, scale) {
  const w = rect.w * scale + SNAP;
  const h = rect.h * scale + SNAP;
  return w <= LIMITS.side && h <= LIMITS.side && w * h <= LIMITS.pixels;
}

/** `true` when `a` and `b` are the same scale. */
export function sameScale(a, b) {
  return Math.abs(a - b) <= 1e-9 * Math.max(a, b);
}

/**
 * The render to ask for: `{scale, viewport, visible}` or `null` when no part
 * of the page is visible.
 *
 * `visible` is the viewport in page px, `page` the page size `{w, h}` in page
 * px, `scale` the wanted device scale (`zoom × devicePixelRatio`). The scale
 * drops only when the visible part alone passes the limits (then the image
 * shows a little soft). The margin shrinks until the window fits.
 */
export function plan({ visible, page, scale }) {
  const pageRect = { x: 0, y: 0, w: page.w, h: page.h };
  const vis = intersect(visible, pageRect);
  if (!vis) return null;
  let s = Math.min(scale, (LIMITS.extent - 1) / Math.max(page.w, page.h));
  if (!fits(vis, s)) {
    const side = Math.min((LIMITS.side - SNAP) / vis.w, (LIMITS.side - SNAP) / vis.h);
    const area = Math.sqrt(LIMITS.pixels / ((vis.w + SNAP / s) * (vis.h + SNAP / s)));
    s = Math.min(s, side, area) * 0.999;
  }
  let k = MARGIN;
  let viewport = vis;
  for (let i = 0; i < 12; i++) {
    const grown = intersect(
      { x: vis.x - vis.w * k, y: vis.y - vis.h * k, w: vis.w * (1 + 2 * k), h: vis.h * (1 + 2 * k) },
      pageRect,
    );
    if (grown && fits(grown, s)) {
      viewport = grown;
      break;
    }
    k /= 2;
  }
  return { scale: s, viewport, visible: vis };
}
