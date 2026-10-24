// Pure helpers of the engine Worker: the project store, the request bytes,
// and base64 decoding. They live apart from `worker.js` so Node tests can
// run them.
//
// Stdin of one module request is the JSON envelope, then, when fonts are
// attached, one NUL byte and the raw bytes of each font file. The envelope
// lists the files as `"fonts": [{"file", "size"}]`, in the order the bytes
// follow. Raw bytes skip base64 and JSON: the CJK fallback face is 5.6 MB.

const encoder = new TextEncoder();
const SEPARATOR = new Uint8Array([0]);

/** The bytes of base64 text `b64`. */
export function decodeBase64(b64) {
  if (typeof Uint8Array.fromBase64 === "function") return Uint8Array.fromBase64(b64);
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

/**
 * The stdin of one request for `method`: `{chunks, jsonBytes, fontBytes}`.
 * `chunks` is the envelope bytes, then, when `project` is set and `fonts`
 * is not empty, the separator and each font file. `params` goes in as
 * JSON. With `project`, `filesJson` (the project files, already JSON text)
 * is spliced into `params`, never parsed or copied by value, and `fonts`
 * (file name -> Uint8Array) is attached in file-name order.
 */
export function requestParts(method, params, project, filesJson, fonts) {
  const head = JSON.stringify(params ?? {});
  const body = project ? `${head.slice(0, -1)}${head === "{}" ? "" : ","}"files":${filesJson}}` : head;
  const files = project ? [...fonts.keys()].sort() : [];
  const list = files.map((file) => ({ file, size: fonts.get(file).length }));
  const tail = list.length ? `,"fonts":${JSON.stringify(list)}` : "";
  const json = encoder.encode(`{"method":${JSON.stringify(method)},"params":${body}${tail}}`);
  const chunks = [json];
  let fontBytes = 0;
  if (list.length) {
    chunks.push(SEPARATOR);
    for (const file of files) {
      const bytes = fonts.get(file);
      chunks.push(bytes);
      fontBytes += bytes.length;
    }
  }
  return { chunks, jsonBytes: json.length, fontBytes };
}

/**
 * The project the Worker holds between requests: the files map as JSON
 * text, and the font files as raw bytes by file name. The page sends a
 * font once. A later file under the same name replaces it, so a request
 * never carries stale bytes.
 */
export class ProjectStore {
  constructor() {
    this.filesJson = "{}";
    /** File name -> Uint8Array. */
    this.fonts = new Map();
  }

  /** Replace the `files` map, drop every font on `resetFonts`, then add `fonts` (file name -> Uint8Array). */
  update({ files, fonts, resetFonts } = {}) {
    if (files) this.filesJson = JSON.stringify(files);
    if (resetFonts) this.fonts.clear();
    for (const [file, bytes] of Object.entries(fonts ?? {})) {
      if (!(bytes instanceof Uint8Array)) throw new Error(`font ${file} is not a Uint8Array; send the raw file bytes`);
      this.fonts.set(file, bytes);
    }
  }

  /** The stdin parts of one request (see `requestParts`). */
  request(method, params, project) {
    return requestParts(method, params, project, this.filesJson, this.fonts);
  }
}

/**
 * Move `result.png_base64` of a parsed module response into an ArrayBuffer.
 * Resolves `null` when the response has none.
 */
export function takePng(response) {
  const result = response.ok ? response.result : null;
  if (!result || typeof result.png_base64 !== "string") return null;
  const bytes = decodeBase64(result.png_base64);
  delete result.png_base64;
  return bytes.buffer.byteLength === bytes.length ? bytes.buffer : bytes.slice().buffer;
}
