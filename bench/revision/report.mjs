// Aggregate runs/*/result.json by arm, skill and model. Usage: node bench/revision/report.mjs [task]
import { readdirSync, readFileSync, existsSync } from "node:fs";
import path from "node:path";

const dir = path.join(import.meta.dirname, "runs");
const task = process.argv[2];
const cells = new Map();
for (const name of readdirSync(dir)) {
  const f = path.join(dir, name, "result.json");
  if (!existsSync(f) || name.includes(".noop.")) continue;
  const r = JSON.parse(readFileSync(f, "utf8"));
  if (!Array.isArray(r.steps)) continue; // build runs have no steps; build/report.mjs covers them
  if (task && r.task !== task) continue;
  const key = `${r.arm}${r.skill ? " +skill" : "      "} | ${r.model}`;
  if (!cells.has(key)) cells.set(key, []);
  cells.get(key).push(r);
}
const mean = (a) => a.reduce((x, y) => x + y, 0) / a.length;
console.log("cell                                       runs  landed  drift   turns/step  tokens in    $/run");
for (const key of [...cells.keys()].sort()) {
  const rs = cells.get(key);
  const steps = rs.flatMap((r) => r.steps);
  const landed = steps.filter((s) => s.landed).length;
  const drift = steps.filter((s) => s.drifted).length;
  console.log(
    `${key.padEnd(42)} ${String(rs.length).padStart(3)}  ${`${landed}/${steps.length}`.padStart(6)}  ${`${drift}/${steps.length}`.padStart(5)}  ${mean(steps.map((s) => s.turns)).toFixed(1).padStart(9)}  ${Math.round(mean(rs.map((r) => r.tokens.input))).toString().padStart(9)}  ${mean(rs.map((r) => r.cost_usd)).toFixed(4)}`,
  );
}
