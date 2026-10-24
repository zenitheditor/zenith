// Shared helpers for the task generators. A generator builds one design spec and
// emits `start.zen`, `start.html` and `task.json` from it, so both arms start
// from the same design. Run a generator once, before any run, to pre-register a task.
import { writeFileSync } from "node:fs";
import path from "node:path";
import { nextOrder } from "../score.mjs";

/** KDL string literal. */
export const kdl = (s) => JSON.stringify(s);

/** HTML text escape. */
export const esc = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

/** `#rrggbb` to [r, g, b]. */
export const rgb = (hex) => [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));

/** Inline CSS box for an absolutely positioned element. */
export const boxCss = (b) => `left:${b.x}px;top:${b.y}px;width:${b.w}px;height:${b.h}px`;

/** Zenith geometry attributes. */
export const boxZen = (b) => `x=(px)${b.x} y=(px)${b.y} w=(px)${b.w} h=(px)${b.h}`;

/** Offset a box. */
export const at = (b, dx = 0, dy = 0) => ({ ...b, x: b.x + dx, y: b.y + dy });

/** 1-based position of each page key after the page operations of steps 0..i-1 and step i. */
export function positions(keys, steps) {
  let order = keys;
  return steps.map((s) => {
    order = nextOrder(order, s.pages);
    return Object.fromEntries(order.map((k, i) => [k, i + 1]));
  });
}

/** Write the three files and report their line counts. Fails when a start file is over `maxLines`. */
export function writeTask(dir, { task, zen, html }, maxLines = 500) {
  for (const [name, text] of [["start.zen", zen], ["start.html", html]]) {
    const lines = text.split("\n").length;
    if (lines > maxLines) throw new Error(`${name} has ${lines} lines, over ${maxLines}`);
    writeFileSync(path.join(dir, name), text);
    console.log(`${name}: ${lines} lines`);
  }
  writeFileSync(path.join(dir, "task.json"), `${JSON.stringify(task, null, 1)}\n`);
  console.log(`task.json: ${task.steps.length} steps, ${task.elements.length} elements`);
}
