// The two arms. Both get the same file tools and the same `view` tool (one page,
// or every page tiled with `all`); the Zenith arm also gets the Zenith CLI. A PNG
// the CLI writes with `--png` or `--contact-sheet` comes back as an image, as it
// would to an agent that opens the file. The HTML arm's `view all` is the same
// capability. Nothing else differs except the document format.
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import path from "node:path";
import { readFileSync, writeFileSync } from "node:fs";
import { zenithBin } from "./render.mjs";
import { contactSheet, decode, encode } from "./png.mjs";

const run = promisify(execFile);
const LIMIT = 8000;
const MAX_SHEET_PAGES = 16;
// Everything except the commands that start a server, change the install, or run an MCP server.
const ZENITH_BLOCKED = new Set(["edit", "update", "plugin", "mcp"]);

const fn = (name, description, properties, required) => ({
  type: "function",
  function: { name, description, parameters: { type: "object", properties, required } },
});

const FILE_TOOLS = [
  fn("read_file", "Read a text file in the working directory.", { path: { type: "string" } }, ["path"]),
  fn("write_file", "Write a text file in the working directory, replacing it.", { path: { type: "string" }, content: { type: "string" } }, ["path", "content"]),
  fn("edit_file", "Replace one exact occurrence of a string in a text file. Fails if the string is missing or appears more than once.", { path: { type: "string" }, old: { type: "string" }, new: { type: "string" } }, ["path", "old", "new"]),
  fn("view", "Render the current design and show it as an image. Pass `page` (1-based) to see another page, or `all: true` to see every page tiled in one image.", { page: { type: "integer" }, all: { type: "boolean" } }, []),
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

export function makeArm(kind, ws, canvas, skill, build) {
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
    system: (build ? buildPrompt(kind, file, canvas, build) : COMMON(file, canvas) + extra) + (build ? "" : "") + skillText,
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
            if (args.all === true) {
              const r = await render(1);
              if (!r.png) return { text: `render failed:\n${clip(r.error)}` };
              const pages = (r.all ?? [r.png]).slice(0, MAX_SHEET_PAGES).map((png) => decode(png));
              return { text: `rendered ${pages.length} page(s); the image follows`, image: encode(contactSheet(pages)) };
            }
            const r = await render(Number.isInteger(args.page) ? args.page : 1);
            return r.png ? { text: "rendered; the image follows", image: r.png } : { text: `render failed:\n${clip(r.error)}` };
          }
          case "zenith": {
            if (kind !== "zenith") return { text: `unknown tool ${name}` };
            const list = Array.isArray(args.args) ? args.args.map(String) : [];
            if (!list.length || ZENITH_BLOCKED.has(list[0])) return { text: `command not available: ${list[0] ?? ""}` };
            if (list.some((a) => path.isAbsolute(a) || a.split(/[\\/]/).includes(".."))) return { text: "paths must stay inside the working directory" };
            try {
              const { stdout, stderr } = await run(zenithBin(), list, { cwd: ws, timeout: 60000, maxBuffer: 4 << 20 });
              const out = { text: clip(`${stdout}${stderr}`.trim() || "(no output)") };
              // A PNG the command wrote comes back as an image, as an agent that opens the file would see it.
              for (const flag of ["--png", "--contact-sheet"]) {
                const joined = list.find((a) => a.startsWith(`${flag}=`));
                const at = list.indexOf(flag);
                const file = joined ? joined.slice(flag.length + 1) : at >= 0 ? list[at + 1] : null;
                if (file) {
                  try {
                    out.image = readFileSync(path.join(ws, file));
                  } catch {}
                }
              }
              return out;
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

/** System prompt for authoring a design from a brief: the file contract differs per arm. */
function buildPrompt(kind, file, canvas, build) {
  const mixed = build.sizes && new Set(build.sizes.map((s) => `${s.w}x${s.h}`)).size > 1;
  const pages = build.pages === 1 ? "one page" : `${build.pages} pages`;
  const sizeText = mixed
    ? `. The pages have these sizes in px, in order: ${build.sizes.map((s, i) => `page ${i + 1} ${s.w}x${s.h}`).join(", ")}.`
    : `, each ${canvas.w}x${canvas.h} px.`;
  const contract =
    kind === "zenith"
      ? "Author a Zenith document (KDL text) in design.zen with the `zenith` tool (start with `zenith --help`). Every page is exactly its stated size."
      : 'Author one HTML file with inline CSS in design.html. Wrap every page in `<section class="page">` that is exactly its stated size in px, with body margin 0, the sections stacked top to bottom with no gap.';
  return `You design a ${build.kind} from a brief, starting from an empty working directory. The design has ${pages}${sizeText} ${contract} Use only the fonts Noto Sans and Noto Sans Mono. Use no images: build the design from text and shapes. Include every text item the user lists, exactly as written. Do not invent other text. Use \`view\` to check each page before you finish. When it is done, reply with one short sentence and no tool call.`;
}
