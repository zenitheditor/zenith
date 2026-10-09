// The canvas gesture checks, run on `gestures.zen` (see run.js --steps).
//
// Every edit step asserts two things: the source diff touches only the
// edited node's lines (before/after text compare), and the canvas shows,
// pixel for pixel, the engine's render of the new source.

import { A, STATE, sleep, check, state, settle, ready, clientOf, agentCommand } from "./helpers.js";
import {
  ALT, CTRL, SHIFT, lineChange, onlyNodes, lineOf, inspect, centerOf, handleOf, drag, clickPage,
  dismissNotices, select, previewShown, landed, canvasMatchesEngine, setSnap,
} from "./canvas_helpers.js";

export const steps = [
  [
    "click selects: outline, 8 grips, rotate grip, hover outline",
    async ({ page, url, shot }) => {
      await page.viewport(1440, 900);
      await page.media("light");
      await page.goto(url);
      await ready(page);
      // Room for the whole 600 px page at 100%: no layers panel, a narrow source pane.
      await page.eval(`(() => { const l = ${A}.layout; if (l.state.left) l.toggle('left'); l.setSplit(0.3); return true; })()`);
      await sleep(200);
      await page.eval(`(() => { ${A}.view.actualSize(); return true; })()`);
      await settle(page);
      check((await state(page)).zoom === 1, "zoom is not 100%");
      const fits = await page.eval(`${A}.view.visible().x <= 0 && ${A}.view.visible().w >= 600 && ${A}.view.visible().h >= 420`);
      check(fits, "the page does not fit the canvas at 100%");
      // Exact pointer deltas: canvas_steps.js covers snapping.
      await setSnap(page, false);
      await select(page, "box");
      const grips = await page.eval("document.querySelectorAll('#overlay .handle.resize').length");
      const rotate = await page.eval("document.querySelectorAll('#overlay .handle.rotate').length");
      check(grips === 8 && rotate === 1, `grips ${grips}, rotate ${rotate}`);
      const stem = await page.eval("document.querySelectorAll('#overlay .stem').length");
      check(stem === 1, `rotate stem ${stem}`);
      const c = await clientOf(page, 280, 80);
      await page.mouse("mouseMoved", c.x, c.y, { button: "none" });
      await page.waitFor("!!document.querySelector('#overlay .hover')", "hover outline on sizer");
      const handle = await handleOf(page, "box", "se");
      const hc = await clientOf(page, handle.x, handle.y);
      await page.mouse("mouseMoved", hc.x, hc.y, { button: "none" });
      await page.waitFor("document.getElementById('viewport').style.cursor === 'nwse-resize'", "resize cursor on the se grip");
      await shot("gesture-selection");
      return { grips, rotate };
    },
  ],
  [
    "drag moves the node with a live preview; undo and redo",
    async ({ page, shot }) => {
      const before = await state(page);
      const from = await centerOf(page, "box");
      await drag(page, from, { x: from.x + 30, y: from.y + 20 }, {
        mid: async () => {
          await previewShown(page);
          await page.waitFor("!document.getElementById('gesture-hint').hidden", "the drag hint");
          const text = await page.eval("document.getElementById('gesture-hint').textContent");
          check(text.includes("Δx 30") && text.includes("Δy 20"), `hint ${text}`);
          check((await state(page)).text === before.text, "the text changed during the drag");
          await shot("gesture-mid-drag");
        },
      });
      await landed(page, before.version);
      const after = await state(page);
      onlyNodes(before.text, after.text, ["box"]);
      check(lineOf(after.text, "box").includes("x=(px)70 y=(px)60"), lineOf(after.text, "box"));
      const compared = await canvasMatchesEngine(page);
      await page.eval("document.getElementById('viewport').focus()");
      await page.key("z", CTRL);
      await page.waitFor(`(${STATE}).text === ${JSON.stringify(before.text)}`, "undo of the move");
      await page.key("Z", CTRL | SHIFT);
      await page.waitFor(`(${STATE}).text === ${JSON.stringify(after.text)}`, "redo of the move");
      await settle(page);
      return { compared };
    },
  ],
  [
    "resize from every corner and the left and top edges",
    async ({ page }) => {
      await select(page, "sizer");
      const cases = [
        ["nw", -10, -10, { x: -10, y: -10, w: 10, h: 10 }],
        ["ne", 10, -10, { x: 0, y: -10, w: 10, h: 10 }],
        ["se", 10, 10, { x: 0, y: 0, w: 10, h: 10 }],
        ["sw", -10, 10, { x: -10, y: 0, w: 10, h: 10 }],
        ["w", -5, 0, { x: -5, y: 0, w: 5, h: 0 }],
        ["n", 0, -5, { x: 0, y: -5, w: 0, h: 5 }],
      ];
      const out = {};
      for (const [grip, dx, dy, want] of cases) {
        const before = await state(page);
        const was = (await inspect(page, "sizer")).edit;
        const h = await handleOf(page, "sizer", grip);
        await drag(page, h, { x: h.x + dx, y: h.y + dy });
        await landed(page, before.version);
        const after = await state(page);
        onlyNodes(before.text, after.text, ["sizer"]);
        const now = (await inspect(page, "sizer")).edit;
        for (const k of ["x", "y", "w", "h"]) {
          check(Math.abs(now[k] - (was[k] + want[k])) < 1e-6, `${grip}: ${k} ${now[k]}, want ${was[k] + want[k]}`);
        }
        out[grip] = lineOf(after.text, "sizer");
      }
      out.compared = await canvasMatchesEngine(page);
      return out;
    },
  ],
  [
    "rotate grip with Shift snaps to 15 degrees",
    async ({ page, shot }) => {
      await select(page, "spin");
      const before = await state(page);
      const h = await handleOf(page, "spin", "rotate");
      const c = await centerOf(page, "spin");
      // From straight above the centre to straight right of it: +90°.
      await drag(page, h, { x: c.x + 100, y: c.y + 3 }, {
        modifiers: SHIFT,
        mid: async () => {
          await previewShown(page);
          await page.waitFor("document.getElementById('gesture-hint').textContent.startsWith('Rotation 90°')", "the rotation hint");
          await shot("gesture-rotate");
        },
      });
      await landed(page, before.version);
      const after = await state(page);
      onlyNodes(before.text, after.text, ["spin"]);
      check(lineOf(after.text, "spin").includes("rotate=(deg)90"), lineOf(after.text, "spin"));
      const compared = await canvasMatchesEngine(page);
      await shot("gesture-rotated");
      return { line: lineOf(after.text, "spin"), compared };
    },
  ],
  [
    "arrow keys nudge 1 px, Shift 10 px",
    async ({ page }) => {
      await select(page, "box");
      const before = await state(page);
      await page.eval("document.getElementById('viewport').focus()");
      for (let i = 0; i < 3; i++) await page.key("ArrowRight");
      await page.key("ArrowDown", SHIFT);
      await page.waitFor(`(${STATE}).text.includes('rect id="box" x=(px)73 y=(px)70')`, "the nudges to land");
      await settle(page);
      const after = await state(page);
      onlyNodes(before.text, after.text, ["box"]);
      check(after.version === before.version + 4, `version ${after.version}, want ${before.version + 4}`);
      const compared = await canvasMatchesEngine(page);
      return { line: lineOf(after.text, "box"), compared };
    },
  ],
  [
    "Escape cancels a drag",
    async ({ page }) => {
      const before = await state(page);
      const from = await centerOf(page, "box");
      await drag(page, from, { x: from.x + 40, y: from.y }, {
        mid: async () => {
          await previewShown(page);
          await page.key("Escape");
          await page.waitFor(`!${A}.renderer.preview && !document.querySelector('#overlay .ghost')`, "the preview to clear");
        },
      });
      await sleep(300);
      const after = await state(page);
      check(after.text === before.text && after.version === before.version, "a cancelled drag changed the text");
      return {};
    },
  ],
  [
    "token-bound axis shows a lock and refuses; the offer and Alt detach",
    async ({ page, shot }) => {
      await select(page, "bound");
      check(await page.eval("!!document.querySelector('#overlay .lock-badge')"), "no lock badge on the token-bound node");
      const before = await state(page);
      const from = await centerOf(page, "bound");
      await drag(page, from, { x: from.x + 20, y: from.y }, {
        mid: async () => {
          await page.waitFor(
            "(() => { const h = document.getElementById('gesture-hint'); return !h.hidden && h.classList.contains('blocked') && h.textContent.includes('token'); })()",
            "the blocked hint",
          );
          check(await page.eval("!!document.querySelector('#overlay .ghost.blocked')"), "no blocked ghost");
          await shot("gesture-rejection-hint");
        },
      });
      await page.waitFor("!!document.querySelector('[data-key=\"error:gesture.commit\"]')", "the rejection notice");
      const notice = await page.eval("document.querySelector('[data-key=\"error:gesture.commit\"]').textContent");
      check(notice.includes("tx.token_bound"), `notice: ${notice}`);
      const buttons = await page.eval("[...document.querySelectorAll('[data-key=\"error:gesture.commit\"] button')].map((b) => b.textContent)");
      check(buttons.includes("Detach from token"), `offer buttons: ${buttons}`);
      await shot("gesture-rejection");
      check((await state(page)).text === before.text, "a refused gesture changed the text");
      await page.eval("[...document.querySelectorAll('[data-key=\"error:gesture.commit\"] button')].find((b) => b.textContent === 'Detach from token').click()");
      await landed(page, before.version);
      const offered = await state(page);
      onlyNodes(before.text, offered.text, ["bound"]);
      check(lineOf(offered.text, "bound").includes("x=(px)320"), lineOf(offered.text, "bound"));
      await page.key("z", CTRL);
      await page.waitFor(`(${STATE}).text === ${JSON.stringify(before.text)}`, "undo of the detach");
      await settle(page);
      // Alt detaches during the drag itself.
      await select(page, "bound");
      const undone = await state(page);
      await drag(page, from, { x: from.x + 20, y: from.y }, { modifiers: ALT });
      await landed(page, undone.version);
      const alt = await state(page);
      onlyNodes(before.text, alt.text, ["bound"]);
      check(lineOf(alt.text, "bound").includes("x=(px)320"), lineOf(alt.text, "bound"));
      check(!(await page.eval("!!document.querySelector('[data-key=\"error:gesture.commit\"]')")), "the rejection notice stayed");
      const compared = await canvasMatchesEngine(page);
      return { compared };
    },
  ],
  [
    "edge-anchored node moves its anchor gap, the cross axis is a note",
    async ({ page }) => {
      const before = await state(page);
      const from = await centerOf(page, "follow");
      await drag(page, from, { x: from.x + 10, y: from.y + 15 });
      await landed(page, before.version);
      const after = await state(page);
      onlyNodes(before.text, after.text, ["follow"]);
      check(lineOf(after.text, "follow").includes("anchor-gap=(px)35"), lineOf(after.text, "follow"));
      check(JSON.stringify(after.sel) === '["follow"]', `selection ${after.sel}`);
      const note = await page.eval("document.querySelector('[data-key=notes]')?.textContent ?? ''");
      check(note.includes("x shift dropped"), `notes notice: ${note}`);
      const compared = await canvasMatchesEngine(page);
      return { compared };
    },
  ],
  [
    "a row child drag offers reorder, the offer reorders",
    async ({ page }) => {
      const before = await state(page);
      const from = await centerOf(page, "cell.a");
      await drag(page, from, { x: from.x + 80, y: from.y });
      await page.waitFor("!!document.querySelector('[data-key=\"error:gesture.commit\"]')", "the layout notice");
      const buttons = await page.eval("[...document.querySelectorAll('[data-key=\"error:gesture.commit\"] button')].map((b) => b.textContent)");
      check(buttons.includes("Reorder in layout") && buttons.includes("Take out of layout"), `offer buttons: ${buttons}`);
      check((await state(page)).text === before.text, "a refused gesture changed the text");
      await page.eval("[...document.querySelectorAll('[data-key=\"error:gesture.commit\"] button')].find((b) => b.textContent === 'Reorder in layout').click()");
      await landed(page, before.version);
      const after = await state(page);
      onlyNodes(before.text, after.text, ["cell.a", "cell.b"]);
      check(after.text.indexOf('id="cell.b"') < after.text.indexOf('id="cell.a"'), "cell.a did not move after cell.b");
      const compared = await canvasMatchesEngine(page);
      return { compared };
    },
  ],
  [
    "a fill-sized resize needs the explicit set-size offer",
    async ({ page }) => {
      await select(page, "grow");
      const before = await state(page);
      // A 20 px tall node shows no edge grips: drag the corner, along x only.
      const e = await handleOf(page, "grow", "se");
      const reason = await page.eval(`${A}.overlay.single().handles.find((h) => h.id === 'se').reason`);
      check(reason === "tx.computed_size", `se grip reason ${reason}`);
      await drag(page, e, { x: e.x + 20, y: e.y });
      await page.waitFor("!!document.querySelector('[data-key=\"error:gesture.commit\"]')", "the computed size notice");
      const buttons = await page.eval("[...document.querySelectorAll('[data-key=\"error:gesture.commit\"] button')].map((b) => b.textContent)");
      check(buttons.includes("Set a fixed size"), `offer buttons: ${buttons}`);
      check((await state(page)).text === before.text, "a refused resize changed the text");
      await page.eval("[...document.querySelectorAll('[data-key=\"error:gesture.commit\"] button')].find((b) => b.textContent === 'Set a fixed size').click()");
      await landed(page, before.version);
      const after = await state(page);
      onlyNodes(before.text, after.text, ["grow"]);
      check(lineOf(after.text, "grow").includes("w=(px)120"), lineOf(after.text, "grow"));
      const compared = await canvasMatchesEngine(page);
      return { compared };
    },
  ],
  [
    "a thin line selects within the click slop; Delete removes it; undo restores",
    async ({ page }) => {
      // 3 px below a 2 px rule: inside the 4 px slop.
      await dismissNotices(page);
      await clickPage(page, { x: 490, y: 333 });
      await page.waitFor(`JSON.stringify(${A}.selectionIds) === '["rule"]'`, "rule selected near its stroke");
      await page.waitFor("document.querySelectorAll('#overlay .handle.endpoint').length === 2", "two endpoint handles");
      const before = await state(page);
      await page.eval("document.getElementById('viewport').focus()");
      await page.key("Delete");
      await landed(page, before.version);
      const after = await state(page);
      const change = onlyNodes(before.text, after.text, ["rule"], { comments: true });
      check(change.added.length === 0 && change.removed.length === 2, `delete changed ${JSON.stringify(change)}`);
      check(after.sel.length === 0, `selection after delete: ${after.sel}`);
      await page.waitFor("!!document.querySelector('[data-key=comments]')", "the removed comments notice");
      const removed = await page.eval("document.querySelector('[data-key=comments] .notice-detail').textContent");
      check(removed === "// A thin rule.", `removed comments: ${JSON.stringify(removed)}`);
      const compared = await canvasMatchesEngine(page);
      await page.eval("document.getElementById('viewport').focus()");
      await page.key("z", CTRL);
      await page.waitFor(`(${STATE}).text === ${JSON.stringify(before.text)}`, "undo of the delete");
      await settle(page);
      return { compared };
    },
  ],
  [
    "a line endpoint drag moves that end only",
    async ({ page }) => {
      await dismissNotices(page);
      await clickPage(page, { x: 490, y: 330 });
      await page.waitFor(`JSON.stringify(${A}.selectionIds) === '["rule"]'`, "rule selected");
      const before = await state(page);
      const end = await handleOf(page, "rule", "end");
      await drag(page, end, { x: end.x, y: end.y - 20 });
      await landed(page, before.version);
      const after = await state(page);
      onlyNodes(before.text, after.text, ["rule"]);
      check(lineOf(after.text, "rule").includes("x2=(px)560 y2=(px)310"), lineOf(after.text, "rule"));
      check(lineOf(after.text, "rule").includes("x1=(px)420 y1=(px)330"), lineOf(after.text, "rule"));
      const compared = await canvasMatchesEngine(page);
      return { compared };
    },
  ],
  [
    "a polygon vertex drag moves that point only",
    async ({ page }) => {
      await select(page, "tri");
      const vertices = await page.eval("document.querySelectorAll('#overlay .handle.vertex').length");
      check(vertices === 3, `vertex handles ${vertices}`);
      const before = await state(page);
      const p0 = await handleOf(page, "tri", "p0");
      await drag(page, p0, { x: p0.x, y: p0.y - 10 });
      await landed(page, before.version);
      const after = await state(page);
      const change = lineChange(before.text, after.text);
      check(
        JSON.stringify(change) === JSON.stringify({ removed: ["        point x=(px)440 y=(px)250"], added: ["        point x=(px)440 y=(px)240"] }),
        `vertex drag changed ${JSON.stringify(change)}`,
      );
      const compared = await canvasMatchesEngine(page);
      return { compared };
    },
  ],
  [
    "Ctrl+D duplicates in place and selects the copy; undo removes it",
    async ({ page }) => {
      await select(page, "box");
      const before = await state(page);
      await page.eval("document.getElementById('viewport').focus()");
      await page.key("d", CTRL);
      await landed(page, before.version);
      const after = await state(page);
      const change = onlyNodes(before.text, after.text, ["box-copy"]);
      check(change.removed.length === 0 && change.added.length === 1, `duplicate changed ${JSON.stringify(change)}`);
      check(JSON.stringify(after.sel) === '["box-copy"]', `selection ${after.sel}`);
      const compared = await canvasMatchesEngine(page);
      await page.key("z", CTRL);
      await page.waitFor(`(${STATE}).text === ${JSON.stringify(before.text)}`, "undo of the duplicate");
      await settle(page);
      return { compared };
    },
  ],
  [
    "a gesture made against a stale version is dropped",
    async ({ page }) => {
      await select(page, "box");
      const before = await state(page);
      const from = await centerOf(page, "box");
      let agent = null;
      await drag(page, from, { x: from.x + 25, y: from.y }, {
        mid: async () => {
          await previewShown(page);
          agent = await agentCommand(page, "node.set", { id: "caption", text: "Agent" });
          check(agent.ok, `agent node.set: ${JSON.stringify(agent.error)}`);
          await page.waitFor(`${A}.sync.version === ${agent.version}`, "the agent edit to reach the pane");
        },
      });
      await page.waitFor("!!document.querySelector('[data-key=gesture-stale]')", "the stale gesture notice");
      const notice = await page.eval("document.querySelector('[data-key=gesture-stale]').textContent");
      check(notice.includes("editor.stale_version"), `notice: ${notice}`);
      await settle(page);
      const after = await state(page);
      onlyNodes(before.text, after.text, ["caption"]);
      check(lineOf(after.text, "box") === lineOf(before.text, "box"), "the stale gesture moved box");
      check(after.version === agent.version, `version ${after.version}, want ${agent.version}`);
      const compared = await canvasMatchesEngine(page);
      await page.eval("document.querySelector('[data-key=gesture-stale] button[title=Dismiss]').click()");
      return { compared };
    },
  ],
  [
    "inspector edits W and fill through node.set",
    async ({ page, shot }) => {
      await select(page, "box");
      await page.waitFor("!!document.querySelector('#inspector input[data-field=w]')", "the W field");
      const before = await state(page);
      await page.eval("(() => { const i = document.querySelector('#inspector input[data-field=w]'); i.focus(); i.select(); return true; })()");
      await page.type("150");
      await page.key("Enter");
      await landed(page, before.version);
      const mid = await state(page);
      onlyNodes(before.text, mid.text, ["box"]);
      check(lineOf(mid.text, "box").includes("w=(px)150"), lineOf(mid.text, "box"));
      await page.waitFor("document.querySelector('#inspector select[data-field=fill]').value === 'color.accent'", "the fill picker");
      await page.eval("(() => { const s = document.querySelector('#inspector select[data-field=fill]'); s.value = 'color.warm'; s.dispatchEvent(new Event('change')); return true; })()");
      await landed(page, mid.version);
      const after = await state(page);
      onlyNodes(mid.text, after.text, ["box"]);
      check(lineOf(after.text, "box").includes('fill=(token)"color.warm"'), lineOf(after.text, "box"));
      const compared = await canvasMatchesEngine(page);
      await shot("gesture-inspector");
      return { compared };
    },
  ],
  [
    "touch: one finger drags the selected node",
    async ({ page }) => {
      await select(page, "spin");
      const before = await state(page);
      await page.send("Emulation.setTouchEmulationEnabled", { enabled: true, maxTouchPoints: 2 });
      const c = await centerOf(page, "spin");
      const a = await clientOf(page, c.x, c.y);
      await page.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ x: a.x, y: a.y }] });
      for (let i = 1; i <= 6; i++) {
        await page.send("Input.dispatchTouchEvent", { type: "touchMove", touchPoints: [{ x: a.x, y: a.y + i * 5 }] });
        await sleep(16);
      }
      await page.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
      await landed(page, before.version);
      await page.send("Emulation.setTouchEmulationEnabled", { enabled: false });
      const after = await state(page);
      onlyNodes(before.text, after.text, ["spin"]);
      check(lineOf(after.text, "spin").includes("y=(px)70"), lineOf(after.text, "spin"));
      const compared = await canvasMatchesEngine(page);
      return { compared };
    },
  ],
  [
    "dark theme selection and handles",
    async ({ page, shot }) => {
      await page.media("dark");
      await select(page, "spin");
      await sleep(200);
      await shot("gesture-selection-dark");
      await select(page, "bound");
      await shot("gesture-lock-dark");
      await page.media("light");
      return {};
    },
  ],
];
