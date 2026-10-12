// Renders every page of a built design and reports its text rectangles.
// Zenith: `zenith render --all-pages` plus `zenith inspect --json`. HTML: one
// Chromium screenshot of the stacked `.page` sections, cropped per page.
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import path from "node:path";
import { readFileSync, mkdtempSync, rmSync, readdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { pathToFileURL } from "node:url";
import { zenithBin } from "../render.mjs";
import { decode, encode, crop } from "../png.mjs";

const run = promisify(execFile);

function textRects(node, out) {
  if ((node.kind === "text" || node.kind === "code") && node.bounds) {
    const b = node.bounds;
    const box = node.box ?? b;
    out.push({
      x: b.x,
      y: b.y,
      w: b.w,
      h: b.h,
      clipped: b.y + b.h > box.y + box.h + 2 || b.x + b.w > box.x + box.w + 2,
    });
  }
  for (const child of node.children ?? []) textRects(child, out);
}

/** { pages: [{ png, rects, w, h }], source } or { error }. */
export async function zenithPages(ws) {
  const dir = mkdtempSync(path.join(tmpdir(), "zbuild-"));
  try {
    const env = { ...process.env };
    const { stdout } = await run(zenithBin(), ["inspect", "design.zen", "--json"], { cwd: ws, timeout: 60000, maxBuffer: 16 << 20, env });
    const info = JSON.parse(stdout);
    await run(zenithBin(), ["render", "design.zen", "--all-pages", dir], { cwd: ws, timeout: 120000, maxBuffer: 16 << 20, env });
    const files = readdirSync(dir).filter((f) => f.endsWith(".png")).sort((a, b) => Number(a.match(/\d+/)) - Number(b.match(/\d+/)));
    const pages = info.pages.map((p, i) => {
      const rects = [];
      for (const child of p.children ?? []) textRects(child, rects);
      return { png: readFileSync(path.join(dir, files[i])), rects, w: p.width, h: p.height };
    });
    return { pages, source: readFileSync(path.join(ws, "design.zen"), "utf8") };
  } catch (err) {
    return { error: `${err.stderr ?? ""}${err.stdout ?? ""}`.trim().slice(0, 3000) || String(err.message ?? err) };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

const EXTRACT = `(() => {
  const pages = [...document.querySelectorAll('.page')];
  return pages.map((p) => {
    const pr = p.getBoundingClientRect();
    const rects = [];
    const walker = document.createTreeWalker(p, NodeFilter.SHOW_TEXT);
    while (walker.nextNode()) {
      const n = walker.currentNode;
      if (!n.textContent.trim()) continue;
      const el = n.parentElement;
      const cs = getComputedStyle(el);
      if (cs.visibility === 'hidden' || cs.display === 'none') continue;
      const r = document.createRange();
      r.selectNodeContents(n);
      const rs = [...r.getClientRects()].filter((x) => x.width >= 1 && x.height >= 1);
      if (!rs.length) continue;
      const x0 = Math.min(...rs.map((x) => x.left)), y0 = Math.min(...rs.map((x) => x.top));
      const x1 = Math.max(...rs.map((x) => x.right)), y1 = Math.max(...rs.map((x) => x.bottom));
      let clipped = false;
      for (let a = el; a && a !== p.parentElement; a = a.parentElement) {
        const s = getComputedStyle(a);
        if (s.overflowX !== 'visible' || s.overflowY !== 'visible') {
          const ar = a.getBoundingClientRect();
          if (x0 < ar.left - 2 || y0 < ar.top - 2 || x1 > ar.right + 2 || y1 > ar.bottom + 2) clipped = true;
        }
      }
      rects.push({ x: x0 - pr.left, y: y0 - pr.top, w: x1 - x0, h: y1 - y0, clipped });
    }
    return { w: pr.width, h: pr.height, top: pr.top + window.scrollY, rects, text: p.innerText };
  });
})()`;

/** Returns a function (ws) => { pages, source } | { error } over one reused CDP page. */
export async function htmlPages(browser, sizes) {
  const page = await browser.newPage();
  const wide = Math.max(...sizes.map((s) => s.w));
  const tall = Math.max(...sizes.map((s) => s.h));
  return async (ws) => {
    const out = mkdtempSync(path.join(tmpdir(), "zbuild-"));
    try {
      await page.viewport(wide, tall);
      await page.goto(pathToFileURL(path.join(ws, "design.html")).href);
      await page.eval("document.fonts.ready.then(() => true)");
      const n = await page.eval("document.querySelectorAll('.page').length");
      if (!n) return { error: 'no element with class "page" found' };
      // Sections stack with no gap: the viewport holds the sum of the page heights, or the stack of mixed sizes when pages differ.
      const heights = Array.from({ length: n }, (_, i) => (sizes[i] ?? sizes[0]).h);
      await page.viewport(wide, heights.reduce((a, b) => a + b, 0));
      await page.eval("document.fonts.ready.then(() => true)");
      const info = await page.eval(EXTRACT);
      const file = path.join(out, "all.png");
      await page.screenshot(file);
      const whole = decode(readFileSync(file));
      const pages = info.map((p, i) => {
        const top = Math.max(0, Math.round(p.top));
        const h = Math.min(Math.round(p.h), whole.height - top);
        const w = Math.min(Math.round(p.w), whole.width);
        const img = crop(whole, 0, top, w, Math.max(1, h));
        return { png: encode(img), rects: p.rects, w: p.w, h: p.h, text: p.text };
      });
      return { pages, source: readFileSync(path.join(ws, "design.html"), "utf8"), text: info.map((p) => p.text).join("\n") };
    } catch (err) {
      return { error: String(err.message ?? err) };
    } finally {
      rmSync(out, { recursive: true, force: true });
    }
  };
}
