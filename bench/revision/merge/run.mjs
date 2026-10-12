// Merge kill experiment runner. One run = one arm x model x seed over the badge task.
// Usage: node bench/revision/merge/run.mjs --arm zenith|html --model glm-5.3-flash [--seed 1] [--skill yes] [--max-turns 40]
//        node bench/revision/merge/run.mjs --rescore <run-id>   (score an existing run again, no model calls)
// Model `noop` writes nothing; every row then scores as a silent drop.
import { appendFileSync, cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { agentStep, cost } from "../agent.mjs";
import { repo } from "../render.mjs";
import { Browser } from "../../../zenith-cli/tests/editor_e2e/cdp.js";
import { makeMergeArm } from "./arms.mjs";
import { dataDir, here, loadCorpus } from "./corpus.mjs";
import { htmlProber, zenithProber } from "./probe.mjs";
import { scoreRows, totals } from "./score.mjs";

const runs = path.resolve(here, "..", "runs");

function flags() {
  const out = { seed: "1", "max-turns": "40" };
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

function instruction(task, csvText, rows) {
  const lines = csvText.trimEnd().split("\n");
  const sample = lines.slice(0, task.sampleRows + 1).join("\n");
  return `${task.brief}\n\nThe data is ${task.csv} in the working directory: ${rows.length} rows with the header \`${lines[0]}\`. Photos are in photos/. The first ${task.sampleRows} rows:\n\`\`\`csv\n${sample}\n\`\`\`\nThe other rows are in the file. Produce a badge for every row, or withhold it as the rules say.`;
}

/** Score the workspace of a finished run. Never calls a model. */
async function scoreWorkspace(kind, ws, runDir, corpus, chromium) {
  const { task, rows, truth } = corpus;
  const probeDir = path.join(runDir, "probe");
  rmSync(probeDir, { recursive: true, force: true });
  mkdirSync(probeDir, { recursive: true });
  let browser = null;
  let probe;
  try {
    if (kind === "html") {
      browser = await Browser.launch(chromium);
      probe = await htmlProber(ws, browser, task.canvas);
    } else if (existsSync(path.join(ws, "badge.zen"))) {
      try {
        probe = await zenithProber(ws, probeDir);
      } catch (err) {
        const msg = `template does not inspect: ${String(err.stderr ?? err.message).slice(0, 300)}`;
        probe = async () => ({ error: msg });
      }
    } else probe = async () => ({ error: "no badge.zen" });
    const rowsFile = path.join(runDir, "rows.jsonl");
    rmSync(rowsFile, { force: true });
    const scored = await scoreRows({ ws, rows, truth, task, probe, onRow: (r) => appendFileSync(rowsFile, `${JSON.stringify(r)}\n`) });
    return totals(scored);
  } finally {
    await browser?.close();
  }
}

const line = (id, t, extra = "") =>
  `${id}: defective=${t.defective}/${t.rows} shipped=${t.shipped} shipped_defective=${t.shippedDefective} withheld=${t.withheld} withheld_benign=${t.withheldBenign} dropped=${t.dropped} kinds=${JSON.stringify(t.kinds)}${extra}`;

async function main() {
  const f = flags();
  const corpus = loadCorpus();
  const { task, csvText, rows } = corpus;
  const chromium = findChromium();
  process.env.ZENITH_E2E_NO_SANDBOX ??= "1";

  if (f.rescore) {
    const runDir = path.join(runs, f.rescore);
    const result = JSON.parse(readFileSync(path.join(runDir, "result.json"), "utf8"));
    result.score = await scoreWorkspace(result.arm.split("+")[0], path.join(runDir, "work"), runDir, corpus, chromium);
    writeFileSync(path.join(runDir, "result.json"), JSON.stringify(result, null, 2));
    console.log(line(result.run, result.score));
    return;
  }

  const kind = f.arm;
  if (kind !== "zenith" && kind !== "html") throw new Error(`--arm must be zenith or html, got ${kind}`);
  const model = f.model;
  if (!model) throw new Error("--model is required");
  const skillOn = kind === "zenith" && f.skill === "yes";
  const armLabel = skillOn ? "zenith+skill" : kind;
  const runId = `merge.${task.id}.${armLabel}.${model}.s${f.seed}`;
  const runDir = path.join(runs, runId);
  rmSync(runDir, { recursive: true, force: true });
  const ws = path.join(runDir, "work");
  mkdirSync(path.join(ws, "out"), { recursive: true });
  cpSync(path.join(dataDir, task.csv), path.join(ws, task.csv));
  cpSync(path.join(dataDir, "photos"), path.join(ws, "photos"), { recursive: true });
  if (kind === "html") cpSync(path.join(repo, "zenith-cli/tests/editor_e2e/cdp.js"), path.join(ws, "cdp.js"));

  let skillBody = "";
  if (skillOn) {
    const skillSrc = path.join(repo, "zenith-cli/assets/skill");
    cpSync(skillSrc, path.join(ws, "skill"), { recursive: true });
    skillBody = readFileSync(path.join(skillSrc, "SKILL.md"), "utf8").replace(/^---[\s\S]*?---\n/, "");
  }
  const arm = makeMergeArm(kind, ws, task, chromium, skillBody);
  const logFile = path.join(runDir, "log.jsonl");
  const log = (o) => appendFileSync(logFile, `${JSON.stringify(o)}\n`);
  const messages = [{ role: "system", content: arm.system }];
  const task0 = instruction(task, csvText, rows);
  log({ system: arm.system, task: task0 });
  const t0 = Date.now();
  const agent = await agentStep({ model, arm, messages, instruction: task0, render: null, seed: Number(f.seed), maxTurns: Number(f["max-turns"]), log, budgetNotice: true });
  const seconds = (Date.now() - t0) / 1000;

  const score = await scoreWorkspace(kind, ws, runDir, corpus, chromium);
  const result = { run: runId, task: task.id, arm: armLabel, model, seed: Number(f.seed), score, ...agent, cost_usd: cost(model, agent.usage), seconds };
  writeFileSync(path.join(runDir, "result.json"), JSON.stringify(result, null, 2));
  console.log(line(runId, score, ` turns=${agent.turns} tokens=${agent.usage.input}+${agent.usage.output} $${(result.cost_usd ?? 0).toFixed(4)}${agent.error ? ` error=${agent.error}` : ""}`));
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
