// Build the static editor: a directory any static file host can serve.
//
//   node zenith-cli/scripts/build-static-editor.mjs --out <dir> [--wasm <file>] [--examples <dir>] [--no-samples]
//
// The wasm module comes from
//   cargo build --target wasm32-wasip1 -p zenith-editor-wasm --profile release-wasm
// `--wasm` names the built file. Without it the script looks in
// $CARGO_TARGET_DIR (default `<repo>/target`) for
// `wasm32-wasip1/release-wasm/zenith-editor-wasm.wasm`.
//
// Output (every path relative, so the site can sit in a subdirectory):
//   index.html, css/, js/, vendor/     the editor page (zenith-cli/assets/editor),
//                                      with `<meta name="zenith-host" content="wasm">`
//   zenith-editor-wasm.wasm            the engine
//   fonts/                             bundled fonts the engine asks for by name
//   samples/                           examples/*.zen, their project files, manifest.json
//   _headers                           content types and CSP for Netlify and Cloudflare Pages
//
// Zero dependencies. Exit code 0 on success.

import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(here, "../..");

function args() {
  const out = { samples: true };
  const argv = process.argv.slice(2);
  for (let i = 0; i < argv.length; i++) {
    const key = argv[i];
    if (key === "--no-samples") out.samples = false;
    else if (["--out", "--wasm", "--examples"].includes(key)) out[key.slice(2)] = argv[++i];
    else throw new Error(`unknown argument ${key}; see the header of build-static-editor.mjs`);
  }
  if (!out.out) throw new Error("missing --out <dir>; see the header of build-static-editor.mjs");
  return out;
}

function findWasm(explicit) {
  const target = process.env.CARGO_TARGET_DIR || path.join(repo, "target");
  const found = explicit || path.join(target, "wasm32-wasip1/release-wasm/zenith-editor-wasm.wasm");
  if (!existsSync(found)) {
    throw new Error(
      `the wasm module ${found} does not exist; build it with ` +
        "`cargo build --target wasm32-wasip1 -p zenith-editor-wasm --profile release-wasm` or pass --wasm <file>",
    );
  }
  return found;
}

/** Every file under `dir` as a sorted list of paths relative to it. */
function walk(dir, prefix = "") {
  const out = [];
  for (const name of readdirSync(dir).sort()) {
    const full = path.join(dir, name);
    if (statSync(full).isDirectory()) out.push(...walk(full, `${prefix}${name}/`));
    else out.push(`${prefix}${name}`);
  }
  return out;
}

function copy(from, to) {
  mkdirSync(path.dirname(to), { recursive: true });
  copyFileSync(from, to);
}

const HEADERS = `# Netlify and Cloudflare Pages. Other hosts: see the "Static editor" section of README.md.
/*
  X-Content-Type-Options: nosniff
  Content-Security-Policy: default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; worker-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' blob: data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'self'
/*.wasm
  Content-Type: application/wasm
`;

function main() {
  const a = args();
  const out = path.resolve(a.out);
  const wasm = findWasm(a.wasm);
  rmSync(out, { recursive: true, force: true });
  mkdirSync(out, { recursive: true });

  const page = path.join(repo, "zenith-cli/assets/editor");
  for (const rel of walk(page)) {
    // The CodeMirror rebuild notes are not part of the site.
    if (rel === "vendor/MANIFEST") continue;
    copy(path.join(page, rel), path.join(out, rel));
  }
  // Pin the host: the page then never asks `/api/state`, which a static host answers with a 404.
  const index = path.join(out, "index.html");
  const html = readFileSync(index, "utf8");
  const marker = '<meta charset="utf-8">';
  if (!html.includes(marker)) throw new Error("index.html lost its charset meta; update build-static-editor.mjs");
  writeFileSync(index, html.replace(marker, `${marker}\n<meta name="zenith-host" content="wasm">`));
  copy(wasm, path.join(out, "zenith-editor-wasm.wasm"));

  const fonts = path.join(repo, "zenith-core/assets/fonts");
  for (const rel of walk(fonts)) copy(path.join(fonts, rel), path.join(out, "fonts", rel));

  let documents = 0;
  if (a.samples) {
    const examples = path.resolve(a.examples || path.join(repo, "examples"));
    // A top-level .png is the preview of a document, not a project file.
    const all = walk(examples).filter((rel) => !(rel.endsWith(".png") && !rel.includes("/")) && rel !== "README.md");
    for (const rel of all) copy(path.join(examples, rel), path.join(out, "samples", rel));
    const docs = all.filter((rel) => rel.endsWith(".zen") && !rel.includes("/"));
    documents = docs.length;
    const files = all.filter((rel) => !docs.includes(rel));
    writeFileSync(
      path.join(out, "samples/manifest.json"),
      `${JSON.stringify({ documents: docs, files }, null, 2)}\n`,
    );
  }
  writeFileSync(path.join(out, "_headers"), HEADERS);

  const size = walk(out).reduce((n, rel) => n + statSync(path.join(out, rel)).size, 0);
  console.log(`static editor: ${out} (${(size / 1048576).toFixed(1)} MiB, ${documents} sample documents)`);
}

try {
  main();
} catch (err) {
  console.error(`build-static-editor: ${err.message}`);
  process.exit(1);
}
