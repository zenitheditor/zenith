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

import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { Browser } from "./cdp.js";
import { startServer } from "./native_host.js";
import { buildSite, serve } from "./static_site.js";

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
  return { child: { kill: () => host.close() }, url: `${host.url}?doc=samples/${encodeURIComponent(name)}`, site, stderr: () => "" };
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
  const server = mode === "static" ? await startStatic(a.wasm, a.example, dir) : await startServer(a.zenith, doc, data);
  const browser = await Browser.launch(a.chromium);
  const results = [];
  let failed = 0;
  try {
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
    };
    for (const [name, fn] of steps) {
      const t0 = Date.now();
      try {
        const detail = await fn(ctx);
        if (page.errors.length) throw new Error(`page errors: ${page.errors.join(" | ")}`);
        results.push({ step: name, ok: true, ms: Date.now() - t0, detail });
      } catch (err) {
        failed++;
        await ctx.shot(`failed-${name.replace(/\W+/g, "-")}`).catch(() => {});
        results.push({ step: name, ok: false, ms: Date.now() - t0, error: err.message });
        page.errors.length = 0;
      }
      console.log(JSON.stringify(results.at(-1)));
    }
  } finally {
    await browser.close();
    server.child.kill();
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
