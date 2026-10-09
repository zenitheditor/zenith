// Offsets between JavaScript strings (UTF-16 code units, the positions
// CodeMirror uses) and UTF-8 byte offsets (the positions the engine uses).
//
// A byte offset inside a character maps to the code units of the
// characters that end at or before it, as the engine's own conversion does.
// `OffsetIndex` keeps a checkpoint every 1024 code units, so one lookup on
// a large text scans at most one chunk.

const CHUNK = 1024;

/** UTF-8 byte length of the code unit at `i` of `text`, and its step. */
function unitBytes(text, i) {
  const c = text.charCodeAt(i);
  if (c < 0x80) return [1, 1];
  if (c < 0x800) return [2, 1];
  if (c >= 0xd800 && c <= 0xdbff && i + 1 < text.length) {
    const d = text.charCodeAt(i + 1);
    if (d >= 0xdc00 && d <= 0xdfff) return [4, 2];
  }
  return [3, 1];
}

/** UTF-8 byte length of `text`. */
export function utf8Length(text) {
  let bytes = 0;
  for (let i = 0; i < text.length; ) {
    const [b, step] = unitBytes(text, i);
    bytes += b;
    i += step;
  }
  return bytes;
}

/** Byte offset of UTF-16 offset `u` in `text` (clamped to the text). */
export function utf16ToByte(text, u) {
  return new OffsetIndex(text).toByte(u);
}

/** UTF-16 offset of byte offset `b` in `text` (clamped to the text). */
export function byteToUtf16(text, b) {
  return new OffsetIndex(text).toUtf16(b);
}

/** Checkpointed offset conversion over one fixed text. */
export class OffsetIndex {
  constructor(text) {
    this.text = text;
    // units[k] and bytes[k]: the position of checkpoint k. A checkpoint
    // never splits a surrogate pair.
    this.units = [0];
    this.bytes = [0];
    let bytes = 0;
    let next = CHUNK;
    for (let i = 0; i < text.length; ) {
      if (i >= next) {
        this.units.push(i);
        this.bytes.push(bytes);
        next = i + CHUNK;
      }
      const [b, step] = unitBytes(text, i);
      bytes += b;
      i += step;
    }
    this.byteLength = bytes;
  }

  /** Byte offset of UTF-16 offset `u`. */
  toByte(u) {
    const target = Math.max(0, Math.min(u, this.text.length));
    let k = upperBound(this.units, target) - 1;
    let i = this.units[k];
    let bytes = this.bytes[k];
    while (i < target) {
      const [b, step] = unitBytes(this.text, i);
      if (i + step > target) break;
      bytes += b;
      i += step;
    }
    return bytes;
  }

  /** UTF-16 offset of byte offset `b`. */
  toUtf16(b) {
    const target = Math.max(0, Math.min(b, this.byteLength));
    let k = upperBound(this.bytes, target) - 1;
    let i = this.units[k];
    let bytes = this.bytes[k];
    while (i < this.text.length) {
      const [n, step] = unitBytes(this.text, i);
      if (bytes + n > target) break;
      bytes += n;
      i += step;
    }
    return i;
  }
}

/** Index of the first entry of sorted `list` greater than `value`. */
function upperBound(list, value) {
  let lo = 0;
  let hi = list.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (list[mid] <= value) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

/**
 * The single replaced range between `before` and `after`: shared prefix
 * and suffix removed. `{from, to, insert}` in UTF-16 units of `before`, or
 * `null` when the texts are equal.
 */
export function diffRange(before, after) {
  if (before === after) return null;
  let start = 0;
  const max = Math.min(before.length, after.length);
  while (start < max && before.charCodeAt(start) === after.charCodeAt(start)) start++;
  let endB = before.length;
  let endA = after.length;
  while (endB > start && endA > start && before.charCodeAt(endB - 1) === after.charCodeAt(endA - 1)) {
    endB--;
    endA--;
  }
  // Keep surrogate pairs whole.
  if (start > 0 && isLow(before.charCodeAt(start))) start--;
  if (endB < before.length && isLow(before.charCodeAt(endB))) {
    endB++;
    endA++;
  }
  return { from: start, to: endB, insert: after.slice(start, endA) };
}

function isLow(c) {
  return c >= 0xdc00 && c <= 0xdfff;
}
