// The engine Worker of a static host. It compiles the wasm module once and
// runs each request on a fresh instance (the module keeps no state).
//
// The Worker also holds the project (see `ProjectStore` in `request.js`):
// the `files` map (path -> base64) as JSON text, and the bundled and
// fallback font files (file -> Uint8Array) as raw bytes. Both are large and
// change rarely, so the page sends them once and each request refers to
// them. The files JSON is spliced into the request text. The font bytes
// follow the envelope on stdin with no base64 and no JSON, so a call never
// copies or re-serializes them on the page thread.
//
// Messages in (all carry `id`):
//   {type: "load", wasmUrl}                  compile the module
//   {type: "project", files?, fonts?, resetFonts?}
//                                            files: replace the files map.
//                                            fonts: add these font files
//                                            (file -> Uint8Array); a file
//                                            replaces one of the same name.
//                                            resetFonts: drop all fonts first.
//   {type: "call", method, params, project}  one request. With `project`,
//                                            the stored files and fonts are
//                                            added to `params`.
// Messages out: `{id, ok: true, ...}` or `{id, ok: false, error}`. A call
// answers `{response, png, instantiateMs, runMs, parseMs, requestBytes,
// fontBytes}` (`requestBytes` counts all of stdin, `fontBytes` the font
// part of it):
// `response` is the parsed module response with `result.png_base64` moved
// to `png`, an ArrayBuffer (transferred) or null.

import { ProjectStore, takePng } from "./request.js";
import { missingImports, runRequest } from "./wasi.js";

let modulePromise = null;
let compileMs = 0;
const store = new ProjectStore();

function loadModule(url) {
  if (!modulePromise) {
    modulePromise = (async () => {
      const t0 = performance.now();
      let module;
      try {
        module = await WebAssembly.compileStreaming(fetch(url));
      } catch (e) {
        // A server that does not send `application/wasm` fails streaming
        // compilation. Compile from the bytes instead.
        if (!(e instanceof TypeError)) throw e;
        const res = await fetch(url);
        if (!res.ok) throw new Error(`${url} answered ${res.status} ${res.statusText}`);
        module = await WebAssembly.compile(await res.arrayBuffer());
      }
      compileMs = performance.now() - t0;
      const missing = missingImports(module);
      if (missing.length) {
        throw new Error(`the WASI shim lacks imports ${missing.join(", ")}; update js/engine/wasm/wasi.js`);
      }
      return module;
    })();
    // A failed load can be retried.
    modulePromise.catch(() => {
      modulePromise = null;
    });
  }
  return modulePromise;
}

async function call({ method, params, project }) {
  const module = await loadModule(self.zenithWasmUrl);
  const stdin = store.request(method, params, project);
  const run = await runRequest(module, stdin.chunks);
  const t0 = performance.now();
  const response = JSON.parse(run.response);
  const png = takePng(response);
  const parseMs = performance.now() - t0;
  const transfer = png ? [png] : [];
  return {
    message: {
      response,
      png,
      stderr: run.stderr,
      exitCode: run.exitCode,
      instantiateMs: run.instantiateMs,
      runMs: run.runMs,
      parseMs,
      requestBytes: stdin.jsonBytes + (stdin.fontBytes ? 1 + stdin.fontBytes : 0),
      fontBytes: stdin.fontBytes,
    },
    transfer,
  };
}

self.onmessage = async (event) => {
  const data = event.data;
  const { id, type } = data;
  try {
    if (type === "load") {
      self.zenithWasmUrl = data.wasmUrl;
      await loadModule(data.wasmUrl);
      self.postMessage({ id, ok: true, compileMs });
    } else if (type === "project") {
      store.update(data);
      self.postMessage({ id, ok: true });
    } else if (type === "call") {
      const { message, transfer } = await call(data);
      self.postMessage({ id, ok: true, ...message }, transfer);
    } else {
      throw new Error(`unknown worker message type '${type}'`);
    }
  } catch (e) {
    self.postMessage({ id, ok: false, error: String(e && e.message ? e.message : e) });
  }
};
