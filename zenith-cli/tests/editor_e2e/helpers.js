// Helpers the e2e step files share: page state, settling, coordinates, and
// the screen-versus-engine image check.

export const A = "window.zenithEditor";
export const STATE = `(() => { const a = ${A}; return {
  version: a.sync.version, synced: a.sync.synced(), text: a.code.text(), sel: a.selectionIds,
  page: a.page, stale: !document.getElementById('stale-badge').hidden,
  errors: a.diagnostics.tally().error, diags: a.diagnosticList,
  sha: a.renderer.shown ? a.renderer.shown.sha256 : null, scale: a.renderer.scale,
  zoom: a.view.zoom, panX: a.view.panX, panY: a.view.panY, dirty: a.dirty(),
  full: document.getElementById('app').dataset.full,
  rendering: !document.getElementById('render-badge').hidden,
}; })()`;

/** The engine summary with the text (`GET /api/state?text=1` on `zenith edit`). */
export const engineState = (page) => page.eval(`${A}.host.state({ text: true })`);

/**
 * Run `command` as another client (an agent): over HTTP on `zenith edit`,
 * through `WasmEngine.run` with a foreign client id on the static host.
 * Resolves the envelope.
 */
export function agentCommand(page, command, params) {
  return page.eval(`(async () => {
    const host = ${A}.host;
    const st = await host.state();
    if (host.kind === 'wasm') {
      return host.run(${JSON.stringify(command)}, ${JSON.stringify(params)}, { version: st.version, client: 'agent-e2e' });
    }
    const res = await fetch('/api/cmd', { method: 'POST',
      headers: { 'Content-Type': 'application/json', 'X-Zenith-Client': 'agent-e2e', ...host.authHeaders() },
      body: JSON.stringify({ command: ${JSON.stringify(command)}, params: ${JSON.stringify(params)}, version: st.version }) });
    return res.json(); })()`);
}

export function check(cond, message) {
  if (!cond) throw new Error(message);
}

export const state = (page) => page.eval(STATE);

/**
 * `true` in the page when nothing is left to run: the pane text reached
 * the engine, no render, refresh, or cursor lookup waits, and no engine
 * call is on the wire.
 */
export const SETTLED = `(() => { const a = ${A}; return a.sync.synced() && !a.sync.pending() && a.renderer.idle()
  && !a.refreshLater.pending() && !a.selection.cursorLater.pending() && a.events.calls === 0; })()`;

/**
 * A snapshot of the page for a failed step: focus, the code cursor, the
 * buffer sync queue, the renderer, the engine calls on the wire, and the
 * last engine calls. Every field reads `null` when the page has no editor.
 */
export const DIAGNOSIS = `(() => {
  const a = window.zenithEditor;
  const el = document.activeElement;
  const out = {
    focus: { page: document.hasFocus(), active: el ? el.tagName.toLowerCase() + (el.id ? '#' + el.id : '') + (el.className && typeof el.className === 'string' ? '.' + el.className.split(/\\s+/).join('.') : '') : null },
  };
  if (!a) return out;
  const s = a.sync;
  const sel = a.code.view.state.selection;
  const text = a.code.text();
  const head = sel.main.head;
  out.code = { focused: a.code.view.hasFocus, head, ranges: sel.ranges.map((r) => [r.anchor, r.head]), length: text.length,
    around: text.slice(Math.max(0, head - 24), head) + '|' + text.slice(head, head + 24) };
  out.sync = { version: s.version, synced: s.synced(), pending: s.pending(), inflight: !!s.inflight, unsent: !s.unsent.empty,
    queued: s.queued.length, waiting: s.waiting, stale: s.stale, running: s.running, held: s.held, typingTimer: s.schedule.pending(),
    baseEqualsPane: s.base === text };
  out.render = { idle: a.renderer.idle(), gen: a.renderer.gen, shownGen: a.renderer.shown ? a.renderer.shown.gen : null, renders: a.renderer.renders };
  out.calls = a.events.calls;
  out.later = { refresh: a.refreshLater.pending(), cursor: a.selection.cursorLater.pending(), cursorWaiting: a.selection.cursorWaiting };
  out.dirty = a.dirty();
  out.selection = a.selectionIds;
  if (typeof a.host.metrics === 'function') out.lastCalls = a.host.metrics().slice(-6);
  return out;
})()`;

/** Wait until the pane text reached the engine and the last render landed. */
export async function settle(page) {
  await page.waitFor(SETTLED, "sync, render, and engine calls to settle");
}

/** Wait until the canvas view took the size of its viewport (after a layout change). */
export async function viewSized(page) {
  await page.waitFor(
    `(() => { const a = ${A}; const r = a.view.viewport.getBoundingClientRect();
      return Math.abs(a.view.vw - r.width) < 1 && Math.abs(a.view.vh - r.height) < 1; })()`,
    "the canvas view to take its new size",
  );
}

/** Wait until `innerWidth` is `width` and the canvas view took its size. */
export async function viewportIs(page, width) {
  await page.waitFor(`innerWidth === ${width}`, `a ${width} px wide window`);
  await viewSized(page);
}

/** Wait for the page to draw the next frame. */
export function nextFrame(page) {
  return page.eval("new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(() => r(true))))");
}

/** Wait until `<body>` has background `color` (a theme switch applied). */
export function bodyBackground(page, color) {
  return page.waitFor(`getComputedStyle(document.body).backgroundColor === ${JSON.stringify(color)}`, `the ${color} background`);
}

export const DARK_BG = "rgb(20, 23, 28)";
export const LIGHT_BG = "rgb(244, 245, 247)";

export async function ready(page) {
  await page.waitFor(
    `document.documentElement.dataset.ready === 'true' && !!${A}.renderer.shown`,
    "the page to load and render",
  );
  await settle(page);
}

/** Client point of page point `(x, y)`. */
export function clientOf(page, x, y) {
  return page.eval(`(() => { const a = ${A}; const r = a.view.viewport.getBoundingClientRect();
    return { x: r.left + a.view.px + ${x} * a.view.zoom, y: r.top + a.view.py + ${y} * a.view.zoom }; })()`);
}

/** The compiled box of node `id` (page px). */
export async function boxOf(page, id) {
  const env = await page.eval(`${A}.engine.run('node.inspect', { id: ${JSON.stringify(id)} })`);
  check(env.ok, `node.inspect ${id}: ${JSON.stringify(env.error)}`);
  return env.result.box;
}

/**
 * `true` in the page when the code pane has focus, its selection is
 * `anchor`..`head`, the DOM selection maps to the same head, and the head is
 * inside the visible part of the pane.
 */
const SELECTED = (anchor, head) => `(() => { const v = ${A}.code.view; const m = v.state.selection.main;
  if (!v.hasFocus || m.anchor !== ${anchor} || m.head !== ${head}) return false;
  const g = getSelection();
  if (!g.focusNode || !v.contentDOM.contains(g.focusNode) || v.posAtDOM(g.focusNode, g.focusOffset) !== ${head}) return false;
  const c = v.coordsAtPos(${head}); const r = v.scrollDOM.getBoundingClientRect();
  return !!c && c.top >= r.top && c.bottom <= r.bottom; })()`;

/**
 * Select `anchor`..`head` (UTF-16 offsets) in the code pane, focus it, and
 * wait until the selection holds on screen and no scroll is due.
 *
 * Typing at a head outside the visible part of the pane makes the browser
 * scroll it into view. That scroll event can run before the mutation
 * observer of the inserted text. CodeMirror then reads the change from its
 * scroll handler with its old DOM selection, and puts the cursor back in
 * front of the typed text. So the head scrolls into view first, and the
 * typing starts one frame after the scroll ended.
 */
export async function selectRange(page, anchor, head) {
  await page.eval(`(() => { const v = ${A}.code.view; v.focus();
    v.dispatch({ selection: { anchor: ${anchor}, head: ${head} }, scrollIntoView: true }); return true; })()`);
  await page.waitFor(SELECTED(anchor, head), `the code selection ${anchor}..${head} with focus, on screen`);
  await nextFrame(page);
  await page.waitFor(SELECTED(anchor, head), `the code selection ${anchor}..${head} to hold after a frame`);
}

/** Put the code cursor at UTF-16 offset `pos` and focus the pane (see `selectRange`). */
export async function cursorAt(page, pos) {
  await selectRange(page, pos, pos);
}

/** Focus the element `selector` matches and wait until it holds the focus. */
export async function focusOn(page, selector) {
  const el = `document.querySelector(${JSON.stringify(selector)})`;
  await page.eval(`(() => { ${el}.focus(); return true; })()`);
  await page.waitFor(`document.hasFocus() && document.activeElement === ${el}`, `the focus on ${selector}`);
}

/** The engine text version the pane last saw. Pass it to `landed` after an action. */
export const versionOf = (page) => page.eval(`${A}.sync.version`);

/**
 * Wait until the text moved past `version` (a keystroke, undo, redo, an
 * engine edit, a reload) and the page settled. Resolves the page state, for
 * the step to check the text.
 */
export async function landed(page, version, what = "the edit to land") {
  await page.waitFor(`${A}.sync.version !== ${version} && ${SETTLED}`, what);
  return state(page);
}

/**
 * Press `key` with `modifiers` (see `Page.key`) for an action that moves
 * the text (undo, redo, a nudge), then `landed`. Resolves the page state.
 */
export async function pressLands(page, key, modifiers, what) {
  const version = await versionOf(page);
  await page.key(key, modifiers);
  return landed(page, version, what);
}

/**
 * Emulate device pixel ratio `dpr` at a `width` × `height` viewport the way
 * browser zoom changes it: together with a resize. From Chrome 154 on, a
 * ratio change alone through `Emulation.setDeviceMetricsOverride` fires no
 * `resize`, no `(resolution)` media query change, and no ResizeObserver
 * entry, so no page can see it. Waits until the view took the new ratio.
 */
export async function setDpr(page, dpr, width = 1440, height = 900) {
  await page.viewport(width + 1, height, false, dpr);
  await page.viewport(width, height, false, dpr);
  await page.waitFor(`devicePixelRatio === ${dpr} && ${A}.view.dpr === ${dpr}`, `DPR ${dpr} in the view`);
}

/** Zoom to `z` about the viewport center; wait for the render at that scale. */
export async function zoomTo(page, z) {
  await page.eval(`(() => { ${A}.view.zoomTo(${z}); return true; })()`);
  await page.waitFor(
    `(() => { const a = ${A}; const s = a.renderer.shown;
      return !!s && Math.abs(s.scale - a.view.zoom * devicePixelRatio) < 1e-9 && a.renderer.idle(); })()`,
    `the render at zoom ${z}`,
  );
  await settle(page);
}

/**
 * Compare the screen with the shown image, pixel for pixel, inside the
 * viewport (inset 10 % to stay clear of floating UI). Also re-sends the
 * image's request to the engine and compares the SHA-256.
 */
export async function sharpness(page) {
  await page.eval(`(() => { const a = ${A}; a.overlay.setSelection([]); a.overlay.setHover(null); return true; })()`);
  await page.mouse("mouseMoved", 1, 1);
  await page.waitFor(
    `!${A}.selection.hoverLater.pending() && ${A}.events.calls === 0 && !${A}.overlay.hover && !document.querySelector('#overlay .hover')`,
    "the hover outline to clear",
  );
  await nextFrame(page);
  const { data } = await page.send("Page.captureScreenshot", { format: "png" });
  return page.eval(`(async () => {
    const a = ${A}; const s = a.renderer.shown; const p = a.paint.last;
    const c = a.paint.canvas; const dpr = devicePixelRatio;
    const box = c.getBoundingClientRect();
    const out = { exact: !!p && p.exact, scale: s.scale, want: a.view.zoom * dpr, dpr,
      backing: [c.width, c.height], css: [box.width, box.height], rect: s.rect, page: s.page_size };
    const again = await a.engine.run('doc.render', s.request);
    out.engineSha = again.ok && again.result.sha256 === s.sha256;
    const im = new Image(); im.src = 'data:image/png;base64,${data}'; await im.decode();
    const shot = new OffscreenCanvas(im.width, im.height).getContext('2d');
    shot.drawImage(im, 0, 0);
    // Transparent page pixels show the white page frame under the canvas.
    const ref = new OffscreenCanvas(s.bitmap.width, s.bitmap.height).getContext('2d');
    ref.fillStyle = '#ffffff';
    ref.fillRect(0, 0, s.bitmap.width, s.bitmap.height);
    ref.drawImage(s.bitmap, 0, 0);
    const vp = a.view.viewport.getBoundingClientRect();
    const vx0 = Math.ceil((vp.left + vp.width * 0.1) * dpr), vy0 = Math.ceil((vp.top + vp.height * 0.1) * dpr);
    const vx1 = Math.floor((vp.right - vp.width * 0.1) * dpr), vy1 = Math.floor((vp.bottom - vp.height * 0.1) * dpr);
    const ox = Math.round(box.left * dpr) + p.x, oy = Math.round(box.top * dpr) + p.y;
    const x0 = Math.max(vx0, ox), y0 = Math.max(vy0, oy);
    const x1 = Math.min(vx1, ox + p.w), y1 = Math.min(vy1, oy + p.h);
    if (x1 <= x0 || y1 <= y0) return { ...out, compared: 0, differ: 0 };
    const got = shot.getImageData(x0, y0, x1 - x0, y1 - y0).data;
    const want = ref.getImageData(x0 - ox, y0 - oy, x1 - x0, y1 - y0).data;
    let differ = 0;
    let where = null;
    const w = x1 - x0;
    for (let i = 0; i < got.length; i += 4) {
      if (got[i] !== want[i] || got[i + 1] !== want[i + 1] || got[i + 2] !== want[i + 2]) {
        differ++;
        const px = x0 + (i / 4) % w, py = y0 + Math.floor(i / 4 / w);
        where = where ? [Math.min(where[0], px), Math.min(where[1], py), Math.max(where[2], px), Math.max(where[3], py)] : [px, py, px, py];
      }
    }
    return { ...out, compared: got.length / 4, differ, where };
  })()`);
}

/** Throw unless `m` (from `sharpness`) shows a 1:1, exact, engine-equal image. */
export function checkSharp(m, what) {
  check(m.exact, `${what}: image not drawn 1:1 (scale ${m.scale}, want ${m.want})`);
  check(Math.abs(m.scale - m.want) < 1e-9, `${what}: render scale ${m.scale}, want ${m.want}`);
  check(
    m.backing[0] === Math.round(m.css[0] * m.dpr) && m.backing[1] === Math.round(m.css[1] * m.dpr),
    `${what}: canvas backing ${m.backing} is not CSS ${m.css} × ${m.dpr}`,
  );
  check(m.engineSha, `${what}: the engine render of the same window has another SHA-256`);
  check(m.compared > 1000, `${what}: only ${m.compared} px compared`);
  check(m.differ === 0, `${what}: ${m.differ} of ${m.compared} screen px differ from the image, in ${m.where}`);
}

/** Lines of `b` that differ from `a` (same line count required). */
export function changedLines(a, b) {
  const la = a.split("\n");
  const lb = b.split("\n");
  check(la.length === lb.length, `line count changed: ${la.length} -> ${lb.length}`);
  return la.flatMap((line, i) => (line === lb[i] ? [] : [i]));
}
