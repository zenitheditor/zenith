// Cross-target equality: the native engine (`zenith edit`, full fonts) and
// the wasm engine (the static site's WasmEngine in Chromium, lazy fonts) run
// the same scripted commands on every `examples/*.zen`. Their `doc.render`
// PNG SHA-256, their gesture results (one node, and a selection: marquee
// hits, the selection box, a snapped preview, and selection gestures), and
// the text after each edit must be equal.
//
// Run: node cross_target.js --zenith <bin> --wasm <file.wasm> --chromium <bin>
//        --examples <dir> [--gesture-sample <n>] [--out <dir>]
// Prints one JSON line per example and a summary. Exit 0 when all are equal.

import { mkdtempSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { Browser } from "./cdp.js";
import { connect, startServer } from "./native_host.js";
import { buildSite, serve } from "./static_site.js";

function args() {
  const out = {};
  const argv = process.argv.slice(2);
  for (let i = 0; i < argv.length; i += 2) out[argv[i].replace(/^--/, "")] = argv[i + 1];
  for (const key of ["zenith", "wasm", "chromium", "examples"]) {
    if (!out[key]) throw new Error(`missing --${key}; see the header of cross_target.js`);
  }
  return out;
}

/**
 * The command script. It runs in Node (native) and in the page (wasm), so it
 * uses only `run` and `state`. Returns a record to compare.
 */
async function scenario(run, state, gestures) {
  const rec = {};
  const pick = (env) => (env.ok ? env.result : { error: env.error.code });
  const render = async (params) => {
    const env = await run("doc.render", params);
    if (!env.ok) return { error: env.error.code };
    const r = env.result;
    return { sha: r.sha256, w: r.width, h: r.height, pages: r.page_count, stale: r.stale, codes: (r.diagnostics ?? []).map((d) => d.code), image: env.image ? env.image.sha256 : null };
  };
  rec.page = await render({ scale: 1 });
  rec.window = await render({ scale: 2.5, viewport: { x: 20.5, y: 10.25, w: 100, h: 80 } });
  const diag = await run("doc.diagnose");
  rec.diagnose = diag.ok ? { valid: diag.result.valid, codes: diag.result.diagnostics.map((d) => `${d.code}@${d.line}:${d.col}`) } : { error: diag.error.code };
  if (!gestures) return rec;
  const outline = await run("doc.outline");
  const pages = outline.ok ? outline.result.pages : [];
  const target = (pages[0]?.children ?? []).filter((c) => c.id && c.visible && !c.guide).pop();
  rec.target = target ? target.id : null;
  if (!target) return rec;
  await run("select.set", { ids: [target.id] });
  const handles = await run("node.handles");
  rec.handles = handles.ok ? handles.result.handles.map((h) => `${h.id}:${h.enabled}`) : { error: handles.error.code };
  const version = async () => (await state()).version;
  const text = async () => (await state()).text;
  const preview = await run("gesture.preview", { dx: 6.5, dy: -4, scale: 1 });
  rec.preview = preview.ok ? { sha: preview.result.sha256, ops: preview.result.ops, notes: preview.result.notes } : { error: preview.error.code };
  const steps = [
    ["move", "gesture.commit", { dx: 6.5, dy: -4 }],
    ["resize", "gesture.commit", { handle: "se", dx: 5, dy: 3 }],
    ["rotate", "gesture.commit", { handle: "rotate", angle: 15 }],
  ];
  rec.steps = [];
  for (const [name, command, params] of steps) {
    const env = await run(command, params, await version());
    rec.steps.push({ name, ok: env.ok, code: env.ok ? null : env.error.code, delta: env.ok ? env.result.delta : null, ops: env.ok ? env.result.ops : null, text: await text() });
  }
  const undo = await run("history.undo", {}, await version());
  rec.undo = { ok: undo.ok, text: await text() };
  rec.after = await render({ scale: 1 });
  // A selection of up to three top-level nodes: marquee hits, the selection
  // box, a snapped preview, and a move, resize, and turn as one.
  const ids = (pages[0]?.children ?? []).filter((c) => c.id && c.visible && !c.guide && !c.locked).slice(0, 3).map((c) => c.id);
  if (ids.length >= 2) {
    rec.marquee = pick(await run("select.marquee", { x: 0, y: 0, w: 100000, h: 100000, select: false }));
    rec.contain = pick(await run("select.marquee", { x: 10, y: 10, w: 300, h: 200, contain: true, select: false }));
    rec.group = pick(await run("node.handles", { ids }));
    const snapped = await run("gesture.preview", { nodes: ids, dx: 7.25, dy: 3, snap_distance: 6, render: false });
    rec.snapped = snapped.ok ? { ops: snapped.result.ops, snap: snapped.result.snap, corners: snapped.result.corners } : { error: snapped.error.code };
    rec.multi = [];
    for (const [name, params] of [
      ["move", { nodes: ids, dx: -3.5, dy: 2 }],
      ["resize", { nodes: ids, handle: "se", dx: 9, dy: 4 }],
      ["rotate", { nodes: ids, handle: "rotate", angle: 20 }],
    ]) {
      const env = await run("gesture.commit", params, await version());
      rec.multi.push({ name, ok: env.ok, code: env.ok ? null : env.error.code, ops: env.ok ? env.result.ops : null, text: await text() });
    }
    rec.multiAfter = await render({ scale: 1 });
  }
  const typed = await run("buffer.set", { text: `${await text()}\n// typed` }, await version());
  rec.typed = { ok: typed.ok, valid: typed.ok ? typed.result.valid : null, text: await text() };
  return rec;
}

async function native(zenith, file, gestures) {
  const data = mkdtempSync(path.join(tmpdir(), "zenith-xt-data-"));
  const server = await startServer(zenith, file, data);
  try {
    const client = await connect(server);
    const rec = await scenario(client.run, client.state, gestures);
    return rec;
  } finally {
    server.child.kill();
    rmSync(data, { recursive: true, force: true });
  }
}

/** The page-side driver: a private WasmEngine, one document per example. */
const PAGE = `(async () => {
  const { WasmEngine } = await import(new URL('js/engine/wasm.js', document.baseURI));
  const manifest = await (await fetch(new URL('samples/manifest.json', document.baseURI))).json();
  const files = {};
  for (const rel of manifest.files) {
    const bytes = new Uint8Array(await (await fetch(new URL('samples/' + rel, document.baseURI))).arrayBuffer());
    let s = '';
    for (let i = 0; i < bytes.length; i += 8192) s += String.fromCharCode(...bytes.subarray(i, i + 8192));
    files[rel] = btoa(s);
  }
  const hex = async (blob) => [...new Uint8Array(await crypto.subtle.digest('SHA-256', await blob.arrayBuffer()))].map((b) => b.toString(16).padStart(2, '0')).join('');
  const scenario = SCENARIO;
  window.__xt = async (name, gestures) => {
    const engine = new WasmEngine();
    try {
      return await scenarioOn(engine, name, gestures);
    } finally {
      // One Worker per example: close it, or the spare instances of 35
      // Workers exhaust the wasm address space.
      engine.close();
    }
  };
  const scenarioOn = async (engine, name, gestures) => {
    await engine.ready();
    const text = await (await fetch(new URL('samples/' + name, document.baseURI))).text();
    await engine.openDocument({ name, text, project: { root: '', files, handles: new Map() } });
    const run = async (command, params, version) => engine.run(command, params, version === undefined ? {} : { version });
    let blobSha = null;
    const wrapped = async (command, params, version) => {
      const env = await run(command, params, version);
      if (command === 'doc.render' && env.ok && blobSha === null) blobSha = (await hex(await engine.image(env.image))) === env.result.sha256 ? 'equal' : 'DIFFERENT';
      return env;
    };
    const rec = await scenario(wrapped, () => engine.state({ text: true }), gestures);
    rec.__blob = blobSha;
    rec.__metrics = engine.metrics().length;
    return rec;
  };
  return true;
})()`;

async function main() {
  const a = args();
  const work = mkdtempSync(path.join(tmpdir(), "zenith-xt-"));
  const site = path.join(work, "site");
  console.log(buildSite(site, a.wasm));
  const host = await serve(site, { prefix: "/x/" });
  const browser = await Browser.launch(a.chromium);
  const names = readdirSync(a.examples).filter((n) => n.endsWith(".zen")).sort();
  const every = Number(a["gesture-sample"] ?? 1);
  let equal = 0;
  let gestured = 0;
  let commits = 0;
  let committed = 0;
  let selections = 0;
  let selectionsOk = 0;
  const bad = [];
  try {
    const page = await browser.newPage();
    await page.goto(`${host.url}`);
    await page.waitFor("document.documentElement.dataset.ready === 'true'", "the static page", 30000);
    await page.eval(PAGE.replace("SCENARIO", scenario.toString()));
    for (const [i, name] of names.entries()) {
      const gestures = i % every === 0;
      const nat = await native(a.zenith, path.join(a.examples, name), gestures);
      const was = await page.eval(`window.__xt(${JSON.stringify(name)}, ${gestures})`);
      const blob = was.__blob;
      delete was.__blob;
      delete was.__metrics;
      const same = JSON.stringify(nat) === JSON.stringify(was) && blob === "equal";
      if (same) equal++;
      else bad.push(name);
      if (gestures && nat.steps) {
        gestured++;
        commits += nat.steps.length;
        committed += nat.steps.filter((c) => c.ok).length;
      }
      if (gestures && nat.multi) {
        selections += nat.multi.length;
        selectionsOk += nat.multi.filter((c) => c.ok).length;
      }
      const diffKeys = Object.keys(nat).filter((k) => JSON.stringify(nat[k]) !== JSON.stringify(was[k]));
      console.log(JSON.stringify({ example: name, equal: same, sha: nat.page?.sha?.slice(0, 12), blob, gestures: !!nat.steps, differ: diffKeys }));
      if (!same) console.log(JSON.stringify({ native: nat, wasm: was }).slice(0, 1500));
    }
  } finally {
    await browser.close();
    await host.close();
    rmSync(work, { recursive: true, force: true });
  }
  console.log(JSON.stringify({ suite: "cross-target", examples: names.length, equal, gestureScripts: gestured, commits, commitsOk: committed, selectionCommits: selections, selectionCommitsOk: selectionsOk, differ: bad }));
  process.exit(bad.length ? 1 : 0);
}

main().catch((err) => {
  console.error(`cross-target run could not finish: ${err.stack}`);
  process.exit(2);
});
