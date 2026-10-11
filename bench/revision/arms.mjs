// The two arms. Both get the same file tools and the same `view` tool; the Zenith
// arm also gets the Zenith CLI. Nothing else differs except the document format.
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import path from "node:path";
import { readFileSync, writeFileSync } from "node:fs";
import { zenithBin } from "./render.mjs";

const run = promisify(execFile);
const LIMIT = 8000;
const ZENITH_COMMANDS = new Set(["schema", "validate", "render", "tx", "inspect", "fmt", "fix", "tokens", "perceive", "new", "help", "--help", "-h"]);

const fn = (name, description, properties, required) => ({
  type: "function",
  function: { name, description, parameters: { type: "object", properties, required } },
});

const FILE_TOOLS = [
  fn("read_file", "Read a text file in the working directory.", { path: { type: "string" } }, ["path"]),
  fn("write_file", "Write a text file in the working directory, replacing it.", { path: { type: "string" }, content: { type: "string" } }, ["path", "content"]),
  fn("edit_file", "Replace one exact occurrence of a string in a text file. Fails if the string is missing or appears more than once.", { path: { type: "string" }, old: { type: "string" }, new: { type: "string" } }, ["path", "old", "new"]),
  fn("view", "Render the current design and show it as an image.", {}, []),
];

const ZENITH_TOOL = fn(
  "zenith",
  "Run the Zenith CLI in the working directory. Pass the arguments after `zenith` as a list, for example [\"validate\", \"design.zen\"].",
  { args: { type: "array", items: { type: "string" } } },
  ["args"],
);

const COMMON = (file, canvas) =>
  `You are revising an existing design in the working directory. The design file is ${file}, a ${canvas.w}x${canvas.h} px canvas. Do exactly what the request asks and change nothing else: every other element must keep its position, size, color and text. Use the tools. When the change is done, reply with one short sentence and no tool call.`;

const clip = (s) => (s.length > LIMIT ? `${s.slice(0, LIMIT)}\n[truncated ${s.length - LIMIT} chars]` : s);

function safe(ws, rel) {
  if (typeof rel !== "string" || !rel || path.isAbsolute(rel) || rel.split(/[\\/]/).includes("..")) {
    throw new Error(`path must be relative and inside the working directory: ${rel}`);
  }
  return path.join(ws, rel);
}

export function makeArm(kind, ws, canvas, skill) {
  const file = kind === "zenith" ? "design.zen" : "design.html";
  const extra =
    kind === "zenith"
      ? " The design is a Zenith document (KDL text). The `zenith` tool runs the Zenith CLI (try `--help` and `schema`)."
      : " The design is one HTML file with inline CSS. `view` renders it in Chromium.";
  const skillText = skill
    ? `\n\nA skill is available for this work. Its SKILL.md follows. Paths in it are relative to the skill/ directory of the working directory, and you can read them with read_file.\n\n--- SKILL.md ---\n${skill}`
    : "";
  return {
    kind,
    file,
    system: COMMON(file, canvas) + extra + skillText,
    tools: kind === "zenith" ? [...FILE_TOOLS, ZENITH_TOOL] : FILE_TOOLS,
    /** Returns { text, image? } where image is PNG bytes. */
    async exec(name, args, render) {
      try {
        switch (name) {
          case "read_file":
            return { text: clip(readFileSync(safe(ws, args.path), "utf8")) };
          case "write_file":
            writeFileSync(safe(ws, args.path), String(args.content ?? ""));
            return { text: `wrote ${args.path}` };
          case "edit_file": {
            const file = safe(ws, args.path);
            const text = readFileSync(file, "utf8");
            const old = String(args.old ?? "");
            const n = old ? text.split(old).length - 1 : 0;
            if (n !== 1) return { text: `error: \`old\` matches ${n} times; it must match exactly once` };
            writeFileSync(file, text.replace(old, () => String(args.new ?? "")));
            return { text: `edited ${args.path}` };
          }
          case "view": {
            const r = await render();
            return r.png ? { text: "rendered; the image follows", image: r.png } : { text: `render failed:\n${clip(r.error)}` };
          }
          case "zenith": {
            if (kind !== "zenith") return { text: `unknown tool ${name}` };
            const list = Array.isArray(args.args) ? args.args.map(String) : [];
            if (!list.length || !ZENITH_COMMANDS.has(list[0])) return { text: `command not available: ${list[0] ?? ""}` };
            if (list.some((a) => path.isAbsolute(a) || a.split(/[\\/]/).includes(".."))) return { text: "paths must stay inside the working directory" };
            try {
              const { stdout, stderr } = await run(zenithBin(), list, { cwd: ws, timeout: 60000, maxBuffer: 4 << 20 });
              return { text: clip(`${stdout}${stderr}`.trim() || "(no output)") };
            } catch (err) {
              return { text: clip(`exit ${err.code ?? "?"}\n${err.stdout ?? ""}${err.stderr ?? ""}`.trim()) };
            }
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
