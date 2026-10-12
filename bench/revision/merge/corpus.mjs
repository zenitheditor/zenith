// Loads the merge task, the CSV rows and the hidden ground truth.
import { readFileSync } from "node:fs";
import path from "node:path";

export const here = import.meta.dirname;
export const dataDir = path.join(here, "data");

/** RFC 4180 CSV: quoted fields, doubled quotes, commas and newlines inside quotes. */
export function parseCsv(text) {
  const rows = [];
  let row = [];
  let cell = "";
  let quoted = false;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (quoted) {
      if (c === '"' && text[i + 1] === '"') {
        cell += '"';
        i++;
      } else if (c === '"') quoted = false;
      else cell += c;
    } else if (c === '"') quoted = true;
    else if (c === ",") {
      row.push(cell);
      cell = "";
    } else if (c === "\n" || c === "\r") {
      if (c === "\r" && text[i + 1] === "\n") i++;
      row.push(cell);
      rows.push(row);
      row = [];
      cell = "";
    } else cell += c;
  }
  if (cell || row.length) {
    row.push(cell);
    rows.push(row);
  }
  const [head, ...body] = rows;
  return body.map((r) => Object.fromEntries(head.map((h, i) => [h, r[i] ?? ""])));
}

/** { task, csvText, rows, truth: Map<id, { tags, hostile }> } */
export function loadCorpus() {
  const task = JSON.parse(readFileSync(path.join(here, "task.json"), "utf8"));
  const csvText = readFileSync(path.join(dataDir, task.csv), "utf8");
  const rows = parseCsv(csvText);
  const truth = new Map(JSON.parse(readFileSync(path.join(dataDir, "truth.json"), "utf8")).rows.map((t) => [t.id, t]));
  return { task, csvText, rows, truth };
}
