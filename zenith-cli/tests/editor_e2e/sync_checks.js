// Buffer sync checks the page suite and the static suite share: an engine
// edit while the user types, a reconnect while a keystroke is on the wire,
// and edits of a CRLF document. Each check compares the pane text with the
// engine text after every edit.

import { A, check, cursorAt, engineState, landed, settle, state, versionOf } from "./helpers.js";

/**
 * Hold the reply of the next page call of `command` until
 * `release(page, command)`. The engine runs the command at once; only the
 * page sees the reply late, as on a slow link.
 */
export async function holdNext(page, command) {
  await page.eval(`(() => {
    const host = ${A}.host;
    if (!window.__holds) {
      window.__holds = new Map();
      const run = host.run;
      host.run = async function (cmd, ...rest) {
        const hold = window.__holds.get(cmd);
        if (!hold) return run.call(this, cmd, ...rest);
        window.__holds.delete(cmd);
        const env = await run.call(this, cmd, ...rest);
        hold.arrived = true;
        await hold.gate;
        return env;
      };
    }
    const hold = { arrived: false };
    hold.gate = new Promise((r) => (hold.open = r));
    window.__holds.set(${JSON.stringify(command)}, hold);
    window.__held = window.__held || {};
    window.__held[${JSON.stringify(command)}] = hold;
    return true;
  })()`);
}

/** Wait until the held call of `command` got its reply. */
export function heldArrived(page, command) {
  return page.waitFor(`!!window.__held[${JSON.stringify(command)}].arrived`, `the ${command} reply`);
}

/** Let the held reply of `command` through (now, or when it comes). */
export function release(page, command) {
  return page.eval(`(() => { const h = window.__held[${JSON.stringify(command)}]; window.__holds.delete(${JSON.stringify(command)}); h.open(); return true; })()`);
}

/** Where `a` and `b` first differ, with context, for a check message. */
function firstDiff(a, b) {
  let i = 0;
  while (i < a.length && a.charCodeAt(i) === b.charCodeAt(i)) i++;
  return { at: i, pane: a.slice(Math.max(0, i - 20), i + 20), engine: b.slice(Math.max(0, i - 20), i + 20) };
}

/** Settle, then require pane text == engine text. Returns the text. */
export async function paneIsEngine(page, what) {
  await settle(page);
  const s = await state(page);
  const e = await engineState(page);
  check(s.text === e.text, `${what}: the pane differs from the engine ${JSON.stringify(firstDiff(s.text, e.text))}`);
  check(s.version === e.version, `${what}: page version ${s.version}, engine ${e.version}`);
  return s.text;
}

/**
 * Run engine edit `trigger` (page JS) and hold its `command` reply. While it
 * is on the wire, type `typed` after `anchor` and let the typing send start
 * (held too, so it can come back after the edit). Then let both through:
 * the pane, the sync base, and the engine must hold one text with both
 * changes in it. Returns that text.
 */
export async function editWhileTyping(page, { command, trigger, anchor, typed }) {
  await settle(page);
  await holdNext(page, command);
  await holdNext(page, "commands.batch");
  await page.eval(`(() => { ${trigger}; return true; })()`);
  await heldArrived(page, command);
  const text = await page.eval(`${A}.code.text()`);
  const at = text.indexOf(anchor);
  check(at >= 0, `anchor ${JSON.stringify(anchor)} not in the text`);
  await cursorAt(page, at + anchor.length);
  await page.type(typed);
  await page.waitFor(`!${A}.sync.schedule.pending()`, "the typing send to start");
  check(!(await page.eval(`${A}.sync.synced()`)), `synced while ${command} is on the wire`);
  await release(page, command);
  await release(page, "commands.batch");
  await page.waitFor(`${A}.sync.synced() && !${A}.sync.pending()`, `${command} and the typing to land`);
  const out = await paneIsEngine(page, `${command} while typing`);
  check(out.includes(anchor + typed), `the typing is not in the text after ${command}`);
  check((await page.eval(`${A}.sync.base`)) === out, "the sync base differs from the pane");
  return out;
}

/**
 * A reconnect (the event stream opens again) and a probe after an offline
 * call, both while a keystroke is on the wire, with the first state read
 * failing. Both resync and refresh: no page error, the offline notice
 * clears, and the pane equals the engine.
 */
export async function reconnectWhileSending(page, { anchor, typed }) {
  await settle(page);
  await holdNext(page, "commands.batch");
  const text = await page.eval(`${A}.code.text()`);
  await cursorAt(page, text.indexOf(anchor) + anchor.length);
  await page.type(typed);
  await heldArrived(page, "commands.batch");
  await page.eval(`(() => {
    const a = ${A};
    const host = a.host;
    const state = host.state;
    let fail = 1;
    host.state = function (...args) {
      if (fail-- > 0) return Promise.reject(Object.assign(new Error('state read failed on purpose'), { code: 'edit.unreachable' }));
      return state.apply(this, args);
    };
    a.events.open();
    window.__probe = null;
    a.events.probe().then(() => (window.__probe = 'done'), (e) => (window.__probe = 'error: ' + e.message));
    return true;
  })()`);
  await release(page, "commands.batch");
  const result = await page.waitFor("window.__probe", "the probe to finish");
  check(result === "done", `probe: ${result}`);
  await page.waitFor("!document.querySelector('[data-key=offline]')", "the offline notice to clear");
  await page.waitFor(`${A}.sync.synced() && !${A}.refreshLater.pending()`, "the resync and the refresh");
  return paneIsEngine(page, "after the reconnect");
}

/**
 * Edits of a CRLF document: select `id`, duplicate it, undo, redo, and
 * type in the first comment. After each one the pane equals the engine.
 * Returns the text.
 */
export async function crlfEdits(page, id) {
  const before = await paneIsEngine(page, "the CRLF document");
  check(before.includes("\r\n"), "the document has no CRLF");
  await page.eval(`${A}.selection.selectIds([${JSON.stringify(id)}], 'layers')`);
  const v0 = await page.eval(`${A}.sync.version`);
  await page.eval(`(() => { ${A}.actions.duplicate(); return true; })()`);
  await page.waitFor(`${A}.sync.version !== ${v0} && ${A}.sync.synced()`, "node.duplicate");
  const duplicated = await paneIsEngine(page, "node.duplicate");
  check(duplicated !== before, "node.duplicate changed nothing");
  await page.eval(`${A}.history('history.undo')`);
  const undone = await paneIsEngine(page, "history.undo");
  check(undone === before, "undo did not restore the CRLF text");
  await page.eval(`${A}.history('history.redo')`);
  const redone = await paneIsEngine(page, "history.redo");
  check(redone === duplicated, "redo did not restore the duplicate");
  const at = redone.indexOf("// ") + 3;
  check(at > 3, "no comment to type in");
  await cursorAt(page, at);
  const v1 = await versionOf(page);
  await page.type("crlf ");
  const landedText = (await landed(page, v1, "the typing to land")).text;
  check(landedText === redone.slice(0, at) + "crlf " + redone.slice(at), "the typing did not land at the cursor");
  const typed = await paneIsEngine(page, "typing");
  check(typed.split("\r").length === redone.split("\r").length, "typing changed the \\r count");
  return typed;
}
