// A compact line diff for notices: the one changed block between two
// texts, with context lines. Enough to show what a disk change did.

const CONTEXT = 2;
const MAX_LINES = 200;

/**
 * The changed block from `before` to `after` as text: `@@ line N` then
 * context lines (two spaces), removed lines (`- `), added lines (`+ `).
 * Empty when the texts are equal.
 */
export function lineDiff(before, after) {
  if (before === after) return "";
  const a = before.split("\n");
  const b = after.split("\n");
  let start = 0;
  while (start < a.length && start < b.length && a[start] === b[start]) start++;
  let endA = a.length;
  let endB = b.length;
  while (endA > start && endB > start && a[endA - 1] === b[endB - 1]) {
    endA--;
    endB--;
  }
  const from = Math.max(0, start - CONTEXT);
  const out = [`@@ line ${start + 1}`];
  for (let i = from; i < start; i++) out.push(`  ${a[i]}`);
  for (let i = start; i < endA; i++) out.push(`- ${a[i]}`);
  for (let i = start; i < endB; i++) out.push(`+ ${b[i]}`);
  for (let i = endA; i < Math.min(a.length, endA + CONTEXT); i++) out.push(`  ${a[i]}`);
  if (out.length > MAX_LINES) {
    const more = out.length - MAX_LINES;
    out.length = MAX_LINES;
    out.push(`… ${more} more lines`);
  }
  return out.join("\n");
}
