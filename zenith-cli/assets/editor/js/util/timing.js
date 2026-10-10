// Debounce and throttle for high-frequency input.

/**
 * `fn` delayed until `ms` pass with no new call. The result has `cancel()`
 * and `flush()` (run a pending call now).
 */
export function debounce(fn, ms) {
  let timer = null;
  let args = null;
  const run = () => {
    timer = null;
    const a = args;
    args = null;
    fn(...a);
  };
  const wrapped = (...a) => {
    args = a;
    if (timer !== null) clearTimeout(timer);
    timer = setTimeout(run, ms);
  };
  wrapped.cancel = () => {
    if (timer !== null) clearTimeout(timer);
    timer = null;
    args = null;
  };
  wrapped.flush = () => {
    if (timer === null) return;
    clearTimeout(timer);
    run();
  };
  wrapped.pending = () => timer !== null;
  return wrapped;
}

/**
 * `fn` at most once per `ms`. The last call in a window runs at its end,
 * so the final position is never lost. The result has `cancel()` and
 * `pending()` (a call waits to run).
 */
export function throttle(fn, ms) {
  let last = 0;
  let timer = null;
  let args = null;
  const run = () => {
    timer = null;
    last = performance.now();
    const a = args;
    args = null;
    fn(...a);
  };
  const wrapped = (...a) => {
    args = a;
    if (timer !== null) return;
    const wait = Math.max(0, ms - (performance.now() - last));
    timer = setTimeout(run, wait);
  };
  wrapped.cancel = () => {
    if (timer !== null) clearTimeout(timer);
    timer = null;
    args = null;
  };
  wrapped.pending = () => timer !== null;
  return wrapped;
}

/** A promise that resolves after `ms`. */
export function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
