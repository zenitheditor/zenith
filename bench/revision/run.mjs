// Revision benchmark runner. One run = one task x arm x model x seed.
// Usage: node bench/revision/run.mjs --task launch-card --arm zenith --model glm-5.3-flash [--skill yes] [--seed 1] [--steps 3] [--max-turns 14]
// Model `noop` makes no tool call: it checks that every check fails on the untouched
// start and that the frozen regions show zero drift.
import { mkdirSync, readFileSync, writeFileSync, cpSync, appendFileSync, rmSync, existsSync } from "node:fs";
import path from "node:path";
import { agentStep, cost } from "./agent.mjs";
import { makeArm } from "./arms.mjs";
import { renderZen, htmlRenderer } from "./render.mjs";
import { regionDrift, runCheck } from "./score.mjs";
import { Browser } from "../../zenith-cli/tests/editor_e2e/cdp.js";
import { execFileSync } from "node:child_process";

const here = import.meta.dirname;

function flags() {
  const out = { seed: 1, "max-turns": 14 };
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

async function main() {
  const f = flags();
  const task = JSON.parse(readFileSync(path.join(here, "tasks", f.task, "task.json"), "utf8"));
  const kind = f.arm;
  const model = f.model;
  const skillOn = f.skill === "yes";
  const runId = `${task.id}.${kind}${skillOn ? "+skill" : ""}.${model}.s${f.seed}`;
  const runDir = path.join(here, "runs", runId);
  rmSync(runDir, { recursive: true, force: true });
  const ws = path.join(runDir, "work");
  mkdirSync(ws, { recursive: true });
  const skillSrc = path.join(kind === "zenith" ? path.resolve(here, "../../zenith-cli/assets/skill") : path.join(here, "skills/html"));
  let skillBody = "";
  if (skillOn) {
    cpSync(skillSrc, path.join(ws, "skill"), { recursive: true });
    skillBody = readFileSync(path.join(skillSrc, "SKILL.md"), "utf8").replace(/^---[\s\S]*?---\n/, "");
  }
  const arm = makeArm(kind, ws, task.canvas, skillBody);
  cpSync(path.join(here, "tasks", task.id, `start.${kind === "zenith" ? "zen" : "html"}`), path.join(ws, arm.file));
  for (const extra of ["assets"]) {
    const d = path.join(here, "tasks", task.id, extra);
    if (existsSync(d)) cpSync(d, path.join(ws, extra), { recursive: true });
  }
  const logFile = path.join(runDir, "log.jsonl");
  const log = (o) => appendFileSync(logFile, `${JSON.stringify(o)}\n`);

  let browser = null;
  let render;
  if (kind === "html") {
    browser = await Browser.launch(findChromium());
    const r = await htmlRenderer(browser, task.canvas);
    render = () => r(ws);
  } else {
    render = () => renderZen(ws);
  }

  try {
    let prev = await render();
    if (!prev.png) throw new Error(`start does not render: ${prev.error}`);
    const steps = [];
    const messages = [{ role: "system", content: arm.system }];
    const limit = f.steps ? Number(f.steps) : task.steps.length;
    for (const [i, step] of task.steps.slice(0, limit).entries()) {
      log({ step: i + 1, instruction: step.instruction });
      const t0 = Date.now();
      const agent = await agentStep({ model, arm, messages, instruction: step.instruction, render, seed: Number(f.seed), maxTurns: Number(f["max-turns"]), log });
      const after = await render();
      const source = readFileSync(path.join(ws, arm.file), "utf8");
      const checks = step.checks.map((c) => ({ ...runCheck(c, { png: after.png, source, background: task.background }), kind: c.kind }));
      const drift = after.png
        ? step.frozen.map((r) => ({ name: r.name, drift: regionDrift(prev.png, after.png, r) }))
        : step.frozen.map((r) => ({ name: r.name, drift: 1 }));
      const result = {
        step: i + 1,
        landed: checks.every((c) => c.ok),
        rendered: Boolean(after.png),
        drifted: drift.some((d) => d.drift > 0),
        checks,
        drift,
        ...agent,
        cost_usd: cost(model, agent.usage),
        seconds: (Date.now() - t0) / 1000,
      };
      steps.push(result);
      if (after.png) {
        writeFileSync(path.join(runDir, `step${i + 1}.png`), after.png);
        prev = after;
      }
      console.log(`${runId} step ${i + 1}: landed=${result.landed} drifted=${result.drifted} turns=${agent.turns} tokens=${agent.usage.input}+${agent.usage.output}${agent.error ? ` error=${agent.error}` : ""}`);
    }
    const total = steps.reduce((a, s) => ({ input: a.input + s.usage.input, output: a.output + s.usage.output }), { input: 0, output: 0 });
    const summary = {
      run: runId,
      task: task.id,
      arm: kind,
      model,
      skill: skillOn,
      seed: Number(f.seed),
      steps,
      landed: steps.filter((s) => s.landed).length,
      drifted: steps.filter((s) => s.drifted).length,
      tokens: total,
      cost_usd: steps.reduce((a, s) => a + (s.cost_usd ?? 0), 0),
    };
    writeFileSync(path.join(runDir, "result.json"), JSON.stringify(summary, null, 2));
    console.log(`${runId}: landed ${summary.landed}/${steps.length}, drifted ${summary.drifted}, tokens ${total.input}+${total.output}, $${summary.cost_usd.toFixed(4)}`);
  } finally {
    await browser?.close();
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
