// End-to-end check of the `zenith edit` page in headless Chromium over CDP.
//
// Run: node run.js --zenith <bin> --chromium <bin> --example <file.zen> --out <dir>
//                   [--steps <module>]
//      node run.js --wasm <file.wasm> --chromium <bin> --example <file.zen> --out <dir>
//                   [--steps <module>]
// The first form drives the page served by `zenith edit`. The second builds
// the static site (zenith-cli/scripts/build-static-editor.mjs), serves it
// from a Node file server under a subdirectory path, and drives the same
// page with the engine in a wasm Worker.
// `--steps` names the step file next to run.js (default `steps.js`; the
// canvas gesture suite is `gesture_steps.js`). A step can carry a third
// element `{host: "server"}` to run only against `zenith edit`. The static
// run of `steps.js` adds the steps of `static_steps.js`. Prints one JSON
// line per step and a summary. Exit code 0 when all pass. Screenshots go to
// --out.

import { chmodSync, copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { Browser } from "./cdp.js";
import { DIAGNOSIS } from "./helpers.js";
import { startServer } from "./native_host.js";
import { buildSite, serve } from "./static_site.js";

/**
 * A step waits on page state with a 60 s guard per wait (`Page.waitFor`).
 * This guard catches a step stuck elsewhere (an evaluation that never
 * resolves). A hung step ends the run: it may still drive the page.
 */
const STEP_GUARD_MS = 300000;
const DIAGNOSIS_GUARD_MS = 10000;

class Hung extends Error {}

/** `promise`, or a `Hung` error after `ms` ms. */
function within(promise, ms, what) {
  let timer;
  const guard = new Promise((_, reject) => {
    timer = setTimeout(() => reject(new Hung(`${what} did not finish in ${ms / 1000} s`)), ms);
  });
  return Promise.race([promise, guard]).finally(() => clearTimeout(timer));
}

function args() {
  const out = {};
  const argv = process.argv.slice(2);
  for (let i = 0; i < argv.length; i += 2) out[argv[i].replace(/^--/, "")] = argv[i + 1];
  for (const key of ["chromium", "example", "out"]) {
    if (!out[key]) throw new Error(`missing --${key}; see the header of run.js`);
  }
  if (!out.zenith && !out.wasm) throw new Error("missing --zenith or --wasm; see the header of run.js");
  return out;
}

/** Build and serve the static site with `example` as `samples/<name>`. */
async function startStatic(wasm, example, work) {
  const site = path.join(work, "site");
  console.log(buildSite(site, wasm));
  const name = path.basename(example);
  mkdirSync(path.join(site, "samples"), { recursive: true });
  copyFileSync(example, path.join(site, "samples", name));
  // A subdirectory path proves every URL the page builds is relative.
  const host = await serve(site, { prefix: "/sub/dir/" });
  return { child: null, url: `${host.url}?doc=samples/${encodeURIComponent(name)}`, site, stderr: () => "", stop: () => host.close() };
}

async function main() {
  const a = args();
  const here = path.dirname(fileURLToPath(import.meta.url));
  const stepFile = a.steps ?? "steps.js";
  if (!/^[\w-]+\.js$/.test(stepFile)) throw new Error(`--steps must name a file next to run.js, got ${stepFile}`);
  const mode = a.wasm ? "static" : "server";
  const module = await import(path.join(here, stepFile));
  let steps = module.steps.filter(([, , options]) => !options?.host || options.host === (mode === "static" ? "static" : "server"));
  if (mode === "static" && stepFile === "steps.js") steps = steps.concat((await import("./static_steps.js")).steps);
  mkdirSync(a.out, { recursive: true });
  const dir = mkdtempSync(path.join(tmpdir(), "zenith-e2e-doc-"));
  const data = mkdtempSync(path.join(tmpdir(), "zenith-e2e-data-"));
  const doc = path.join(dir, path.basename(a.example));
  copyFileSync(a.example, doc);
  const results = [];
  let failed = 0;
  let server = null;
  let browser = null;
  try {
    server = mode === "static" ? await startStatic(a.wasm, a.example, dir) : await startServer(a.zenith, doc, data);
    browser = await Browser.launch(a.chromium);
    const page = await browser.newPage();
    const ctx = {
      page,
      mode,
      site: server.site,
      url: server.url,
      doc,
      original: readFileSync(doc, "utf8"),
      readDoc: () => readFileSync(doc, "utf8"),
      writeDoc: (text) => writeFileSync(doc, text),
      shot: (name) => page.screenshot(path.join(a.out, `${name}.png`)),
      // Server only, and only where signals and modes exist.
      signal: mode === "server" && process.platform !== "win32" ? (sig) => server.child.kill(sig) : null,
      setReadonly: process.platform !== "win32" ? (file, on) => chmodSync(file, on ? 0o444 : 0o644) : null,
    };
    for (const [name, fn] of steps) {
      const t0 = Date.now();
      try {
        const detail = await within(fn(ctx), STEP_GUARD_MS, `step "${name}"`);
        if (page.errors.length) throw new Error(`page errors: ${page.errors.join(" | ")}`);
        results.push({ step: name, ok: true, ms: Date.now() - t0, detail });
      } catch (err) {
        failed++;
        const ms = Date.now() - t0;
        const page_ = await within(page.eval(DIAGNOSIS), DIAGNOSIS_GUARD_MS, "the diagnosis").catch((e) => `no diagnosis: ${e.message}`);
        await within(ctx.shot(`failed-${name.replace(/\W+/g, "-")}`), DIAGNOSIS_GUARD_MS, "the screenshot").catch(() => {});
        results.push({ step: name, ok: false, ms, error: err.message, page: page_, pageErrors: page.errors.slice() });
        // A check message can hold whole sources: print it unescaped too.
        if (err.message.includes("\n")) console.error(`--- step "${name}" failed ---\n${err.message}\n--- end of step "${name}" ---`);
        page.errors.length = 0;
        if (err instanceof Hung) {
          console.log(JSON.stringify(results.at(-1)));
          break;
        }
      }
      console.log(JSON.stringify(results.at(-1)));
    }
  } finally {
    // Every step result is out; a cleanup error must not leave the server.
    await browser?.close().catch((err) => console.error(`closing chromium: ${err.message}`));
    await server?.stop();
    rmSync(dir, { recursive: true, force: true });
    rmSync(data, { recursive: true, force: true });
  }
  console.log(JSON.stringify({ suite: `editor-e2e ${mode} ${stepFile}`, passed: results.length - failed, failed }));
  process.exit(failed ? 1 : 0);
}

main().catch((err) => {
  console.error(`editor e2e could not run: ${err.message}`);
  process.exit(2);
});
