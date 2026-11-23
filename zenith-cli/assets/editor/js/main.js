// Entry point of the editor page. One page, two hosts:
//
//   server  `zenith edit` answers `/api/state`: the engine is native, over HTTP.
//   wasm    a static site: the engine is the wasm module in a Worker.
//
// `<meta name="zenith-host" content="http|wasm|auto">` pins the host. The
// default `auto` takes `http` when the URL carries a `zenith edit` token,
// else asks `/api/state` once.
//
// The token: `zenith edit` opens `/#token=<t>`. A fragment never reaches a
// server and is never in a `Referer`. The page reads it, drops it from the
// address bar and history, and keeps it in memory. A copy in
// `sessionStorage` (this tab and this origin only) lets a reload of the tab
// work. The token dies with the server run, so a stored copy grants nothing
// once `zenith edit` stops.

import { curatedExample, loadSample } from "./app/samples.js";
import { EngineError, HttpEngine, WasmEngine } from "./engine/index.js";
import { hydrateIcons } from "./ui/icons.js";
import { byId, h } from "./util/dom.js";
import { sleep } from "./util/timing.js";

const TOKEN_KEY = "zenith-edit-token";

/** The token from the fragment (then removed from the URL), else from this tab's storage. */
function takeToken() {
  const fragment = new URLSearchParams(location.hash.slice(1));
  const fromUrl = fragment.get("token");
  if (fromUrl) {
    history.replaceState(null, "", `${location.pathname}${location.search}`);
    try {
      sessionStorage.setItem(TOKEN_KEY, fromUrl);
    } catch {
      // Storage off: the token lives in memory only, and a reload needs the URL again.
    }
    return fromUrl;
  }
  try {
    return sessionStorage.getItem(TOKEN_KEY);
  } catch {
    return null;
  }
}

async function main() {
  const token = takeToken();
  const host = await chooseHost(token);
  if (host === "http") {
    hydrateIcons(document);
    await mount(await startServer(token), host);
    return;
  }

  const { Drafts, starterSource } = await import("./app/drafts.js");
  const drafts = new Drafts(async (spec) => {
    const app = window.zenithEditor;
    if (app?.files) return app.files.load(spec);
    const choices = byId("welcome-choices");
    choices.inert = true;
    try {
      await openWelcome(spec);
    } finally {
      choices.inert = false;
    }
  });
  async function openWelcome(spec) {
    try {
      await mountWasm(spec, drafts);
    } catch (error) {
      if (!byId("welcome").hidden) throw error;
      showWasmStartError(error);
      return;
    }
    byId("viewport").focus();
  }
  const path = new URLSearchParams(location.search).get("doc");
  if (path === null) {
    const { showWelcome } = await import("./app/welcome.js");
    showWelcome(openWelcome);
    return;
  }
  byId("app").hidden = false;
  banner("info", "Opening the document.");
  try {
    const spec = await loadSample(path);
    const example = curatedExample(path);
    if (example) spec.draftSource = await starterSource(example, spec);
    await mountWasm(spec, drafts, example);
  } catch (error) {
    showWasmStartError(error);
  }
}

async function mountWasm(spec, drafts, example = null) {
  const startup = await startWasm(spec);
  try {
    await mount(startup, "wasm", spec, example, drafts);
  } catch (error) {
    startup.engine.close();
    throw error;
  }
}

function showWasmStartError(error) {
  delete window.zenithEditor;
  byId("app").hidden = true;
  byId("skip-source").hidden = true;
  byId("welcome-choices").hidden = true;
  const status = byId("welcome-status");
  status.dataset.error = "true";
  const reason = h("span", { id: "startup-error", text: `Could not open the editor. ${error.message}` });
  const back = h("a", { href: location.pathname, text: "Back to start", "aria-describedby": reason.id });
  status.replaceChildren(reason, " ", back);
  byId("welcome").hidden = false;
  back.focus();
}

async function mount({ engine, summary }, host, spec, example, drafts) {
  const { App } = await import("./app/app.js");
  if (host === "wasm") hydrateIcons(document);
  byId("welcome").hidden = true;
  byId("app").hidden = false;
  byId("banners").replaceChildren();
  const app = new App(engine, summary, { example, drafts });
  window.zenithEditor = app;
  drafts?.bind(app.name, summary.text, spec.draftSource ?? null, spec.draftRecord);
  drafts?.attach(app);
  await app.start();
  const project = spec?.project;
  if (project?.skipped?.length) {
    const { showSkippedFiles } = await import("./app/project-picker.js");
    showSkippedFiles(app.notices, project);
  }
  app.files?.showDraftWarning(spec.draftWarning);
  document.documentElement.dataset.host = host;
  document.documentElement.dataset.ready = "true";
}

/** `"http"` when `zenith edit` serves this page, else `"wasm"`. */
async function chooseHost(token) {
  const pinned = document.querySelector('meta[name="zenith-host"]')?.content;
  if (pinned === "http" || pinned === "wasm") return pinned;
  if (token) return "http";
  try {
    const res = await fetch("/api/state", { credentials: "omit" });
    const json = await res.json().catch(() => null);
    const server = json && (json.ok === true || String(json.error?.code ?? "").startsWith("edit."));
    return server ? "http" : "wasm";
  } catch {
    // No answer at all: a stopped `zenith edit`. Its retry loop handles it.
    return "http";
  }
}

async function startServer(token) {
  if (!token) {
    banner("error", "edit.unauthorized: this page has no token. Open the URL zenith edit printed in its terminal (it ends in #token=...).");
    await new Promise(() => {});
  }
  const engine = new HttpEngine(token);
  // The server answers once its first validation is done; a large
  // document takes a while.
  banner("info", "Loading the document.");
  let summary = null;
  for (let attempt = 0; summary === null; attempt++) {
    try {
      summary = await engine.state({ text: true });
    } catch (err) {
      fatal(err, attempt);
      if (err.code === "edit.unauthorized") {
        // A token from an earlier run: forget it.
        try {
          sessionStorage.removeItem(TOKEN_KEY);
        } catch {
          // Nothing stored.
        }
        await new Promise(() => {});
      }
      await sleep(Math.min(8000, 500 * 2 ** attempt));
    }
  }
  return { engine, summary };
}

async function startWasm(spec) {
  const meta = (name) => document.querySelector(`meta[name="${name}"]`)?.content || undefined;
  const engine = new WasmEngine({ wasmUrl: meta("zenith-wasm"), fontsUrl: meta("zenith-fonts") });
  try {
    await engine.ready();
    await engine.openDocument(spec);
    return { engine, summary: await engine.state({ text: true }) };
  } catch (err) {
    engine.close();
    throw err;
  }
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
