// Screenshot tool for the browser editor. Not a test: neither run.js nor
// the Rust test `editor_page.rs` runs it.
//
// Run: node zenith-cli/tests/editor_e2e/shot.mjs <file.zen> <out_dir>
//        [--view split|canvas|diagnostics|all] [--select <node-id>]
//        [--width 1600] [--height 1000] [--dpr 2] [--host server|static]
//        [--break]
//
// Writes `editor-<view>.png` into <out_dir> and prints the written paths as
// one JSON line. Read the PNGs to check a UI change by eye.
//
// The tool works on a temp copy of the document and of the assets it
// references by relative path. It never modifies the source file. It always
// removes the server, the browser, and the temp directories on exit.
//
// Views:
//   split        code pane and canvas (the default).
//   canvas       the full-screen canvas.
//   diagnostics  the code pane with a diagnostic. Needs a diagnostic: pass
//                `--break` to replace the first `(token)"name"` reference in
//                the temp copy with an unknown token. `all` adds this view
//                only when `--break` is given or the document has one.
//
// Hosts:
//   server  `zenith edit`. Binary: ZENITH_E2E_ZENITH, else
//           target/release/zenith, else target/debug/zenith.
//   static  the static site. Wasm module: ZENITH_E2E_WASM, else
//           target/wasm32-wasip1/release-wasm/zenith-editor-wasm.wasm.
//
// Browser: ZENITH_E2E_CHROMIUM, else chromium, chromium-browser,
// google-chrome, or google-chrome-stable on PATH. ZENITH_E2E_NO_SANDBOX=1
// adds `--no-sandbox`.
//
// Regenerate the README images in assets/editor/ (build the release binary
// first with `cargo build --release`):
//   node zenith-cli/tests/editor_e2e/shot.mjs assets/showcase/zenith-brand.zen \
//     assets/editor --view all --select t.title --break \
//     --width 1600 --height 1000 --dpr 2
// This writes editor-split.png, editor-canvas.png, and editor-diagnostics.png.
// The split and canvas views show `t.title` selected.

import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { Browser } from "./cdp.js";
import { A, ready, settle } from "./helpers.js";
import { startServer } from "./native_host.js";
import { buildSite, serve } from "./static_site.js";

const here = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(here, "../../..");
const VIEWS = ["split", "canvas", "diagnostics"];
const USAGE =
  "usage: node shot.mjs <file.zen> <out_dir> [--view split|canvas|diagnostics|all] [--select <node-id>] " +
  "[--width 1600] [--height 1000] [--dpr 2] [--host server|static] [--break]";

function parseArgs(argv) {
  const opts = { view: "split", select: null, width: 1600, height: 1000, dpr: 2, host: "server", break: false };
  const positional = [];
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (!arg.startsWith("--")) {
      positional.push(arg);
      continue;
    }
    const key = arg.slice(2);
    if (key === "break") {
      opts.break = true;
      continue;
    }
    if (!["view", "select", "width", "height", "dpr", "host"].includes(key)) throw new Error(`unknown flag ${arg}\n${USAGE}`);
    const value = argv[++i];
    if (value === undefined) throw new Error(`${arg} needs a value\n${USAGE}`);
    opts[key] = ["width", "height", "dpr"].includes(key) ? Number(value) : value;
  }
  if (positional.length !== 2) throw new Error(USAGE);
  [opts.file, opts.out] = positional;
  if (![...VIEWS, "all"].includes(opts.view)) throw new Error(`--view must be split, canvas, diagnostics, or all\n${USAGE}`);
  if (!["server", "static"].includes(opts.host)) throw new Error(`--host must be server or static\n${USAGE}`);
  for (const key of ["width", "height", "dpr"]) {
    if (!Number.isFinite(opts[key]) || opts[key] <= 0) throw new Error(`--${key} must be a positive number`);
  }
  return opts;
}

function onPath(names) {
  for (const dir of (process.env.PATH ?? "").split(path.delimiter).filter(Boolean)) {
    for (const name of names) {
      const file = path.join(dir, name);
      if (existsSync(file) && statSync(file).isFile()) return file;
    }
  }
  return null;
}

function findChromium() {
  const found =
    process.env.ZENITH_E2E_CHROMIUM || onPath(["chromium", "chromium-browser", "google-chrome", "google-chrome-stable"]);
  if (!found) throw new Error("no Chromium found: put chromium or google-chrome on PATH, or set ZENITH_E2E_CHROMIUM");
  return found;
}

function findZenith() {
  const env = process.env.ZENITH_E2E_ZENITH;
  if (env) {
    if (!existsSync(env)) throw new Error(`ZENITH_E2E_ZENITH is set but the file does not exist: ${env}`);
    return env;
  }
  const found = [path.join(repo, "target/release/zenith"), path.join(repo, "target/debug/zenith")].find((c) => existsSync(c));
  if (!found) throw new Error("no zenith binary found: run `cargo build --release`, or set ZENITH_E2E_ZENITH");
  return found;
}

function findWasm() {
  const found = process.env.ZENITH_E2E_WASM || path.join(repo, "target/wasm32-wasip1/release-wasm/zenith-editor-wasm.wasm");
  if (!existsSync(found)) throw new Error("no wasm module found: build zenith-editor-wasm (profile release-wasm), or set ZENITH_E2E_WASM");
  return found;
}

/** Copy `file` into `dest`, plus every existing file it names by a relative path in quotes. */
function copyDocument(file, dest) {
  const srcDir = path.dirname(path.resolve(file));
  const name = path.basename(file);
  copyFileSync(file, path.join(dest, name));
  const text = readFileSync(file, "utf8");
  for (const m of text.matchAll(/"([^"\s]+\.[A-Za-z0-9]+)"/g)) {
    const rel = m[1];
    if (path.isAbsolute(rel) || rel.split(/[\\/]/).includes("..")) continue;
    const from = path.join(srcDir, rel);
    if (!existsSync(from) || !statSync(from).isFile()) continue;
    mkdirSync(path.dirname(path.join(dest, rel)), { recursive: true });
    copyFileSync(from, path.join(dest, rel));
  }
  return name;
}

/** `text` with the first `(token)"name"` reference renamed to an unknown token. */
function breakTokenReference(text) {
  const m = /\(token\)"([^"]+)"/.exec(text);
  if (!m) throw new Error("--break needs a `(token)\"name\"` reference in the document, and found none");
  const at = m.index + m[0].length - 1;
  return `${text.slice(0, at)}__missing${text.slice(at)}`;
}

async function select(page, nodeId) {
  const needle = `id="${nodeId}"`;
  const found = await page.eval(`(() => {
    const v = ${A}.code.view;
    const i = v.state.doc.toString().indexOf(${JSON.stringify(needle)});
    if (i < 0) return false;
    v.focus();
    v.dispatch({ selection: { anchor: i + 4 }, scrollIntoView: true, userEvent: 'select' });
    return true;
  })()`);
  if (!found) throw new Error(`--select: no node with id "${nodeId}" in the document`);
  await page.waitFor(`${A}.selectionIds.includes(${JSON.stringify(nodeId)})`, "the node to be selected");
  await page.waitFor("document.querySelectorAll('#overlay .selected').length >= 1", "the selection overlay");
  await settle(page);
}

const pause = (ms) => new Promise((r) => setTimeout(r, ms));

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  if (!existsSync(opts.file) || !statSync(opts.file).isFile()) throw new Error(`document not found: ${opts.file}`);
  const chromium = findChromium();
  const engine = opts.host === "static" ? findWasm() : findZenith();
  const out = path.resolve(opts.out);
  mkdirSync(out, { recursive: true });

  const dirs = [];
  const tempDir = (prefix) => {
    const d = mkdtempSync(path.join(tmpdir(), prefix));
    dirs.push(d);
    return d;
  };
  // Backstop for exits that skip `finally` (a signal handled below, `process.exit`).
  process.on("exit", () => dirs.forEach((d) => rmSync(d, { recursive: true, force: true })));
  for (const sig of ["SIGINT", "SIGTERM", "SIGHUP"]) process.on(sig, () => process.exit(130));

  const docDir = tempDir("zenith-shot-doc-");
  const data = tempDir("zenith-shot-data-");
  const name = copyDocument(opts.file, docDir);
  const doc = path.join(docDir, name);
  const original = readFileSync(doc, "utf8");

  let server = null;
  let browser = null;
  try {
    if (opts.host === "static") {
      const site = path.join(tempDir("zenith-shot-site-"), "site");
      buildSite(site, engine);
      mkdirSync(path.join(site, "samples"), { recursive: true });
      cpDir(docDir, path.join(site, "samples"));
      const host = await serve(site, { prefix: "/sub/dir/" });
      server = { url: `${host.url}?doc=samples/${encodeURIComponent(name)}`, stop: () => host.close() };
    } else {
      server = await startServer(engine, doc, data);
    }
    browser = await Browser.launch(chromium);
    const page = await browser.newPage();
    await page.viewport(opts.width, opts.height, false, opts.dpr);
    await page.media("light");
    await page.goto(server.url);
    await ready(page);

    const wanted = opts.view === "all" ? VIEWS : [opts.view];
    const written = [];
    const shot = async (view) => {
      await pause(500);
      const file = path.join(out, `editor-${view}.png`);
      await page.screenshot(file);
      written.push(file);
    };

    if (opts.select) await select(page, opts.select);
    if (wanted.includes("split")) await shot("split");
    if (wanted.includes("canvas")) {
      await page.clickSelector("#full-canvas");
      await page.waitFor("document.getElementById('app').dataset.full === 'canvas'", "the full-screen canvas");
      await settle(page);
      await shot("canvas");
      await page.key("Escape");
      await page.waitFor("document.getElementById('app').dataset.full === 'none'", "the split view to return");
      await settle(page);
    }
    if (wanted.includes("diagnostics")) {
      if (opts.break) {
        const broken = breakTokenReference(original);
        await page.eval(`(() => { const v = ${A}.code.view; v.dispatch({ changes: { from: 0, to: v.state.doc.length, insert: ${JSON.stringify(broken)} } }); return true; })()`);
      }
      const has = async () => (await page.eval(`${A}.diagnosticList.length`)) > 0;
      if (opts.break) await page.waitFor(`${A}.diagnosticList.length > 0`, "a diagnostic");
      await settle(page);
      if (await has()) {
        await page.eval(`(() => { const v = ${A}.code.view; const d = ${A}.diagnosticList[0]; v.dispatch({ selection: { anchor: d.line ? v.state.doc.line(d.line).from + 4 : 0 }, scrollIntoView: true }); return true; })()`);
        await settle(page);
        await shot("diagnostics");
      } else if (opts.view === "diagnostics") {
        throw new Error("the document has no diagnostic: pass --break to add one in the temp copy");
      }
    }
    console.log(JSON.stringify({ host: opts.host, written }));
  } finally {
    await browser?.close().catch((err) => console.error(`closing chromium: ${err.message}`));
    await server?.stop().catch((err) => console.error(`stopping the server: ${err.message}`));
    dirs.forEach((d) => rmSync(d, { recursive: true, force: true }));
  }
}

/** Copy the files under `from` into `to`, keeping relative paths. */
function cpDir(from, to) {
  for (const entry of readdirSyncRecursive(from)) {
    mkdirSync(path.dirname(path.join(to, entry)), { recursive: true });
    copyFileSync(path.join(from, entry), path.join(to, entry));
  }
}

function readdirSyncRecursive(dir) {
  return readdirSync(dir, { recursive: true, withFileTypes: true })
    .filter((e) => e.isFile())
    .map((e) => path.relative(dir, path.join(e.parentPath ?? e.path, e.name)));
}

main().then(
  () => process.exit(0),
  (err) => {
    console.error(`shot failed: ${err.message}`);
    process.exit(1);
  },
);
