// Tests of `WasmEngine` in Node, over the real wasm module and a Node
// stand-in for the Worker (same project splicing as `worker.js`). They run
// without a browser: the envelope and events, stale versions, lazy fonts,
// and file open, save, reload, and conflict over a mock file handle.
//
// Run: ZENITH_EDITOR_WASM=<file.wasm> node unit_wasm.js
// Skips (exit 0, message) when ZENITH_EDITOR_WASM is unset or missing.

import assert from "node:assert/strict";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const wasmFile = process.env.ZENITH_EDITOR_WASM;
if (!wasmFile || !existsSync(wasmFile)) {
  console.log(JSON.stringify({ suite: "wasm-engine-unit", skipped: "set ZENITH_EDITOR_WASM to a built zenith-editor-wasm.wasm" }));
  process.exit(0);
}

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, "../../..");
const assets = path.join(root, "zenith-cli/assets/editor/js");

// Browser globals the engine touches.
const listeners = [];
globalThis.document = { baseURI: "http://localhost/site/", visibilityState: "visible", addEventListener() {}, removeEventListener() {} };
globalThis.window = { addEventListener: (n, f) => listeners.push([n, f]), removeEventListener() {} };

const { WasmEngine } = await import(path.join(assets, "engine/wasm.js"));
const { requestText, takePng } = await import(path.join(assets, "engine/wasm/request.js"));
const { runRequest } = await import(path.join(assets, "engine/wasm/wasi.js"));
const { fontAdvisoryKey } = await import(path.join(assets, "engine/wasm/fonts.js"));
const { applyDelta } = await import("./delta.js");

const module = await WebAssembly.compile(readFileSync(wasmFile));

/** The Worker's job, in this process. */
class FakeWorker {
  constructor() {
    this.filesJson = "{}";
    this.fonts = {};
    this.fontsJson = "{}";
    this.timings = [];
    this.compileMs = 0;
    this.calls = [];
  }
  async ready() {}
  async project(update) {
    if (update.files) this.filesJson = JSON.stringify(update.files);
    if (update.resetFonts) this.fonts = {};
    if (update.fonts) Object.assign(this.fonts, update.fonts);
    this.fontsJson = JSON.stringify(this.fonts);
  }
  async call(method, params, { project = true, command = "" } = {}) {
    this.calls.push(command);
    const text = requestText(method, params, project, this.filesJson, this.fontsJson);
    const run = await runRequest(module, text);
    const response = JSON.parse(run.response);
    return { response, png: takePng(response), runMs: run.runMs, instantiateMs: run.instantiateMs, parseMs: 0, requestBytes: text.length };
  }
}

const fontFiles = path.join(root, "zenith-core/assets/fonts");
const fontsFromDisk = (log = []) => ({
  get: async (file) => {
    log.push(file);
    return readFileSync(path.join(fontFiles, file)).toString("base64");
  },
});

function engine({ fonts = fontsFromDisk() } = {}) {
  const e = new WasmEngine();
  e.worker = new FakeWorker();
  e.fonts = fonts;
  e.seen = [];
  e.subscribe({ event: (name, data) => e.seen.push([name, data]) });
  return e;
}

const DOC = (extra = "") => `zenith version=1 {
  project id="p" name="P"
  tokens format="zenith-token-v1" {
    token id="c" type="color" value="#102030"
  }
  styles {}
  document id="d" title="D" {
    page id="pg" w=(px)200 h=(px)100 {
      rect id="r" x=(px)10 y=(px)10 w=(px)80 h=(px)40 fill=(token)"c"${extra}
    }
  }
}
`;
const example = (name) => readFileSync(path.join(root, "examples", name), "utf8");

/** A mock `FileSystemFileHandle` over a string. */
function fileHandle(name, text) {
  const state = { text, mtime: 1000, writes: [] };
  return {
    state,
    name,
    kind: "file",
    async getFile() {
      return { name, lastModified: state.mtime, async text() { return state.text; } };
    },
    async createWritable() {
      let buf = "";
      return {
        async write(t) { buf += t; },
        async close() { state.text = buf; state.mtime++; state.writes.push(buf); },
      };
    },
  };
}

let passed = 0;
const failures = [];
async function test(name, fn) {
  try {
    await fn();
    passed++;
  } catch (err) {
    failures.push(`${name}: ${err.stack}`);
  }
}

await test("request text splices the project and stays valid JSON", () => {
  const text = requestText("editor", { command: { command: "x" }, path: "a.zen" }, true, '{"a.png":"QQ=="}', '{"f.ttf":"QQ=="}');
  const parsed = JSON.parse(text);
  assert.deepEqual(parsed.params.files, { "a.png": "QQ==" });
  assert.deepEqual(parsed.params.fonts, { "f.ttf": "QQ==" });
  assert.equal(parsed.params.path, "a.zen");
  assert.deepEqual(JSON.parse(requestText("ping", {}, true, "{}", "{}")).params, { files: {}, fonts: {} });
  assert.deepEqual(JSON.parse(requestText("ping", { x: 1 }, false, "{}", "{}")).params, { x: 1 });
});

await test("takePng moves the base64 PNG out of the response", () => {
  const response = { ok: true, result: { png_base64: Buffer.from("PNG!").toString("base64"), a: 1 } };
  const buf = takePng(response);
  assert.equal(Buffer.from(buf).toString(), "PNG!");
  assert.equal(response.result.png_base64, undefined);
  assert.equal(takePng({ ok: true, result: {} }), null);
  assert.equal(takePng({ ok: false, error: {} }), null);
});

await test("font advisory key only for font.unresolved", () => {
  assert.equal(fontAdvisoryKey([{ code: "x", message: "m" }]), null);
  assert.equal(fontAdvisoryKey(undefined), null);
  assert.equal(fontAdvisoryKey([{ code: "font.unresolved", message: "b" }, { code: "font.unresolved", message: "a" }]), "a\nb");
});

await test("open, render, envelope and image blob", async () => {
  const e = engine();
  await e.openDocument({ name: "d.zen", text: DOC() });
  const r = await e.run("doc.render", { scale: 1 });
  assert.deepEqual(Object.keys(r).sort(), ["command", "dirty", "image", "ok", "result", "version", "work"]);
  assert.equal(r.ok, true);
  assert.equal(r.dirty, false);
  assert.equal(r.image.sha256, r.result.sha256);
  assert.equal(r.image.width, 200);
  assert.equal(r.image.height, 100);
  const blob = await e.image(r.image);
  assert.equal(blob.type, "image/png");
  assert.deepEqual([...new Uint8Array(await blob.arrayBuffer()).slice(1, 4)], [80, 78, 71]);
  await assert.rejects(e.image({ sha256: "nope" }), { code: "edit.image_expired" });
  assert.equal(r.work.rasters, 1);
});

await test("a stale version is rejected and changes nothing", async () => {
  const e = engine();
  await e.openDocument({ name: "d.zen", text: DOC() });
  const s = await e.state({ text: true });
  const env = await e.run("buffer.set", { text: "x" }, { version: s.version - 1 });
  assert.equal(env.ok, false);
  assert.equal(env.error.code, "editor.stale_version");
  assert.equal(env.version, s.version);
  assert.equal((await e.state({ text: true })).text, DOC());
  assert.equal((await e.save({ version: s.version + 5 })).error.code, "editor.stale_version");
  assert.equal((await e.run("file.reload", {})).error.code, "editor.missing_version");
});

await test("a text change emits a session event with a delta; a foreign client id is kept", async () => {
  const e = engine();
  await e.openDocument({ name: "d.zen", text: DOC() });
  const v = (await e.state()).version;
  e.seen.length = 0;
  const env = await e.run("buffer.set", { text: DOC(' opacity=0.5') }, { version: v, client: "agent-1", diff: true });
  assert.equal(env.ok, true);
  assert.ok(env.diff.includes("opacity"), "diff names the change");
  const ev = e.seen.find(([n]) => n === "session");
  assert.equal(ev[1].client, "agent-1");
  assert.equal(ev[1].base_version, v);
  assert.equal(applyDelta(DOC(), ev[1].delta), DOC(' opacity=0.5'));
  assert.equal(env.dirty, true);
});

await test("commands.list adds the file commands", async () => {
  const e = engine();
  await e.openDocument({ name: "d.zen", text: DOC() });
  const list = await e.run("commands.list");
  const ids = list.result.commands.map((c) => c.id);
  for (const id of ["file.save", "file.reload", "file.state"]) assert.ok(ids.includes(id), id);
  assert.equal(list.result.commands.find((c) => c.id === "file.reload").enabled, false);
});

await test("openDocument keeps versions rising and tells the page what changed", async () => {
  const e = engine();
  await e.openDocument({ name: "a.zen", text: DOC() });
  const v1 = (await e.state()).version;
  e.seen.length = 0;
  await e.openDocument({ name: "b.zen", text: DOC(" opacity=0.25") });
  const ev = e.seen.find(([n]) => n === "opened")[1];
  assert.equal(ev.base_version, v1);
  assert.ok(ev.version > v1);
  assert.equal(applyDelta(DOC(), ev.delta), DOC(" opacity=0.25"));
  assert.equal(ev.dirty, false);
  assert.equal((await e.state()).path, "b.zen");
});

await test("lazy fonts: the named files load, the render equals one with the fonts passed in", async () => {
  const fetched = [];
  const e = engine({ fonts: fontsFromDisk(fetched) });
  const text = example("code.zen");
  await e.openDocument({ name: "code.zen", text });
  assert.ok(e.loadedFonts.has("NotoSansMono-Regular.ttf"), `loaded ${[...e.loadedFonts]}`);
  const faces = (await e.run("fonts.required")).result.faces;
  assert.deepEqual(faces, []);
  const diag = (await e.run("doc.diagnose")).result.diagnostics.filter((d) => d.code === "font.unresolved");
  assert.equal(diag.length, 0);
  const lazy = (await e.run("doc.render", { scale: 1 })).result.sha256;
  // The same document with every font file given up front.
  const all = engine();
  await all.worker.project({ fonts: Object.fromEntries([...e.loadedFonts].map((f) => [f, readFileSync(path.join(fontFiles, f)).toString("base64")])) });
  await all.openDocument({ name: "code.zen", text });
  assert.equal((await all.run("doc.render", { scale: 1 })).result.sha256, lazy);
  assert.equal(new Set(fetched).size, fetched.length, "each font file is fetched once");
});

await test("a font file that cannot be fetched leaves the advisory and ends the loop", async () => {
  const e = engine({ fonts: { get: async () => { throw new Error("404"); } } });
  const warn = console.warn;
  console.warn = () => {};
  try {
    await e.openDocument({ name: "code.zen", text: example("code.zen") });
  } finally {
    console.warn = warn;
  }
  const diag = (await e.run("doc.diagnose")).result.diagnostics.filter((d) => d.code === "font.unresolved");
  assert.ok(diag.length > 0, "the advisory stays");
  const calls = e.worker.calls.filter((c) => c === "fonts.required").length;
  assert.ok(calls <= 3, `${calls} fonts.required calls`);
});

await test("project files reach the engine: an image asset resolves, and is missing without them", async () => {
  const text = example("image.zen");
  const dir = path.join(root, "examples/assets");
  const files = {};
  for (const n of readdirSync(dir)) files[`assets/${n}`] = readFileSync(path.join(dir, n)).toString("base64");
  const withFiles = engine();
  await withFiles.openDocument({ name: "image.zen", text, project: { root: "", files, handles: new Map() } });
  const ok = (await withFiles.run("doc.diagnose")).result;
  assert.equal(ok.diagnostics.filter((d) => d.severity === "error").length, 0, JSON.stringify(ok.diagnostics));
  const lone = engine();
  await lone.openDocument({ name: "image.zen", text });
  const missing = (await lone.run("doc.diagnose")).result.diagnostics.map((d) => d.code);
  assert.ok(missing.includes("asset.missing"), missing.join());
});

await test("save writes the handle; reload, poll, conflict and overwrite follow the server's rules", async () => {
  const h = fileHandle("d.zen", DOC());
  const e = engine();
  await e.openDocument({ name: "d.zen", text: DOC(), handle: h });
  let v = (await e.state()).version;
  // Edit and save.
  await e.run("buffer.set", { text: DOC(" opacity=0.5") }, { version: v });
  assert.equal((await e.state()).dirty, true);
  v = (await e.state()).version;
  const saved = await e.save({ version: v });
  assert.equal(saved.ok, true, JSON.stringify(saved.error));
  assert.equal(h.state.text, DOC(" opacity=0.5"));
  assert.equal(saved.result.saved, true);
  assert.equal(saved.result.bytes, Buffer.byteLength(DOC(" opacity=0.5")));
  assert.equal(saved.dirty, false);
  assert.ok(e.seen.some(([n]) => n === "saved"));
  // A clean session takes a changed file.
  e.seen.length = 0;
  h.state.text = DOC(" opacity=0.75");
  h.state.mtime++;
  assert.equal(await e.pollDisk(), "reloaded");
  assert.equal((await e.state({ text: true })).text, DOC(" opacity=0.75"));
  const ext = e.seen.find(([n]) => n === "external_change")[1];
  assert.equal(ext.reloaded, true);
  assert.equal(ext.conflict, false);
  // A dirty session keeps its text and records a conflict.
  v = (await e.state()).version;
  await e.run("buffer.set", { text: DOC(" opacity=0.1") }, { version: v });
  h.state.text = DOC(" opacity=0.9");
  h.state.mtime++;
  assert.equal(await e.pollDisk(), "conflict");
  const st = await e.state({ text: true });
  assert.equal(st.conflict, true);
  assert.equal(st.disk_text, DOC(" opacity=0.9"));
  assert.equal(st.text, DOC(" opacity=0.1"));
  v = st.version;
  const refused = await e.save({ version: v });
  assert.equal(refused.error.code, "edit.conflict");
  assert.deepEqual(refused.error.offers.map((o) => o.id), ["reload", "overwrite"]);
  assert.equal(h.state.text, DOC(" opacity=0.9"), "a refused save wrote nothing");
  // Reload takes the disk text as one undoable change.
  const reloaded = await e.run("file.reload", {}, { version: v });
  assert.equal(reloaded.ok, true);
  assert.equal(reloaded.result.changed, true);
  assert.equal((await e.state({ text: true })).text, DOC(" opacity=0.9"));
  assert.equal((await e.state()).conflict, false);
  v = (await e.state()).version;
  assert.equal((await e.run("history.undo", {}, { version: v })).ok, true);
  assert.equal((await e.state({ text: true })).text, DOC(" opacity=0.1"));
  // Overwrite replaces the file.
  v = (await e.state()).version;
  h.state.text = DOC(" opacity=0.6");
  h.state.mtime++;
  assert.equal((await e.save({ version: v })).error.code, "edit.conflict");
  const over = await e.save({ version: v, overwrite: true });
  assert.equal(over.ok, true);
  assert.equal(h.state.text, DOC(" opacity=0.1"));
  assert.equal((await e.state()).conflict, false);
});

await test("a clean session that finds a changed file reloads and refuses the save", async () => {
  const h = fileHandle("d.zen", DOC());
  const e = engine();
  await e.openDocument({ name: "d.zen", text: DOC(), handle: h });
  h.state.text = DOC(" opacity=0.3");
  h.state.mtime++;
  const env = await e.save({ version: (await e.state()).version });
  assert.equal(env.error.code, "edit.reloaded");
  assert.equal((await e.state({ text: true })).text, DOC(" opacity=0.3"));
});

await test("save without a handle downloads through the browser, and says so", async () => {
  const e = engine();
  await e.openDocument({ name: "d.zen", text: DOC() });
  const v = (await e.state()).version;
  await e.run("buffer.set", { text: DOC(" opacity=0.5") }, { version: v });
  // `download` needs a document; stub the one call.
  const made = [];
  globalThis.URL.createObjectURL = (b) => (made.push(b), "blob:x");
  globalThis.URL.revokeObjectURL = () => {};
  globalThis.document.createElement = () => ({ click() { made.push("click"); }, remove() {}, set href(v) {}, set download(v) { made.push(`name:${v}`); }, set hidden(v) {} });
  globalThis.document.body = { appendChild() {} };
  const env = await e.save({ version: (await e.state()).version });
  assert.equal(env.ok, true, JSON.stringify(env.error));
  assert.ok(env.result.warning.includes("cannot write files back"));
  assert.ok(made.includes("name:d.zen"));
  assert.equal(await made[0].text(), DOC(" opacity=0.5"));
  assert.equal((await e.state()).dirty, false);
});

await test("commands run in order: concurrent calls see each other's sessions", async () => {
  const e = engine();
  await e.openDocument({ name: "d.zen", text: DOC() });
  const v = (await e.state()).version;
  const first = e.run("buffer.set", { text: DOC(" opacity=0.5") }, { version: v });
  const second = e.run("buffer.set", { text: DOC(" opacity=0.4") }, { version: v + 1 });
  const stale = e.run("buffer.set", { text: DOC(" opacity=0.3") }, { version: v + 1 });
  const [a, b, c] = await Promise.all([first, second, stale]);
  assert.equal(a.ok, true);
  assert.equal(b.ok, true);
  assert.equal(c.error.code, "editor.stale_version");
});

for (const f of failures) console.error(`FAIL ${f}`);
console.log(JSON.stringify({ suite: "wasm-engine-unit", passed, failed: failures.length }));
process.exit(failures.length ? 1 : 0);
