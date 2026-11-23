// Hosted document links point to samples/ or starters/ beside the editor.
//
// A `manifest.json` next to the document lists the project files it needs
// (`{"files": ["assets/logo.png", ...]}`, paths relative to that directory).
// The build writes one for `samples/`.

import { blobToBase64 } from "../engine/wasm/files.js";

const EXAMPLES = new Set([
  "starters/poster.zen",
  "starters/harbor-report.zen",
  "starters/print-workflow.zen",
  "starters/keynote-slide.zen",
  "starters/spec-sheet.zen",
  "starters/launch-card.zen",
]);

export const BLANK_PRESETS = {
  poster: { title: "Untitled Poster", name: "Untitled.zen", w: 600, h: 720 },
  a4: { title: "Untitled A4", name: "document.zen", w: 595, h: 842 },
  slide: { title: "Untitled Slide", name: "slide.zen", w: 1920, h: 1080 },
  square: { title: "Untitled Square", name: "square.zen", w: 1080, h: 1080 },
  story: { title: "Untitled Story", name: "story.zen", w: 1080, h: 1920 },
  custom: { title: "Untitled", name: "untitled.zen", w: 800, h: 600 },
};

/** Generate a valid blank `.zen` document with 1 Page + minimal starter tokens (0 starter shapes). */
export function blankPreset(key = "poster") {
  const p = BLANK_PRESETS[key] ?? BLANK_PRESETS.poster;
  const text = `zenith version=1 {
  project id="project" name="${p.title}"
  tokens format="zenith-token-v1" {
    token id="paper" type="color" value="#ffffff"
    token id="ink" type="color" value="#0b1a2e"
    token id="accent" type="color" value="#0870a6"
    token id="sans" type="fontFamily" value="Noto Sans"
    token id="serif" type="fontFamily" value="Noto Serif"
  }
  styles {}
  document id="doc" title="${p.title}" {
    page id="page" w=(px)${p.w} h=(px)${p.h} background=(token)"paper" {
    }
  }
}
`;
  return { name: p.name, text, project: null, draftSource: { kind: "blank" } };
}

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
