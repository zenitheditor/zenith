// Renders the current design of a workspace to PNG bytes, per arm.
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { readFileSync, mkdtempSync, rmSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";

const run = promisify(execFile);

export const repo = path.resolve(import.meta.dirname, "../..");

export function zenithBin() {
  const env = process.env.ZENITH_BENCH_ZENITH;
  if (env) return env;
  for (const c of ["target/release/zenith", "target/debug/zenith"]) {
    if (existsSync(path.join(repo, c))) return path.join(repo, c);
  }
  throw new Error("no zenith binary: run `cargo build --release` or set ZENITH_BENCH_ZENITH");
}

/** Returns { png: Buffer } or { error: string }. */
export async function renderZen(ws) {
  const out = mkdtempSync(path.join(tmpdir(), "zbench-"));
  try {
    const file = path.join(out, "r.png");
    await run(zenithBin(), ["render", "design.zen", "--png", file], { cwd: ws, timeout: 60000 });
    return { png: readFileSync(file) };
  } catch (err) {
    return { error: `${err.stderr ?? ""}${err.stdout ?? ""}`.trim().slice(0, 4000) || String(err) };
  } finally {
    rmSync(out, { recursive: true, force: true });
  }
}

/** Returns a renderer of `design.html` at the canvas size; it reuses one page of `browser`. */
export async function htmlRenderer(browser, canvas) {
  const page = await browser.newPage();
  await page.viewport(canvas.w, canvas.h);
  return async (ws) => {
    const out = mkdtempSync(path.join(tmpdir(), "zbench-"));
    try {
      await page.goto(pathToFileURL(path.join(ws, "design.html")).href);
      await page.eval("document.fonts.ready.then(() => true)");
      const file = path.join(out, "r.png");
      await page.screenshot(file);
      return { png: readFileSync(file) };
    } catch (err) {
      return { error: String(err.message ?? err) };
    } finally {
      rmSync(out, { recursive: true, force: true });
    }
  };
}
