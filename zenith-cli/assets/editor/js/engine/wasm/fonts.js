// Lazy bundled fonts of the static host.
//
// The wasm module embeds Noto Sans Regular and Bold. Any other bundled face
// the document needs comes from the `fonts` protocol (the `fonts.required`
// command): the engine names the files, the page fetches them from the
// static site, and asks again until the list is empty. A fetched file is
// cached for the life of the page.

import { blobToBase64 } from "./files.js";

export class FontCache {
  /** `baseUrl`: the directory URL that holds the font files. */
  constructor(baseUrl) {
    this.baseUrl = baseUrl;
    this.cache = new Map();
  }

  /** The base64 text of font file `file`. Rejects when it cannot be fetched. */
  get(file) {
    let entry = this.cache.get(file);
    if (!entry) {
      entry = (async () => {
        const res = await fetch(new URL(file, this.baseUrl));
        if (!res.ok) throw new Error(`${file} answered ${res.status} ${res.statusText}`);
        return blobToBase64(await res.blob());
      })();
      entry.catch(() => this.cache.delete(file));
      this.cache.set(file, entry);
    }
    return entry;
  }
}

/**
 * A key for the `font.unresolved` diagnostics in `list`: equal keys mean the
 * same faces are unresolved, so the files need no new look. `null` when the
 * list has none.
 */
export function fontAdvisoryKey(list) {
  const messages = (list ?? []).filter((d) => d.code === "font.unresolved").map((d) => d.message);
  return messages.length ? messages.sort().join("\n") : null;
}
