// Aggregate build runs by arm, skill and model. Usage: node bench/revision/build/report.mjs
import { readdirSync, readFileSync, existsSync } from "node:fs";
import path from "node:path";
const dir = path.join(import.meta.dirname, "..", "runs");
const cells = new Map();
for (const name of readdirSync(dir)) {
  const f = path.join(dir, name, "result.json");
  if (!name.startsWith("build.") || name.includes(".noop.") || !existsSync(f)) continue;
  const r = JSON.parse(readFileSync(f, "utf8"));
  const key = `${r.arm}${r.skill ? "+skill" : ""} | ${r.model}`;
  (cells.get(key) ?? cells.set(key, []).get(key)).push(r);
}
const mean = (a) => a.reduce((x, y) => x + y, 0) / (a.length || 1);
console.log("cell                               runs  clean  content  pagesOk  issues/run  turns  $/run");
for (const key of [...cells.keys()].sort()) {
  const rs = cells.get(key);
  const iss = (r) => (r.audit ? r.audit.tooClose + r.audit.overlaps + r.audit.lowContrast + r.audit.clipped + r.audit.outOfCanvas + r.audit.invisible : 99);
  console.log(
    `${key.padEnd(34)} ${String(rs.length).padStart(3)}  ${String(rs.filter((r) => r.clean).length).padStart(4)}  ${(100 * mean(rs.map((r) => r.content.found / r.content.total))).toFixed(0).padStart(5)}%  ${String(rs.filter((r) => r.pagesOk).length).padStart(6)}  ${mean(rs.map(iss)).toFixed(1).padStart(9)}  ${mean(rs.map((r) => r.turns)).toFixed(1).padStart(5)}  ${mean(rs.map((r) => r.cost_usd ?? 0)).toFixed(4)}`,
  );
}
