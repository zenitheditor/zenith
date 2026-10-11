// Build-from-brief benchmark runner. One run = one brief x arm x model x seed.
// Usage: node bench/revision/build/run.mjs --brief flyer --arm zenith --model glm-5.3-flash [--skill yes] [--seed 1] [--max-turns 24]
// Model `noop` writes nothing and scores the empty result.
import { mkdirSync, readFileSync, writeFileSync, cpSync, appendFileSync, rmSync } from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { agentStep, cost } from "../agent.mjs";
import { makeArm } from "../arms.mjs";
import { Browser } from "../../../zenith-cli/tests/editor_e2e/cdp.js";
import { decode } from "../png.mjs";
import { zenithPages, htmlPages } from "./render.mjs";
import { auditPages, contentScore } from "./audit.mjs";

const here = import.meta.dirname;
const root = path.resolve(here, "..");

function flags() {
  const out = { seed: 1, "max-turns": 30 };
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

/** Score a built design against its brief. */
export function score(built, brief) {
  if (built.error) return { rendered: false, error: built.error.slice(0, 400), pagesOk: false, content: { found: 0, total: brief.required.length, missing: brief.required }, audit: null };
  const sizeOk = built.pages.every((p) => Math.abs(p.w - brief.canvas.w) <= 1 && Math.abs(p.h - brief.canvas.h) <= 1);
  const pagesOk = built.pages.length === brief.pages && sizeOk;
  const haystack = built.text ?? built.source;
  const audit = auditPages(
    built.pages.map((p) => ({ img: decode(p.png), rects: p.rects })),
    { margin: brief.margin },
  );
  return { rendered: true, pagesOk, pageCount: built.pages.length, content: contentScore(haystack, brief.required), audit };
}

/** A design is clean when it renders, has the right pages and every text, and the audit finds nothing. */
export function clean(s) {
  const a = s.audit;
  return Boolean(s.rendered && s.pagesOk && s.content.found === s.content.total && a && a.outOfCanvas + a.tooClose + a.overlaps + a.lowContrast + a.clipped + a.invisible === 0);
}

async function main() {
  const f = flags();
  const brief = JSON.parse(readFileSync(path.join(here, "briefs.json"), "utf8")).find((b) => b.id === f.brief);
  if (!brief) throw new Error(`unknown brief ${f.brief}`);
  const kind = f.arm;
  const model = f.model;
  const skillOn = f.skill === "yes";
  const runId = `build.${brief.id}.${kind}${skillOn ? "+skill" : ""}.${model}.s${f.seed}`;
  const runDir = path.join(root, "runs", runId);
  rmSync(runDir, { recursive: true, force: true });
  const ws = path.join(runDir, "work");
  mkdirSync(ws, { recursive: true });

  const skillSrc = kind === "zenith" ? path.resolve(root, "../../zenith-cli/assets/skill") : path.join(root, "skills/html-author");
  let skillBody = "";
  if (skillOn) {
    cpSync(skillSrc, path.join(ws, "skill"), { recursive: true });
    skillBody = readFileSync(path.join(skillSrc, "SKILL.md"), "utf8").replace(/^---[\s\S]*?---\n/, "");
  }
  const arm = makeArm(kind, ws, brief.canvas, skillBody, { kind: brief.kind, pages: brief.pages });
  const logFile = path.join(runDir, "log.jsonl");
  const log = (o) => appendFileSync(logFile, `${JSON.stringify(o)}\n`);

  let browser = null;
  let build;
  if (kind === "html") {
    browser = await Browser.launch(findChromium());
    build = await htmlPages(browser, brief.canvas);
  } else {
    build = zenithPages;
  }
  const view = async (n) => {
    const r = await build(ws);
    if (r.error) return { error: r.error };
    const page = r.pages[Math.min(Math.max(n, 1), r.pages.length) - 1];
    return { png: page.png };
  };

  try {
    const task = `Brief: ${brief.brief}\n\nText to include, exactly as written, one item per line (decide the hierarchy and grouping yourself):\n${brief.required.map((t) => `- ${t}`).join("\n")}`;
    const messages = [{ role: "system", content: arm.system }];
    log({ brief: brief.id, task });
    const t0 = Date.now();
    const agent = await agentStep({ model, arm, messages, instruction: task, render: view, seed: Number(f.seed), maxTurns: Number(f["max-turns"]), log, budgetNotice: true });
    const built = model === "noop" ? { error: "noop wrote nothing" } : await build(ws);
    const s = score(built, brief);
    if (built.pages) built.pages.forEach((p, i) => writeFileSync(path.join(runDir, `page-${i + 1}.png`), p.png));
    const result = {
      run: runId,
      brief: brief.id,
      arm: kind,
      model,
      skill: skillOn,
      seed: Number(f.seed),
      ...s,
      rects: built.pages ? built.pages.map((p) => p.rects) : [],
      clean: clean(s),
      ...agent,
      cost_usd: cost(model, agent.usage),
      seconds: (Date.now() - t0) / 1000,
    };
    writeFileSync(path.join(runDir, "result.json"), JSON.stringify(result, null, 2));
    const a = s.audit;
    console.log(
      `${runId}: clean=${result.clean} rendered=${s.rendered} pages=${s.pagesOk} content=${s.content.found}/${s.content.total}` +
        (a ? ` issues[edge=${a.tooClose} overlap=${a.overlaps} contrast=${a.lowContrast} clipped=${a.clipped} out=${a.outOfCanvas}]` : "") +
        ` turns=${agent.turns} tokens=${agent.usage.input}+${agent.usage.output} $${(result.cost_usd ?? 0).toFixed(4)}`,
    );
  } finally {
    await browser?.close();
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
