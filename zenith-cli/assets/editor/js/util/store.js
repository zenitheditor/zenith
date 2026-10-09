// Per-viewer preferences in localStorage. Storage can be missing or throw
// (private windows, blocked site data): every access falls back to the
// default and the page works without it.

const PREFIX = "zenith-edit.";

/** The stored JSON value of `key`, or `fallback`. */
export function load(key, fallback) {
  try {
    const raw = window.localStorage.getItem(PREFIX + key);
    if (raw === null) return fallback;
    const value = JSON.parse(raw);
    return value === null || typeof value !== typeof fallback ? fallback : value;
  } catch {
    return fallback;
  }
}

/** Store `value` as JSON under `key`. Returns `false` when storage fails. */
export function save(key, value) {
  try {
    window.localStorage.setItem(PREFIX + key, JSON.stringify(value));
    return true;
  } catch {
    return false;
  }
}
