// Async unit tests of page modules, run by `unit.js`: the buffer sync over a
// fake engine (CRLF text, UTF-16 offsets, typing while an engine edit is on
// the wire, resync), the engine Worker client over a fake Worker (restart,
// reply timeout, project replay), file writes, swatch colors, and the
// intent queue (clicks and keys run in input order over a slow engine).
//
// Exports `tests`: `[[name, async fn], …]`. Zero dependencies.

import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import path from "node:path";

const here = path.dirname(fileURLToPath(import.meta.url));
const assets = path.join(here, "..", "..", "assets", "editor");
const { EditorState, ChangeSet } = await import(path.join(assets, "vendor", "codemirror.js"));
const { BufferSync, changeOf } = await import(path.join(assets, "js", "sync", "buffer.js"));
const { WorkerClient } = await import(path.join(assets, "js", "engine", "wasm", "rpc.js"));
const { writeHandle, isReadOnlyError } = await import(path.join(assets, "js", "engine", "wasm", "files.js"));
const { readEvents } = await import(path.join(assets, "js", "engine", "http.js"));
const { cssColor } = await import(path.join(assets, "js", "util", "color.js"));
const { Serial } = await import(path.join(assets, "js", "util", "serial.js"));
const { SelectionController } = await import(path.join(assets, "js", "app", "selection.js"));
const { NodeActions } = await import(path.join(assets, "js", "app", "actions.js"));
const { applyDelta } = await import("./delta.js");

/** A promise and its `resolve`. */
function gate() {
  let open;
  const promise = new Promise((r) => (open = r));
  return { promise, open };
}

/** Resolve after pending microtasks and timers of 0 ms ran. */
const tick = () => new Promise((r) => setTimeout(r, 0));

/** The code pane: a CodeMirror state that splits lines on `\n` only, as the page does. */
class Pane {
  constructor(text) {
    this.state = EditorState.create({ doc: text, extensions: [EditorState.lineSeparator.of("\n")] });
    this.sync = null;
  }
  text() {
    return this.state.doc.toString();
  }
  applyRemote(changes) {
    if (changes.empty) return;
    this.state = this.state.update({ changes }).state;
  }
  /** A keystroke: replace `from..to` with `insert` and tell the sync. */
  type(from, insert, to = from) {
    const tr = this.state.update({ changes: { from, to, insert } });
    this.state = tr.state;
    this.sync.localChange(tr.changes);
  }
}

/**
 * The engine session: text and version. `buffer.set` takes the text at the
 * current version; `edit.replace {from, to, insert}` is an engine edit with
 * a delta. `hold(command)` keeps the reply of the next such call until the
 * returned gate opens (the engine has applied it already, as a slow reply).
 */
class Engine {
  constructor(text) {
    this.text = text;
    this.version = 1;
    this.holds = new Map();
    this.calls = [];
    this.stateFailures = 0;
  }
  hold(command) {
    const g = gate();
    const arrived = gate();
    this.holds.set(command, { g, arrived });
    return { release: g.open, arrived: arrived.promise };
  }
  async run(command, params, { version } = {}) {
    this.calls.push(command);
    let env;
    if (version !== undefined && version !== this.version) {
      env = { ok: false, command, version: this.version, error: { code: "editor.stale_version", message: "stale" } };
    } else if (command === "buffer.set") {
      if (params.text !== this.text) {
        this.text = params.text;
        this.version++;
      }
      env = { ok: true, command, version: this.version, dirty: true, result: {} };
    } else if (command === "edit.replace") {
      const delta = { from: params.from, to: params.to, insert: params.insert };
      this.text = applyDelta(this.text, delta);
      this.version++;
      env = { ok: true, command, version: this.version, dirty: true, result: { delta } };
    } else {
      throw new Error(`fake engine: unknown command ${command}`);
    }
    const held = this.holds.get(command);
    if (held) {
      this.holds.delete(command);
      held.arrived.open();
      await held.g.promise;
    }
    return env;
  }
  async state() {
    if (this.stateFailures > 0) {
      this.stateFailures--;
      throw Object.assign(new Error("unreachable"), { code: "edit.unreachable" });
    }
    return { text: this.text, version: this.version, dirty: true };
  }
}

/** A pane, an engine, and the sync between them over `text`. */
function setup(text) {
  const engine = new Engine(text);
  const pane = new Pane(text);
  const log = { changed: [], offline: 0, errors: [] };
  const sync = new BufferSync(engine, pane, { text, version: engine.version }, {
    changed: (info) => log.changed.push(info),
    dirty: () => {},
    error: (env) => log.errors.push(env),
    offline: () => log.offline++,
  });
  pane.sync = sync;
  return { engine, pane, sync, log };
}

/** The pane, the sync base, and the engine hold one text. */
function agree({ engine, pane, sync }, what) {
  assert.equal(pane.text(), engine.text, `${what}: pane differs from the engine`);
  assert.equal(sync.base, engine.text, `${what}: sync base differs from the engine`);
  assert.equal(sync.version, engine.version, `${what}: version`);
  assert.ok(sync.synced(), `${what}: not synced`);
}

const CRLF = 'zenith version=1 {\r\n  rect id="a" fill="#fff"\r\n}\r\n';

/**
 * A page with the selection controller and the key actions over a fake
 * `zenith edit` engine. The engine runs each call when it arrives, as the
 * server does. `holdNext()` keeps the reply of the next call until the
 * returned function runs, as on a slow link. `select.hit` takes the node
 * id as `x`; `extend` toggles it in the selection, as the engine does.
 */
function intentPage() {
  const engine = {
    selection: ["a", "b"],
    sent: [],
    held: null,
    holdNext() {
      const g = gate();
      this.held = g;
      return g.open;
    },
    async run(command, params) {
      this.sent.push({ command, params });
      let result = {};
      if (command === "select.hit") {
        const id = params.x;
        if (!params.extend) this.selection = [id];
        else if (this.selection.includes(id)) this.selection = this.selection.filter((s) => s !== id);
        else this.selection = [...this.selection, id];
        result = { selection: [...this.selection] };
      }
      const held = this.held;
      this.held = null;
      if (held) await held.promise;
      return { ok: true, command, result };
    },
  };
  const app = {
    engine,
    view: { zoom: 1 },
    valid: true,
    stale: false,
    selectionIds: ["a", "b"],
    intents: new Serial(),
    layout: { full: "none" },
    gestures: { cancel: () => false },
    notices: {
      error: (command, env) => {
        throw new Error(`${command}: ${JSON.stringify(env)}`);
      },
      toast() {},
      hide() {},
    },
    announce() {},
    sync: { edit: (run) => run(1) },
  };
  app.selection = new SelectionController(app);
  // The overlay, inspector, and code pane are not here.
  app.selection.refresh = async () => {};
  app.actions = new NodeActions(app);
  return { app, engine };
}

/** A keydown event of `key` with no modifiers. */
const keyEvent = (key) => ({ key, shiftKey: false, ctrlKey: false, metaKey: false, altKey: false });

/** Wait until every intent of `app` ran. */
async function intentsDone(app) {
  while (!app.intents.idle()) await tick();
}

/** The `gesture.commit` targets `engine` got, in order. */
const commits = (engine) =>
  engine.sent.filter((s) => s.command === "gesture.commit").map((s) => s.params.nodes ?? s.params.node);

export const tests = [
  [
    "readEvents: records split across chunks, comments skipped, multi-line data joined",
    async () => {
      const chunks = [
        "id: 1\nevent: state\ndata: {\"a\"",
        ":1}\n\n: ping\n\n",
        "id: 2\nevent: saved\ndata: {}\n\nevent: x\ndata: one\ndata: two\n\n",
      ];
      const body = new ReadableStream({
        start(controller) {
          for (const c of chunks) controller.enqueue(new TextEncoder().encode(c));
          controller.close();
        },
      });
      const seen = [];
      await readEvents(body, (name, data) => seen.push([name, data]));
      assert.deepEqual(seen, [
        ["state", '{"a":1}'],
        ["saved", "{}"],
        ["x", "one\ntwo"],
      ]);
    },
  ],
  [
    "isReadOnlyError: read-only and refused writes, nothing else",
    async () => {
      assert.ok(isReadOnlyError({ name: "NoModificationAllowedError" }));
      assert.ok(isReadOnlyError({ name: "NotAllowedError" }));
      assert.ok(!isReadOnlyError({ name: "QuotaExceededError" }));
      assert.ok(!isReadOnlyError(null));
    },
  ],
  [
    "change sets keep \\r: a CRLF delta lands byte for byte",
    async () => {
      const c = changeOf({ from: 1, to: 1, insert: "x\r\ny\r\n" }, 3);
      assert.equal(c.apply(EditorState.create({ doc: "a\nb", extensions: [EditorState.lineSeparator.of("\n")] }).doc).toString(), "ax\r\ny\r\n\nb");
      // The vendored default splits on \r\n and drops the \r: the reason for the separator.
      assert.equal(ChangeSet.of([{ from: 0, insert: "p\r\nq" }], 0).newLength, 3);
      assert.equal(c.newLength, 3 + 6);
    },
  ],
  [
    "CRLF document: engine edit, typing, and resync keep every \\r",
    async () => {
      const t = setup(CRLF);
      const at = CRLF.indexOf("}");
      const env = await t.sync.edit((version) => t.engine.run("edit.replace", { from: at, to: at, insert: '  rect id="b"\r\n' }, { version }), "test");
      assert.ok(env.ok);
      agree(t, "after the engine edit");
      // A keystroke after the edit lands at the same offset in both texts.
      const pos = t.pane.text().indexOf('"b"') + 2;
      t.pane.type(pos, "c");
      await t.sync.flush();
      agree(t, "after typing");
      assert.ok(t.engine.text.includes('rect id="bc"\r\n}\r\n'), JSON.stringify(t.engine.text));
      assert.equal(t.engine.text.split("\r\n").length, CRLF.split("\r\n").length + 1);
      // A resync over a CRLF difference.
      t.engine.text = t.engine.text.replace("\r\n}", "\r\n  // d\r\n}");
      t.engine.version++;
      await t.sync.resync();
      agree(t, "after the resync");
    },
  ],
  [
    "UTF-16 offsets: deltas after emoji and CRLF land where the engine put them",
    async () => {
      const text = 'a😀\r\nb—é\r\n"😃"\r\n';
      const t = setup(text);
      const at = text.indexOf("😃");
      await t.sync.edit((version) => t.engine.run("edit.replace", { from: at, to: at + 2, insert: "😺\r\n" }, { version }), "test");
      agree(t, "emoji replace");
      t.pane.type(t.pane.text().indexOf("—") + 1, "€");
      await t.sync.flush();
      agree(t, "typing after a 3-byte character");
      // Diagnostics map engine positions into the pane through `toPane`.
      t.pane.type(0, "😀😀");
      assert.equal(t.sync.toPane(1), 5);
      await t.sync.flush();
      agree(t, "typing before");
    },
  ],
  [
    "typing while an engine edit is on the wire rebases, never throws",
    async () => {
      const t = setup(CRLF);
      const held = t.engine.hold("edit.replace");
      const at = CRLF.indexOf("}");
      const edit = t.sync.edit((version) => t.engine.run("edit.replace", { from: at, to: at, insert: '  ellipse id="e"\r\n' }, { version }), "undo");
      await held.arrived;
      assert.ok(!t.sync.synced(), "synced while the engine edit is on the wire");
      // Keystrokes in the gap, and the debounced send firing.
      t.pane.type(0, "// x\r\n");
      t.pane.type(t.pane.text().length, "// tail");
      const send = t.sync.send();
      await tick();
      assert.deepEqual(t.engine.calls, ["edit.replace"], "a keystroke went out while the engine edit was on the wire");
      held.release();
      const env = await edit;
      await send;
      assert.ok(env.ok);
      await t.sync.flush();
      agree(t, "after the edit and the typing");
      assert.ok(t.engine.text.startsWith("// x\r\nzenith"), JSON.stringify(t.engine.text));
      assert.ok(t.engine.text.endsWith('  ellipse id="e"\r\n}\r\n// tail'), JSON.stringify(t.engine.text));
    },
  ],
  [
    "a gesture against an old version runs nothing and keeps the typing",
    async () => {
      const t = setup(CRLF);
      const v = t.sync.version;
      t.pane.type(0, "x");
      const env = await t.sync.editAt(v, (version) => t.engine.run("edit.replace", { from: 0, to: 0, insert: "g" }, { version }), "gesture");
      assert.equal(env.error.code, "editor.stale_version");
      assert.ok(!t.engine.text.includes("g"));
      agree(t, "after the stale gesture");
    },
  ],
  [
    "events during a task wait for it; the echo of the edit is skipped",
    async () => {
      const t = setup(CRLF);
      const held = t.engine.hold("edit.replace");
      const edit = t.sync.edit((version) => t.engine.run("edit.replace", { from: 0, to: 0, insert: "A\r\n" }, { version }), "test");
      await held.arrived;
      // The event of the same change arrives before the reply.
      t.sync.remote({ delta: { from: 0, to: 0, insert: "A\r\n" }, baseVersion: 1, version: 2, source: "remote", extra: {} });
      assert.equal(t.pane.text(), CRLF, "an event applied while a task held the text");
      held.release();
      await edit;
      agree(t, "after the edit");
      assert.equal(t.engine.text, "A\r\n" + CRLF, "the delta applied twice");
      // A later event from another client applies at once.
      t.engine.text = applyDelta(t.engine.text, { from: 0, to: 1, insert: "B" });
      t.engine.version++;
      t.sync.remote({ delta: { from: 0, to: 1, insert: "B" }, baseVersion: 2, version: 3, source: "remote", extra: {} });
      agree(t, "after the foreign event");
    },
  ],
  [
    "resync while a buffer.set is on the wire resolves to the summary",
    async () => {
      const t = setup(CRLF);
      const held = t.engine.hold("buffer.set");
      t.pane.type(0, "x");
      const send = t.sync.send();
      await held.arrived;
      const resync = t.sync.resync();
      held.release();
      const summary = await resync;
      await send;
      assert.equal(typeof summary, "object");
      assert.equal(summary.text, t.engine.text);
      assert.equal(summary.version, t.engine.version);
      agree(t, "after the resync");
    },
  ],
  [
    "resync retries a failed state read and still resolves the summary",
    async () => {
      const t = setup(CRLF);
      t.engine.stateFailures = 1;
      t.engine.text += "// more\r\n";
      t.engine.version++;
      const summary = await t.sync.resync();
      assert.equal(t.log.offline, 1, "the failed read did not reach the offline hook");
      assert.equal(summary.version, t.engine.version);
      agree(t, "after the retried resync");
    },
  ],
  [
    "Worker restart: a late error of the closed Worker leaves the new one alone",
    async () => {
      const fake = installFakeWorker();
      const client = new WorkerClient("w.js", "e.wasm");
      const first = client.ready();
      client.close();
      const second = client.ready();
      await assert.rejects(first, (e) => e.code === "wasm.load_failed");
      await second;
      assert.equal(fake.instances.length, 2);
      assert.ok(fake.instances[0].terminated);
      assert.ok(!fake.instances[1].terminated, "the old load's error terminated the new Worker");
      fake.instances[0].onerror?.({ message: "late" });
      const reply = await client.call("editor", { command: { command: "doc.outline" } });
      assert.equal(reply.response.ok, true);
      assert.ok(!fake.instances[1].terminated);
      client.close();
    },
  ],
  [
    "Worker reply timeout: the call rejects, the Worker restarts with the project",
    async () => {
      const fake = installFakeWorker();
      const client = new WorkerClient("w.js", "e.wasm", { timeoutMs: 40 });
      await client.project({ files: { "a.svg": "eA==" }, resetFonts: true });
      await client.project({ fonts: { "F.ttf": "AA==" } });
      fake.hang = true;
      await assert.rejects(
        client.call("editor", { command: { command: "doc.render" } }),
        (e) => e.code === "wasm.worker_failed" && /did not answer 'doc.render'/.test(e.message),
      );
      assert.ok(fake.instances[0].terminated, "the stuck Worker still runs");
      fake.hang = false;
      const reply = await client.call("editor", { command: { command: "doc.outline" } });
      assert.equal(reply.response.ok, true);
      const replay = fake.instances[1].seen.find((m) => m.type === "project");
      assert.deepEqual(replay.files, { "a.svg": "eA==" });
      assert.deepEqual(replay.fonts, { "F.ttf": "AA==" });
      client.close();
    },
  ],
  [
    "writeHandle: a failed write aborts and reports the write error",
    async () => {
      const log = [];
      const handle = {
        async createWritable() {
          return {
            async write() {
              log.push("write");
              throw new Error("disk full");
            },
            async abort() {
              log.push("abort");
              throw new Error("abort failed");
            },
            async close() {
              log.push("close");
              throw new Error("close failed");
            },
          };
        },
      };
      await assert.rejects(writeHandle(handle, "text"), /disk full/);
      assert.deepEqual(log, ["write", "abort"]);
      const ok = [];
      const good = {
        async createWritable() {
          return { async write(t) { ok.push(t); }, async close() { ok.push("close"); } };
        },
        async getFile() {
          return { lastModified: 7 };
        },
      };
      assert.equal(await writeHandle(good, "t"), 7);
      assert.deepEqual(ok, ["t", "close"]);
      const failingClose = {
        async createWritable() {
          return { async write() {}, async close() { throw new Error("close failed"); } };
        },
      };
      await assert.rejects(writeHandle(failingClose, "t"), /close failed/);
    },
  ],
  [
    "swatch colors: only a whole CSS color passes",
    async () => {
      for (const good of ["#102030", "#fff", "#ffff", "#11223344", "rgb(1, 2, 3)", "rgba(1,2,3,0.5)", "rgb(10% 20% 30% / 50%)", "hsl(120deg 50% 50%)", "hsla(120, 50%, 50%, .3)", " #abc "]) {
        assert.equal(cssColor(good), good.trim(), good);
      }
      for (const bad of [
        "red;background:url(https://x/)",
        "#fff;position:fixed",
        "#fffff",
        "rgb(1,2,3);background-image:url(x)",
        'rgb(1,2,3)" onmouseover="alert(1)',
        "rgba(0,0,0,0.5) url(x)",
        "hsl(1, 2%, 3%) ;",
        "rgb(1,2,3",
        "rgb(1,2,3)\n;x:y",
        "expression(alert(1))",
        "var(--x)",
        "",
        null,
        42,
        `rgb(${"1".repeat(200)},2,3)`,
      ]) {
        assert.equal(cssColor(bad), null, JSON.stringify(bad));
      }
    },
  ],
  [
    "Serial: tasks run one at a time in call order; a rejected task does not stop the next",
    async () => {
      const serial = new Serial();
      const log = [];
      const first = gate();
      const a = serial.run(async () => {
        log.push("a start");
        await first.promise;
        log.push("a end");
        throw new Error("a failed");
      });
      const b = serial.run(async () => {
        log.push("b");
        return 2;
      });
      await tick();
      assert.deepEqual(log, ["a start"]);
      assert.ok(!serial.idle());
      first.open();
      await assert.rejects(a, /a failed/);
      assert.equal(await b, 2);
      assert.deepEqual(log, ["a start", "a end", "b"]);
      await tick();
      assert.ok(serial.idle());
    },
  ],
  [
    "intents: a key pressed while a click's reply is on the wire acts on the selection the click made",
    async () => {
      const { app, engine } = intentPage();
      const release = engine.holdNext();
      const click = app.selection.canvasClick("a", 0, { shiftKey: false });
      await tick();
      // Two nudges at once, before the click reply: both wait for it.
      assert.ok(app.actions.key(keyEvent("ArrowRight")));
      assert.ok(app.actions.key(keyEvent("ArrowRight")));
      await tick();
      assert.deepEqual(commits(engine), [], "a nudge went out before the click reply");
      assert.deepEqual(app.selectionIds, ["a", "b"]);
      release();
      await click;
      await intentsDone(app);
      assert.deepEqual(app.selectionIds, ["a"]);
      assert.deepEqual(commits(engine), ["a", "a"], "the nudges did not act on the clicked selection");
    },
  ],
  [
    "intents: a Shift+click waits for the click before it, so the engine and the page agree",
    async () => {
      const { app, engine } = intentPage();
      const release = engine.holdNext();
      const first = app.selection.canvasClick("b", 0, { shiftKey: false });
      const second = app.selection.canvasClick("a", 0, { shiftKey: true });
      await tick();
      // Sent at once, the Shift+click could reach the server first and
      // toggle a out of the old selection [a, b].
      assert.equal(engine.sent.filter((s) => s.command === "select.hit").length, 1, "the Shift+click went out before the click reply");
      release();
      await Promise.all([first, second]);
      assert.deepEqual(engine.sent.map((s) => [s.params.x, !!s.params.extend]), [["b", false], ["a", true]]);
      assert.deepEqual(engine.selection, ["b", "a"]);
      assert.deepEqual(app.selectionIds, ["b", "a"]);
      // A key now acts on both.
      app.actions.key(keyEvent("ArrowLeft"));
      await intentsDone(app);
      assert.deepEqual(commits(engine), [["b", "a"]]);
    },
  ],
  [
    "intents: with nothing selected and no intent waiting, keys are not actions",
    async () => {
      const { app, engine } = intentPage();
      app.selectionIds = [];
      assert.equal(app.actions.key(keyEvent("ArrowRight")), false);
      // A click on the wire can still select: the key waits for it.
      const release = engine.holdNext();
      const click = app.selection.canvasClick("b", 0, { shiftKey: false });
      assert.ok(app.actions.key(keyEvent("Delete")));
      release();
      await click;
      await intentsDone(app);
      const removed = engine.sent.filter((s) => s.command === "node.remove").map((s) => s.params.ids);
      assert.deepEqual(removed, [["b"]]);
    },
  ],
];

/** Replace the global `Worker` with a fake. `hang` stops replies to `call`. */
function installFakeWorker() {
  const fake = { instances: [], hang: false };
  globalThis.Worker = class {
    constructor() {
      this.terminated = false;
      this.seen = [];
      this.onmessage = null;
      this.onerror = null;
      fake.instances.push(this);
    }
    postMessage(msg) {
      this.seen.push(msg);
      if (msg.type === "call" && fake.hang) return;
      setTimeout(() => {
        if (this.terminated) return;
        const reply = msg.type === "load" ? { compileMs: 1 } : msg.type === "call" ? { response: { ok: true }, png: null } : {};
        this.onmessage?.({ data: { id: msg.id, ok: true, ...reply } });
      }, 1);
    }
    terminate() {
      this.terminated = true;
    }
  };
  return fake;
}
