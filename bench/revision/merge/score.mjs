// Arm-neutral row scoring. The harness runs it after the agent stops; the agent
// never sees it. Inputs per shipped row: the shipped PNG, plus the text
// rectangles and text that the arm's probe rebuild reports (see probe.mjs).
// Every check below is the same code for both arms.
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { decode } from "../png.mjs";
import { auditPages, inkOf } from "../build/audit.mjs";

const PLACEHOLDER = /\{\{|\}\}|\$\{|\bundefined\b|\bnull\b|\bNaN\b|\[object|\bdata\.[a-z]|\bN\/A\b|\bTBD\b|lorem ipsum/i;
const LABEL = { title: /\b(job\s+)?title\b/i, company: /\b(company|organi[sz]ation)\b/i };

/** Fraction of pixels whose largest channel difference exceeds 48. */
function pixelDiff(a, b) {
  if (a.width !== b.width || a.height !== b.height) return 1;
  let n = 0;
  for (let i = 0; i < a.data.length; i += 4) {
    const d = Math.max(Math.abs(a.data[i] - b.data[i]), Math.abs(a.data[i + 1] - b.data[i + 1]), Math.abs(a.data[i + 2] - b.data[i + 2]));
    if (d > 48) n++;
  }
  return n / (a.width * a.height);
}

/**
 * Missing-glyph boxes inside `rect`. A missing glyph draws the font's .notdef
 * glyph, a hollow upright rectangle, in Chromium and in Zenith alike. The test
 * looks for an ink component whose four edges are inked and whose inside is
 * empty. The corpus holds no real glyph of that shape (no 口, no Hangul ㅁ).
 */
export function missingGlyphBoxes(img, rect) {
  const { bg } = inkOf(img, rect);
  const x0 = Math.max(0, Math.floor(rect.x) - 1);
  const y0 = Math.max(0, Math.floor(rect.y) - 1);
  const x1 = Math.min(img.width, Math.ceil(rect.x + rect.w) + 1);
  const y1 = Math.min(img.height, Math.ceil(rect.y + rect.h) + 1);
  const w = x1 - x0;
  const h = y1 - y0;
  if (w <= 0 || h <= 0) return 0;
  const mask = new Uint8Array(w * h);
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const i = ((y0 + y) * img.width + (x0 + x)) * 4;
      const d = Math.hypot(img.data[i] - bg[0], img.data[i + 1] - bg[1], img.data[i + 2] - bg[2]);
      mask[y * w + x] = d > 40 ? 1 : 0;
    }
  }
  const seen = new Uint8Array(w * h);
  let boxes = 0;
  for (let s = 0; s < mask.length; s++) {
    if (!mask[s] || seen[s]) continue;
    const stack = [s];
    seen[s] = 1;
    const px = [];
    let bx0 = w, by0 = h, bx1 = -1, by1 = -1;
    while (stack.length) {
      const p = stack.pop();
      px.push(p);
      const x = p % w, y = (p - x) / w;
      if (x < bx0) bx0 = x;
      if (x > bx1) bx1 = x;
      if (y < by0) by0 = y;
      if (y > by1) by1 = y;
      for (const q of [p - 1, p + 1, p - w, p + w]) {
        if (q < 0 || q >= mask.length || seen[q] || !mask[q]) continue;
        if ((q === p - 1 && x === 0) || (q === p + 1 && x === w - 1)) continue;
        seen[q] = 1;
        stack.push(q);
      }
    }
    if (isHollowBox(px, w, bx0, by0, bx1 - bx0 + 1, by1 - by0 + 1)) boxes++;
  }
  return boxes;
}

function isHollowBox(px, stride, bx, by, bw, bh) {
  if (bw < 5 || bh < 8 || bh / bw < 0.9 || bh / bw > 2.6) return false;
  const t = Math.max(1, Math.round(Math.min(bw, bh) * 0.15));
  const top = new Uint8Array(bw), bottom = new Uint8Array(bw), left = new Uint8Array(bh), right = new Uint8Array(bh);
  let inner = 0;
  for (const p of px) {
    const x = (p % stride) - bx, y = Math.floor(p / stride) - by;
    if (y < t) top[x] = 1;
    if (y >= bh - t) bottom[x] = 1;
    if (x < t) left[y] = 1;
    if (x >= bw - t) right[y] = 1;
    if (x >= 2 * t && x < bw - 2 * t && y >= 2 * t && y < bh - 2 * t) inner++;
  }
  const cover = (a) => a.reduce((s, v) => s + v, 0) / a.length;
  const innerArea = Math.max(1, (bw - 4 * t) * (bh - 4 * t));
  return [top, bottom, left, right].every((a) => cover(a) >= 0.85) && inner / innerArea < 0.03;
}

const squash = (s) => s.toLowerCase().replace(/\s+/g, "");

/** The defects of one shipped row. `probe` is { png, rects, text } or { error }. */
export function rowDefects(row, img, probe, task) {
  const defects = [];
  if (img.width !== task.canvas.w || img.height !== task.canvas.h) defects.push(`size ${img.width}x${img.height}`);
  if (probe.error) return [...defects, `unverifiable: ${probe.error.slice(0, 160)}`];
  const diff = pixelDiff(img, decode(probe.png));
  if (diff > 0.005) defects.push(`probe_mismatch ${(100 * diff).toFixed(1)}%`);
  const a = auditPages([{ img, rects: probe.rects }], { margin: task.margin, minContrast: task.minContrast });
  for (const k of ["clipped", "outOfCanvas", "overlaps", "lowContrast", "invisible"]) if (a[k]) defects.push(`${k} x${a[k]}`);
  const tofu = probe.rects.reduce((n, r) => n + missingGlyphBoxes(img, r), 0);
  if (tofu) defects.push(`missing_glyph x${tofu}`);
  const hay = squash(probe.text ?? "");
  for (const want of [task.event, row.name, row.title, row.company]) {
    if (want?.trim() && !hay.includes(squash(want))) defects.push(`text_missing "${want.slice(0, 40)}"`);
  }
  const text = probe.text ?? "";
  if (PLACEHOLDER.test(text)) defects.push("placeholder");
  for (const col of ["title", "company"]) if (col in row && !row[col].trim() && LABEL[col].test(text)) defects.push(`placeholder_${col}`);
  return defects;
}

/** Ids listed in out/withheld.txt (first token of each line). */
function withheldIds(ws) {
  const f = path.join(ws, "out", "withheld.txt");
  if (!existsSync(f)) return new Set();
  return new Set(readFileSync(f, "utf8").split("\n").map((l) => l.trim().split(/[\s,:]+/)[0]).filter(Boolean));
}

/**
 * Score every CSV row. A row is defective when it ships with a defect, when it
 * is missing and not listed in withheld.txt (a silent drop), or when a benign
 * row is withheld. A hostile row withheld and listed is not a defect.
 */
export async function scoreRows({ ws, rows, truth, task, probe, onRow }) {
  const flagged = withheldIds(ws);
  const out = [];
  for (const row of rows) {
    const t = truth.get(row.id) ?? { tags: [], hostile: false };
    const file = path.join(ws, "out", `${row.id}.png`);
    const r = { id: row.id, tags: t.tags, hostile: t.hostile, shipped: existsSync(file), flagged: flagged.has(row.id), defects: [] };
    if (r.shipped) {
      let img = null;
      try {
        img = decode(readFileSync(file));
      } catch (err) {
        r.defects.push(`bad_png: ${err.message}`);
      }
      if (img) r.defects.push(...rowDefects(row, img, await probe(row), task));
      r.status = r.defects.length ? "shipped_defective" : "shipped_clean";
    } else if (!r.flagged) {
      r.status = "dropped";
      r.defects.push("dropped without a withheld entry");
    } else if (!t.hostile) {
      r.status = "withheld_benign";
      r.defects.push("benign row withheld");
    } else r.status = "withheld";
    r.defective = r.defects.length > 0;
    out.push(r);
    onRow?.(r);
  }
  return out;
}

/** Totals over scored rows. `defective` is the number the decision rule reads. */
export function totals(scored) {
  const n = (f) => scored.filter(f).length;
  const kinds = {};
  for (const r of scored) for (const d of r.defects) kinds[d.split(/[\s:]/)[0]] = (kinds[d.split(/[\s:]/)[0]] ?? 0) + 1;
  return {
    rows: scored.length,
    shipped: n((r) => r.shipped),
    shippedClean: n((r) => r.status === "shipped_clean"),
    shippedDefective: n((r) => r.status === "shipped_defective"),
    shippedDefectiveHostile: n((r) => r.status === "shipped_defective" && r.hostile),
    shippedDefectiveBenign: n((r) => r.status === "shipped_defective" && !r.hostile),
    withheld: n((r) => r.status === "withheld"),
    withheldBenign: n((r) => r.status === "withheld_benign"),
    dropped: n((r) => r.status === "dropped"),
    defective: n((r) => r.defective),
    kinds,
  };
}
