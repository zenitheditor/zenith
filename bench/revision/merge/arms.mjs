// The two merge arms. Both get the same file tools, `view_png`, `list_files`,
// `remove_file` and `node`. The Zenith arm also gets the Zenith CLI; the HTML
// arm gets the build bench's CDP helper as cdp.js. The prompt is shared except
// for one paragraph that names each arm's file contract.
import { spawn } from "node:child_process";
import path from "node:path";
import { existsSync, readdirSync, readFileSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { makeArm } from "../arms.mjs";
import { contactSheet, decode, encode } from "../png.mjs";

const LIMIT = 8000;
const NODE_TIMEOUT = 300000;
const MAX_SHEET = 16;

const fn = (name, description, properties, required) => ({
  type: "function",
  function: { name, description, parameters: { type: "object", properties, required } },
});
const str = { type: "string" };
const list = { type: "array", items: { type: "string" } };

const TOOLS = [
  fn("read_file", "Read a text file in the working directory.", { path: str }, ["path"]),
  fn("write_file", "Write a text file in the working directory, replacing it.", { path: str, content: str }, ["path", "content"]),
  fn("edit_file", "Replace one exact occurrence of a string in a text file. Fails if the string is missing or appears more than once.", { path: str, old: str, new: str }, ["path", "old", "new"]),
  fn("remove_file", "Remove one file in the working directory.", { path: str }, ["path"]),
  fn("list_files", "List the files of a directory in the working directory (default: the working directory itself).", { path: str }, []),
  fn("view_png", "Show PNG files as one image. One path shows that file; several (up to 16) are tiled in a grid.", { paths: list }, ["paths"]),
  fn("node", "Run a Node.js script in the working directory: pass the script path and its arguments as a list, for example [\"render.mjs\"], or [\"-e\", \"<code>\"] for inline code. The code can read and write only inside the working directory. Timeout 300 s.", { args: list }, ["args"]),
];
const ZENITH = fn("zenith", "Run the Zenith CLI in the working directory. Pass the arguments after `zenith` as a list, for example [\"validate\", \"badge.zen\"].", { args: list }, ["args"]);

const clip = (s) => (s.length > LIMIT ? `${s.slice(0, LIMIT)}\n[truncated ${s.length - LIMIT} chars]` : s);
const relOk = (rel) => typeof rel === "string" && rel && !path.isAbsolute(rel) && !rel.split(/[\\/]/).includes("..");

// Tool output never carries the API key, even if a script reads it from somewhere.
function redact(text) {
  const key = process.env.OLLAMA_API_KEY;
  return key && key.length > 8 ? text.split(key).join("[redacted]") : text;
}

/**
 * Run `node <args>` under the Node permission model, with a minimal environment, and kill its process group at the end.
 * The system temp directory stays writable: cdp.js keeps the Chromium profile there, and Chromium crashes when TMPDIR
 * points at a disk directory.
 */
function runNode(ws, args, chromium) {
  const tmp = tmpdir();
  const env = { PATH: process.env.PATH, HOME: ws, CHROMIUM: chromium, ZENITH_E2E_NO_SANDBOX: "1", LANG: "C.UTF-8" };
  const flags = ["--permission", `--allow-fs-read=${ws}`, `--allow-fs-read=${tmp}`, `--allow-fs-write=${ws}`, `--allow-fs-write=${tmp}`, "--allow-child-process", "--allow-net", "--no-warnings"];
  return new Promise((resolve) => {
    const child = spawn(process.execPath, [...flags, ...args], { cwd: ws, env, detached: true, stdio: ["ignore", "pipe", "pipe"] });
    let out = "";
    const add = (d) => {
      if (out.length < 4 * LIMIT) out += d;
    };
    child.stdout.on("data", add);
    child.stderr.on("data", add);
    const killGroup = () => {
      try {
        process.kill(-child.pid, "SIGKILL");
      } catch {}
    };
    const timer = setTimeout(() => {
      out += `\n[killed after ${NODE_TIMEOUT / 1000} s]`;
      killGroup();
    }, NODE_TIMEOUT);
    child.on("close", (code) => {
      clearTimeout(timer);
      killGroup();
      resolve(`exit ${code ?? "killed"}\n${out.trim() || "(no output)"}`);
    });
    child.on("error", (err) => {
      clearTimeout(timer);
      resolve(`error: ${err.message}`);
    });
  });
}

const RULES = (task) => `Rules for every badge:
- The badge is exactly ${task.canvas.w}x${task.canvas.h} px, saved as out/<id>.png, where <id> is the row's \`id\` cell.
- Show the name, title and company exactly as in the CSV. Never truncate, abbreviate or add an ellipsis. Wrapping onto more lines and smaller type are fine.
- Show no text other than the event name and those three fields.
- An empty cell shows nothing for that field: no placeholder, no label, no dash.
- All text is fully inside the badge and inside its own box, does not overlap other text, keeps ${task.margin} px from the badge edge, and has a contrast of at least ${task.minContrast}:1 against what is behind it, including the photo.
- Every character renders as its real glyph, with no empty boxes for missing glyphs. Noto Sans is the main font; any font installed on this machine is allowed for characters it lacks.
- When a row cannot render correctly, do not ship it: leave no out/<id>.png for it and add a line \`<id> <reason>\` to out/withheld.txt.`;

const CONTRACT = {
  zenith: (task) =>
    `Author the template in badge.zen, a Zenith document (KDL text), with the \`zenith\` tool. Start with \`zenith --help\` and \`zenith merge --help\`. Mark the bound nodes with role="data.<column>": text nodes for name, title and company, and an image node for photo. Render the rows with \`zenith merge badge.zen ${task.csv} --out-dir out --name-by id\`. Add \`--json\` for a per-row report. The harness rebuilds each shipped row from badge.zen and the CSV, so the files in out/ must come from the final badge.zen.`,
  html: (task) =>
    `Author the template in badge.html and a Node script render.mjs that fills the template for each row and screenshots it with headless Chromium. Each row page is one \`<section class="page">\` of exactly ${task.canvas.w}x${task.canvas.h} px, with body margin 0. Save the exact HTML of each row page as out/<id>.html and screenshot it to out/<id>.png at a ${task.canvas.w}x${task.canvas.h} viewport. Put any fitting logic in the page's own script, so the saved page renders the same when reloaded: the harness reloads out/<id>.html to check the row. Run the script with the \`node\` tool. The helper cdp.js in the working directory drives Chromium; its path is in the CHROMIUM environment variable:
\`\`\`js
import { Browser } from "./cdp.js";
const browser = await Browser.launch(process.env.CHROMIUM);
const page = await browser.newPage();
await page.viewport(${task.canvas.w}, ${task.canvas.h});
await page.goto(pathToFileURL(path.resolve("out/A001.html")).href);
const value = await page.eval("document.fonts.ready.then(() => 1)"); // evaluates an expression in the page, awaited
await page.screenshot("out/A001.png");
await browser.close();
\`\`\``,
};

/** The arm for `kind` ("zenith" | "html") over workspace `ws`. */
export function makeMergeArm(kind, ws, task, chromium, skillBody = "") {
  // The shared arm supplies read_file, write_file, edit_file and zenith with the build bench's semantics.
  const base = makeArm("zenith", ws, task.canvas, "", null);
  const system = `You produce one conference badge image per row of a CSV file. The working directory holds the CSV and the photos it names. Work only through the tools. After you finish, the harness checks every row with checks you cannot see. A row counts against you when its image ships with a defect, or when its image is missing and the row is not in out/withheld.txt. Withholding a row that could render correctly also counts against you.\n\n${RULES(task)}\n\n${CONTRACT[kind](task)}\n\nWhen every row is done, reply with one short sentence and no tool call.${skillBody ? `\n\nSkill (also readable under skill/):\n${skillBody}` : ""}`;
  return {
    kind,
    system,
    tools: kind === "zenith" ? [...TOOLS, ZENITH] : TOOLS,
    async exec(name, args) {
      const r = await this.execRaw(name, args);
      return { ...r, text: redact(r.text) };
    },
    async execRaw(name, args) {
      try {
        switch (name) {
          case "read_file":
          case "write_file":
          case "edit_file":
            return base.exec(name, args);
          case "zenith":
            return kind === "zenith" ? base.exec(name, args) : { text: `unknown tool ${name}` };
          case "remove_file":
            if (!relOk(args.path)) return { text: `error: path must be relative and inside the working directory: ${args.path}` };
            rmSync(path.join(ws, args.path), { force: true });
            return { text: `removed ${args.path}` };
          case "list_files": {
            const rel = args.path || ".";
            if (rel !== "." && !relOk(rel)) return { text: `error: path must be relative and inside the working directory: ${rel}` };
            const dir = path.join(ws, rel);
            if (!existsSync(dir) || !statSync(dir).isDirectory()) return { text: `error: no directory ${rel}` };
            const names = readdirSync(dir).filter((n) => !n.startsWith(".")).sort();
            return { text: clip(`${names.length} entries\n${names.join("\n")}`) };
          }
          case "view_png": {
            const paths = (Array.isArray(args.paths) ? args.paths : [args.paths]).map(String).slice(0, MAX_SHEET);
            const bad = paths.find((p) => !relOk(p));
            if (!paths.length || bad !== undefined) return { text: `error: paths must be relative and inside the working directory` };
            const imgs = paths.map((p) => decode(readFileSync(path.join(ws, p))));
            const png = imgs.length === 1 ? readFileSync(path.join(ws, paths[0])) : encode(contactSheet(imgs));
            return { text: `showing ${paths.join(", ")}; the image follows`, image: png };
          }
          case "node": {
            const list = Array.isArray(args.args) ? args.args.map(String) : [];
            const inline = (list[0] === "-e" || list[0] === "--eval") && list.length >= 2;
            const script = /\.(m?js|cjs)$/.test(list[0] ?? "") && relOk(list[0]);
            if (!inline && !script) return { text: "error: the first argument must be a relative .js or .mjs script path, or -e followed by code" };
            return { text: clip(await runNode(ws, list, chromium)) };
          }
          default:
            return { text: `unknown tool ${name}` };
        }
      } catch (err) {
        return { text: `error: ${err.message}` };
      }
    },
  };
}
