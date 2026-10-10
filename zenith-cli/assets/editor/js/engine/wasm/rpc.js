// The page side of the engine Worker (see `worker.js`): request ids,
// replies, timeouts, and per-call timings.
//
// Each started Worker is one generation with its own pending calls. A
// generation stops when its Worker fails, when a reply does not come in
// time, or on `close()`. Its calls reject with `wasm.worker_failed`, and the
// next call starts a new Worker. Only the stopped generation is touched, so
// a late error of an old Worker never stops its successor. The project
// (files and fonts) the page sent goes again to each new Worker.

import { EngineError } from "../errors.js";

const KEEP_TIMINGS = 4000;
/** How long a Worker may take to answer one message before it is replaced. */
export const REPLY_TIMEOUT_MS = 120_000;

export class WorkerClient {
  /**
   * `workerUrl`: the module Worker script. `wasmUrl`: the engine module.
   * `timeoutMs`: the reply timeout (default `REPLY_TIMEOUT_MS`).
   */
  constructor(workerUrl, wasmUrl, { timeoutMs = REPLY_TIMEOUT_MS } = {}) {
    this.workerUrl = workerUrl;
    this.wasmUrl = wasmUrl;
    this.timeoutMs = timeoutMs;
    /** The running generation: `{worker, pending, loading, stopped}`, or `null`. */
    this.current = null;
    this.nextId = 1;
    /** The project files map last sent, or `null`; sent again to a new Worker. */
    this.files = null;
    /** Every font file sent since the last reset; sent again to a new Worker. */
    this.fonts = {};
    /** One entry per call: `{method, command, ms, runMs, instantiateMs, parseMs, bytes}`. */
    this.timings = [];
    /** Milliseconds the module took to compile, once loaded. */
    this.compileMs = 0;
  }

  /** The running Worker, or `null`. */
  get worker() {
    return this.current?.worker ?? null;
  }

  /** Start the Worker and compile the module. Safe to call again. */
  ready() {
    return this.generation().then((gen) => gen.reply);
  }

  /** The running generation, started when there is none, once loaded. */
  async generation() {
    if (!this.current) this.current = this.start();
    const gen = this.current;
    await gen.loading;
    return gen;
  }

  /** Start a Worker: load the module, then send the project it must hold. */
  start() {
    const gen = { worker: null, pending: new Map(), loading: null, reply: null, stopped: false };
    gen.loading = (async () => {
      try {
        gen.worker = new Worker(this.workerUrl, { type: "module" });
      } catch (err) {
        throw new EngineError("wasm.worker_failed", `cannot start the engine Worker (${err.message}); serve the page over http(s) from the same origin as js/engine/wasm/worker.js`);
      }
      gen.worker.onmessage = (e) => this.onMessage(gen, e.data);
      gen.worker.onerror = (e) => this.stop(gen, `the engine Worker failed (${e.message || "script error"}); check that js/engine/wasm/worker.js is served as JavaScript from the same origin`);
      gen.reply = await this.send(gen, { type: "load", wasmUrl: this.wasmUrl }).catch((err) => {
        throw new EngineError(
          "wasm.load_failed",
          `cannot load ${this.wasmUrl} (${err.message}); check that the file exists on this site, is served as application/wasm, and that the Content-Security-Policy allows 'wasm-unsafe-eval' in script-src`,
        );
      });
      if (this.files !== null || Object.keys(this.fonts).length) {
        await this.send(gen, { type: "project", files: this.files ?? {}, fonts: this.fonts, resetFonts: true });
      }
      this.compileMs = gen.reply.compileMs;
    })();
    gen.loading.catch((err) => this.stop(gen, err.message));
    return gen;
  }

  /**
   * Stop generation `gen`: end its Worker and reject its pending calls with
   * `wasm.worker_failed` and `message`. The next call starts a new Worker.
   */
  stop(gen, message) {
    if (gen.stopped) return;
    gen.stopped = true;
    gen.worker?.terminate();
    const err = new EngineError("wasm.worker_failed", message);
    for (const waiting of gen.pending.values()) {
      clearTimeout(waiting.timer);
      waiting.reject(err);
    }
    gen.pending.clear();
    if (this.current === gen) this.current = null;
  }

  /**
   * Stop the Worker. Pending calls reject with `wasm.worker_failed`. The
   * next call starts a new Worker. A page that makes many engines (a test)
   * closes each one: a Worker's spare module instances hold address space
   * until its own garbage collection runs, which another Worker cannot start.
   */
  close() {
    if (this.current) this.stop(this.current, "the engine Worker was closed");
  }

  onMessage(gen, data) {
    const waiting = gen.pending.get(data.id);
    if (!waiting) return;
    gen.pending.delete(data.id);
    clearTimeout(waiting.timer);
    if (data.ok) waiting.resolve(data);
    else waiting.reject(new EngineError("wasm.engine_failed", data.error));
  }

  /** Post `message` to the Worker of `gen`; resolves its reply. */
  send(gen, message, transfer = []) {
    return new Promise((resolve, reject) => {
      if (gen.stopped) {
        reject(new EngineError("wasm.worker_failed", "the engine Worker stopped; retry the command"));
        return;
      }
      const id = this.nextId++;
      const what = message.type === "call" ? `'${message.params?.command?.command ?? message.method}'` : `'${message.type}'`;
      const timer = setTimeout(() => {
        this.stop(gen, `the engine Worker did not answer ${what} within ${Math.round(this.timeoutMs / 1000)} s; it was stopped, and the next command starts a new one`);
      }, this.timeoutMs);
      gen.pending.set(id, { resolve, reject, timer });
      gen.worker.postMessage({ id, ...message }, transfer);
    });
  }

  /** Replace the project `files`, add `fonts`, or `resetFonts`. */
  async project(update) {
    if (update.files) this.files = update.files;
    if (update.resetFonts) this.fonts = {};
    if (update.fonts) Object.assign(this.fonts, update.fonts);
    const gen = await this.generation();
    await this.send(gen, { type: "project", ...update });
  }

  /**
   * One module request: `method` with `params`, and the stored project when
   * `project`. Resolves the Worker reply `{response, png, ...}`.
   */
  async call(method, params, { project = true, command = "" } = {}) {
    const gen = await this.generation();
    const t0 = performance.now();
    const reply = await this.send(gen, { type: "call", method, params, project });
    this.timings.push({
      method,
      command,
      ms: performance.now() - t0,
      runMs: reply.runMs,
      instantiateMs: reply.instantiateMs,
      parseMs: reply.parseMs,
      bytes: reply.requestBytes,
    });
    if (this.timings.length > KEEP_TIMINGS) this.timings.splice(0, this.timings.length - KEEP_TIMINGS);
    return reply;
  }
}
