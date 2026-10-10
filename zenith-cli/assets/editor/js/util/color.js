// CSS color values the page paints into a swatch.
//
// A token value comes from the document, so it is untrusted text. Only a
// value that matches the full grammar below reaches the CSSOM, and callers
// set it with `style.setProperty`, never through a `style` attribute string.

const NUM = String.raw`[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?`;
const PCT = `${NUM}%`;
const NUM_OR_PCT = `${NUM}%?`;
const HUE = `${NUM}(?:deg|grad|rad|turn)?`;
const S = String.raw`\s*`;

const HEX = /^#(?:[0-9a-fA-F]{3}|[0-9a-fA-F]{4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/;
const RGB_COMMA = new RegExp(
  `^rgba?\\(${S}${NUM_OR_PCT}${S},${S}${NUM_OR_PCT}${S},${S}${NUM_OR_PCT}${S}(?:,${S}${NUM_OR_PCT}${S})?\\)$`,
);
const RGB_SPACE = new RegExp(
  `^rgba?\\(${S}${NUM_OR_PCT}\\s+${NUM_OR_PCT}\\s+${NUM_OR_PCT}${S}(?:/${S}${NUM_OR_PCT}${S})?\\)$`,
);
const HSL_COMMA = new RegExp(
  `^hsla?\\(${S}${HUE}${S},${S}${PCT}${S},${S}${PCT}${S}(?:,${S}${NUM_OR_PCT}${S})?\\)$`,
);
const HSL_SPACE = new RegExp(
  `^hsla?\\(${S}${HUE}\\s+${PCT}\\s+${PCT}${S}(?:/${S}${NUM_OR_PCT}${S})?\\)$`,
);

/**
 * `value` when it is a CSS hex, `rgb()`/`rgba()`, or `hsl()`/`hsla()` color
 * with nothing before or after it, else `null`.
 */
export function cssColor(value) {
  if (typeof value !== "string" || value.length > 128) return null;
  const v = value.trim();
  if (HEX.test(v) || RGB_COMMA.test(v) || RGB_SPACE.test(v) || HSL_COMMA.test(v) || HSL_SPACE.test(v)) return v;
  return null;
}
