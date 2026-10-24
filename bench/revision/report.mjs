// Aggregate runs/*/result.json per task, arm and model.
// Usage: node bench/revision/report.mjs [task] [model]
// Columns: landed and drifted are step counts. late counts steps that landed in a
// later step (runs scored before `landed_late` existed show "-"). collateral sums the changed
// elements outside each step's intended set. Tokens, $, and edit counts are
// means per run. Edits count calls on the design file: applied `zenith tx`
// (failed in parentheses), `edit_file`, and `write_file`. noop and oracle runs are left out.
import { readdirSync, readFileSync, existsSync } from "node:fs";
import path from "node:path";

const dir = path.join(import.meta.dirname, "runs");
const [task, model] = process.argv.slice(2);
const cells = new Map();
for (const name of readdirSync(dir)) {
  const f = path.join(dir, name, "result.json");
  if (!existsSync(f) || /\.(noop|oracle)\./.test(name)) continue;
  const r = JSON.parse(readFileSync(f, "utf8"));
  if (!Array.isArray(r.steps)) continue; // build runs have no steps; build/report.mjs covers them
  if ((task && r.task !== task) || (model && r.model !== model)) continue;
  const key = [r.task, `${r.arm}${r.skill ? "+skill" : ""}`, r.model].join("\t");
  if (!cells.has(key)) cells.set(key, []);
  cells.get(key).push(r);
}

const mean = (a) => (a.length ? a.reduce((x, y) => x + y, 0) / a.length : 0);
const pad = (s, n) => String(s).padStart(n);
const cols = [
  ["task", 9, true],
  ["arm", 12, true],
  ["model", 20, true],
  ["runs", 4],
  ["landed", 7],
  ["late", 4],
  ["drift", 6],
  ["coll", 5],
  ["turns/st", 8],
  ["tok in", 9],
  ["tok out", 8],
  ["$/run", 7],
  ["tx(fail)", 9],
  ["edit_file", 9],
  ["write", 5],
];
const row = (vals) => vals.map((v, i) => (cols[i][2] ? String(v).padEnd(cols[i][1]) : pad(v, cols[i][1]))).join(" ");
console.log(row(cols.map((c) => c[0])));
let lastTask = null;
for (const key of [...cells.keys()].sort()) {
  const rs = cells.get(key);
  const [t, arm, m] = key.split("\t");
  if (lastTask && t !== lastTask) console.log("");
  lastTask = t;
  const steps = rs.flatMap((r) => r.steps);
  const has = (k) => rs.every((r) => r[k] !== undefined);
  const e = (k) => mean(rs.map((r) => r.edits?.[k] ?? 0));
  const one = (v) => (Number.isInteger(v) ? v : v.toFixed(1));
  console.log(
    row([
      t,
      arm,
      m,
      rs.length,
      `${steps.filter((s) => s.landed).length}/${steps.length}`,
      has("landed_late") ? steps.reduce((a, s) => a + (s.landed_late?.length ?? 0), 0) : "-",
      `${steps.filter((s) => s.drifted).length}`,
      has("collateral") ? steps.reduce((a, s) => a + (s.collateral ?? 0), 0) : "-",
      mean(steps.map((s) => s.turns)).toFixed(1),
      Math.round(mean(rs.map((r) => r.tokens.input))),
      Math.round(mean(rs.map((r) => r.tokens.output))),
      mean(rs.map((r) => r.cost_usd)).toFixed(4),
      has("edits") ? `${one(e("tx"))}(${one(e("tx_failed"))})` : "-",
      has("edits") ? one(e("edit_file")) : "-",
      has("edits") ? one(e("write_file")) : "-",
    ]),
  );
}
