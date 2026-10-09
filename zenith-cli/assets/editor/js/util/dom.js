// Small DOM helpers: element creation, lookup, and text formatting.

/**
 * Create element `tag` with attributes `attrs` and `children`.
 * `attrs` keys: `class`, `text`, `on<event>` (listeners), `dataset`,
 * boolean values set or remove the attribute, others set it as a string.
 */
export function h(tag, attrs = {}, ...children) {
  const el = document.createElement(tag);
  for (const [key, value] of Object.entries(attrs)) {
    if (value === undefined || value === null || value === false) continue;
    if (key === "class") el.className = value;
    else if (key === "text") el.textContent = value;
    else if (key === "dataset") Object.assign(el.dataset, value);
    else if (key.startsWith("on") && typeof value === "function") {
      el.addEventListener(key.slice(2), value);
    } else if (value === true) el.setAttribute(key, "");
    else el.setAttribute(key, String(value));
  }
  append(el, children);
  return el;
}

function append(el, children) {
  for (const child of children) {
    if (child === undefined || child === null || child === false) continue;
    if (Array.isArray(child)) append(el, child);
    else if (child instanceof Node) el.appendChild(child);
    else el.appendChild(document.createTextNode(String(child)));
  }
}

/** The element with id `id`. Throws when the page has no such element. */
export function byId(id) {
  const el = document.getElementById(id);
  if (!el) throw new Error(`the page has no element #${id}`);
  return el;
}

/** Remove every child of `el`. */
export function clear(el) {
  while (el.firstChild) el.removeChild(el.firstChild);
}

/** `n` with up to `digits` decimals, trailing zeros dropped. */
export function num(n, digits = 2) {
  if (typeof n !== "number" || !Number.isFinite(n)) return String(n);
  return String(Number(n.toFixed(digits)));
}

/** `count` with the singular or plural of `word`. */
export function plural(count, word, many = `${word}s`) {
  return `${count} ${count === 1 ? word : many}`;
}

/** `true` when `event` has the platform command modifier (Cmd or Ctrl). */
export function isMod(event) {
  return navigator.platform.startsWith("Mac") ? event.metaKey : event.ctrlKey;
}

/** `true` when the user asked for reduced motion. */
export function reducedMotion() {
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}
