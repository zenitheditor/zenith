// The engine Worker of a static host. It compiles the wasm module once and
// runs each request on a fresh instance (the module keeps no state).
//
// The Worker also holds the project: the `files` map (path -> base64) and
// the bundled `fonts` map (file -> base64). Both are large and change
// rarely, so the page sends them once and each request refers to them.
// The Worker splices their JSON text into the request, so a call never
// copies or re-serializes them on the page thread.
//
// Messages in (all carry `id`):
//   {type: "load", wasmUrl}                  compile the module
//   {type: "project", files?, fonts?, resetFonts?}
//                                            files: replace the files map.
//                                            fonts: add these font files.
//                                            resetFonts: drop all fonts first.
//   {type: "call", method, params, project}  one request. With `project`,
//                                            the stored files and fonts are
//                                            added to `params`.
// Messages out: `{id, ok: true, ...}` or `{id, ok: false, error}`. A call
// answers `{response, png, instantiateMs, runMs, parseMs, requestBytes}`:
// `response` is the parsed module response with `result.png_base64` moved
// to `png`, an ArrayBuffer (transferred) or null.

import { requestText, takePng } from "./request.js";
import { missingImports, runRequest } from "./wasi.js";

let modulePromise = null;
let compileMs = 0;
let filesJson = "{}";
let fonts = {};
let fontsJson = "{}";

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
  const text = requestText(method, params, project, filesJson, fontsJson);
  const run = await runRequest(module, text);
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
      requestBytes: text.length,
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
      if (data.files) filesJson = JSON.stringify(data.files);
      if (data.resetFonts) fonts = {};
      if (data.fonts) Object.assign(fonts, data.fonts);
      if (data.resetFonts || data.fonts) fontsJson = JSON.stringify(fonts);
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
