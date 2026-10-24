// Lazy bundled and fallback fonts of the static host.
//
// The wasm module embeds Noto Sans Regular and Bold. Any other bundled face
// the document needs comes from the `fonts` protocol (the `fonts.required`
// command): the engine names the files, the page fetches them from the
// static site, and asks again until the list is empty. The fallback faces
// (Arabic, Hebrew, CJK, emoji) use the same protocol: the engine names the
// file when text has a char only that face covers. The engine registers
// fallback files in chain order, so fetch order never changes the render.
// A fetched file is cached for the life of the page as raw bytes. The page
// gives it to the Worker once, and the Worker sends the bytes after the
// envelope of each request (see `request.js`).

export class FontCache {
  /** `baseUrl`: the directory URL that holds the font files. */
  constructor(baseUrl) {
    this.baseUrl = baseUrl;
    this.cache = new Map();
  }

  /** The bytes (Uint8Array) of font file `file`. Rejects when it cannot be fetched. */
  get(file) {
    let entry = this.cache.get(file);
    if (!entry) {
      entry = (async () => {
        const res = await fetch(new URL(file, this.baseUrl));
        if (!res.ok) throw new Error(`${file} answered ${res.status} ${res.statusText}`);
        return new Uint8Array(await res.arrayBuffer());
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
