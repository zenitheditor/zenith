// The selection, marquee, snapping, path point, and inspector checks, run
// on `canvas.zen` (see run.js --steps).
//
// Every edit step asserts two things: the source diff touches only the
// edited nodes' lines (and the tokens an edit creates), and the canvas
// shows, pixel for pixel, the engine's render of the new source.

import { A, STATE, sleep, check, state, ready, settle } from "./helpers.js";
import {
  ALT, CTRL, SHIFT, lineOf, onlyNodes, onlyNodesAndTokens, onlyBlock, centerOf, handleOf, drag,
  dismissNotices, select, selectMany, selectedAs, groupHandle, previewShown, landed,
  canvasMatchesEngine, cornersOf, cornersMapped, setSnap,
} from "./canvas_helpers.js";

/** Drag a marquee band from page point `a` to `b`; `mid` runs before release. */
async function band(page, a, b, { modifiers = 0, mid = null } = {}) {
  await drag(page, a, b, { modifiers, mid });
  await sleep(150);
  await settle(page);
}

/** The selection box `{x0, y0, x1, y1}` of `ids` (page bounds of their drawn boxes). */
async function boundsOf(page, ids) {
  const pts = [];
  for (const id of ids) pts.push(...(await cornersOf(page, id)));
  const xs = pts.map((p) => p[0]);
  const ys = pts.map((p) => p[1]);
  return { x0: Math.min(...xs), y0: Math.min(...ys), x1: Math.max(...xs), y1: Math.max(...ys) };
}

/** Type `text` into the inspector input `field` and press Enter. */
async function typeInto(page, field, text) {
  await page.waitFor(`!!document.querySelector('#inspector [data-field="${field}"]')`, `the ${field} field`);
  await page.eval(`(() => { const i = document.querySelector('#inspector [data-field="${field}"]'); i.focus(); i.select?.(); return true; })()`);
  await page.type(text);
  await page.key("Enter");
}

/** Pick `value` in the inspector select `field`. */
async function pick(page, field, value) {
  await page.waitFor(`!!document.querySelector('#inspector select[data-field="${field}"]')`, `the ${field} picker`);
  await page.eval(`(() => { const s = document.querySelector('#inspector select[data-field="${field}"]'); s.value = ${JSON.stringify(value)}; s.dispatchEvent(new Event('change')); return true; })()`);
}

/** Toggle the inspector checkbox `field`. */
async function toggle(page, field) {
  await page.waitFor(`!!document.querySelector('#inspector input[data-field="${field}"]')`, `the ${field} box`);
  await page.eval(`(() => { document.querySelector('#inspector input[data-field="${field}"]').click(); return true; })()`);
}

/** Run inspector edit `edit` on `id`; check the diff names only `id` (and `tokens`). */
async function inspectorEdit(page, id, edit, tokens = []) {
  await select(page, id);
  const before = await state(page);
  await edit();
  await landed(page, before.version);
  const after = await state(page);
  onlyNodesAndTokens(before.text, after.text, [id], tokens);
  return { before, after, line: lineOf(after.text, id) };
}

export const steps = [
  [
    "setup: 100% zoom, snapping off for exact deltas",
    async ({ page, url }) => {
      await page.viewport(1440, 900);
      await page.media("light");
      await page.goto(url);
      await ready(page);
      await page.eval(`(() => { const l = ${A}.layout; if (l.state.left) l.toggle('left'); l.setSplit(0.3); return true; })()`);
      await sleep(200);
      await page.eval(`(() => { ${A}.view.actualSize(); return true; })()`);
      await settle(page);
      check((await state(page)).zoom === 1, "zoom is not 100%");
      check(await page.eval("document.getElementById('snap-toggle').getAttribute('aria-pressed') === 'true'"), "snapping is not on by default");
      await setSnap(page, false);
      return {};
    },
  ],
  [
    "marquee: an empty drag selects what the band touches; locked nodes never count",
    async ({ page, shot }) => {
      // Band over a and b from the empty top-left corner.
      await band(page, { x: 20, y: 20 }, { x: 200, y: 80 }, {
        mid: async () => {
          await page.waitFor("!!document.querySelector('#overlay .marquee')", "the marquee band");
          await shot("canvas-marquee");
        },
      });
      await selectedAs(page, ["a", "b"]);
      check(!(await page.eval("!!document.querySelector('#overlay .marquee')")), "the band stayed after release");
      const members = await page.eval("document.querySelectorAll('#overlay .selected.member').length");
      const box = await page.eval("document.querySelectorAll('#overlay .group').length");
      const grips = await page.eval("document.querySelectorAll('#overlay .handle.resize').length");
      const rotate = await page.eval("document.querySelectorAll('#overlay .handle.rotate').length");
      check(members === 2 && box === 1 && grips === 8 && rotate === 1, `members ${members}, box ${box}, grips ${grips}, rotate ${rotate}`);
      await shot("canvas-multi-selection");
      // Only the locked node under the band: nothing.
      await band(page, { x: 130, y: 140 }, { x: 176, y: 186 });
      await selectedAs(page, []);
      return { members, grips };
    },
  ],
  [
    "marquee: Alt keeps only nodes wholly inside, Shift adds",
    async ({ page }) => {
      await band(page, { x: 20, y: 20 }, { x: 200, y: 120 }, { modifiers: ALT });
      await selectedAs(page, ["a"]);
      await band(page, { x: 170, y: 125 }, { x: 270, y: 50 }, { modifiers: SHIFT });
      await selectedAs(page, ["a", "b"]);
      return {};
    },
  ],
  [
    "dragging inside the selection moves every node in one transaction; one undo",
    async ({ page, shot }) => {
      const before = await state(page);
      const ca = await cornersOf(page, "a");
      const cb = await cornersOf(page, "b");
      const from = await centerOf(page, "a");
      await drag(page, from, { x: from.x + 30, y: from.y + 20 }, {
        mid: async () => {
          await previewShown(page);
          const ghosts = await page.eval("document.querySelectorAll('#overlay .ghost.member').length");
          check(ghosts === 2, `member ghosts ${ghosts}`);
          check((await state(page)).text === before.text, "the text changed during the drag");
          await shot("canvas-multi-drag");
        },
      });
      await landed(page, before.version);
      const after = await state(page);
      onlyNodes(before.text, after.text, ["a", "b"]);
      check(lineOf(after.text, "a").includes("x=(px)70 y=(px)60"), lineOf(after.text, "a"));
      check(lineOf(after.text, "b").includes("x=(px)210 y=(px)80"), lineOf(after.text, "b"));
      check(after.version === before.version + 1, "one edit, one version");
      cornersMapped(ca, await cornersOf(page, "a"), ([x, y]) => [x + 30, y + 20], "a");
      cornersMapped(cb, await cornersOf(page, "b"), ([x, y]) => [x + 30, y + 20], "b");
      const compared = await canvasMatchesEngine(page);
      await selectedAs(page, ["a", "b"]);
      await page.eval("document.getElementById('viewport').focus()");
      await page.key("z", CTRL);
      await page.waitFor(`(${STATE}).text === ${JSON.stringify(before.text)}`, "one undo of the move");
      await page.key("Z", CTRL | SHIFT);
      await page.waitFor(`(${STATE}).text === ${JSON.stringify(after.text)}`, "redo of the move");
      await settle(page);
      return { compared };
    },
  ],
  [
    "a selection grip resizes every node about the selection box",
    async ({ page }) => {
      await selectMany(page, ["a", "b"]);
      const before = await state(page);
      const box = await boundsOf(page, ["a", "b"]);
      const ca = await cornersOf(page, "a");
      const cb = await cornersOf(page, "b");
      const se = await groupHandle(page, "se");
      await drag(page, se, { x: se.x + 44, y: se.y + 14 });
      await landed(page, before.version);
      const after = await state(page);
      onlyNodes(before.text, after.text, ["a", "b"]);
      const sx = (box.x1 - box.x0 + 44) / (box.x1 - box.x0);
      const sy = (box.y1 - box.y0 + 14) / (box.y1 - box.y0);
      const map = ([x, y]) => [box.x0 + (x - box.x0) * sx, box.y0 + (y - box.y0) * sy];
      cornersMapped(ca, await cornersOf(page, "a"), map, "a", 1e-4);
      cornersMapped(cb, await cornersOf(page, "b"), map, "b", 1e-4);
      const compared = await canvasMatchesEngine(page);
      return { sx, sy, compared };
    },
  ],
  [
    "the selection rotate grip turns every node about the selection centre (Shift: 15°)",
    async ({ page, shot }) => {
      await selectMany(page, ["a", "b"]);
      const before = await state(page);
      const box = await boundsOf(page, ["a", "b"]);
      const c = { x: (box.x0 + box.x1) / 2, y: (box.y0 + box.y1) / 2 };
      const r = await groupHandle(page, "rotate");
      await drag(page, r, { x: c.x + 120, y: c.y + 4 }, {
        modifiers: SHIFT,
        mid: async () => {
          await previewShown(page);
          await shot("canvas-multi-rotate");
        },
      });
      await landed(page, before.version);
      const after = await state(page);
      onlyNodes(before.text, after.text, ["a", "b"]);
      for (const id of ["a", "b"]) check(lineOf(after.text, id).includes("rotate=(deg)90"), lineOf(after.text, id));
      const compared = await canvasMatchesEngine(page);
      await page.eval("document.getElementById('viewport').focus()");
      await page.key("z", CTRL);
      await page.waitFor(`(${STATE}).text === ${JSON.stringify(before.text)}`, "undo of the turn");
      await settle(page);
      return { compared };
    },
  ],
  [
    "keys act on the whole selection: arrows, Ctrl+arrows, brackets, Ctrl+D, Delete",
    async ({ page }) => {
      await selectMany(page, ["a", "b"]);
      const start = await state(page);
      await page.eval("document.getElementById('viewport').focus()");
      await page.key("ArrowRight");
      await page.key("ArrowRight", SHIFT);
      await page.waitFor(`(${STATE}).version === ${start.version + 2}`, "two nudges");
      await settle(page);
      let now = await state(page);
      onlyNodes(start.text, now.text, ["a", "b"]);
      const xa = (t) => Number(/rect id="a" x=\(px\)([0-9.]+)/.exec(t)[1]);
      check(xa(now.text) === xa(start.text) + 11, `a moved ${xa(now.text) - xa(start.text)}`);
      const sized = now;
      await page.key("ArrowDown", CTRL);
      await page.waitFor(`(${STATE}).version === ${sized.version + 1}`, "a selection resize");
      await settle(page);
      now = await state(page);
      onlyNodes(sized.text, now.text, ["a", "b"]);
      const turned = now;
      await page.key("]");
      await page.waitFor(`(${STATE}).version === ${turned.version + 1}`, "a selection turn");
      await settle(page);
      now = await state(page);
      onlyNodes(turned.text, now.text, ["a", "b"]);
      for (const id of ["a", "b"]) check(lineOf(now.text, id).includes("rotate=(deg)15"), lineOf(now.text, id));
      await canvasMatchesEngine(page);
      // Ctrl+D copies both; undo removes the copies.
      await selectMany(page, ["a", "b"]);
      const dup = await state(page);
      await page.eval("document.getElementById('viewport').focus()");
      await page.key("d", CTRL);
      await landed(page, dup.version);
      const copied = await state(page);
      const change = onlyNodes(dup.text, copied.text, ["a-copy", "b-copy"]);
      check(change.removed.length === 0 && change.added.length === 2, `duplicate changed ${JSON.stringify(change)}`);
      check(JSON.stringify(copied.sel) === '["a-copy","b-copy"]', `selection ${copied.sel}`);
      await canvasMatchesEngine(page);
      await page.eval("document.getElementById('viewport').focus()");
      await page.key("z", CTRL);
      await page.waitFor(`(${STATE}).text === ${JSON.stringify(dup.text)}`, "undo of the duplicate");
      await settle(page);
      // Delete removes both (and the comment line above a); undo restores.
      await selectMany(page, ["a", "b"]);
      const del = await state(page);
      await page.eval("document.getElementById('viewport').focus()");
      await page.key("Delete");
      await landed(page, del.version);
      const gone = await state(page);
      const removed = onlyNodes(del.text, gone.text, ["a", "b"], { comments: true });
      check(removed.added.length === 0, `delete added ${removed.added}`);
      await canvasMatchesEngine(page);
      await page.eval("document.getElementById('viewport').focus()");
      await page.key("z", CTRL);
      await page.waitFor(`(${STATE}).text === ${JSON.stringify(del.text)}`, "undo of the delete");
      await settle(page);
      return {};
    },
  ],
  [
    "mixed parents: a node and a turned group's child move by the same page delta",
    async ({ page }) => {
      await selectMany(page, ["tok", "inner"]);
      // tok is token-bound on x: move along y only.
      const before = await state(page);
      const ct = await cornersOf(page, "tok");
      const ci = await cornersOf(page, "inner");
      const from = await centerOf(page, "inner");
      await drag(page, from, { x: from.x, y: from.y + 24 });
      await landed(page, before.version);
      const after = await state(page);
      onlyNodes(before.text, after.text, ["tok", "inner"]);
      cornersMapped(ct, await cornersOf(page, "tok"), ([x, y]) => [x, y + 24], "tok", 1e-6);
      cornersMapped(ci, await cornersOf(page, "inner"), ([x, y]) => [x, y + 24], "inner", 1e-6);
      const compared = await canvasMatchesEngine(page);
      return { inner: lineOf(after.text, "inner"), compared };
    },
  ],
  [
    "a refusing node refuses the selection; its offer goes ahead for every node",
    async ({ page, shot }) => {
      await selectMany(page, ["b", "tok"]);
      const before = await state(page);
      const from = await centerOf(page, "b");
      await drag(page, from, { x: from.x + 20, y: from.y }, {
        mid: async () => {
          await page.waitFor(
            "(() => { const h = document.getElementById('gesture-hint'); return !h.hidden && h.classList.contains('blocked') && h.textContent.includes('token'); })()",
            "the blocked hint",
          );
          await shot("canvas-multi-rejection");
        },
      });
      await page.waitFor("!!document.querySelector('[data-key=\"error:gesture.commit\"]')", "the rejection notice");
      const notice = await page.eval("document.querySelector('[data-key=\"error:gesture.commit\"]').textContent");
      check(notice.includes("tx.token_bound"), `notice: ${notice}`);
      check((await state(page)).text === before.text, "a refused gesture changed the text");
      await page.eval("[...document.querySelectorAll('[data-key=\"error:gesture.commit\"] button')].find((b) => b.textContent === 'Detach from token').click()");
      await landed(page, before.version);
      const after = await state(page);
      onlyNodes(before.text, after.text, ["b", "tok"]);
      check(lineOf(after.text, "tok").includes("x=(px)60"), lineOf(after.text, "tok"));
      const compared = await canvasMatchesEngine(page);
      return { compared };
    },
  ],
  [
    "snapping: a move lands on the page edge and draws a guide; off and Ctrl skip it",
    async ({ page, shot }) => {
      await setSnap(page, true);
      await select(page, "dock");
      const before = await state(page);
      const from = await centerOf(page, "dock");
      // The right edge 560 + 37 = 597 is 3 px from the page edge 600.
      await drag(page, from, { x: from.x + 37, y: from.y }, {
        mid: async () => {
          await previewShown(page);
          await page.waitFor("document.querySelectorAll('#overlay .guide').length > 0", "a snap guide");
          await page.waitFor("document.getElementById('gesture-hint').textContent.includes('snapped')", "the snapped hint");
          await shot("canvas-snap-guides");
        },
      });
      await landed(page, before.version);
      let after = await state(page);
      onlyNodes(before.text, after.text, ["dock"]);
      check(lineOf(after.text, "dock").includes("x=(px)500"), lineOf(after.text, "dock"));
      check(!(await page.eval("!!document.querySelector('#overlay .guide')")), "a guide stayed after release");
      const compared = await canvasMatchesEngine(page);
      await page.eval("document.getElementById('viewport').focus()");
      await page.key("z", CTRL);
      await page.waitFor(`(${STATE}).text === ${JSON.stringify(before.text)}`, "undo of the snapped move");
      await settle(page);
      // Ctrl held during a move: no snap.
      await select(page, "dock");
      await drag(page, from, { x: from.x + 37, y: from.y }, { modifiers: CTRL });
      await landed(page, before.version);
      after = await state(page);
      check(lineOf(after.text, "dock").includes("x=(px)497"), lineOf(after.text, "dock"));
      await page.eval("document.getElementById('viewport').focus()");
      await page.key("z", CTRL);
      await page.waitFor(`(${STATE}).text === ${JSON.stringify(before.text)}`, "undo of the Ctrl move");
      await settle(page);
      // Snapping off: no snap.
      await setSnap(page, false);
      await select(page, "dock");
      await drag(page, from, { x: from.x + 37, y: from.y });
      await landed(page, before.version);
      after = await state(page);
      check(lineOf(after.text, "dock").includes("x=(px)497"), lineOf(after.text, "dock"));
      await canvasMatchesEngine(page);
      return { compared };
    },
  ],
  [
    "path points: an anchor, a control handle with Shift, an anchor with Alt",
    async ({ page, shot }) => {
      await select(page, "wave");
      const anchors = await page.eval("document.querySelectorAll('#overlay .handle.vertex').length");
      const controls = await page.eval("document.querySelectorAll('#overlay .handle.control').length");
      check(anchors === 3 && controls === 4, `anchors ${anchors}, controls ${controls}`);
      await shot("canvas-path-handles");
      const out = {};
      const cases = [
        ["a0.1", 0, -20, 0, "anchor x=(px)460 y=(px)310"],
        ["a0.0.out", 10, 10, SHIFT, "anchor x=(px)360 y=(px)330 out-x=(px)410 out-y=(px)280"],
        ["a0.2", -10, 0, ALT, "anchor x=(px)550 y=(px)330"],
      ];
      for (const [handle, dx, dy, modifiers, want] of cases) {
        const before = await state(page);
        const h = await handleOf(page, "wave", handle);
        await drag(page, h, { x: h.x + dx, y: h.y + dy }, { modifiers });
        await landed(page, before.version);
        const after = await state(page);
        const change = onlyBlock(before.text, after.text, "wave");
        check(change.added.length === 1 && change.added[0].trim().startsWith(want), `${handle}: ${JSON.stringify(change)}`);
        out[handle] = await canvasMatchesEngine(page);
        // The commit keeps the path selected.
        await selectedAs(page, ["wave"]);
      }
      return out;
    },
  ],
  [
    "inspector: raw colors with alpha, tokens, stroke width, and radius",
    async ({ page, shot }) => {
      const fill = await inspectorEdit(page, "a", () => typeInto(page, "fill-value", "#ff000080"), ["color.custom.ff000080"]);
      check(fill.line.includes('fill=(token)"color.custom.ff000080"'), fill.line);
      check(fill.after.text.includes('token id="color.custom.ff000080" type="color" value="#ff000080"'), "no new color token");
      const mint = await inspectorEdit(page, "a", () => pick(page, "fill", "color.mint"));
      check(mint.line.includes('fill=(token)"color.mint"'), mint.line);
      const stroke = await inspectorEdit(page, "a", () => typeInto(page, "stroke-value", "#1e293b"));
      check(stroke.line.includes('stroke=(token)"color.ink"'), `an equal token is reused: ${stroke.line}`);
      const width = await inspectorEdit(page, "a", () => typeInto(page, "stroke_width-value", "3"), ["size.stroke-width.3"]);
      check(width.line.includes('stroke-width=(token)"size.stroke-width.3"'), width.line);
      const radius = await inspectorEdit(page, "a", () => typeInto(page, "radius-value", "8"), ["size.radius.8"]);
      check(radius.line.includes('radius=(token)"size.radius.8"'), radius.line);
      const compared = await canvasMatchesEngine(page);
      await select(page, "a");
      await shot("canvas-inspector-style");
      return { compared };
    },
  ],
  [
    "inspector: font family, size, weight, alignment, span text",
    async ({ page }) => {
      const family = await inspectorEdit(page, "title", () => pick(page, "font_family", "font.serif"));
      check(family.line.includes('font-family=(token)"font.serif"'), family.line);
      const size = await inspectorEdit(page, "title", () => typeInto(page, "font_size-value", "22"), ["size.font-size.22"]);
      check(size.line.includes('font-size=(token)"size.font-size.22"'), size.line);
      const weight = await inspectorEdit(page, "title", () => typeInto(page, "font_weight-value", "700"));
      check(weight.line.includes('font-weight=(token)"weight.bold"'), `an equal token is reused: ${weight.line}`);
      const align = await inspectorEdit(page, "title", () => pick(page, "align", "center"));
      check(align.line.includes('align="center"'), align.line);
      await canvasMatchesEngine(page);
      // Span text of a multi-span text keeps the span's own weight.
      await select(page, "rich");
      const before = await state(page);
      await page.waitFor("!!document.querySelector('#inspector textarea[data-field=\"span-1\"]')", "the span fields");
      await page.eval("(() => { const t = document.querySelector('#inspector textarea[data-field=\"span-1\"]'); t.value = 'strong'; t.dispatchEvent(new Event('change')); return true; })()");
      await landed(page, before.version);
      const after = await state(page);
      const change = onlyBlock(before.text, after.text, "rich");
      check(
        change.added.length === 1 && change.added[0].trim() === 'span "strong" font-weight=(token)"weight.bold"',
        JSON.stringify(change),
      );
      const compared = await canvasMatchesEngine(page);
      return { compared };
    },
  ],
  [
    "inspector: visibility and lock",
    async ({ page }) => {
      const hide = await inspectorEdit(page, "b", () => toggle(page, "visible"));
      check(hide.line.includes("visible=#false"), hide.line);
      await canvasMatchesEngine(page);
      // A hidden node draws no box: select it from the layers or the code.
      await page.eval(`${A}.selection.selectIds(['b'], 'test').then(() => true)`);
      await page.waitFor("!!document.querySelector('#inspector input[data-field=\"visible\"]')", "the visible box");
      const shownBefore = await state(page);
      await toggle(page, "visible");
      await landed(page, shownBefore.version);
      const shown = await state(page);
      onlyNodes(shownBefore.text, shown.text, ["b"]);
      check(lineOf(shown.text, "b").includes("visible=#true"), lineOf(shown.text, "b"));
      const lock = await inspectorEdit(page, "b", () => toggle(page, "locked"));
      check(lock.line.includes("locked=#true"), lock.line);
      // A locked node still unlocks from the inspector.
      await page.eval(`${A}.selection.selectIds(['b'], 'test').then(() => true)`);
      await page.waitFor("!!document.querySelector('#inspector input[data-field=\"locked\"]')", "the locked box");
      const lockedBefore = await state(page);
      await toggle(page, "locked");
      await landed(page, lockedBefore.version);
      const unlocked = await state(page);
      onlyNodes(lockedBefore.text, unlocked.text, ["b"]);
      check(lineOf(unlocked.text, "b").includes("locked=#false"), lineOf(unlocked.text, "b"));
      const compared = await canvasMatchesEngine(page);
      return { compared };
    },
  ],
  [
    "dark theme and phone width: selection box, marquee, guides",
    async ({ page, shot }) => {
      await page.media("dark");
      await selectMany(page, ["a", "b"]);
      await sleep(200);
      await shot("canvas-multi-selection-dark");
      await dismissNotices(page);
      await drag(page, { x: 20, y: 205 }, { x: 330, y: 300 }, {
        mid: async () => {
          await page.waitFor("!!document.querySelector('#overlay .marquee')", "the marquee band");
          await shot("canvas-marquee-dark");
        },
      });
      await settle(page);
      await setSnap(page, true);
      await select(page, "dock");
      const from = await centerOf(page, "dock");
      const before = await state(page);
      await drag(page, from, { x: from.x - 20, y: from.y + 1 }, {
        mid: async () => {
          await previewShown(page);
          await page.waitFor("document.querySelectorAll('#overlay .guide').length > 0", "a snap guide");
          await shot("canvas-snap-guides-dark");
        },
      });
      await landed(page, before.version);
      await canvasMatchesEngine(page);
      await page.media("light");
      await page.viewport(390, 844);
      await sleep(400);
      await page.eval(`(() => { ${A}.view.fit(); return true; })()`);
      await settle(page);
      await page.eval(`${A}.selection.selectIds(['title', 'rich'], 'test').then(() => true)`);
      await selectedAs(page, ["title", "rich"]);
      await sleep(300);
      await page.waitFor(`!${A}.renderer.preview && ${A}.renderer.idle()`, "the phone render");
      await shot("canvas-phone");
      const overflow = await page.eval("document.documentElement.scrollWidth > window.innerWidth");
      check(!overflow, "the page scrolls sideways at phone width");
      await page.viewport(1440, 900);
      await setSnap(page, false);
      return {};
    },
  ],
];
