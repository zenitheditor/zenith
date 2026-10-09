// The document a static site opens at startup: `?doc=<path>` (a path on
// the same site, relative to the page), else the bundled sample.
//
// A `manifest.json` next to the document lists the project files it needs
// (`{"files": ["assets/logo.png", ...]}`, paths relative to that directory).
// The build writes one for `samples/`.

import { blobToBase64 } from "../engine/wasm/files.js";

const DEFAULT_SAMPLE = "samples/stack.zen";

/** A same-site URL for `path`, or throws. */
function siteUrl(path) {
  const url = new URL(path, document.baseURI);
  if (url.origin !== location.origin) {
    throw new Error(`?doc=${path} is on another site; use a path on this site`);
  }
  return url;
}

async function fetchOk(url) {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${url} answered ${res.status} ${res.statusText}`);
  return res;
}

/**
 * Fetch the sample document `path` (default: the bundled sample) and its
 * project files. Resolves the `WasmEngine.openDocument` argument.
 */
export async function loadSample(path) {
  const url = siteUrl(path || DEFAULT_SAMPLE);
  const dir = new URL("./", url);
  const name = decodeURIComponent(url.pathname.split("/").pop());
  const text = await (await fetchOk(url)).text();
  let project = null;
  try {
    const manifest = await (await fetchOk(new URL("manifest.json", dir))).json();
    const files = {};
    await Promise.all(
      (manifest.files ?? []).map(async (rel) => {
        files[rel] = await blobToBase64(await (await fetchOk(new URL(rel, dir))).blob());
      }),
    );
    project = { root: "", files, handles: new Map() };
  } catch {
    // No manifest: the document stands alone.
  }
  return { name, text, project };
}
