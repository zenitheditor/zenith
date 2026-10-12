// Arm-neutral audit of a built design. Input per page: a decoded render and the
// text rectangles the arm reports (HTML: DOM client rects; Zenith: ink bounds).
// Every metric comes from the rendered pixels and those rectangles, with the
// same code for both arms. The rectangles are tightened to their ink first.

const lum = ([r, g, b]) => {
  const f = (c) => ((c /= 255) <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
  return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
};
const ratio = (a, b) => {
  const [hi, lo] = [lum(a), lum(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
};
const at = (img, x, y) => {
  const i = (y * img.width + x) * 4;
  return [img.data[i], img.data[i + 1], img.data[i + 2]];
};
const dist = (a, b) => Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);
const q = (c) => c.map((v) => v >> 4).join(",");

/** Most common colour among the pixels of `rect` expanded by `pad`, excluding the inside when `ringOnly`. */
function modeColor(img, rect, pad, ringOnly) {
  const x0 = Math.max(0, Math.floor(rect.x - pad));
  const y0 = Math.max(0, Math.floor(rect.y - pad));
  const x1 = Math.min(img.width, Math.ceil(rect.x + rect.w + pad));
  const y1 = Math.min(img.height, Math.ceil(rect.y + rect.h + pad));
  const counts = new Map();
  for (let y = y0; y < y1; y++) {
    for (let x = x0; x < x1; x++) {
      if (ringOnly && x >= rect.x && x < rect.x + rect.w && y >= rect.y && y < rect.y + rect.h) continue;
      const c = at(img, x, y);
      const k = q(c);
      const e = counts.get(k) ?? { n: 0, c };
      e.n++;
      counts.set(k, e);
    }
  }
  let best = null;
  for (const e of counts.values()) if (!best || e.n > best.n) best = e;
  return best ? best.c : [255, 255, 255];
}

/** Ink lines (one rectangle per band of ink rows) and text colour of `rect`; no lines when nothing is drawn. */
export function inkOf(img, rect) {
  const bg = modeColor(img, rect, 3, true);
  const x0 = Math.max(0, Math.floor(rect.x));
  const y0 = Math.max(0, Math.floor(rect.y));
  const x1 = Math.min(img.width, Math.ceil(rect.x + rect.w));
  const y1 = Math.min(img.height, Math.ceil(rect.y + rect.h));
  const rows = [];
  const px = [];
  let far = 0;
  for (let y = y0; y < y1; y++) {
    let minX = Infinity, maxX = -1;
    for (let x = x0; x < x1; x++) {
      const c = at(img, x, y);
      const d = dist(c, bg);
      if (d > 40) {
        px.push([c, d]);
        far = Math.max(far, d);
        if (x < minX) minX = x;
        if (x > maxX) maxX = x;
      }
    }
    rows.push(maxX >= 0 ? { y, minX, maxX } : null);
  }
  if (!px.length) return { bg, fg: bg, lines: [] };
  const strong = px.filter(([, d]) => d >= far * 0.8);
  const fg = [0, 1, 2].map((i) => Math.round(strong.reduce((s, [c]) => s + c[i], 0) / strong.length));
  const lines = [];
  let cur = null;
  for (const row of rows) {
    if (row) {
      if (!cur) cur = { x0: row.minX, x1: row.maxX, y0: row.y, y1: row.y };
      else {
        cur.x0 = Math.min(cur.x0, row.minX);
        cur.x1 = Math.max(cur.x1, row.maxX);
        cur.y1 = row.y;
      }
    } else if (cur) {
      lines.push({ x: cur.x0, y: cur.y0, w: cur.x1 - cur.x0 + 1, h: cur.y1 - cur.y0 + 1 });
      cur = null;
    }
  }
  if (cur) lines.push({ x: cur.x0, y: cur.y0, w: cur.x1 - cur.x0 + 1, h: cur.y1 - cur.y0 + 1 });
  return { bg, fg, lines };
}

const overlapArea = (a, b) => {
  const w = Math.min(a.x + a.w, b.x + b.w) - Math.max(a.x, b.x);
  const h = Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y);
  return w > 0 && h > 0 ? w * h : 0;
};

/**
 * pages: [{ img, rects: [{x,y,w,h, clipped?}] }]. Returns counts and the first few examples of each issue.
 */
export function auditPages(pages, { margin, minContrast = 3 }) {
  const out = { texts: 0, invisible: 0, outOfCanvas: 0, tooClose: 0, overlaps: 0, lowContrast: 0, clipped: 0, notes: [] };
  const note = (s) => out.notes.length < 12 && out.notes.push(s);
  pages.forEach((page, pi) => {
    const { img } = page;
    const inks = [];
    for (const r of page.rects) {
      out.texts++;
      if (r.clipped) {
        out.clipped++;
        note(`p${pi + 1}: text clipped at (${Math.round(r.x)},${Math.round(r.y)})`);
      }
      if (r.x < -4 || r.y < -4 || r.x + r.w > img.width + 4 || r.y + r.h > img.height + 4) {
        out.outOfCanvas++;
        note(`p${pi + 1}: text leaves the canvas at (${Math.round(r.x)},${Math.round(r.y)} ${Math.round(r.w)}x${Math.round(r.h)})`);
      }
      const { bg, fg, lines } = inkOf(img, r);
      if (!lines.length) {
        out.invisible++;
        note(`p${pi + 1}: text at (${Math.round(r.x)},${Math.round(r.y)}) draws no ink`);
        continue;
      }
      inks.push(...lines);
      if (ratio(fg, bg) < minContrast) {
        out.lowContrast++;
        note(`p${pi + 1}: contrast ${ratio(fg, bg).toFixed(1)} at (${lines[0].x},${lines[0].y})`);
      }
      for (const ink of lines) {
        const near = Math.min(ink.x, ink.y, img.width - (ink.x + ink.w), img.height - (ink.y + ink.h));
        if (near < margin) {
          out.tooClose++;
          note(`p${pi + 1}: text ${near}px from the edge at (${ink.x},${ink.y}), want >= ${margin}`);
          break;
        }
      }
    }
    for (let i = 0; i < inks.length; i++) {
      for (let j = i + 1; j < inks.length; j++) {
        const a = overlapArea(inks[i], inks[j]);
        if (a > 0.25 * Math.min(inks[i].w * inks[i].h, inks[j].w * inks[j].h)) {
          out.overlaps++;
          note(`p${pi + 1}: text overlaps text at (${inks[i].x},${inks[i].y})`);
        }
      }
    }
  });
  return out;
}

/** Fraction of required strings found in `haystack`, plus the missing ones. */
export function contentScore(haystack, required) {
  const norm = (s) => s.replace(/\s+/g, " ").trim();
  const hay = norm(haystack);
  const missing = required.filter((r) => !hay.includes(norm(r)));
  return { found: required.length - missing.length, total: required.length, missing };
}
