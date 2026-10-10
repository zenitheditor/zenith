// Per-call timings of the wasm engine in Chromium on the largest example:
// the typing loop (buffer.set + doc.render as two calls, the page's
// keystroke batch, and the calls the page makes for one real keystroke) and
// the gesture.preview loop. Informational:
// wall-clock time on a shared machine swings, so the call counts are the
// stable numbers. Nothing here gates on time.
//
// Run: node timings.js --wasm <file.wasm> --chromium <bin> --examples <dir> [--doc <file.zen>]
// Without `--doc` it measures the largest file in `--examples`. A `--doc`
// from another directory is copied into the site with its sibling files.
// Prints one JSON object.

import { copyFileSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { Browser } from "./cdp.js";
import { A, cursorAt, ready, settle } from "./helpers.js";
import { buildSite, serve } from "./static_site.js";

function args() {
  const out = {};
  const argv = process.argv.slice(2);
  for (let i = 0; i < argv.length; i += 2) out[argv[i].replace(/^--/, "")] = argv[i + 1];
  for (const key of ["wasm", "chromium", "examples"]) {
    if (!out[key]) throw new Error(`missing --${key}; see the header of timings.js`);
  }
  return out;
}

const PAGE = `(async (name) => {
  const { WasmEngine } = await import(new URL('js/engine/wasm.js', document.baseURI));
  const manifest = await (await fetch(new URL('samples/manifest.json', document.baseURI))).json();
  const files = {};
  for (const rel of manifest.files) {
    const bytes = new Uint8Array(await (await fetch(new URL('samples/' + rel, document.baseURI))).arrayBuffer());
    let s = '';
    for (let i = 0; i < bytes.length; i += 8192) s += String.fromCharCode(...bytes.subarray(i, i + 8192));
    files[rel] = btoa(s);
  }
  const text = await (await fetch(new URL('samples/' + name, document.baseURI))).text();
  const engine = new WasmEngine();
  const t0 = performance.now();
  await engine.ready();
  const readyMs = performance.now() - t0;
  const o0 = performance.now();
  await engine.openDocument({ name, text, project: { root: '', files, handles: new Map() } });
  const openMs = performance.now() - o0;
  const stat = (xs) => { const s = [...xs].sort((a, b) => a - b); return { n: s.length, median: s[Math.floor(s.length / 2)], p95: s[Math.min(s.length - 1, Math.floor(s.length * 0.95))], min: s[0], max: s[s.length - 1] }; };
  const group = (m) => { const by = {}; for (const e of m) (by[e.command] ??= []).push(e); return by; };
  const summarize = (m) => Object.fromEntries(Object.entries(group(m)).map(([k, es]) => [k, {
    calls: es.length, total: stat(es.map((e) => e.ms)), module: stat(es.map((e) => e.runMs)),
    instantiate: stat(es.map((e) => e.instantiateMs)), parse: stat(es.map((e) => e.parseMs)),
    overhead: stat(es.map((e) => e.ms - e.runMs - e.instantiateMs - e.parseMs)), requestBytes: stat(es.map((e) => e.bytes)),
  }]));
  const outline = await engine.run('doc.outline');
  const candidates = outline.result.pages[0].children.filter((c) => c.id && c.visible && !c.guide).reverse();
  let target = null;
  for (const c of candidates) {
    await engine.run('select.set', { ids: [c.id] });
    const p = await engine.run('gesture.preview', { dx: 3, dy: 2, scale: 1 });
    if (p.ok) { target = c; break; }
  }
  const render = (scale) => engine.run('doc.render', { scale });
  const page = (await render(1)).result;
  const v = { x: 0, y: 0, w: Math.min(page.width, 1000), h: Math.min(page.height, 700) };

  // Typing loop: buffer.set then doc.render of the visible window, 40 times.
  engine.metrics({ clear: true });
  let cur = (await engine.state({ text: true })).text;
  let version = (await engine.state()).version;
  const typing = [];
  for (let i = 0; i < 40; i++) {
    cur += i % 7 === 6 ? '\\n// t' : 'x';
    const t = performance.now();
    const set = await engine.run('buffer.set', { text: cur }, { version });
    version = set.version;
    const r = await engine.run('doc.render', { scale: 1, viewport: v });
    typing.push(performance.now() - t);
  }
  const typingMetrics = summarize(engine.metrics({ clear: true }));

  // The page's keystroke: one commands.batch of buffer.set, the viewport
  // render, outline, color tokens, and the node under the cursor, 40 times.
  const typingBatch = [];
  let batchWork = null;
  for (let i = 0; i < 40; i++) {
    cur += i % 7 === 6 ? '\\n// t' : 'x';
    const steps = [
      { command: 'buffer.set', params: { text: cur }, version },
      { command: 'doc.render', params: { scale: 1, viewport: v } },
      { command: 'doc.outline', params: {} },
      { command: 'doc.tokens', params: { type: 'color' } },
      { command: 'select.at_offset', params: { offset: 0 } },
    ];
    const t = performance.now();
    const b = await engine.run('commands.batch', { steps });
    typingBatch.push(performance.now() - t);
    version = b.version;
    batchWork = b.work;
  }
  const typingBatchMetrics = summarize(engine.metrics({ clear: true }));

  // gesture.preview loop on the last node, 40 pointer steps.
  await engine.run('select.set', { ids: [target.id] });
  const previews = [];
  for (let i = 0; i < 40; i++) {
    const t = performance.now();
    const p = await engine.run('gesture.preview', { dx: 2 + i * 0.5, dy: -1 - i * 0.25, scale: 1, viewport: v });
    previews.push(performance.now() - t);
    if (!p.ok) break;
  }
  const previewMetrics = summarize(engine.metrics({ clear: true }));
  const work = (await engine.run('gesture.preview', { dx: 3, dy: 1, scale: 1, viewport: v })).work;
  return { name, bytes: text.length, readyMs, compileMs: engine.worker.compileMs, openMs, target: target.id, window: v, page: { w: page.width, h: page.height },
    typingRoundTrip: stat(typing), typingMetrics, typingBatchRoundTrip: stat(typingBatch), typingBatchMetrics, typingBatchWork: batchWork, previewRoundTrip: stat(previews), previewMetrics, previewWork: work,
    loadedFonts: [...engine.loadedFonts] };
})`;

async function main() {
  const a = args();
  const names = readdirSync(a.examples).filter((n) => n.endsWith(".zen"));
  let largest = names.sort((x, y) => statSync(path.join(a.examples, y)).size - statSync(path.join(a.examples, x)).size)[0];
  const work = mkdtempSync(path.join(tmpdir(), "zenith-timings-"));
  const site = path.join(work, "site");
  buildSite(site, a.wasm);
  if (a.doc) {
    largest = path.basename(a.doc);
    const dir = path.dirname(a.doc);
    const manifestFile = path.join(site, "samples/manifest.json");
    const manifest = JSON.parse(readFileSync(manifestFile, "utf8"));
    for (const name of readdirSync(dir)) {
      if (name.endsWith(".pdf") || !statSync(path.join(dir, name)).isFile()) continue;
      copyFileSync(path.join(dir, name), path.join(site, "samples", name));
      if (!name.endsWith(".zen")) manifest.files.push(name);
    }
    writeFileSync(manifestFile, JSON.stringify(manifest));
  }
  const host = await serve(site, { prefix: "/t/" });
  const browser = await Browser.launch(a.chromium);
  try {
    const page = await browser.newPage();
    await page.viewport(1440, 900);
    await page.goto(host.url);
    await ready(page);
    const result = await page.eval(`(${PAGE})(${JSON.stringify(largest)})`);
    // The calls of the real page for one keystroke, on the same document.
    await page.goto(`${host.url}?doc=samples/${largest}`);
    await ready(page);
    await page.eval(`${A}.host.metrics({ clear: true }); true`);
    await cursorAt(page, 0);
    const bursts = 12;
    for (let i = 0; i < bursts; i++) {
      await page.type("x");
      // One burst per keystroke: wait until its debounced send went out.
      await page.waitFor(`!${A}.sync.schedule.pending()`, "the keystroke send");
      await settle(page);
    }
    const calls = await page.eval(`${A}.host.metrics({ clear: true }).map((e) => e.command)`);
    const count = {};
    for (const c of calls) count[c] = (count[c] ?? 0) + 1;
    result.pageKeystroke = { bursts, totalCalls: calls.length, perKeystroke: Object.fromEntries(Object.entries(count).map(([k, n]) => [k, +(n / bursts).toFixed(2)])) };
    console.log(JSON.stringify(result, null, 1));
  } finally {
    await browser.close();
    await host.close();
    rmSync(work, { recursive: true, force: true });
  }
}

main().catch((err) => {
  console.error(`timings run could not finish: ${err.stack}`);
  process.exit(2);
});
