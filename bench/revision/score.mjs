// Scoring of a step: did the change land, and did the frozen regions stay put.
import { decode } from "./png.mjs";

const px = (img, x, y) => {
  const i = (y * img.width + x) * 4;
  return [img.data[i], img.data[i + 1], img.data[i + 2]];
};

/** Fraction of pixels in `rect` whose channels differ by more than `tol` between two renders. */
export function regionDrift(before, after, rect, tol = 2) {
  const a = decode(before);
  const b = decode(after);
  if (a.width !== b.width || a.height !== b.height) return 1;
  let changed = 0;
  for (let y = rect.y; y < rect.y + rect.h; y++) {
    for (let x = rect.x; x < rect.x + rect.w; x++) {
      const p = px(a, x, y);
      const q = px(b, x, y);
      if (Math.abs(p[0] - q[0]) > tol || Math.abs(p[1] - q[1]) > tol || Math.abs(p[2] - q[2]) > tol) changed++;
    }
  }
  return changed / (rect.w * rect.h);
}

/** Runs one check against the render and source. Returns { ok, detail }. */
export function runCheck(check, { png, source, background }) {
  const img = png ? decode(png) : null;
  switch (check.kind) {
    case "pixel": {
      if (!img) return { ok: false, detail: "no render" };
      const got = px(img, check.x, check.y);
      const ok = got.every((c, i) => Math.abs(c - check.rgb[i]) <= (check.tol ?? 2));
      return { ok, detail: `(${check.x},${check.y}) = ${got.join(",")}, want ${check.rgb.join(",")}` };
    }
    case "ink": {
      if (!img) return { ok: false, detail: "no render" };
      let n = 0;
      for (let y = check.y; y < check.y + check.h; y++) {
        for (let x = check.x; x < check.x + check.w; x++) {
          const p = px(img, x, y);
          if (p.some((c, i) => Math.abs(c - background[i]) > 12)) n++;
        }
      }
      const ok = (check.min === undefined || n >= check.min) && (check.max === undefined || n <= check.max);
      return { ok, detail: `${n} ink pixels, want ${check.min !== undefined ? `>= ${check.min}` : ""}${check.max !== undefined ? `<= ${check.max}` : ""}` };
    }
    case "colorcount": {
      if (!img) return { ok: false, detail: "no render" };
      let n = 0;
      for (let y = check.y; y < check.y + check.h; y++) {
        for (let x = check.x; x < check.x + check.w; x++) {
          const p = px(img, x, y);
          if (p.every((c, i) => Math.abs(c - check.rgb[i]) <= check.tol)) n++;
        }
      }
      return { ok: n >= check.min, detail: `${n} pixels near ${check.rgb.join(",")} at (${check.x},${check.y}), want >= ${check.min}` };
    }
    case "source-contains":
      return { ok: source.includes(check.text), detail: `contains "${check.text}"` };
    case "source-absent":
      return { ok: !source.includes(check.text), detail: `absent "${check.text}"` };
    default:
      throw new Error(`unknown check kind ${check.kind}`);
  }
}
