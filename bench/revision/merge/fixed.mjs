// Fixed-template merge comparison. No model authors anything: the engineer-made
// templates in fixed/ run over all CSV rows, and the shared score.mjs checks
// each row. Arms: zenith-after and zenith-before (`zenith merge` with each
// binary over fixed/badge.zen) and html (fixed/render.mjs over fixed/badge.html).
// Usage: node bench/revision/merge/fixed.mjs [arm ...]
// The binaries are read from ~/.cache/zenith-bench/zenith-<after|before>.
// Each withheld row is also rendered with its check off and scored, so a
// withheld row with no defect counts as a false withhold.
import { execFile, execFileSync } from "node:child_process";
import { promisify } from "node:util";
import { appendFileSync, cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import path from "node:path";
import { repo } from "../render.mjs";
import { htmlPages } from "../build/render.mjs";
import { decode } from "../png.mjs";
import { Browser } from "../../../zenith-cli/tests/editor_e2e/cdp.js";
import { dataDir, here, loadCorpus } from "./corpus.mjs";
import { htmlProber, zenithProber } from "./probe.mjs";
import { rowDefects, scoreRows, totals } from "./score.mjs";

const run = promisify(execFile);
const runs = path.resolve(here, "..", "runs", "fixed");
const fixed = path.join(here, "fixed");
const bins = { "zenith-after": "zenith-after", "zenith-before": "zenith-before" };

function findChromium() {
  if (process.env.ZENITH_E2E_CHROMIUM) return process.env.ZENITH_E2E_CHROMIUM;
  for (const c of ["chromium", "chromium-browser", "google-chrome", "google-chrome-stable"]) {
    try {
      return execFileSync("which", [c], { encoding: "utf8" }).trim();
    } catch {}
  }
  throw new Error("no Chromium on PATH; set ZENITH_E2E_CHROMIUM");
}

/** A fresh workspace with the CSV and photos. */
function workspace(arm, task) {
  const ws = path.join(runs, arm, "work");
  rmSync(path.dirname(ws), { recursive: true, force: true });
  mkdirSync(path.join(ws, "out"), { recursive: true });
  cpSync(path.join(dataDir, task.csv), path.join(ws, task.csv));
  cpSync(path.join(dataDir, "photos"), path.join(ws, "photos"), { recursive: true });
  return ws;
}

/**
 * Runs `zenith merge` and writes out/withheld.txt from its JSON report.
 * Merge writes no withheld list itself: a row with status "failed" is withheld.
 */
async function runZenith(bin, ws, task) {
  cpSync(path.join(fixed, "badge.zen"), path.join(ws, "badge.zen"));
  let stdout;
  try {
    ({ stdout } = await run(bin, ["merge", "badge.zen", task.csv, "--out-dir", "out", "--name-by", "id", "--json"], { cwd: ws, maxBuffer: 64 << 20 }));
  } catch (err) {
    // Merge exits 1 when any row fails. The report on stdout is still whole.
    if (!err.stdout) throw err;
    stdout = err.stdout;
  }
  writeFileSync(path.join(ws, "merge.json"), stdout);
  const report = JSON.parse(stdout);
  const held = report.rows.filter((r) => r.status !== "ok");
  const lines = held.map((r) => `${r.key} ${[...new Set(r.diagnostics.filter((d) => d.severity === "error").map((d) => d.code))].join(",") || r.status}`);
  writeFileSync(path.join(ws, "out", "withheld.txt"), lines.length ? `${lines.join("\n")}\n` : "");
}

/** Runs fixed/render.mjs in the workspace with the build bench's CDP helper. */
async function runHtml(ws, chromium) {
  for (const f of ["badge.html", "render.mjs"]) cpSync(path.join(fixed, f), path.join(ws, f));
  cpSync(path.join(repo, "zenith-cli/tests/editor_e2e/cdp.js"), path.join(ws, "cdp.js"));
  const env = { ...process.env, CHROMIUM: chromium, ZENITH_E2E_NO_SANDBOX: "1" };
  const { stdout } = await run(process.execPath, ["render.mjs"], { cwd: ws, env, maxBuffer: 16 << 20, timeout: 1800000 });
  writeFileSync(path.join(ws, "render.log"), stdout);
}

/** Groups defect strings into the report's kinds. */
function kindOf(d) {
  if (d.startsWith("missing_glyph")) return "missing_glyph";
  if (d.startsWith("clipped") || d.startsWith("outOfCanvas")) return "clipped";
  if (d.startsWith("lowContrast") || d.startsWith("invisible")) return "contrast";
  if (d.startsWith("placeholder")) return "placeholder";
  if (d.startsWith("benign") || d.startsWith("dropped")) return "withhold";
  return d.split(/[\s:]/)[0];
}

/** Rows per defect kind over the shipped rows. */
function kindRows(scored) {
  const out = {};
  for (const r of scored.filter((x) => x.shipped && x.defective)) {
    for (const k of new Set(r.defects.map(kindOf))) out[k] = (out[k] ?? 0) + 1;
  }
  return out;
}

/** Scores each withheld row as if it had shipped. Returns { id: defects[] }. */
async function heldDefects(arm, ws, scored, corpus, zProbe, browser) {
  const { rows, task } = corpus;
  const byId = new Map(rows.map((r) => [r.id, r]));
  const build = arm === "html" ? await htmlPages(browser, [task.canvas]) : null;
  const out = {};
  for (const s of scored.filter((x) => !x.shipped)) {
    const row = byId.get(s.id);
    let probe;
    if (build) {
      const r = await build(ws, `held/${s.id}.html`);
      probe = r.error ? { error: r.error } : { png: r.pages[0].png, rects: r.pages[0].rects, text: r.pages[0].text };
    } else probe = await zProbe(row);
    out[s.id] = probe.error ? [`unverifiable: ${probe.error.slice(0, 160)}`] : rowDefects(row, decode(probe.png), probe, task);
  }
  return out;
}

async function runArm(arm, corpus, chromium) {
  const { task, rows, truth } = corpus;
  const ws = workspace(arm, task);
  const runDir = path.dirname(ws);
  const probeDir = path.join(runDir, "probe");
  mkdirSync(probeDir, { recursive: true });
  let browser = null;
  try {
    let probe;
    let zProbe = null;
    if (arm === "html") {
      await runHtml(ws, chromium);
      browser = await Browser.launch(chromium);
      probe = await htmlProber(ws, browser, task.canvas);
    } else {
      const bin = path.join(homedir(), ".cache", "zenith-bench", bins[arm]);
      if (!existsSync(bin)) throw new Error(`no binary at ${bin}`);
      // The probe rebuild uses the same binary as the merge.
      process.env.ZENITH_BENCH_ZENITH = bin;
      await runZenith(bin, ws, task);
      zProbe = probe = await zenithProber(ws, probeDir);
    }
    const rowsFile = path.join(runDir, "rows.jsonl");
    const scored = await scoreRows({ ws, rows, truth, task, probe, onRow: (r) => appendFileSync(rowsFile, `${JSON.stringify(r)}\n`) });
    const held = await heldDefects(arm, ws, scored, corpus, zProbe, browser);
    const t = totals(scored);
    const heldIds = Object.keys(held);
    const result = {
      arm,
      rows: t.rows,
      shipped: t.shipped,
      shippedDefective: t.shippedDefective,
      withheld: heldIds.length,
      falseWithholds: heldIds.filter((id) => held[id].length === 0).length,
      unverifiableWithholds: heldIds.filter((id) => held[id].some((d) => d.startsWith("unverifiable"))).length,
      dropped: t.dropped,
      defectRows: kindRows(scored),
      held,
    };
    writeFileSync(path.join(runDir, "result.json"), JSON.stringify(result, null, 2));
    return result;
  } finally {
    await browser?.close();
  }
}

async function main() {
  const corpus = loadCorpus();
  const chromium = findChromium();
  process.env.ZENITH_E2E_NO_SANDBOX ??= "1";
  const arms = process.argv.slice(2).length ? process.argv.slice(2) : ["zenith-after", "zenith-before", "html"];
  console.log("arm            shipped  defective_shipped  withheld  false_withholds  unverifiable_withholds  dropped  defect_rows_by_kind");
  for (const arm of arms) {
    if (arm !== "html" && !bins[arm]) throw new Error(`unknown arm ${arm}; use zenith-after, zenith-before or html`);
    const r = await runArm(arm, corpus, chromium);
    console.log(
      `${arm.padEnd(14)} ${String(r.shipped).padStart(7)}  ${String(r.shippedDefective).padStart(17)}  ${String(r.withheld).padStart(8)}  ${String(r.falseWithholds).padStart(15)}  ${String(r.unverifiableWithholds).padStart(22)}  ${String(r.dropped).padStart(7)}  ${JSON.stringify(r.defectRows)}`,
    );
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
