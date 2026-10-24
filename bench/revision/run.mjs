// Revision benchmark runner. One run = one task x arm x model x seed.
// Usage: node bench/revision/run.mjs --task launch-card --arm zenith --model glm-5.3-flash [--skill yes] [--seed 1] [--steps 3] [--max-turns 14] [--max-calls 24]
//        node bench/revision/run.mjs --rescore <run-id>   (score an existing run again, no model calls)
// A rescore rebuilds each step's design file (see replay.mjs), renders it again and
// rewrites result.json. The first rescore keeps the old file as result.orig.json.
// Scoring rules are in steps.mjs.
// The task's `maxTurns` and `maxCalls` apply to every arm; a flag overrides both.
// Model `noop` makes no tool call: it checks that every check fails on the untouched
// start and that the frozen regions show zero drift. Model `oracle` applies the
// reference edits in tasks/<id>/oracle.mjs: it checks that every check can pass.
import { mkdirSync, readFileSync, writeFileSync, cpSync, appendFileSync, rmSync, existsSync } from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { agentStep, cost, emptyEdits } from "./agent.mjs";
import { makeArm } from "./arms.mjs";
import { renderZen, htmlRenderer } from "./render.mjs";
import { zenithAllPages, htmlAllPages } from "./pages.mjs";
import { makeScorer } from "./steps.mjs";
import { loggedCalls, rebuildStep, savedMismatch } from "./replay.mjs";
import { Browser } from "../../zenith-cli/tests/editor_e2e/cdp.js";

const here = import.meta.dirname;

function flags() {
  const out = { seed: 1 };
  const a = process.argv.slice(2);
  for (let i = 0; i < a.length; i += 2) out[a[i].replace(/^--/, "")] = a[i + 1];
  return out;
}

function findChromium() {
  if (process.env.ZENITH_E2E_CHROMIUM) return process.env.ZENITH_E2E_CHROMIUM;
  for (const c of ["chromium", "chromium-browser", "google-chrome", "google-chrome-stable"]) {
    try {
      return execFileSync("which", [c], { encoding: "utf8" }).trim();
    } catch {}
  }
  throw new Error("no Chromium on PATH; set ZENITH_E2E_CHROMIUM");
}

/** A whole-document render: { pages: [png], rects } or { error }. Single-page tasks render page 1 only. */
async function makeRenderer(kind, task, ws) {
  if (task.pages) {
    if (kind === "zenith") return { draw: () => zenithAllPages(ws) };
    const browser = await Browser.launch(findChromium());
    const r = await htmlAllPages(browser, task.canvas);
    return { draw: () => r(ws), browser };
  }
  const one = (r) => (r.png ? { pages: [r.png], rects: [[]] } : { error: r.error });
  if (kind === "zenith") return { draw: async () => one(await renderZen(ws)) };
  const browser = await Browser.launch(findChromium());
  const r = await htmlRenderer(browser, task.canvas);
  return { draw: async () => one(await r(ws)), browser };
}

/** One reference edit of the design file, with no model call. */
async function oracleStep(taskId, kind, i, ws, file) {
  const { default: edits } = await import(path.join(here, "tasks", taskId, "oracle.mjs"));
  const p = path.join(ws, file);
  writeFileSync(p, edits[kind][i](readFileSync(p, "utf8")));
  return { turns: 0, finished: true, error: null, usage: { input: 0, output: 0, cached: 0 }, calls: {}, edits: emptyEdits() };
}

const add = (a, b) => Object.fromEntries(Object.keys(a).map((k) => [k, a[k] + (b[k] ?? 0)]));

/** Copies the start file, assets and skill of a task into a fresh workspace. Returns the arm. */
function setup(task, kind, skillOn, ws) {
  mkdirSync(ws, { recursive: true });
  const skillSrc = kind === "zenith" ? path.resolve(here, "../../zenith-cli/assets/skill") : path.join(here, "skills/html");
  let skillBody = "";
  if (skillOn) {
    cpSync(skillSrc, path.join(ws, "skill"), { recursive: true });
    skillBody = readFileSync(path.join(skillSrc, "SKILL.md"), "utf8").replace(/^---[\s\S]*?---\n/, "");
  }
  const arm = makeArm(kind, ws, task.canvas, skillBody, undefined, Boolean(task.pages));
  cpSync(path.join(here, "tasks", task.id, `start.${kind === "zenith" ? "zen" : "html"}`), path.join(ws, arm.file));
  const assets = path.join(here, "tasks", task.id, "assets");
  if (existsSync(assets)) cpSync(assets, path.join(ws, "assets"), { recursive: true });
  return arm;
}

/** Writes the renders and the design file of step `i` next to the run. */
function saveStep(runDir, task, arm, i, doc, source) {
  writeFileSync(path.join(runDir, `step${i + 1}.src${path.extname(arm.file)}`), source);
  if (doc.error) return;
  doc.pages.forEach((png, n) => writeFileSync(path.join(runDir, `step${i + 1}${task.pages ? `.p${n + 1}` : ""}.png`), png));
}

const stepLine = (runId, r) =>
  `${runId} step ${r.step}: landed=${r.landed}${r.landed_late.length ? ` late=${r.landed_late.join(",")}` : ""} drifted=${r.drifted} collateral=${r.collateral} turns=${r.turns} tokens=${r.usage.input}+${r.usage.output}${r.error ? ` error=${r.error}` : ""}`;

/** The run summary written to result.json. */
function summarize(head, steps) {
  const tokens = steps.reduce((a, s) => ({ input: a.input + s.usage.input, output: a.output + s.usage.output, cached: a.cached + s.usage.cached }), { input: 0, output: 0, cached: 0 });
  const summary = {
    ...head,
    steps,
    landed: steps.filter((s) => s.landed).length,
    landed_late: steps.reduce((a, s) => a + (s.landed_late?.length ?? 0), 0),
    drifted: steps.filter((s) => s.drifted).length,
    collateral: steps.reduce((a, s) => a + s.collateral, 0),
    turns: steps.reduce((a, s) => a + s.turns, 0),
    tokens,
    edits: steps.reduce((a, s) => add(a, s.edits), emptyEdits()),
    cost_usd: steps.reduce((a, s) => a + (s.cost_usd ?? 0), 0),
  };
  console.log(
    `${summary.run}: landed ${summary.landed}/${steps.length} (+${summary.landed_late} late), drifted ${summary.drifted}, collateral ${summary.collateral}, turns ${summary.turns}, tokens ${tokens.input}+${tokens.output}, $${summary.cost_usd.toFixed(4)}, edits ${JSON.stringify(summary.edits)}`,
  );
  return summary;
}

/** Scores an existing run again from its log or saved step files. Makes no model call. */
async function rescore(runId) {
  const runDir = path.join(here, "runs", runId);
  const old = JSON.parse(readFileSync(path.join(runDir, "result.json"), "utf8"));
  if (!Array.isArray(old.steps)) throw new Error(`${runId} is not a revision run`);
  const task = JSON.parse(readFileSync(path.join(here, "tasks", old.task, "task.json"), "utf8"));
  const ws = path.join(runDir, "replay");
  rmSync(ws, { recursive: true, force: true });
  const arm = setup(task, old.arm, old.skill, ws);
  const calls = loggedCalls(path.join(runDir, "log.jsonl"));
  const oracle = old.model === "oracle" ? (i) => oracleStep(task.id, old.arm, i, ws, arm.file) : null;
  const renderer = await makeRenderer(old.arm, task, ws);
  try {
    const startDoc = await renderer.draw();
    if (startDoc.error) throw new Error(`start does not render: ${startDoc.error}`);
    const scorer = makeScorer(task, startDoc);
    const steps = [];
    for (const [i, prior] of old.steps.entries()) {
      const how = await rebuildStep({ runDir, ws, arm, calls, i, oracle });
      const afterDoc = await renderer.draw();
      const source = readFileSync(path.join(ws, arm.file), "utf8");
      const mismatch = savedMismatch(runDir, afterDoc, i, Boolean(task.pages));
      if (mismatch.length) console.log(`${runId} step ${i + 1}: render differs from the saved PNGs (${how}): ${mismatch.join(", ")}`);
      const r = { ...prior, ...scorer.score(task.steps[i], i, afterDoc, source), rebuilt: how, render_mismatch: mismatch };
      steps.push(r);
      writeFileSync(path.join(runDir, `step${i + 1}.src${path.extname(arm.file)}`), source);
      console.log(stepLine(runId, r));
    }
    // `zenith tx` stamps a fresh ULID `doc-id` on first apply, so the comparison skips it.
    const bare = (file) => readFileSync(file, "utf8").replace(/ doc-id="[0-9A-Z]{26}"/, "");
    const final = path.join(runDir, "work", arm.file);
    if (existsSync(final) && bare(final) !== bare(path.join(ws, arm.file))) console.log(`${runId}: the rebuilt design file differs from work/${arm.file}`);
    const orig = path.join(runDir, "result.orig.json");
    if (!existsSync(orig)) cpSync(path.join(runDir, "result.json"), orig);
    const { steps: _steps, landed, landed_late, drifted, collateral, turns, tokens, edits, cost_usd, ...head } = old;
    writeFileSync(path.join(runDir, "result.json"), JSON.stringify(summarize(head, steps), null, 2));
  } finally {
    await renderer.browser?.close();
    rmSync(ws, { recursive: true, force: true });
  }
}

async function main() {
  const f = flags();
  if (f.rescore) return rescore(f.rescore);
  const task = JSON.parse(readFileSync(path.join(here, "tasks", f.task, "task.json"), "utf8"));
  const kind = f.arm;
  const model = f.model;
  const skillOn = f.skill === "yes";
  const maxTurns = Number(f["max-turns"] ?? task.maxTurns ?? 14);
  const maxCalls = Number(f["max-calls"] ?? task.maxCalls ?? Infinity);
  const runId = `${task.id}.${kind}${skillOn ? "+skill" : ""}.${model}.s${f.seed}`;
  const runDir = path.join(here, "runs", runId);
  rmSync(runDir, { recursive: true, force: true });
  const ws = path.join(runDir, "work");
  const arm = setup(task, kind, skillOn, ws);
  const logFile = path.join(runDir, "log.jsonl");
  const log = (o) => appendFileSync(logFile, `${JSON.stringify(o)}\n`);

  const renderer = await makeRenderer(kind, task, ws);
  // The agent's `view`: one page as PNG, plus every page for `view all`.
  const render = async (page = 1) => {
    const d = await renderer.draw();
    if (d.error) return { error: d.error };
    if (page < 1 || page > d.pages.length) return { error: `no page ${page}; the design has ${d.pages.length} page(s)` };
    return { png: d.pages[page - 1], all: d.pages };
  };

  try {
    const startDoc = await renderer.draw();
    if (startDoc.error) throw new Error(`start does not render: ${startDoc.error}`);
    const scorer = makeScorer(task, startDoc);
    const steps = [];
    const messages = [{ role: "system", content: arm.system }];
    const limit = f.steps ? Number(f.steps) : task.steps.length;
    for (const [i, step] of task.steps.slice(0, limit).entries()) {
      log({ step: i + 1, instruction: step.instruction });
      const t0 = Date.now();
      const agent = model === "oracle" ? await oracleStep(task.id, kind, i, ws, arm.file) : await agentStep({ model, arm, messages, instruction: step.instruction, render, seed: Number(f.seed), maxTurns, maxCalls, log });
      const afterDoc = await renderer.draw();
      const source = readFileSync(path.join(ws, arm.file), "utf8");
      const result = { step: i + 1, ...scorer.score(step, i, afterDoc, source), ...agent, cost_usd: cost(model, agent.usage), seconds: (Date.now() - t0) / 1000 };
      steps.push(result);
      saveStep(runDir, task, arm, i, afterDoc, source);
      console.log(stepLine(runId, result));
    }
    const head = { run: runId, task: task.id, arm: kind, model, skill: skillOn, seed: Number(f.seed), limits: { maxTurns, maxCalls: Number.isFinite(maxCalls) ? maxCalls : null } };
    writeFileSync(path.join(runDir, "result.json"), JSON.stringify(summarize(head, steps), null, 2));
  } finally {
    await renderer.browser?.close();
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
