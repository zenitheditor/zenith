// The page side of the engine Worker (see `worker.js`): request ids,
// replies, and per-call timings.

import { EngineError } from "../errors.js";

const KEEP_TIMINGS = 4000;

export class WorkerClient {
  /** `workerUrl`: the module Worker script. `wasmUrl`: the engine module. */
  constructor(workerUrl, wasmUrl) {
    this.workerUrl = workerUrl;
    this.wasmUrl = wasmUrl;
    this.worker = null;
    this.loading = null;
    this.nextId = 1;
    this.pending = new Map();
    /** One entry per call: `{method, command, ms, runMs, instantiateMs, parseMs, bytes}`. */
    this.timings = [];
    /** Milliseconds the module took to compile, once loaded. */
    this.compileMs = 0;
  }

  /** Start the Worker and compile the module. Safe to call again. */
  ready() {
    if (!this.loading) {
      this.loading = (async () => {
        let worker;
        try {
          worker = new Worker(this.workerUrl, { type: "module" });
        } catch (err) {
          throw new EngineError("wasm.worker_failed", `cannot start the engine Worker (${err.message}); serve the page over http(s) from the same origin as js/engine/wasm/worker.js`);
        }
        this.worker = worker;
        worker.onmessage = (e) => this.onMessage(e.data);
        worker.onerror = (e) => this.fail(`the engine Worker failed (${e.message || "script error"}); check that js/engine/wasm/worker.js is served as JavaScript from the same origin`);
        const reply = await this.send({ type: "load", wasmUrl: this.wasmUrl }).catch((err) => {
          throw new EngineError(
            "wasm.load_failed",
            `cannot load ${this.wasmUrl} (${err.message}); check that the file exists on this site, is served as application/wasm, and that the Content-Security-Policy allows 'wasm-unsafe-eval' in script-src`,
          );
        });
        this.compileMs = reply.compileMs;
        return reply;
      })();
      this.loading.catch(() => {
        this.worker?.terminate();
        this.worker = null;
        this.loading = null;
      });
    }
    return this.loading;
  }

  /**
   * Stop the Worker. Pending calls reject with `wasm.worker_failed`. The
   * next call starts a new Worker. A page that makes many engines (a test)
   * closes each one: a Worker's spare module instances hold address space
   * until its own garbage collection runs, which another Worker cannot start.
   */
  close() {
    this.worker?.terminate();
    this.fail("the engine Worker was closed");
    this.worker = null;
    this.loading = null;
  }

  onMessage(data) {
    const waiting = this.pending.get(data.id);
    if (!waiting) return;
    this.pending.delete(data.id);
    if (data.ok) waiting.resolve(data);
    else waiting.reject(new EngineError("wasm.engine_failed", data.error));
  }

  fail(message) {
    const err = new EngineError("wasm.worker_failed", message);
    for (const waiting of this.pending.values()) waiting.reject(err);
    this.pending.clear();
  }

  send(message, transfer = []) {
    return new Promise((resolve, reject) => {
      const id = this.nextId++;
      this.pending.set(id, { resolve, reject });
      this.worker.postMessage({ id, ...message }, transfer);
    });
  }

  /** Replace the project `files`, add `fonts`, or `resetFonts`. */
  async project(update) {
    await this.ready();
    await this.send({ type: "project", ...update });
  }

  /**
   * One module request: `method` with `params`, and the stored project when
   * `project`. Resolves the Worker reply `{response, png, ...}`.
   */
  async call(method, params, { project = true, command = "" } = {}) {
    await this.ready();
    const t0 = performance.now();
    const reply = await this.send({ type: "call", method, params, project });
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
