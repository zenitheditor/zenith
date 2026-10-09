// Pure helpers of the engine Worker: the request text and base64 decoding.
// They live apart from `worker.js` so Node tests can run them.

/** The bytes of base64 text `b64`. */
export function decodeBase64(b64) {
  if (typeof Uint8Array.fromBase64 === "function") return Uint8Array.fromBase64(b64);
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

/**
 * The request text for `method`: `params` as JSON, plus the project's
 * `files` and `fonts` (already JSON text) when `project` is set. The
 * project text is spliced in, never parsed or copied by value.
 */
export function requestText(method, params, project, filesJson, fontsJson) {
  const head = JSON.stringify(params ?? {});
  const body = project
    ? `${head.slice(0, -1)}${head === "{}" ? "" : ","}"files":${filesJson},"fonts":${fontsJson}}`
    : head;
  return `{"method":${JSON.stringify(method)},"params":${body}}`;
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
