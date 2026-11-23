// Check a prebuilt Pages candidate or a live HTTPS editor in Chromium.
// node pages_smoke.mjs (--site <dir> | --url <url> [--sha <commit>]) --chromium <bin>

import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { parseArgs } from "node:util";
import { gzipSync } from "node:zlib";
import { Browser } from "./cdp.js";
import { A, check, ready } from "./helpers.js";
import { serve } from "./static_site.js";

const { values: { site, url, chromium, sha } } = parseArgs({
  options: {
    site: { type: "string" }, url: { type: "string" }, chromium: { type: "string" }, sha: { type: "string" },
  },
});
check(chromium && Boolean(site) !== Boolean(url), "pass --chromium and either --site or --url");
check(!sha || (url && /^[a-f0-9]{40}$/.test(sha)), "--sha requires a live URL and a commit SHA");
const live = url ? new URL(url) : null;
if (live) {
  check(live.protocol === "https:" && !live.search && !live.hash, "pass the bare HTTPS editor URL");
  if (!live.pathname.endsWith("/")) live.pathname += "/";
}

async function download(dir, filename) {
  const file = path.join(dir, filename);
  const deadline = Date.now() + 10_000;
  while (!existsSync(file) && Date.now() < deadline) {
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  check(existsSync(file), `Chromium did not download ${filename}`);
  return readFileSync(file);
}

async function smoke(base, downloads, requestedFiles) {
  const browser = await Browser.launch(chromium);
  try {
    await browser.send("Browser.setDownloadBehavior", { behavior: "allow", downloadPath: downloads });
    const page = await browser.newPage();
    await page.viewport(1440, 900);
    const start = performance.now();
    await page.goto(base);
    await page.waitFor("!document.getElementById('welcome').hidden", "the welcome screen");
    await page.waitFor("document.querySelector('#welcome img').naturalWidth > 0", "the featured preview");
    const loadedEditor = await page.eval(`performance.getEntriesByType('resource')
      .some((entry) => entry.name.endsWith('.wasm') || entry.name.endsWith('/vendor/codemirror.js'))`);
    check(!loadedEditor, "the welcome screen loaded editor code");
    const welcomeFiles = [...requestedFiles];
    await page.clickSelector("#welcome-poster");
    await ready(page);
    const canvasMs = Math.round(performance.now() - start);
    const firstCanvasFiles = [...requestedFiles];

    const headline = "#inspector textarea[data-field=text]";
    const edited = "Zenith Pages smoke";
    check(await page.eval(`document.querySelector('${headline}').value === 'Night Garden'`), "the featured headline did not load");
    await page.clickSelector(headline);
    await page.key("a", 2, "KeyA");
    await page.type(edited);
    await page.key("Tab");
    await page.waitFor(`${A}.code.text().includes('${edited}') && ${A}.sync.synced()`, "the keyboard edit");
    await page.waitFor("document.getElementById('status-draft').textContent === 'Saved in this browser'", "the draft write");
    const source = await page.eval(`${A}.code.text()`);

    await page.clickSelector("#export-menu-toggle");
    await page.clickSelector("#export-png");
    const png = await download(downloads, "poster.png");
    check(png.length >= 24 && png.subarray(0, 8).toString("hex") === "89504e470d0a1a0a"
      && png.readUInt32BE(16) === 600 && png.readUInt32BE(20) === 720, "the PNG export is missing or has the wrong size");
    await page.clickSelector("#export-menu-toggle");
    await page.clickSelector("#download");
    check((await download(downloads, "poster.zen")).toString("utf8") === source, "the source download differs from the editor");

    const direct = new URL(base);
    direct.searchParams.set("doc", "starters/poster.zen");
    await page.goto(direct.href);
    await ready(page);
    check(await page.eval(`${A}.code.text().includes('Night Garden')`), "the starter link opened the saved draft");
    direct.searchParams.set("doc", "samples/article.zen");
    await page.goto(direct.href);
    await ready(page);
    check(await page.eval(`${A}.valid && Boolean(${A}.host.worker.files['copy/article.md'])`), "the article asset did not load");
    await page.goto(base);
    await page.waitFor("!document.getElementById('welcome-drafts').hidden", "the resume action");
    await page.clickSelector("#welcome-drafts");
    const resume = '#draft-list button[aria-label="Resume poster.zen"]';
    await page.waitFor(`Boolean(document.querySelector(${JSON.stringify(resume)}))`, "the saved poster row");
    await page.clickSelector(resume);
    await ready(page);
    check(await page.eval(`${A}.code.text()`) === source, "draft recovery differs from the editor");
    check(page.errors.length === 0, page.errors.join(" | "));
    return { welcomeFiles, firstCanvasFiles, canvasMs };
  } finally {
    await browser.close();
  }
}

function gzipBytes(dir, files) {
  return files.reduce((total, rel) => total + gzipSync(readFileSync(path.join(dir, rel))).length, 0);
}

async function liveBuild(base) {
  const deadline = Date.now() + 120_000;
  const manifest = new URL("build.json", base);
  let error;
  do {
    try {
      manifest.searchParams.set("check", String(Date.now()));
      const response = await fetch(manifest, { cache: "no-store", signal: AbortSignal.timeout(10_000) });
      check(response.ok, `build.json returned HTTP ${response.status}`);
      const build = await response.json();
      check(typeof build.tag === "string" && /^[a-f0-9]{40}$/.test(build.sha), "invalid build.json");
      check(!sha || build.sha === sha, `the live site reports ${build.sha}, expected ${sha}`);
      return build;
    } catch (caught) {
      error = caught;
    }
    if (sha) await new Promise((resolve) => setTimeout(resolve, 5_000));
  } while (sha && Date.now() < deadline);
  throw error;
}

const downloads = mkdtempSync(path.join(tmpdir(), "zenith-pages-downloads-"));
let host;
try {
  const requestedFiles = new Set();
  host = site ? await serve(site, {
    prefix: "/zenith/", wasmType: false, csp: false, onFile: (rel) => requestedFiles.add(rel),
  }) : null;
  const base = host?.url ?? live.href;
  const build = live ? await liveBuild(base) : null;
  const { welcomeFiles, firstCanvasFiles, canvasMs } = await smoke(base, downloads, requestedFiles);
  const report = { url: base, canvasMs };
  if (site) {
    report.welcomeGzipBytes = gzipBytes(site, welcomeFiles);
    report.firstCanvasGzipBytes = gzipBytes(site, firstCanvasFiles);
  } else {
    report.build = build;
    const [root, wasm] = await Promise.all([
      fetch(base, { method: "HEAD" }),
      fetch(new URL("zenith-editor-wasm.wasm", base), { method: "HEAD" }),
    ]);
    check(root.ok, `the editor URL returned HTTP ${root.status}`);
    check(wasm.ok, `Wasm returned HTTP ${wasm.status}`);
    const headers = ["content-type", "content-security-policy", "x-content-type-options", "cache-control"];
    report.headers = Object.fromEntries([["editor", root], ["wasm", wasm]].map(([name, response]) =>
      [name, Object.fromEntries(headers.map((key) => [key, response.headers.get(key)]))]));
  }
  console.log(JSON.stringify(report));
} finally {
  if (host) await host.close();
  rmSync(downloads, { recursive: true, force: true });
}
