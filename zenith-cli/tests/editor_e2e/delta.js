// Apply a `{from, to, insert}` delta (UTF-16 offsets) to a string.
export function applyDelta(text, delta) {
  if (!delta) return text;
  return text.slice(0, delta.from) + delta.insert + text.slice(delta.to);
}
