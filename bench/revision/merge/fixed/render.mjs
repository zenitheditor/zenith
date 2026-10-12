// Renders one badge per CSV row from badge.html with headless Chromium.
// Usage (in the workspace): node render.mjs [attendees.csv]
// A row ships as out/<id>.png plus out/<id>.html. A row the page script cannot
// fit, or that has a missing glyph, ships no PNG: its page goes to held/<id>.html
// and its id and reason go to out/withheld.txt.
import { mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { Browser } from "./cdp.js";

/** RFC 4180 CSV: quoted fields, doubled quotes, commas and newlines inside quotes. */
function parseCsv(text) {
  const rows = [];
  let row = [], cell = "", quoted = false;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (quoted) {
      if (c === '"' && text[i + 1] === '"') (cell += '"'), i++;
      else if (c === '"') quoted = false;
      else cell += c;
    } else if (c === '"') quoted = true;
    else if (c === ",") row.push(cell), (cell = "");
    else if (c === "\n" || c === "\r") {
      if (c === "\r" && text[i + 1] === "\n") i++;
      row.push(cell), rows.push(row), (row = []), (cell = "");
    } else cell += c;
  }
  if (cell || row.length) row.push(cell), rows.push(row);
  const [head, ...body] = rows;
  return body.map((r) => Object.fromEntries(head.map((h, i) => [h, r[i] ?? ""])));
}

const csv = process.argv[2] ?? "attendees.csv";
const rows = parseCsv(readFileSync(csv, "utf8"));
const template = readFileSync("badge.html", "utf8");
const slot = '<script type="application/json" id="row">{}</script>';
if (!template.includes(slot)) throw new Error(`badge.html has no row slot: ${slot}`);

for (const d of ["out", "held"]) {
  rmSync(d, { recursive: true, force: true });
  mkdirSync(d, { recursive: true });
}

const browser = await Browser.launch(process.env.CHROMIUM);
const withheld = [];
try {
  const page = await browser.newPage();
  await page.viewport(900, 600);
  for (const row of rows) {
    // `<` is escaped so a cell can never close the script element.
    const json = JSON.stringify(row).replace(/</g, "\\u003c");
    const html = template.replace(slot, `<script type="application/json" id="row">${json}</script>`);
    const file = path.join("out", `${row.id}.html`);
    writeFileSync(file, html);
    await page.goto(pathToFileURL(path.resolve(file)).href);
    const verdict = await page.waitFor("window.__badge", `badge ${row.id} script`);
    if (verdict.ok) {
      await page.screenshot(path.join("out", `${row.id}.png`));
    } else {
      renameSync(file, path.join("held", `${row.id}.html`));
      withheld.push(`${row.id} ${verdict.problems.join("; ")}`);
    }
  }
} finally {
  await browser.close();
}
writeFileSync(path.join("out", "withheld.txt"), withheld.length ? `${withheld.join("\n")}\n` : "");
console.log(`rows=${rows.length} shipped=${rows.length - withheld.length} withheld=${withheld.length}`);
