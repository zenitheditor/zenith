// Blinded vision judge for build runs. It sees the brief and the rendered pages only:
// no arm, model, source, or audit result. One call per run, temperature 0, cached in
// the run directory as judge.<model>.json.
// Usage: node bench/revision/build/judge.mjs [--judge kimi-k3] [--summary] <run-dir-substring>...
// Empty filter list judges every run that has a page image and no cached verdict.
import { readFileSync, writeFileSync, readdirSync, existsSync } from "node:fs";
import path from "node:path";
import { chat } from "../llm.mjs";

const here = import.meta.dirname;
const runs = path.resolve(here, "../runs");
const briefs = JSON.parse(readFileSync(path.join(here, "briefs.json"), "utf8"));

const AXES = ["hierarchy", "legibility", "spacing", "polish", "fit"];
const RUBRIC = `You are a strict senior graphic designer judging a finished design from its rendered pages.
Score each axis as an integer from 1 to 10. Use the whole range: 5 is an ordinary template, 8 is work you would put your name on, 10 is exceptional.
- hierarchy: the eye lands on the right thing first and the reading order is clear.
- legibility: type is large and contrasted enough to read at the intended distance, with no crowded or cut-off text.
- spacing: margins, gaps, and alignment are deliberate and consistent; nothing floats or collides.
- polish: color, type pairing, and detail look professionally designed, not default or accidental.
- fit: the layout suits the kind of document the brief asks for.
Reply with only a JSON object: {"hierarchy":n,"legibility":n,"spacing":n,"polish":n,"fit":n,"note":"one sentence on the biggest flaw"}.`;

function flags() {
  const out = { judge: "kimi-k3", filters: [], summary: false };
  const a = process.argv.slice(2);
  for (let i = 0; i < a.length; i++) {
    if (a[i] === "--judge") out.judge = a[++i];
    else if (a[i] === "--summary") out.summary = true;
    else out.filters.push(a[i]);
  }
  return out;
}

function pagesOf(dir) {
  return readdirSync(dir)
    .filter((f) => /^page-\d+\.png$/.test(f))
    .sort((a, b) => parseInt(a.slice(5)) - parseInt(b.slice(5)))
    .map((f) => path.join(dir, f));
}

export async function judgeRun(dirName, judge) {
  const dir = path.join(runs, dirName);
  const cache = path.join(dir, `judge.${judge.replace(/[^\w.-]/g, "_")}.json`);
  if (existsSync(cache)) return JSON.parse(readFileSync(cache, "utf8"));
  const result = JSON.parse(readFileSync(path.join(dir, "result.json"), "utf8"));
  const brief = briefs.find((b) => b.id === result.brief);
  const pages = pagesOf(dir);
  if (!brief || pages.length === 0) return null;
  const content = [{ type: "text", text: `${RUBRIC}\n\nBrief: ${brief.brief}\n${pages.length} page(s) follow in order.` }];
  for (const p of pages.slice(0, 6)) {
    content.push({ type: "image_url", image_url: { url: `data:image/png;base64,${readFileSync(p).toString("base64")}` } });
  }
  let verdict = null;
  for (let attempt = 0; attempt < 3 && !verdict; attempt++) {
    const r = await chat({ model: judge, messages: [{ role: "user", content }], maxTokens: 4000, temperature: 0 });
    const m = String(r.message.content ?? "").match(/\{[\s\S]*\}/);
    try {
      const j = JSON.parse(m?.[0] ?? "");
      if (AXES.every((k) => Number.isInteger(j[k]) && j[k] >= 1 && j[k] <= 10)) verdict = j;
    } catch {}
  }
  if (!verdict) return null;
  verdict.mean = AXES.reduce((s, k) => s + verdict[k], 0) / AXES.length;
  writeFileSync(cache, JSON.stringify(verdict, null, 2));
  return verdict;
}

/** Group key: arm[+skill].model, from the run directory name. */
function groupOf(name) {
  const m = name.match(/^build\.\w+\.([^.]+(?:\+skill)?)\.(.+)\.s\d+$/);
  return m ? `${m[1]} ${m[2]}` : name;
}

async function main() {
  const f = flags();
  const names = readdirSync(runs).filter(
    (n) => n.startsWith("build.") && existsSync(path.join(runs, n, "result.json")) && (f.filters.length === 0 || f.filters.some((x) => n.includes(x))),
  );
  const groups = {};
  for (const n of names) {
    const v = await judgeRun(n, f.judge);
    if (!v) continue;
    const g = (groups[groupOf(n)] ??= { n: 0, sum: 0, axes: Object.fromEntries(AXES.map((k) => [k, 0])) });
    g.n++;
    g.sum += v.mean;
    for (const k of AXES) g.axes[k] += v[k];
    if (!f.summary) console.log(`${n}: ${v.mean.toFixed(1)} ${AXES.map((k) => v[k]).join("/")} ${v.note ?? ""}`);
  }
  console.log(`\njudge ${f.judge}: mean score by group (${AXES.join("/")})`);
  for (const [g, x] of Object.entries(groups)) {
    console.log(`  ${g.padEnd(40)} n=${String(x.n).padEnd(3)} ${(x.sum / x.n).toFixed(2)}  ${AXES.map((k) => (x.axes[k] / x.n).toFixed(1)).join("/")}`);
  }
}

if (import.meta.url === `file://${process.argv[1]}`) await main();
