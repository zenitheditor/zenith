// Hosted document links point to samples/ or starters/ beside the editor.
//
// A `manifest.json` next to the document lists the project files it needs
// (`{"files": ["assets/logo.png", ...]}`, paths relative to that directory).
// The build writes one for `samples/`.

import { blobToBase64 } from "../engine/wasm/files.js";

export const FEATURED_POSTER = "starters/poster.zen";
const EXAMPLES = new Set([FEATURED_POSTER, "samples/article.zen", "samples/flowchart.zen"]);

export function curatedExample(path) {
  return document.querySelector('meta[name="zenith-gallery"]')?.content === "true" && EXAMPLES.has(path)
    ? path
    : null;
}

function pathSegments(path) {
  if (typeof path !== "string" || /[\\:?#\u0000-\u001f\u007f]|%[0-9a-f]{2}/i.test(path)) return null;
  const segments = path.split("/");
  return segments.some((segment) => !segment || segment === "." || segment === "..") ? null : segments;
}

function bundledUrl(segments, dir) {
  return new URL(segments.map(encodeURIComponent).join("/"), dir);
}

async function fetchBundled(url, { optional = false } = {}) {
  const res = await fetch(url, { mode: "same-origin", redirect: "error" });
  if (optional && res.status === 404) return null;
  if (!res.ok) throw new Error(`Could not fetch ${url.pathname} (HTTP ${res.status}).`);
  return res;
}

export async function loadSample(path) {
  const segments = pathSegments(path);
  if (!segments || segments.length < 2 || !["samples", "starters"].includes(segments[0]) || !/\.zen$/i.test(segments.at(-1))) {
    throw new Error("Choose a .zen document under samples/ or starters/.");
  }
  const url = bundledUrl(segments, new URL("./", location.href));
  const dir = new URL("./", url);
  const name = segments.at(-1);
  const text = await (await fetchBundled(url)).text();
  const manifestResponse = await fetchBundled(new URL("manifest.json", dir), { optional: true });
  if (!manifestResponse) return { name, text, project: null };

  const manifest = await manifestResponse.json();
  const paths = manifest?.files ?? [];
  if (!manifest || typeof manifest !== "object" || Array.isArray(manifest) || !Array.isArray(paths)) {
    throw new Error("The sample manifest has an invalid file list.");
  }
  const assets = [...new Set(paths)].map((rel) => {
    const parts = pathSegments(rel);
    if (!parts) throw new Error("The sample manifest contains an invalid asset path.");
    return [rel, bundledUrl(parts, dir)];
  });
  const files = Object.fromEntries(await Promise.all(assets.map(async ([rel, asset]) =>
    [rel, await blobToBase64(await (await fetchBundled(asset)).blob())])));
  const project = { root: "", files, handles: new Map() };
  return { name, text, project };
}
