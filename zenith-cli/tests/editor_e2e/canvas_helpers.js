// Canvas helpers the gesture step files share: source diff checks, node
// geometry from the engine, pointer drags in page px, selection, and the
// screen-versus-engine check after an edit.

import { A, check, state, settle, clientOf, sharpness, checkSharp, engineState, nextFrame, viewSized } from "./helpers.js";

/** CDP modifier bits. */
export const ALT = 1;
export const CTRL = 2;
export const SHIFT = 8;

/** `true` in the page when no intent (click, key, undo, save) waits or runs. */
export const INTENTS_IDLE = `${A}.intents.idle()`;

/** The lines removed from `before` and added in `after` (a line diff). */
export function lineChange(before, after) {
  const a = before.split("\n");
  const b = after.split("\n");
  // Common head and tail first, then a longest common subsequence of the
  // rest, so separate edits (two nodes, a node and a token) do not pull the
  // unchanged lines between them into the change.
  let head = 0;
  while (head < a.length && head < b.length && a[head] === b[head]) head++;
  let tail = 0;
  while (tail < a.length - head && tail < b.length - head && a[a.length - 1 - tail] === b[b.length - 1 - tail]) tail++;
  const x = a.slice(head, a.length - tail);
  const y = b.slice(head, b.length - tail);
  const lcs = Array.from({ length: x.length + 1 }, () => new Array(y.length + 1).fill(0));
  for (let i = x.length - 1; i >= 0; i--) {
    for (let j = y.length - 1; j >= 0; j--) {
      lcs[i][j] = x[i] === y[j] ? lcs[i + 1][j + 1] + 1 : Math.max(lcs[i + 1][j], lcs[i][j + 1]);
    }
  }
  const removed = [];
  const added = [];
  let i = 0;
  let j = 0;
  while (i < x.length && j < y.length) {
    if (x[i] === y[j]) {
      i++;
      j++;
    } else if (lcs[i + 1][j] >= lcs[i][j + 1]) {
      removed.push(x[i++]);
    } else {
      added.push(y[j++]);
    }
  }
  removed.push(...x.slice(i));
  added.push(...y.slice(j));
  return { removed, added };
}

/** `text` as the tail of a check message, under a `label` header. */
function source(label, text) {
  return `\n--- ${label} source ---\n${text}`;
}

/** A check message: `what`, the line change from `before` to `after`, and both full sources. */
export function textReport(what, before, after) {
  return `${what}\nchange: ${JSON.stringify(lineChange(before, after))}${source("before", before)}${source("after", after)}`;
}

/**
 * Throw unless the change from `before` to `after` touches only lines that
 * name one of `ids` (and, with `comments`, `//` lines). Returns the change.
 */
export function onlyNodes(before, after, ids, { comments = false } = {}) {
  const change = lineChange(before, after);
  check(change.removed.length + change.added.length > 0, textReport(`the edit of ${ids.join(", ")} did not change the source`, before, after));
  for (const line of [...change.removed, ...change.added]) {
    const named = ids.some((id) => line.includes(`id="${id}"`));
    const comment = comments && line.trim().startsWith("//");
    check(named || comment, textReport(`the edit of ${ids.join(", ")} touched another line: ${JSON.stringify(line)}`, before, after));
  }
  return change;
}

/** The line of node `id` in `text`. */
export function lineOf(text, id) {
  return text.split("\n").find((l) => l.includes(`id="${id}"`)) ?? null;
}

/**
 * Throw unless the line of node `id` in `text` holds every string in
 * `wants` (one string or a list). The message names the line, what it
 * lacks, the change from `before`, and both full sources. Returns the line.
 */
export function lineHas(text, id, wants, before) {
  const line = lineOf(text, id);
  const missing = [wants].flat().filter((w) => !line?.includes(w));
  check(missing.length === 0, textReport(`${id}: want ${JSON.stringify(missing)} in ${JSON.stringify(line)}`, before, text));
  return line;
}

/** Throw unless `actual` is `expected`. The message has the line change and both full sources. */
export function sameText(actual, expected, what) {
  check(actual === expected, textReport(what, expected, actual));
}

/** `node.inspect` of `id`. */
export async function inspect(page, id) {
  const env = await page.eval(`${A}.engine.run('node.inspect', { id: ${JSON.stringify(id)} })`);
  check(env.ok, `node.inspect ${id}: ${JSON.stringify(env.error)}`);
  return env.result;
}

/** The page-px centre of node `id`'s drawn box. */
export async function centerOf(page, id) {
  const c = (await inspect(page, id)).box.corners;
  return { x: (c[0][0] + c[2][0]) / 2, y: (c[0][1] + c[2][1]) / 2 };
}

/** The page-px point of handle `handle` of the one selected node, from the overlay. */
export async function handleOf(page, id, handle) {
  await page.waitFor(
    `(() => { const s = ${A}.overlay.single(); return !!s && s.id === ${JSON.stringify(id)} && !${A}.overlay.ghost; })()`,
    `the overlay of ${id}`,
  );
  const h = await page.eval(`(() => { const h = ${A}.overlay.single().handles.find((x) => x.id === ${JSON.stringify(handle)}); return h ? { x: h.x, y: h.y } : null; })()`);
  check(h, `${id} has no handle ${handle}`);
  return h;
}

/**
 * Press at page point `from`, move to `to` in `steps`, run `mid`, release.
 * The button goes up even when `mid` throws, so a failed step leaves no
 * drag running into the next one.
 */
export async function drag(page, from, to, { steps = 8, modifiers = 0, mid = null } = {}) {
  const a = await clientOf(page, from.x, from.y);
  const b = await clientOf(page, to.x, to.y);
  await page.mouse("mouseMoved", a.x, a.y, { button: "none", modifiers });
  await page.mouse("mousePressed", a.x, a.y, { modifiers });
  try {
    for (let i = 1; i <= steps; i++) {
      const x = a.x + ((b.x - a.x) * i) / steps;
      const y = a.y + ((b.y - a.y) * i) / steps;
      await page.mouse("mouseMoved", x, y, { buttons: 1, modifiers });
      // One move per frame, as a real pointer delivers them.
      await nextFrame(page);
    }
    if (mid) await mid();
  } finally {
    await page.mouse("mouseReleased", b.x, b.y, { modifiers });
  }
}

/** Click page point `p`. */
export async function clickPage(page, p, modifiers = 0) {
  const c = await clientOf(page, p.x, p.y);
  await page.click(c.x, c.y, { modifiers });
}

/** Dismiss every notice, so the canvas keeps its size and place. */
export async function dismissNotices(page) {
  await page.eval("(() => { for (const b of document.querySelectorAll('#banners [title=Dismiss]')) b.click(); return true; })()");
  await page.waitFor("!document.querySelector('#banners [title=Dismiss]')", "the notices to go");
  await viewSized(page);
}

/**
 * Select `id` by a click on its centre (notices dismissed first), unless it
 * is the one selected node. Returns once the overlay outlines `id` and the
 * inspector shows it: the selection draws the outline before `node.inspect`
 * returns, and until then the inspector fields edit the node selected before.
 */
export async function select(page, id) {
  await dismissNotices(page);
  // A click, key, or undo still on its way can change the selection.
  await page.waitFor(INTENTS_IDLE, "the intents before the selection");
  // Already the one selected node: a click could only race the check below
  // (and miss a stroke-only shape whose centre is off its stroke).
  const already = await page.eval(`JSON.stringify(${A}.selectionIds) === ${JSON.stringify(JSON.stringify([id]))}`);
  if (!already) await clickPage(page, await centerOf(page, id));
  await page.waitFor(`${INTENTS_IDLE} && JSON.stringify(${A}.selectionIds) === ${JSON.stringify(JSON.stringify([id]))}`, `${id} selected`);
  await page.waitFor(`(() => { const s = ${A}.overlay.single(); return !!s && s.id === ${JSON.stringify(id)}; })()`, `${id} outlined`);
  await page.waitFor(`${A}.inspector.shownId === ${JSON.stringify(id)}`, `${id} in the inspector`);
}

/** Wait until a live preview image shows. */
export async function previewShown(page) {
  await page.waitFor(`!!${A}.renderer.preview && !!document.querySelector('#overlay .ghost')`, "a live preview and ghost");
}

/**
 * The canvas shows the engine render of the current source: no preview
 * left, the image is of the latest text, the session text is the pane
 * text, and the screen equals the engine render of the same window.
 */
export async function canvasMatchesEngine(page) {
  await page.waitFor(`!${A}.renderer.preview && ${A}.renderer.shown && ${A}.renderer.shown.gen === ${A}.renderer.gen`, "the render of the new text");
  await settle(page);
  const server = await engineState(page);
  const s = await state(page);
  check(server.text === s.text, "the session text differs from the pane text");
  check(server.version === s.version, `session version ${server.version}, pane ${s.version}`);
  const m = await sharpness(page);
  checkSharp(m, "after the gesture");
  // `sharpness` clears the overlay: show the selection again.
  await page.eval(`${A}.selection.refresh('test').then(() => true)`);
  return m.compared;
}


/** Wait until the selection is `ids` and the overlay shows it (with the selection box for several). */
export async function selectedAs(page, ids) {
  const want = JSON.stringify(ids);
  await page.waitFor(`JSON.stringify(${A}.selectionIds) === ${JSON.stringify(want)}`, `selection ${want}`);
  const drawn = ids.length > 1 ? `!!${A}.overlay.group && ${A}.overlay.selection.length === ${ids.length}` : `${A}.overlay.single()?.id === ${JSON.stringify(ids[0])}`;
  if (ids.length) await page.waitFor(`(${drawn}) && !${A}.overlay.ghost`, `the overlay of ${want}`);
  if (ids.length === 1) await page.waitFor(`${A}.inspector.shownId === ${JSON.stringify(ids[0])}`, `${ids[0]} in the inspector`);
}

/**
 * Select `ids`: a click on the first node's centre, Shift+clicks on the
 * rest. After each click it waits for that click's own reply: the
 * selection the clicks so far make, with no intent left. The selection
 * before the clicks can already hold every id (the last step's
 * selection), so a check that only asks for an id passes on stale state.
 */
export async function selectMany(page, ids) {
  await dismissNotices(page);
  await page.waitFor(INTENTS_IDLE, "the intents before the selection");
  for (const [i, id] of ids.entries()) {
    await clickPage(page, await centerOf(page, id), i === 0 ? 0 : SHIFT);
    const want = JSON.stringify(ids.slice(0, i + 1));
    await page.waitFor(`${INTENTS_IDLE} && JSON.stringify(${A}.selectionIds) === ${JSON.stringify(want)}`, `selection ${want}`);
  }
  await selectedAs(page, ids);
}

/** The page-px point of handle `handle` of the selection box. */
export async function groupHandle(page, handle) {
  await page.waitFor(`!!${A}.overlay.group && !${A}.overlay.ghost`, "the selection box");
  const h = await page.eval(`(() => { const h = ${A}.overlay.group.handles.find((x) => x.id === ${JSON.stringify(handle)}); return h ? { x: h.x, y: h.y } : null; })()`);
  check(h, `the selection box has no handle ${handle}`);
  return h;
}

/**
 * Throw unless the change from `before` to `after` touches only lines that
 * name one of the nodes `ids` or declare one of the tokens `tokens`.
 */
export function onlyNodesAndTokens(before, after, ids, tokens) {
  const change = lineChange(before, after);
  check(change.removed.length + change.added.length > 0, textReport(`the edit of ${ids.join(", ")} did not change the source`, before, after));
  for (const line of [...change.removed, ...change.added]) {
    const named = ids.some((id) => line.includes(`id="${id}"`));
    const token = tokens.some((t) => line.trim().startsWith(`token id="${t}"`));
    check(named || token, textReport(`the edit of ${ids.join(", ")} touched another line: ${JSON.stringify(line)}`, before, after));
  }
  return change;
}

/**
 * Throw unless the change from `before` to `after` stays inside the block
 * of node `id` (its line through the matching closing brace). Returns the
 * changed lines.
 */
export function onlyBlock(before, after, id) {
  const block = (text) => {
    const lines = text.split("\n");
    const start = lines.findIndex((l) => l.includes(`id="${id}"`));
    check(start >= 0, `no node ${id}`);
    const indent = lines[start].length - lines[start].trimStart().length;
    let end = start;
    if (lines[start].trimEnd().endsWith("{")) {
      end = lines.findIndex((l, i) => i > start && l.trim() === "}" && l.length - l.trimStart().length === indent);
    }
    return { head: lines.slice(0, start).join("\n"), body: lines.slice(start, end + 1), tail: lines.slice(end + 1).join("\n") };
  };
  const a = block(before);
  const b = block(after);
  check(a.head === b.head && a.tail === b.tail, textReport(`the edit of ${id} touched lines outside its block`, before, after));
  const change = lineChange(a.body.join("\n"), b.body.join("\n"));
  check(change.removed.length + change.added.length > 0, textReport(`the edit of ${id} did not change the source`, before, after));
  return change;
}

/** The drawn corners of `id` from the engine. */
export async function cornersOf(page, id) {
  return (await inspect(page, id)).box.corners;
}

/** Throw unless every corner of `after` is `before` mapped by `map` (page px). */
export function cornersMapped(before, after, map, what, eps = 1e-6) {
  for (let i = 0; i < 4; i++) {
    const [x, y] = map(before[i]);
    check(Math.abs(after[i][0] - x) < eps && Math.abs(after[i][1] - y) < eps, `${what} corner ${i}: ${after[i]} vs ${[x, y]}`);
  }
}

/** Set the canvas Snap toggle to `on` (the toolbar button). */
export async function setSnap(page, on) {
  const now = await page.eval("document.getElementById('snap-toggle').getAttribute('aria-pressed') === 'true'");
  if (now !== on) await page.eval("(() => { document.getElementById('snap-toggle').click(); return true; })()");
  await page.waitFor(`document.getElementById('snap-toggle').getAttribute('aria-pressed') === '${on}' && ${A}.gestures.snapOn === ${on}`, `snap ${on ? "on" : "off"}`);
}
