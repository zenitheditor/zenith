// Step scoring over a run. Pages are tracked by the page ids of each actual render:
// Zenith `page id`, HTML `<section class="page" id>`. The start pages take keys
// p1..pN by position. A new id takes the insert key of the step that adds it.
// An agent can swap two pages by swapping their content and keeping the ids. So a
// page that matches a different previous page pixel for pixel, and not the page
// its id names, takes that page's key, and its id moves to that key.
// Frozen regions and elements address pages by key, so a wrong page operation in
// one step does not shift the pages that later steps are scored against.
//
// A check's `page` is a position in the oracle's page order after its step
// (`nextOrder`). The scorer maps it to a key, then to a position in the actual
// render. A step with a page operation reads the position the operation must
// produce from the actual previous order. Any other step reads the page by id.
//
// A step that does not land stays pending. Its touch and mask join the intended
// set of every later step, so a deferred edit is not collateral. Each later step
// runs the pending step's checks again; when they pass, the step counts as landed
// late and its `relocate` moves the element boxes then.
import { collateral, frozenDrift, nextOrder, relocate, runCheck } from "./score.mjs";
import { decode } from "./png.mjs";

/** Decoded pages of a render, by 1-based position or by page key through `keys`. */
export function view(doc, keys) {
  const cache = new Map();
  const byPos = (n) => {
    if (!doc.pages || n < 1 || n > doc.pages.length) return null;
    if (!cache.has(n)) cache.set(n, decode(doc.pages[n - 1]));
    return cache.get(n);
  };
  const image = (key) => (keys.includes(key) ? byPos(keys.indexOf(key) + 1) : null);
  return { byPos, has: (key) => keys.includes(key), image, count: doc.pages?.length ?? 0 };
}

const usable = (ids, n) => Array.isArray(ids) && ids.length === n && ids.every((id) => typeof id === "string" && id) && new Set(ids).size === n;

/**
 * Page keys of a render. `idKey` maps page id to key and grows with new ids.
 * Without a full set of distinct ids, the oracle order `fallback` is the answer.
 */
export function pageKeys(doc, idKey, op, fallback) {
  const n = doc.pages?.length ?? 0;
  if (!usable(doc.ids, n)) return fallback;
  const taken = new Set(idKey.values());
  return doc.ids.map((id) => {
    if (!idKey.has(id)) idKey.set(id, op?.insert && !taken.has(op.key) ? op.key : `new:${id}`);
    taken.add(idKey.get(id));
    return idKey.get(id);
  });
}

// A page is the same as another when at most 0.2% of its pixels differ.
function samePage(a, b) {
  if (!a || !b || a.width !== b.width || a.height !== b.height) return false;
  const limit = a.width * a.height * 0.002;
  let diff = 0;
  for (let i = 0; i < a.data.length; i += 4) {
    if (Math.abs(a.data[i] - b.data[i]) > 2 || Math.abs(a.data[i + 1] - b.data[i + 1]) > 2 || Math.abs(a.data[i + 2] - b.data[i + 2]) > 2) {
      if (++diff > limit) return false;
    }
  }
  return true;
}

/**
 * Moves keys from ids to content: a page unlike its id's previous page that is
 * the same as another previous page takes that page's key. A claim on a key whose
 * id page still matches, or a key claimed twice, is dropped.
 */
export function claimByContent(keys, ids, idKey, after, prev, prevKeys) {
  const owns = keys.map((k, n) => samePage(after.byPos(n + 1), prev.image(k)));
  const kept = new Set(keys.filter((k, n) => owns[n]));
  const claims = keys.map((k, n) => (owns[n] ? null : (prevKeys.find((j) => j !== k && !kept.has(j) && samePage(after.byPos(n + 1), prev.image(j))) ?? null)));
  const out = keys.map((k, n) => {
    const c = claims[n];
    return c && claims.filter((x) => x === c).length === 1 ? c : k;
  });
  if (new Set(out).size !== out.length) return keys;
  out.forEach((k, n) => {
    if (ids && k !== keys[n]) idKey.set(ids[n], k);
  });
  return out;
}

/** The key order a page operation must produce from the actual order `prev`; `oracle` names its positions. */
export function intendedOrder(prev, oracle, op) {
  const out = [...prev];
  if (!op) return out;
  let frame = [...oracle];
  if (op.insert) {
    const before = frame[op.insert - 2];
    out.splice(before === undefined ? 0 : out.indexOf(before) + 1, 0, op.key);
    frame = nextOrder(frame, { insert: op.insert, key: op.key });
  }
  if (op.delete) {
    const at = out.indexOf(frame[op.delete - 1]);
    if (at >= 0) out.splice(at, 1);
    frame = nextOrder(frame, { delete: op.delete });
  }
  if (op.swap) {
    const [a, b] = op.swap.map((n) => out.indexOf(frame[n - 1]));
    if (a >= 0 && b >= 0) [out[a], out[b]] = [out[b], out[a]];
  }
  return out;
}

const IMAGELESS = new Set(["source-contains", "source-absent", "source-count", "page-count"]);
const REF_KINDS = new Set(["ink-ratio", "same-as", "changed"]);

/**
 * A copy of `check` with page numbers moved from oracle positions to positions of
 * the actual renders. `frame` holds the oracle orders (`after`, `prev`) and the
 * actual orders to read from (`afterKeys`, `prevKeys`). A missing page becomes -1.
 */
export function placeCheck(check, frame) {
  if (IMAGELESS.has(check.kind)) return check;
  const at = (oracle, actual, n) => actual.indexOf(oracle[n - 1]) + 1 || -1;
  const out = { ...check };
  if (check.page !== undefined || check.kind !== "no-clip") out.page = at(frame.after, frame.afterKeys, check.page ?? 1);
  if (REF_KINDS.has(check.kind)) {
    const ref = check.ref ?? (check.kind === "ink-ratio" ? "start" : "prev");
    const n = check.ref_page ?? check.page ?? 1;
    out.ref = ref;
    out.ref_page = ref === "prev" ? at(frame.prev, frame.prevKeys, n) : n;
  }
  return out;
}

const runChecks = (checks, frame, ctx) => checks.map((c) => ({ ...runCheck(placeCheck(c, frame), ctx), kind: c.kind }));

/** Scores the steps of one run in order. `startDoc` is { pages, rects, ids }. */
export function makeScorer(task, startDoc) {
  const idKey = new Map();
  let expect = Array.from({ length: startDoc.pages.length }, (_, i) => `p${i + 1}`);
  if (usable(startDoc.ids, expect.length)) startDoc.ids.forEach((id, i) => idKey.set(id, expect[i]));
  const start = view(startDoc, expect);
  let prevKeys = expect;
  let prevDoc = startDoc;
  let elements = task.elements ?? [];
  const pending = [];

  /** Scoring fields of one step. `afterDoc` is { pages, rects, ids } or { error }. */
  function score(step, index, afterDoc, source) {
    const ok = !afterDoc.error;
    const expectPrev = expect;
    const expectAfter = nextOrder(expectPrev, step.pages);
    const prev = view(prevDoc, prevKeys);
    const idKeys = ok ? pageKeys(afterDoc, idKey, step.pages, expectAfter) : [];
    const afterKeys = ok ? claimByContent(idKeys, usable(afterDoc.ids, idKeys.length) ? afterDoc.ids : null, idKey, view(afterDoc, idKeys), prev, prevKeys) : [];
    const after = view(ok ? afterDoc : {}, afterKeys);
    const ctx = { after, prev, start, rects: afterDoc.rects ?? [], source, background: task.background };
    const readKeys = step.pages ? intendedOrder(prevKeys, expectPrev, step.pages) : afterKeys;
    const checks = runChecks(step.checks, { after: expectAfter, afterKeys: readKeys, prev: expectPrev, prevKeys }, ctx);
    const drift = frozenDrift(task, step, prev, after);
    const intended = { touch: [...(step.touch ?? []), ...pending.flatMap((p) => p.step.touch ?? [])], mask: [...(step.mask ?? []), ...pending.flatMap((p) => p.step.mask ?? [])] };
    const changed = ok ? collateral(task, intended, prev, after, elements) : elements.map((e) => e.id);
    const landed = checks.every((c) => c.ok);
    const late = [];
    if (ok) {
      for (const p of [...pending]) {
        const again = runChecks(p.step.checks, { after: p.expectAfter, afterKeys, prev: p.expectPrev, prevKeys: p.prevKeys }, { ...ctx, prev: p.prev });
        if (again.every((c) => c.ok)) {
          late.push(p.index + 1);
          pending.splice(pending.indexOf(p), 1);
        }
      }
    }
    if (landed) elements = relocate(elements, step);
    for (const n of late) elements = relocate(elements, task.steps[n - 1]);
    if (!landed && !step.pages) pending.push({ step, index, expectAfter, expectPrev, prevKeys, prev });
    expect = expectAfter;
    if (ok) {
      prevDoc = afterDoc;
      prevKeys = afterKeys;
    }
    return {
      landed,
      landed_late: late,
      rendered: ok,
      drifted: drift.some((d) => d.drift > 0),
      collateral: changed.length,
      collateral_ids: changed,
      page_keys: afterKeys,
      checks,
      drift,
    };
  }
  return { score };
}
