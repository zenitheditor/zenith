// Multi-page renders for revision tasks. Each renderer returns
// { pages: [png], rects: [[{ x, y, w, h, clipped }]], ids: [page id or null] } or { error }.
// A text rect is clipped when its text leaves its own box: Zenith compares the
// inspect `bounds` with the `box`; HTML compares the text's line boxes with
// its nearest block-level ancestor and with every ancestor that clips overflow.
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { readFileSync, mkdtempSync, rmSync, readdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { zenithBin } from "./render.mjs";
import { decode, encode, crop } from "./png.mjs";

const run = promisify(execFile);
const TOL = 2;

const outside = (b, box) => b.y + b.h > box.y + box.h + TOL || b.x + b.w > box.x + box.w + TOL || b.y < box.y - TOL || b.x < box.x - TOL;

function zenRects(node, out) {
  if ((node.kind === "text" || node.kind === "code") && node.bounds && node.box) {
    out.push({ id: node.id, ...node.bounds, clipped: outside(node.bounds, node.box) });
  }
  for (const child of node.children ?? []) zenRects(child, out);
}

/** Every page of `design.zen` in `ws`. */
export async function zenithAllPages(ws) {
  const dir = mkdtempSync(path.join(tmpdir(), "zpages-"));
  try {
    const opts = { cwd: ws, timeout: 120000, maxBuffer: 32 << 20 };
    await run(zenithBin(), ["render", "design.zen", "--all-pages", dir], opts);
    const { stdout } = await run(zenithBin(), ["inspect", "design.zen", "--json"], opts);
    const info = JSON.parse(stdout);
    const files = readdirSync(dir)
      .filter((f) => f.endsWith(".png"))
      .sort((a, b) => Number(a.match(/\d+/)) - Number(b.match(/\d+/)));
    const pages = files.map((f) => readFileSync(path.join(dir, f)));
    const rects = info.pages.map((p) => {
      const out = [];
      for (const child of p.children ?? []) zenRects(child, out);
      // Master and component children appear only under `expanded`.
      for (const [id, e] of Object.entries(p.expanded ?? {})) {
        if (e.bounds && e.box) out.push({ id, ...e.bounds, clipped: outside(e.bounds, e.box) });
      }
      return out;
    });
    return { pages, rects, ids: info.pages.map((p) => p.id ?? null) };
  } catch (err) {
    return { error: `${err.stderr ?? ""}${err.stdout ?? ""}`.trim().slice(0, 4000) || String(err.message ?? err) };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

const EXTRACT = `(() => {
  const TOL = ${TOL};
  const out = (r, b) => r.bottom > b.bottom + TOL || r.right > b.right + TOL || r.top < b.top - TOL || r.left < b.left - TOL;
  return [...document.querySelectorAll('.page')].map((p) => {
    const pr = p.getBoundingClientRect();
    const rects = [];
    const walker = document.createTreeWalker(p, NodeFilter.SHOW_TEXT);
    while (walker.nextNode()) {
      const n = walker.currentNode;
      if (!n.textContent.trim()) continue;
      const el = n.parentElement;
      const cs = getComputedStyle(el);
      if (cs.visibility === 'hidden' || cs.display === 'none') continue;
      const range = document.createRange();
      range.selectNodeContents(n);
      const rs = [...range.getClientRects()].filter((x) => x.width >= 1 && x.height >= 1);
      if (!rs.length) continue;
      const r = { left: Math.min(...rs.map((x) => x.left)), top: Math.min(...rs.map((x) => x.top)), right: Math.max(...rs.map((x) => x.right)), bottom: Math.max(...rs.map((x) => x.bottom)) };
      let clipped = false;
      let block = el;
      while (block !== p && getComputedStyle(block).display.startsWith('inline')) block = block.parentElement;
      if (block !== p && out(r, block.getBoundingClientRect())) clipped = true;
      for (let a = el; a && a !== p.parentElement; a = a.parentElement) {
        const s = getComputedStyle(a);
        if ((s.overflowX !== 'visible' || s.overflowY !== 'visible') && out(r, a.getBoundingClientRect())) clipped = true;
      }
      rects.push({ x: r.left - pr.left, y: r.top - pr.top, w: r.right - r.left, h: r.bottom - r.top, clipped });
    }
    return { id: p.id || null, top: pr.top + window.scrollY, w: pr.width, h: pr.height, rects };
  });
})()`;

/** A renderer of `design.html` in a workspace, over one reused CDP page. Pages are `.page` sections. */
export async function htmlAllPages(browser, canvas) {
  const page = await browser.newPage();
  return async (ws) => {
    const dir = mkdtempSync(path.join(tmpdir(), "zpages-"));
    try {
      await page.viewport(canvas.w, canvas.h);
      await page.goto(pathToFileURL(path.join(ws, "design.html")).href);
      await page.eval("document.fonts.ready.then(() => true)");
      const n = await page.eval("document.querySelectorAll('.page').length");
      if (!n) return { error: 'no element with class "page" found' };
      const height = await page.eval("Math.ceil(document.documentElement.scrollHeight)");
      await page.viewport(canvas.w, Math.max(canvas.h, height));
      await page.eval("document.fonts.ready.then(() => true)");
      const info = await page.eval(EXTRACT);
      const file = path.join(dir, "all.png");
      await page.screenshot(file);
      const whole = decode(readFileSync(file));
      const pages = info.map((p) => {
        const top = Math.max(0, Math.round(p.top));
        const h = Math.max(1, Math.min(Math.round(p.h), whole.height - top));
        const w = Math.max(1, Math.min(Math.round(p.w), whole.width));
        return encode(crop(whole, 0, top, w, h));
      });
      return { pages, rects: info.map((p) => p.rects), ids: info.map((p) => p.id) };
    } catch (err) {
      return { error: String(err.message ?? err) };
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  };
}
