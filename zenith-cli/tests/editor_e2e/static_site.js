// The static build for the e2e runs: build it with the real build script,
// then serve it from a tiny Node file server that behaves like a strict
// static host (right content types, the `_headers` CSP, no `/api`).

import { spawnSync } from "node:child_process";
import { createReadStream, existsSync, readFileSync, statSync } from "node:fs";
import http from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
export const BUILD_SCRIPT = path.resolve(here, "../../scripts/build-static-editor.mjs");

const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".json": "application/json",
  ".wasm": "application/wasm",
  ".ttf": "font/ttf",
  ".zen": "text/plain; charset=utf-8",
  ".png": "image/png",
  ".svg": "image/svg+xml",
  ".txt": "text/plain; charset=utf-8",
};

/** Build the static editor into `out` from the wasm module `wasm`. */
export function buildSite(out, wasm) {
  const r = spawnSync(process.execPath, [BUILD_SCRIPT, "--out", out, "--wasm", wasm], { encoding: "utf8" });
  if (r.status !== 0) throw new Error(`the static build failed: ${r.stderr || r.stdout}`);
  return r.stdout.trim();
}

/** The headers `_headers` gives every path (the `/*` block). */
function globalHeaders(dir) {
  const file = path.join(dir, "_headers");
  if (!existsSync(file)) return {};
  const out = {};
  let inBlock = false;
  for (const line of readFileSync(file, "utf8").split("\n")) {
    if (line.startsWith("#") || !line.trim()) continue;
    if (!line.startsWith(" ")) inBlock = line.trim() === "/*";
    else if (inBlock) {
      const i = line.indexOf(":");
      out[line.slice(0, i).trim()] = line.slice(i + 1).trim();
    }
  }
  return out;
}

/**
 * Serve `dir` under URL path `prefix` (default `/`). `wasmType: false`
 * serves `.wasm` as `application/octet-stream`. Resolves `{url, close}`;
 * `url` is the page address.
 */
export function serve(dir, { prefix = "/", wasmType = true, csp = true } = {}) {
  const headers = globalHeaders(dir);
  if (!csp) delete headers["Content-Security-Policy"];
  const server = http.createServer((req, res) => {
    const url = new URL(req.url, "http://localhost");
    const send = (status, body, extra = {}) => {
      res.writeHead(status, { "Content-Type": "text/plain", ...extra });
      res.end(body);
    };
    if (!url.pathname.startsWith(prefix)) return send(404, "not found");
    let rel = decodeURIComponent(url.pathname.slice(prefix.length));
    if (rel === "" || rel.endsWith("/")) rel += "index.html";
    const file = path.resolve(dir, rel);
    if (!file.startsWith(path.resolve(dir) + path.sep) || !existsSync(file) || !statSync(file).isFile()) {
      return send(404, "not found");
    }
    const ext = path.extname(file);
    let type = TYPES[ext] ?? "application/octet-stream";
    if (ext === ".wasm" && !wasmType) type = "application/octet-stream";
    res.writeHead(200, { ...headers, "Content-Type": type, "Cache-Control": "no-cache" });
    createReadStream(file).pipe(res);
  });
  return new Promise((resolve) => {
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address();
      resolve({
        url: `http://127.0.0.1:${port}${prefix}`,
        close: () => new Promise((r) => server.close(r)),
      });
    });
  });
}
