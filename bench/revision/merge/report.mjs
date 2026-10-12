// Aggregate merge runs per arm and model, and apply the decision rule.
// Usage: node bench/revision/merge/report.mjs [--noop]   (--noop also lists noop runs)
// Decision rule, fixed before any run: drop the merge wedge when Zenith ships at
// most 30% fewer defective rows than HTML, or costs more than 2x per run.
import { existsSync, readdirSync, readFileSync } from "node:fs";
import path from "node:path";

const dir = path.join(import.meta.dirname, "..", "runs");
const withNoop = process.argv.includes("--noop");
const MIN_REDUCTION = 0.3;
const MAX_COST_RATIO = 2;
const FULL_SEEDS = 3;

const results = [];
for (const name of readdirSync(dir)) {
  const f = path.join(dir, name, "result.json");
  if (!name.startsWith("merge.") || !existsSync(f)) continue;
  const r = JSON.parse(readFileSync(f, "utf8"));
  if (!r.score || !r.model || (r.model === "noop" && !withNoop)) continue;
  results.push(r);
}

const mean = (a) => a.reduce((x, y) => x + y, 0) / (a.length || 1);
const group = (rs) => {
  const s = (k) => mean(rs.map((r) => r.score[k]));
  return {
    runs: rs.length,
    defective: s("defective"),
    shippedDefective: s("shippedDefective"),
    shipped: s("shipped"),
    withheld: s("withheld"),
    withheldBenign: s("withheldBenign"),
    dropped: s("dropped"),
    cost: mean(rs.map((r) => r.cost_usd ?? 0)),
    tokensIn: mean(rs.map((r) => r.usage?.input ?? 0)),
    tokensOut: mean(rs.map((r) => r.usage?.output ?? 0)),
    turns: mean(rs.map((r) => r.turns ?? 0)),
  };
};

const cells = new Map();
for (const r of results) {
  const key = `${r.arm}|${r.model}`;
  (cells.get(key) ?? cells.set(key, []).get(key)).push(r);
}

const n1 = (x) => x.toFixed(1).padStart(7);
console.log("Means per run. defective = shipped with a defect + dropped without a withheld entry + benign rows withheld.");
console.log("arm          model                  runs  defective  shipped_def  shipped  withheld  withheld_benign  dropped   $/run    tok_in  tok_out  turns");
for (const key of [...cells.keys()].sort()) {
  const [arm, model] = key.split("|");
  const g = group(cells.get(key));
  console.log(
    `${arm.padEnd(12)} ${model.padEnd(22)} ${String(g.runs).padStart(4)}  ${n1(g.defective)}    ${n1(g.shippedDefective)}  ${n1(g.shipped)}  ${n1(g.withheld)}  ${n1(g.withheldBenign).padStart(15)}  ${n1(g.dropped)}  ${g.cost.toFixed(4)}  ${String(Math.round(g.tokensIn)).padStart(8)} ${String(Math.round(g.tokensOut)).padStart(7)}  ${g.turns.toFixed(1).padStart(5)}`,
  );
}

function verdict(label, zen, html) {
  if (!zen.length || !html.length) return console.log(`${label}: no verdict (needs runs of both arms)`);
  const z = group(zen);
  const h = group(html);
  const reduction = h.defective > 0 ? (h.defective - z.defective) / h.defective : 0;
  const ratio = h.cost > 0 ? z.cost / h.cost : z.cost > 0 ? Infinity : 1;
  const drop = reduction <= MIN_REDUCTION || ratio > MAX_COST_RATIO;
  const seeds = Math.min(z.runs, h.runs);
  const note = seeds < FULL_SEEDS ? ` (preliminary: ${seeds} run(s) per arm, the rule needs ${FULL_SEEDS})` : "";
  console.log(
    `${label}: zenith ${z.defective.toFixed(1)} vs html ${h.defective.toFixed(1)} defective rows, reduction ${(100 * reduction).toFixed(0)}% (keep needs > ${100 * MIN_REDUCTION}%), cost ratio ${ratio.toFixed(2)}x (keep needs <= ${MAX_COST_RATIO}x) -> ${drop ? "DROP" : "KEEP"}${note}`,
  );
}

console.log("\nDecision rule:");
const models = [...new Set(results.map((r) => r.model))].filter((m) => m !== "noop").sort();
const real = results.filter((r) => r.model !== "noop");
for (const zenithArm of ["zenith", "zenith+skill"]) {
  for (const m of models) verdict(`${zenithArm} ${m}`, results.filter((r) => r.model === m && r.arm === zenithArm), results.filter((r) => r.model === m && r.arm === "html"));
  verdict(`${zenithArm} all models`, real.filter((r) => r.arm === zenithArm), real.filter((r) => r.arm === "html"));
}
