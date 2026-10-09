// Entry point of the editor page. One page, two hosts:
//
//   server  `zenith edit` answers `/api/state`: the engine is native, over HTTP.
//   wasm    a static site: the engine is the wasm module in a Worker.
//
// `<meta name="zenith-host" content="http|wasm|auto">` pins the host. The
// default `auto` asks `/api/state` once.

import { App } from "./app/app.js";
import { loadSample } from "./app/samples.js";
import { EngineError, HttpEngine, WasmEngine } from "./engine/index.js";
import { hydrateIcons } from "./ui/icons.js";
import { byId } from "./util/dom.js";
import { sleep } from "./util/timing.js";

async function main() {
  hydrateIcons(document);
  const host = await chooseHost();
  const { engine, summary } = host === "wasm" ? await startWasm() : await startServer();
  byId("banners").replaceChildren();
  const app = new App(engine, summary);
  window.zenithEditor = app;
  await app.start();
  document.documentElement.dataset.host = host;
  document.documentElement.dataset.ready = "true";
}

/** `"http"` when `zenith edit` serves this page, else `"wasm"`. */
async function chooseHost() {
  const pinned = document.querySelector('meta[name="zenith-host"]')?.content;
  if (pinned === "http" || pinned === "wasm") return pinned;
  try {
    const res = await fetch("/api/state", { credentials: "same-origin" });
    const json = await res.json().catch(() => null);
    const server = json && (json.ok === true || String(json.error?.code ?? "").startsWith("edit."));
    return server ? "http" : "wasm";
  } catch {
    // No answer at all: a stopped `zenith edit`. Its retry loop handles it.
    return "http";
  }
}

async function startServer() {
  // The token authenticated this load and set the cookie. Drop it from the
  // address bar and history.
  if (new URLSearchParams(location.search).has("token")) history.replaceState(null, "", "/");
  const engine = new HttpEngine();
  let summary = null;
  for (let attempt = 0; summary === null; attempt++) {
    try {
      summary = await engine.state({ text: true });
    } catch (err) {
      fatal(err, attempt);
      if (err.code === "edit.unauthorized") await new Promise(() => {});
      await sleep(Math.min(8000, 500 * 2 ** attempt));
    }
  }
  return { engine, summary };
}

async function startWasm() {
  const meta = (name) => document.querySelector(`meta[name="${name}"]`)?.content || undefined;
  const engine = new WasmEngine({ wasmUrl: meta("zenith-wasm"), fontsUrl: meta("zenith-fonts") });
  banner("info", "Loading the engine.");
  await engine.ready();
  banner("info", "Opening the sample document.");
  const sample = await loadSample(new URLSearchParams(location.search).get("doc"));
  await engine.openDocument(sample);
  return { engine, summary: await engine.state({ text: true }) };
}

/** Show one notice row in the banner area, replacing what is there. */
function banner(level, text) {
  const banners = byId("banners");
  banners.replaceChildren();
  const row = document.createElement("div");
  row.className = `notice ${level}`;
  row.setAttribute("role", level === "error" ? "alert" : "status");
  const body = document.createElement("div");
  body.className = "notice-text";
  body.textContent = text;
  row.appendChild(body);
  banners.appendChild(row);
}

function fatal(err, attempt) {
  banner(
    "error",
    err.code === "edit.unauthorized"
      ? `${err.code}: ${err.message}. Open the URL zenith edit printed in its terminal.`
      : `${err.code ?? "edit.error"}: ${err.message}. Retrying (attempt ${attempt + 1}).`,
  );
}

main().catch((err) => {
  const e = err instanceof EngineError ? err : { code: "edit.page_error", message: `the page could not start: ${err.message}` };
  banner("error", `${e.code}: ${e.message}`);
  console.error(err);
});
