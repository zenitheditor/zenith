// Replay of a finished revision run, for `run.mjs --rescore`. It rebuilds the
// design file after each step with no model call: from the saved `step<N>.src.*`
// copy when the run has one, else by running the logged file-changing tool calls
// again in a fresh workspace. Read-only tools are skipped.
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { changedPixels } from "./score.mjs";
import { decode } from "./png.mjs";

// Zenith commands that never write the design file.
const READ_ONLY = new Set(["render", "inspect", "validate", "schema", "help", "--help", "-h", "--version"]);

/** Logged tool calls per step, from `log.jsonl`: [[{ tool, args }]]. */
export function loggedCalls(logFile) {
  const steps = [];
  for (const line of readFileSync(logFile, "utf8").split("\n")) {
    if (!line.trim()) continue;
    const e = JSON.parse(line);
    if (e.step !== undefined && e.instruction !== undefined) steps.push([]);
    else if (e.tool && steps.length) steps.at(-1).push({ tool: e.tool, args: e.args ?? {} });
  }
  return steps;
}

/** Brings the design file in `ws` to its state after step `i`. Returns how: "saved", "replay" or "oracle". */
export async function rebuildStep({ runDir, ws, arm, calls, i, oracle }) {
  const saved = path.join(runDir, `step${i + 1}.src${path.extname(arm.file)}`);
  if (existsSync(saved)) {
    writeFileSync(path.join(ws, arm.file), readFileSync(saved));
    return "saved";
  }
  if (oracle) {
    await oracle(i);
    return "oracle";
  }
  for (const c of calls[i] ?? []) {
    if (c.tool === "view" || c.tool === "read_file") continue;
    if (c.tool === "zenith" && READ_ONLY.has(String(c.args.args?.[0] ?? ""))) continue;
    await arm.exec(c.tool, c.args, null);
  }
  return "replay";
}

/** 1-based positions of pages in `doc` that differ from the PNGs the run saved for step `i`. */
export function savedMismatch(runDir, doc, i, multi) {
  if (doc.error) return ["render error"];
  const out = [];
  doc.pages.forEach((png, n) => {
    const file = path.join(runDir, `step${i + 1}${multi ? `.p${n + 1}` : ""}.png`);
    if (!existsSync(file)) return out.push(n + 1);
    const old = readFileSync(file);
    if (old.equals(png)) return;
    const a = decode(old);
    const b = decode(png);
    if (changedPixels(a, b, { x: 0, y: 0, w: a.width, h: a.height }) > 0) out.push(n + 1);
  });
  const extra = path.join(runDir, `step${i + 1}.p${doc.pages.length + 1}.png`);
  if (multi && existsSync(extra)) out.push(`count ${doc.pages.length}`);
  return out;
}
