// Per-row probes. The harness rebuilds each shipped row from the arm's own source
// to read its text rectangles and text, then renders that rebuild so the score
// can check it matches the shipped PNG. Zenith: the template plus the same
// replace_text / set_asset ops `zenith merge` applies, via `zenith tx`, then
// `inspect --json` and `render`. HTML: the per-row page `out/<id>.html`, loaded
// in Chromium by the build bench's `htmlPages`.
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import path from "node:path";
import { copyFileSync, readFileSync, rmSync } from "node:fs";
import { zenithBin } from "../render.mjs";
import { htmlPages } from "../build/render.mjs";

const run = promisify(execFile);
const opts = (ws) => ({ cwd: ws, timeout: 120000, maxBuffer: 16 << 20 });

function walk(node, out) {
  out.push(node);
  for (const c of node.children ?? []) walk(c, out);
}

/** Every node of every page of an inspect report, flattened. */
function nodesOf(info) {
  const out = [];
  for (const p of info.pages ?? []) for (const c of p.children ?? []) walk(c, out);
  return out;
}

/** Text rectangles and text of an inspect report. `clipped` when the ink leaves the text box. */
function zenithRects(info) {
  const rects = [];
  const texts = [];
  for (const n of nodesOf(info)) {
    if (n.kind !== "text" && n.kind !== "code") continue;
    const text = String(n.text ?? "");
    if (text.trim()) texts.push(text);
    const b = n.bounds;
    if (!b || !text.trim() || b.w < 1 || b.h < 1) continue;
    const box = n.box ?? b;
    // Right and bottom use the build bench's 2 px slack. Left and top allow glyph overhang: a 56 px "J" starts 4.4 px left of its box.
    const hang = Math.max(4, 0.12 * b.h);
    const clipped = b.y + b.h > box.y + box.h + 2 || b.x + b.w > box.x + box.w + 2 || b.x < box.x - hang || b.y < box.y - hang;
    rects.push({ x: b.x, y: b.y, w: b.w, h: b.h, clipped, text });
  }
  return { rects, text: texts.join("\n") };
}

/**
 * Zenith prober over `ws/badge.zen`. Returns (row) => { png, rects, text } | { error }.
 * Throws when the template is missing or does not inspect.
 */
export async function zenithProber(ws, probeDir) {
  const bin = zenithBin();
  const { stdout } = await run(bin, ["inspect", "badge.zen", "--json"], opts(ws));
  const bindings = nodesOf(JSON.parse(stdout))
    .filter((n) => typeof n.role === "string" && n.role.startsWith("data."))
    .map((n) => ({ id: n.id, kind: n.kind, column: n.role.slice(5) }));
  // The probe document sits next to the template so asset paths resolve the same way.
  const doc = "_probe.zen";
  return async (row) => {
    const ops = [];
    for (const b of bindings) {
      const cell = row[b.column] ?? "";
      if (b.kind === "image") {
        if (!cell.trim()) continue;
        const asset = `asset.probe.${b.column}`;
        ops.push({ op: "add_asset", id: asset, kind: "image", src: cell.trim() });
        ops.push({ op: "set_asset", node_id: b.id, asset_id: asset });
      } else {
        ops.push({ op: "replace_text", node: b.id, spans: [{ text: cell }] });
      }
    }
    const png = path.join(probeDir, `${row.id}.png`);
    try {
      copyFileSync(path.join(ws, "badge.zen"), path.join(ws, doc));
      if (ops.length) await run(bin, ["tx", doc, "--ops", JSON.stringify({ ops }), "--apply"], opts(ws));
      const info = JSON.parse((await run(bin, ["inspect", doc, "--json"], opts(ws))).stdout);
      await run(bin, ["render", doc, "--png", png], opts(ws));
      return { png: readFileSync(png), ...zenithRects(info) };
    } catch (err) {
      return { error: `${err.stderr ?? ""}${err.stdout ?? ""}`.trim().slice(0, 600) || String(err.message ?? err) };
    } finally {
      rmSync(path.join(ws, doc), { force: true });
    }
  };
}

/** HTML prober over `ws/out/<id>.html`. Returns (row) => { png, rects, text } | { error }. */
export async function htmlProber(ws, browser, canvas) {
  const build = await htmlPages(browser, [canvas]);
  return async (row) => {
    const r = await build(ws, `out/${row.id}.html`);
    if (r.error) return { error: r.error };
    if (r.pages.length !== 1) return { error: `expected one .page section, found ${r.pages.length}` };
    const p = r.pages[0];
    return { png: p.png, rects: p.rects, text: p.text };
  };
}
