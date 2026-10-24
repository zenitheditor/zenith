// Scoring of a step: did the change land, did the frozen regions stay put, and
// how many tracked elements outside the step's intended set changed (collateral).
// Pages are addressed two ways: a check's `page` is the 1-based position in the
// oracle order after the step (`nextOrder`); a frozen region's or element's `page`
// is a page key that survives inserts, deletes and reorders. steps.mjs maps both
// to positions in the actual render.
import { decode } from "./png.mjs";

const px = (img, x, y) => {
  const i = (y * img.width + x) * 4;
  return [img.data[i], img.data[i + 1], img.data[i + 2]];
};

const near = (p, q, tol) => p.every((c, i) => Math.abs(c - q[i]) <= tol);

/** Number of pixels in `rect` that differ by more than `tol` per channel, skipping points inside `mask` rects. */
export function changedPixels(a, b, rect, mask = [], tol = 2) {
  if (!a || !b || a.width !== b.width || a.height !== b.height) return rect.w * rect.h;
  let changed = 0;
  const x1 = Math.min(rect.x + rect.w, a.width);
  const y1 = Math.min(rect.y + rect.h, a.height);
  for (let y = Math.max(0, rect.y); y < y1; y++) {
    for (let x = Math.max(0, rect.x); x < x1; x++) {
      if (mask.some((m) => x >= m.x && x < m.x + m.w && y >= m.y && y < m.y + m.h)) continue;
      if (!near(px(a, x, y), px(b, x, y), tol)) changed++;
    }
  }
  return changed;
}

/** Fraction of pixels in `rect` that differ between two PNG renders. */
export function regionDrift(before, after, rect, tol = 2) {
  return changedPixels(decode(before), decode(after), rect, [], tol) / (rect.w * rect.h);
}

/** The page-key order after a step's page operation. Positions are 1-based. */
export function nextOrder(order, op) {
  const out = [...order];
  if (!op) return out;
  if (op.insert) out.splice(op.insert - 1, 0, op.key);
  if (op.delete) out.splice(op.delete - 1, 1);
  if (op.swap) {
    const [a, b] = op.swap.map((n) => n - 1);
    [out[a], out[b]] = [out[b], out[a]];
  }
  return out;
}

const glob = (pattern, id) => new RegExp(`^${pattern.replace(/[.+?^${}()|[\]\\]/g, "\\$&").replace(/\*/g, ".*")}$`).test(id);

/**
 * Element ids that changed outside `step.touch`. `elements` is the run's current
 * element list. Intended element boxes and `step.mask` rects are not compared.
 * Elements on a page the step removes are skipped.
 */
export function collateral(task, step, prev, after, elements) {
  const full = { x: 0, y: 0, w: task.canvas.w, h: task.canvas.h };
  const intended = elements.filter((e) => (step.touch ?? []).some((t) => glob(t, e.id)));
  const out = [];
  for (const e of elements) {
    if (intended.includes(e) || !after.has(e.page)) continue;
    const a = prev.image(e.page);
    if (!a) continue;
    const mask = [...intended, ...(step.mask ?? [])].filter((m) => (m.page ?? "p1") === (e.page ?? "p1"));
    if (changedPixels(a, after.image(e.page), e.w ? e : full, mask) > 4) out.push(e.id);
  }
  return out;
}

/** The element list after a step: `step.relocate` entries move or resize the matching elements by dx, dy, dw, dh. */
export function relocate(elements, step) {
  return elements.map((e) => {
    const r = (step.relocate ?? []).find((m) => glob(m.ids, e.id));
    return r ? { ...e, x: e.x + (r.dx ?? 0), y: e.y + (r.dy ?? 0), w: e.w + (r.dw ?? 0), h: e.h + (r.dh ?? 0) } : e;
  });
}

/** Frozen regions: { name, drift } with drift the changed fraction (1 when the page is missing). */
export function frozenDrift(task, step, prev, after) {
  const full = { x: 0, y: 0, w: task.canvas.w, h: task.canvas.h };
  return (step.frozen ?? []).map((r) => {
    const rect = r.w ? r : full;
    const a = prev.image(r.page ?? "p1");
    const b = after.image(r.page ?? "p1");
    return { name: r.name, drift: a && b ? changedPixels(a, b, rect) / (rect.w * rect.h) : 1 };
  });
}

function inkPixels(img, r, background) {
  let n = 0;
  let x0 = Infinity, x1 = -Infinity, y0 = Infinity, y1 = -Infinity;
  for (let y = r.y; y < r.y + r.h; y++) {
    for (let x = r.x; x < r.x + r.w; x++) {
      if (!near(px(img, x, y), background, 12)) {
        n++;
        x0 = Math.min(x0, x), x1 = Math.max(x1, x), y0 = Math.min(y0, y), y1 = Math.max(y1, y);
      }
    }
  }
  return { count: n, w: n ? x1 - x0 + 1 : 0, h: n ? y1 - y0 + 1 : 0 };
}

const count = (hay, needle, ci) => (ci ? hay.toLowerCase().split(needle.toLowerCase()) : hay.split(needle)).length - 1;

/**
 * Runs one check. `ctx` holds `after`, `prev` and `start` views (each with
 * `byPos(n)` decoded images and `count`), `rects` per page, `source` and `background`.
 * Returns { ok, detail }.
 */
export function runCheck(check, ctx) {
  const pos = check.page ?? 1;
  const img = ctx.after.byPos(pos);
  const bg = check.background ?? ctx.background;
  const needImg = !["source-contains", "source-absent", "source-count", "page-count", "no-clip"].includes(check.kind);
  if (needImg && !img) return { ok: false, detail: `no render of page ${pos}` };
  switch (check.kind) {
    case "pixel": {
      const got = px(img, check.x, check.y);
      return { ok: near(got, check.rgb, check.tol ?? 2), detail: `p${pos} (${check.x},${check.y}) = ${got.join(",")}, want ${check.rgb.join(",")}` };
    }
    case "ink": {
      const n = inkPixels(img, check, bg).count;
      const ok = (check.min === undefined || n >= check.min) && (check.max === undefined || n <= check.max);
      return { ok, detail: `p${pos} ${n} ink pixels, want ${check.min !== undefined ? `>= ${check.min} ` : ""}${check.max !== undefined ? `<= ${check.max}` : ""}` };
    }
    case "colorcount": {
      let n = 0;
      for (let y = check.y; y < check.y + check.h; y++) {
        for (let x = check.x; x < check.x + check.w; x++) if (near(px(img, x, y), check.rgb, check.tol)) n++;
      }
      const ok = (check.min === undefined || n >= check.min) && (check.max === undefined || n <= check.max);
      return { ok, detail: `p${pos} ${n} pixels near ${check.rgb.join(",")} at (${check.x},${check.y}), want ${check.min ?? 0}..${check.max ?? "∞"}` };
    }
    case "ink-ratio": {
      // Ink extent (`w`, `h`) or ink pixel `count` in the region, divided by the same measure in a reference render.
      const ref = ctx[check.ref ?? "start"].byPos(check.ref_page ?? pos);
      if (!ref) return { ok: false, detail: `no reference page ${check.ref_page ?? pos}` };
      const m = check.metric ?? "w";
      const a = inkPixels(img, check, bg)[m];
      const b = inkPixels(ref, check, bg)[m];
      const ratio = b ? a / b : 0;
      return { ok: ratio >= (check.min ?? 0) && ratio <= (check.max ?? Infinity), detail: `p${pos} ink ${m} ratio ${ratio.toFixed(3)}, want ${check.min ?? 0}..${check.max ?? "∞"}` };
    }
    case "same-as": {
      // The region after the step equals a region of a reference render, offset by (dx, dy) there.
      const ref = ctx[check.ref ?? "prev"].byPos(check.ref_page ?? pos);
      if (!ref) return { ok: false, detail: `no reference page ${check.ref_page ?? pos}` };
      const r = check.w ? check : { x: 0, y: 0, w: img.width, h: img.height };
      let diff = 0;
      for (let y = r.y; y < r.y + r.h; y++) {
        for (let x = r.x; x < r.x + r.w; x++) if (!near(px(img, x, y), px(ref, x + (check.dx ?? 0), y + (check.dy ?? 0)), check.tol ?? 2)) diff++;
      }
      return { ok: diff <= (check.max ?? 4), detail: `p${pos} differs from ${check.ref ?? "prev"} p${check.ref_page ?? pos} in ${diff} pixels` };
    }
    case "changed": {
      const ref = ctx[check.ref ?? "prev"].byPos(check.ref_page ?? pos);
      if (!ref) return { ok: false, detail: `no reference page ${check.ref_page ?? pos}` };
      const n = changedPixels(img, ref, check);
      return { ok: n >= (check.min ?? 20), detail: `p${pos} ${n} pixels changed vs ${check.ref ?? "prev"}` };
    }
    case "no-clip": {
      // No text on the page (or inside the optional region) leaves its box.
      const pages = check.page ? [check.page] : ctx.rects.map((_, i) => i + 1);
      const bad = pages.flatMap((p) =>
        (ctx.rects[p - 1] ?? []).filter((t) => t.clipped && (!check.w || (t.x < check.x + check.w && t.x + t.w > check.x && t.y < check.y + check.h && t.y + t.h > check.y))).map((t) => `p${p} ${t.id ?? `(${Math.round(t.x)},${Math.round(t.y)})`}`),
      );
      const missing = pages.filter((p) => !ctx.rects[p - 1]);
      return { ok: !bad.length && !missing.length, detail: bad.length ? `clipped: ${bad.join(", ")}` : missing.length ? `no page ${missing.join(",")}` : "no clipped text" };
    }
    case "page-count":
      return { ok: ctx.after.count === check.n, detail: `${ctx.after.count} pages, want ${check.n}` };
    case "source-contains":
      return { ok: count(ctx.source, check.text, check.ci) > 0, detail: `contains "${check.text}"` };
    case "source-absent":
      return { ok: count(ctx.source, check.text, check.ci) === 0, detail: `absent "${check.text}"` };
    case "source-count": {
      const n = count(ctx.source, check.text, check.ci);
      return { ok: n >= (check.min ?? 0) && n <= (check.max ?? Infinity), detail: `"${check.text}" occurs ${n} times, want ${check.min ?? 0}..${check.max ?? "∞"}` };
    }
    default:
      throw new Error(`unknown check kind ${check.kind}`);
  }
}
