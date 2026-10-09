// The checks of the editor e2e run, in order. Each step gets the shared
// context from `run.js` and throws on a failed check.

import {
  A,
  STATE,
  sleep,
  check,
  state,
  settle,
  ready,
  clientOf,
  boxOf,
  cursorAt,
  zoomTo,
  sharpness,
  checkSharp,
  changedLines,
  engineState,
  agentCommand,
} from "./helpers.js";

export const steps = [
  [
    "load and first render",
    async ({ page, url, shot }) => {
      await page.viewport(1440, 900);
      await page.media("light");
      await page.goto(url);
      await ready(page);
      const s = await state(page);
      check(s.sha, "no render shown");
      check(s.errors === 0, `unexpected errors: ${JSON.stringify(s.diags)}`);
      const addr = await page.eval("location.href");
      check(!addr.includes("token"), `token still in the address bar: ${addr}`);
      const layers = await page.eval("document.querySelectorAll('#layers [role=treeitem]').length");
      check(layers >= 8, `layers panel shows ${layers} items`);
      await shot("light");
      return { version: s.version, layers };
    },
  ],
  [
    "syntax error shows stale badge and diagnostic, fix updates preview",
    async ({ page, shot }) => {
      const before = await state(page);
      const anchor = before.text.indexOf('rect id="accent"');
      check(anchor > 0, "accent rect not found");
      const at = anchor + 'rect id="accent"'.length;
      const line = before.text.slice(0, at).split("\n").length;
      await cursorAt(page, at);
      await page.type(' w=(px)"');
      await page.waitFor(`(${STATE}).errors > 0`, "an error diagnostic");
      await settle(page);
      const bad = await state(page);
      check(bad.stale, "stale badge hidden while the text has errors");
      const err = bad.diags.find((d) => d.severity === "error");
      check(err && err.line === line, `error at line ${err && err.line}, want ${line}: ${JSON.stringify(err)}`);
      const rows = await page.eval("document.querySelectorAll('#diag-list .sev-error').length");
      check(rows > 0, "diagnostics panel lists no error");
      const marks = await page.eval("document.querySelectorAll('.cm-lintRange-error').length");
      check(marks > 0, "no error underline in the source");
      check(bad.sha === before.sha, "preview changed while the text has errors");
      await shot("error-stale");
      for (let i = 0; i < ' w=(px)"'.length; i++) await page.key("Backspace");
      await page.waitFor(`(${STATE}).errors === 0`, "errors to clear");
      await settle(page);
      const fixed = await state(page);
      check(!fixed.stale, "stale badge still shown after the fix");
      check(fixed.text === before.text, "text differs after the fix");
      // A visible edit renders a new preview.
      const title = fixed.text.indexOf("Featured Projects") + "Featured ".length;
      await page.eval(`(() => { const v = ${A}.code.view; v.dispatch({ selection: { anchor: ${title}, head: ${title + "Projects".length} } }); v.focus(); return true; })()`);
      await page.type("Work");
      await page.waitFor(`(${STATE}).sha !== ${JSON.stringify(fixed.sha)}`, "a new preview");
      await settle(page);
      const edited = await state(page);
      check(edited.text.includes("Featured Work"), "edit did not land");
      return { errorLine: err.line, errorCol: err.col, code: err.code };
    },
  ],
  [
    "undo and redo go through the engine",
    async ({ page }) => {
      const before = await state(page);
      await page.key("z", 2);
      await page.waitFor(`(${STATE}).version !== ${before.version}`, "undo");
      await settle(page);
      const undone = await state(page);
      check(undone.text !== before.text, "undo changed nothing");
      await page.key("Z", 2 | 8);
      await page.waitFor(`(${STATE}).text.includes('Featured Work')`, "redo");
      await settle(page);
      const redone = await state(page);
      check(redone.text === before.text, "redo did not restore the text");
      return { undoVersion: undone.version, redoVersion: redone.version };
    },
  ],
  [
    "canvas click selects, inspector and source highlight follow",
    async ({ page, shot }) => {
      const b = await boxOf(page, "card.b");
      const p = await clientOf(page, b.x + 6, b.y + 6);
      await page.click(p.x, p.y);
      await page.waitFor(`JSON.stringify(${A}.selectionIds) === '["card.b"]'`, "card.b selected");
      await page.waitFor(
        "document.querySelector('.inspector-id') && document.querySelector('.inspector-id').textContent === 'card.b'",
        "inspector shows card.b",
      );
      await page.waitFor(
        "[...document.querySelectorAll('.cm-zen-node')].map(e => e.textContent).join('').includes('rect id=\"card.b\"')",
        "source highlight on card.b",
      );
      // The scrolled-to lines carry syntax highlighting.
      await page.waitFor(
        "(() => { const l = [...document.querySelectorAll('.cm-line')].find((e) => e.textContent.includes('rect id=\"card.b\"')); return !!l && l.querySelectorAll('span[class^=\"ͼ\"]').length > 10; })()",
        "highlighted source lines at the selection",
        3000,
      );
      const outlines = await page.eval("document.querySelectorAll('#overlay .selected').length");
      check(outlines === 1, `overlay shows ${outlines} selection outlines`);
      const handles = await page.eval("document.querySelectorAll('#overlay .handle').length");
      check(handles >= 4, `overlay shows ${handles} handles`);
      const attrs = await page.eval("document.querySelectorAll('#inspector .kv dt').length");
      check(attrs >= 5, `inspector lists ${attrs} attributes`);
      const chip = await page.eval("!!document.querySelector('#inspector .chip')");
      check(chip, "inspector shows no token binding");
      await shot("selected");
      return { handles, attrs };
    },
  ],
  [
    "code cursor selects the node on the canvas",
    async ({ page }) => {
      const s = await state(page);
      const pos = s.text.indexOf('text id="label.c"') + 6;
      const c = await page.eval(`(async () => { const v = ${A}.code.view;
        v.dispatch({ effects: v.constructor.scrollIntoView(${pos}, { y: 'center' }) });
        await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
        const r = v.coordsAtPos(${pos}); return { x: r.left + 1, y: (r.top + r.bottom) / 2 }; })()`);
      await page.click(c.x, c.y);
      await page.waitFor(`JSON.stringify(${A}.selectionIds) === '["label.c"]'`, "label.c selected from the cursor");
      await page.waitFor("document.querySelectorAll('#overlay .selected').length === 1", "overlay outline");
      await page.waitFor("document.querySelector('.inspector-id').textContent === 'label.c'", "inspector shows label.c");
      const inspector = "label.c";
      // Multi-byte text before the cursor: the byte offset must still land.
      const dash = s.text.indexOf("Zenith —") + "Zenith —".length;
      await page.eval(`(() => { const v = ${A}.code.view; v.dispatch({ selection: { anchor: ${dash} }, userEvent: 'select' }); return true; })()`);
      await page.waitFor(`JSON.stringify(${A}.selectionIds) === '["label.a"]'`, "label.a selected past a multi-byte dash");
      return { selected: inspector };
    },
  ],
  [
    "layers panel selects",
    async ({ page }) => {
      await page.clickSelector('#layers [data-id="title"] > .row');
      await page.waitFor(`JSON.stringify(${A}.selectionIds) === '["title"]'`, "title selected from layers");
      await page.waitFor(
        "[...document.querySelectorAll('.cm-zen-node')].map(e => e.textContent).join('').includes('text id=\"title\"')",
        "source highlight on title",
      );
      const selected = await page.eval("document.querySelector('#layers [aria-selected=true]').dataset.id");
      check(selected === "title", `layers marks ${selected}`);
      // Keyboard: Down moves focus, Enter selects.
      await page.key("ArrowDown");
      await page.key("Enter");
      await page.waitFor(`JSON.stringify(${A}.selectionIds) === '["accent"]'`, "accent selected by keyboard");
      return { selected };
    },
  ],
  [
    "pages panel switches the page",
    async ({ page }) => {
      const s = await state(page);
      const at = s.text.lastIndexOf("  }\n}");
      const extra =
        '    page id="page.two" w=(px)200 h=(px)100 {\n' +
        '      rect id="r2" x=(px)10 y=(px)10 w=(px)50 h=(px)50 fill=(token)"color.accent"\n' +
        "    }\n";
      await page.eval(`(() => { ${A}.code.view.dispatch({ changes: { from: ${at}, insert: ${JSON.stringify(extra)} }, userEvent: "input" }); return true; })()`);
      await page.waitFor("document.querySelectorAll('#pages .row').length === 2", "two pages listed");
      await settle(page);
      await page.clickSelector("#pages li:nth-child(2) .row");
      await page.waitFor(`${A}.page === 2 && ${A}.renderer.shown && ${A}.renderer.shown.page === 2`, "page 2 rendered");
      await page.waitFor("!!document.querySelector('#layers [data-id=\"r2\"]')", "page 2 layers");
      const current = await page.eval("document.querySelector('#pages [aria-current=page] .row-label').textContent");
      check(current === "page.two", `current page row ${current}`);
      const w = await page.eval(`${A}.view.pageW`);
      check(Math.abs(w - 200) < 1, `page 2 width ${w}`);
      await page.clickSelector("#pages li:nth-child(1) .row");
      await page.waitFor(`${A}.page === 1 && ${A}.renderer.shown.page === 1`, "page 1 rendered");
      await page.eval(`(() => { ${A}.code.view.dispatch({ changes: { from: ${at}, to: ${at + extra.length} }, userEvent: "delete" }); return true; })()`);
      await page.waitFor("document.querySelectorAll('#pages .row').length === 1", "back to one page");
      await settle(page);
      check((await state(page)).text === s.text, "text differs after removing the page");
      return { pages: 2 };
    },
  ],
  [
    "divider drag and keyboard resize",
    async ({ page }) => {
      const split = () => page.eval("parseFloat(getComputedStyle(document.getElementById('center')).getPropertyValue('--split'))");
      const s0 = await split();
      const d = await page.center("#divider");
      const center = await page.center("#center");
      await page.drag({ x: d.x, y: d.y }, { x: d.x - 200, y: d.y });
      const s1 = await split();
      const want = s0 - 200 / center.w;
      check(Math.abs(s1 - want) < 0.02, `drag split ${s1}, want about ${want}`);
      await page.eval("document.getElementById('divider').focus()");
      await page.key("ArrowRight");
      await page.key("ArrowRight");
      const s2 = await split();
      check(Math.abs(s2 - (s1 + 0.04)) < 0.005, `keyboard split ${s2}, want ${s1 + 0.04}`);
      const now = await page.eval("document.getElementById('divider').getAttribute('aria-valuenow')");
      check(Number(now) === Math.round(s2 * 100), `aria-valuenow ${now}`);
      await page.key("Enter");
      const s3 = await split();
      check(Math.abs(s3 - 0.5) < 1e-9, `Enter reset split to ${s3}`);
      return { drag: s1, keys: s2 };
    },
  ],
  [
    "fullscreen code and canvas, Esc restores",
    async ({ page, shot }) => {
      await page.clickSelector("#full-code");
      await page.waitFor("document.getElementById('app').dataset.full === 'code'", "fullscreen code");
      check(await page.eval("document.getElementById('pane-canvas').offsetParent === null"), "canvas still shown");
      await shot("full-code");
      await page.key("Escape");
      await page.waitFor("document.getElementById('app').dataset.full === 'none'", "Esc restores from code");
      await page.clickSelector("#full-canvas");
      await page.waitFor("document.getElementById('app').dataset.full === 'canvas'", "fullscreen canvas");
      check(await page.eval("document.getElementById('pane-code').offsetParent === null"), "code still shown");
      await settle(page);
      await shot("full-canvas");
      await page.key("Escape");
      await page.waitFor("document.getElementById('app').dataset.full === 'none'", "Esc restores from canvas");
      await page.clickSelector("#full-canvas");
      await page.clickSelector("#full-canvas");
      await page.waitFor("document.getElementById('app').dataset.full === 'none'", "button restores");
      return {};
    },
  ],
  [
    "collapsed panels persist across reload",
    async ({ page }) => {
      for (const id of ["toggle-left", "toggle-right", "toggle-bottom"]) await page.clickSelector(`#${id}`);
      const attrs = "(() => { const d = document.getElementById('app').dataset; return [d.left, d.right, d.bottom].join(); })()";
      check((await page.eval(attrs)) === "closed,closed,closed", "panels did not collapse");
      await page.reload();
      await ready(page);
      check((await page.eval(attrs)) === "closed,closed,closed", "collapse state lost on reload");
      for (const id of ["toggle-left", "toggle-right", "toggle-bottom"]) await page.clickSelector(`#${id}`);
      check((await page.eval(attrs)) === "open,open,open", "panels did not reopen");
      await settle(page);
      return {};
    },
  ],
  [
    "zoom, pan, fit, 100%, deep zoom",
    async ({ page, shot }) => {
      await page.clickSelector("#zoom-fit");
      const fit = await state(page);
      await page.clickSelector("#zoom-in");
      const z1 = await state(page);
      check(Math.abs(z1.zoom - fit.zoom * 1.25) < 1e-6, `zoom-in ${z1.zoom} from ${fit.zoom}`);
      const v = await page.center("#viewport");
      await page.wheel(v.x, v.y, 0, -200, 2);
      const z2 = await state(page);
      check(z2.zoom > z1.zoom, `ctrl+wheel did not zoom in (${z2.zoom})`);
      await page.wheel(v.x, v.y, 30, 60, 0);
      const p1 = await state(page);
      check(Math.abs(p1.panX - (z2.panX - 30)) < 1e-6 && Math.abs(p1.panY - (z2.panY - 60)) < 1e-6, "wheel did not pan");
      await page.drag({ x: v.x, y: v.y }, { x: v.x + 50, y: v.y + 40 }, 6, { button: "middle" });
      const p2 = await state(page);
      check(Math.abs(p2.panX - p1.panX - 50) < 1 && Math.abs(p2.panY - p1.panY - 40) < 1, "middle drag did not pan");
      await page.eval("document.getElementById('viewport').focus()");
      await page.key("0", 0, "Digit0");
      const actual = await state(page);
      check(actual.zoom === 1, `0 key zoom ${actual.zoom}`);
      await page.key("1", 0, "Digit1");
      const refit = await state(page);
      check(Math.abs(refit.zoom - fit.zoom) < 1e-6, `1 key fit ${refit.zoom}, want ${fit.zoom}`);
      // Past the old 4x render cap: the render follows the zoom exactly.
      for (let i = 0; i < 14; i++) await page.clickSelector("#zoom-in");
      await page.waitFor(
        `(() => { const a = ${A}; return !!a.renderer.shown && Math.abs(a.renderer.scale - a.view.zoom) < 1e-9; })()`,
        "a render at the zoom",
      );
      await settle(page);
      const deep = await state(page);
      check(deep.zoom > 4 && Math.abs(deep.scale - deep.zoom) < 1e-9, `deep zoom ${deep.zoom} rendered at ${deep.scale}`);
      check(!deep.rendering, "Rendering still shown after the render");
      await shot("deep-zoom");
      await page.clickSelector("#zoom-fit");
      await settle(page);
      return { fit: fit.zoom, max: deep.zoom };
    },
  ],
  [
    "viewport render is 1:1 and engine-exact at 81%, 100%, 300%",
    async ({ page, shot }) => {
      const out = {};
      for (const z of [0.81, 1, 3]) {
        await zoomTo(page, z);
        const m = await sharpness(page);
        checkSharp(m, `zoom ${z}`);
        out[z] = { rect: m.rect, compared: m.compared };
        await shot(`zoom-${Math.round(z * 100)}`);
      }
      return out;
    },
  ],
  [
    "a pan past the margin renders a new window",
    async ({ page, shot }) => {
      const before = await page.eval(`${A}.renderer.shown.rect`);
      const renders = await page.eval(`${A}.renderer.renders`);
      const vw = await page.eval(`${A}.view.vw`);
      // Inside the margin: no render.
      await page.eval(`(() => { ${A}.view.panBy(-${Math.round(vw * 0.2)}, 0); return true; })()`);
      await sleep(150);
      check(await page.eval(`${A}.renderer.idle()`), "a pan inside the margin started a render");
      check((await page.eval(`${A}.renderer.renders`)) === renders, "a pan inside the margin rendered");
      // Past it: a new window.
      await page.eval(`(() => { ${A}.view.panBy(-${Math.round(vw * 0.9)}, -40); return true; })()`);
      await page.waitFor(`${A}.renderer.renders > ${renders}`, "a render of the new window");
      await settle(page);
      const m = await sharpness(page);
      checkSharp(m, "after the pan");
      await shot("zoom-300-panned");
      return { before, after: m.rect };
    },
  ],
  [
    "device pixel ratio 2 renders at twice the scale",
    async ({ page, shot }) => {
      await page.viewport(1440, 900, false, 2);
      await page.waitFor(`devicePixelRatio === 2`, "DPR 2");
      await page.waitFor(
        `(() => { const a = ${A}; const s = a.renderer.shown; return !!s && Math.abs(s.scale - a.view.zoom * 2) < 1e-9 && a.renderer.idle(); })()`,
        "a render at zoom × 2",
      );
      await settle(page);
      const m = await sharpness(page);
      checkSharp(m, "DPR 2");
      await shot("dpr2-zoom-300");
      await zoomTo(page, 0.81);
      const m81 = await sharpness(page);
      checkSharp(m81, "DPR 2 at 81%");
      await shot("dpr2-zoom-81");
      await page.viewport(1440, 900);
      await page.waitFor(`devicePixelRatio === 1`, "DPR 1");
      await page.clickSelector("#zoom-fit");
      await settle(page);
      return { scale: m.scale, rect: m.rect, scale81: m81.scale };
    },
  ],
  [
    "extreme zoom on a 20000 px page renders only the window",
    async ({ page, shot }) => {
      const s = await state(page);
      const at = s.text.lastIndexOf("  }\n}");
      const extra =
        '    page id="page.big" w=(px)20000 h=(px)12000 {\n' +
        '      ellipse id="big.dot" x=(px)9000 y=(px)5000 w=(px)2000 h=(px)2000 fill=(token)"color.accent"\n' +
        "    }\n";
      await page.eval(`(() => { ${A}.code.view.dispatch({ changes: { from: ${at}, insert: ${JSON.stringify(extra)} }, userEvent: "input" }); return true; })()`);
      await page.waitFor("document.querySelectorAll('#pages .row').length === 2", "two pages listed");
      await settle(page);
      await page.clickSelector("#pages li:nth-child(2) .row");
      await page.waitFor(`${A}.page === 2 && ${A}.renderer.shown && ${A}.renderer.shown.page === 2`, "page 2 rendered");
      await settle(page);
      await page.eval(`(() => { const v = ${A}.view; v.zoomTo(8); v.panBy(v.vw / 2 - (v.panX + 9000 * 8), v.vh / 2 - (v.panY + 6000 * 8)); return true; })()`);
      await page.waitFor(
        `(() => { const a = ${A}; const s = a.renderer.shown; return !!s && s.page === 2 && Math.abs(s.scale - 8) < 1e-9 && a.renderer.idle(); })()`,
        "a render at 800%",
      );
      await settle(page);
      const m = await sharpness(page);
      checkSharp(m, "800% on the large page");
      const vw = await page.eval(`${A}.view.vw`);
      check(m.page.w === 20000 && m.page.h === 12000, `page size ${JSON.stringify(m.page)}`);
      check(m.rect.w <= Math.ceil(vw * 2) + 2, `window ${m.rect.w} px wide for a ${vw} px viewport`);
      await shot("large-page-800");
      await page.clickSelector("#pages li:nth-child(1) .row");
      await page.waitFor(`${A}.page === 1 && ${A}.renderer.shown.page === 1`, "page 1 rendered");
      await page.eval(`(() => { ${A}.code.view.dispatch({ changes: { from: ${at}, to: ${at + extra.length} }, userEvent: "delete" }); return true; })()`);
      await page.waitFor("document.querySelectorAll('#pages .row').length === 1", "back to one page");
      await settle(page);
      check((await state(page)).text === s.text, "text differs after removing the page");
      return { rect: m.rect };
    },
  ],
  [
    "save keeps comments outside the edited node",
    async ({ page, original, readDoc }) => {
      await cursorAt(page, 0);
      await page.key("s", 2);
      await page.waitFor(`!${A}.dirty()`, "save to finish");
      await settle(page);
      const disk = readDoc();
      const s = await state(page);
      check(disk === s.text, "disk text differs from the editor text");
      const lines = original.split("\n");
      const changed = changedLines(original, disk);
      for (const i of changed) {
        // The title edit, and the doc-id stamp of a first save on the root node.
        const ok = lines[i].includes("Featured Projects") || /^zenith version=/.test(lines[i]);
        check(ok, `save changed line ${i + 1}: ${JSON.stringify(lines[i])} -> ${JSON.stringify(disk.split("\n")[i])}`);
      }
      const comments = lines.filter((l) => l.trim().startsWith("//"));
      for (const c of comments) check(disk.includes(c), `comment lost: ${c}`);
      return { changedLines: changed.map((i) => i + 1), comments: comments.length };
    },
    { host: "server" },
  ],
  [
    "disk change on a clean buffer reloads",
    async ({ page, readDoc, writeDoc }) => {
      const before = await state(page);
      const next = readDoc().replace("// Card B — below card A.", "// Card B sits below card A.");
      check(next !== readDoc(), "fixture comment not found");
      writeDoc(next);
      await page.waitFor(`(${STATE}).text.includes('Card B sits below')`, "external change to reach the pane");
      await settle(page);
      const s = await state(page);
      check(s.text === next, "pane text differs from the disk text");
      check(s.version > before.version, "version did not move");
      check(!s.dirty, "reload left the buffer dirty");
      return { version: s.version };
    },
    { host: "server" },
  ],
  [
    "an agent edit from another client reaches the pane",
    async ({ page }) => {
      const before = await state(page);
      const env = await agentCommand(page, "node.reorder", { id: "label.c", to: "backward" });
      check(env.ok, `agent node.reorder: ${JSON.stringify(env.error)}`);
      await page.waitFor(`${A}.sync.version === ${env.version}`, "the agent change to reach the pane");
      await settle(page);
      const server = await engineState(page);
      const s = await state(page);
      check(s.text === server.text, "pane text differs from the session text");
      check(s.text !== before.text, "the agent edit changed nothing");
      await page.key("z", 2);
      await page.waitFor(`(${STATE}).text === ${JSON.stringify(before.text)}`, "undo of the agent edit");
      await settle(page);
      return { version: env.version };
    },
  ],
  [
    "conflict banner: Reload takes the disk text",
    async ({ page, readDoc, writeDoc, shot }) => {
      const s = await state(page);
      const at = s.text.indexOf("// Card C") + 2;
      await cursorAt(page, at);
      await page.type(" local");
      await settle(page);
      check((await state(page)).dirty, "typing did not make the buffer dirty");
      const disk = readDoc().replace("// Card C", "// Disk C");
      writeDoc(disk);
      await page.waitFor("!!document.querySelector('[data-key=disk].error')", "conflict banner");
      await shot("conflict");
      const reload = await page.eval("[...document.querySelectorAll('[data-key=disk] button')].map(b => b.textContent)");
      check(reload.includes("Reload") && reload.includes("Overwrite"), `banner buttons: ${reload}`);
      await page.eval("[...document.querySelectorAll('[data-key=disk] button')].find(b => b.textContent === 'Reload').click()");
      await page.waitFor("!document.querySelector('[data-key=disk]')", "banner to clear");
      await settle(page);
      const after = await state(page);
      check(after.text === disk, "pane text is not the disk text after Reload");
      check(!after.dirty, "still dirty after Reload");
      return {};
    },
    { host: "server" },
  ],
  [
    "conflict banner: Overwrite writes the editor text",
    async ({ page, readDoc, writeDoc }) => {
      const s = await state(page);
      const at = s.text.indexOf("// Disk C") + 2;
      await cursorAt(page, at);
      await page.type(" mine");
      await settle(page);
      writeDoc(readDoc().replace("// Disk C", "// Theirs C"));
      await page.waitFor("!!document.querySelector('[data-key=disk].error')", "conflict banner");
      await page.eval("[...document.querySelectorAll('[data-key=disk] button')].find(b => b.textContent === 'Overwrite').click()");
      await page.waitFor("!document.querySelector('[data-key=disk]')", "banner to clear");
      await page.waitFor(`!${A}.dirty()`, "save to finish");
      await settle(page);
      const after = await state(page);
      check(readDoc() === after.text, "disk text is not the editor text after Overwrite");
      check(after.text.includes("// mine Disk C"), "editor text lost the local edit");
      return {};
    },
    { host: "server" },
  ],
  [
    "dark theme renders",
    async ({ page, shot }) => {
      await page.media("dark");
      await sleep(200);
      const bg = await page.eval("getComputedStyle(document.body).backgroundColor");
      check(bg === "rgb(20, 23, 28)", `dark background is ${bg}`);
      await shot("dark");
      await page.media("light");
      await sleep(100);
      const light = await page.eval("getComputedStyle(document.body).backgroundColor");
      check(light === "rgb(244, 245, 247)", `light background is ${light}`);
      return { dark: bg, light };
    },
  ],
  [
    "phone width stacks with no horizontal scroll",
    async ({ page, shot }) => {
      await page.viewport(390, 844, true);
      await sleep(400);
      const m = await page.eval(`(() => ({
        doc: document.documentElement.scrollWidth, body: document.body.scrollWidth,
        ws: document.getElementById('workspace').scrollWidth, inner: innerWidth,
        orient: document.getElementById('divider').getAttribute('aria-orientation'),
      }))()`);
      check(m.doc <= m.inner && m.body <= m.inner && m.ws <= m.inner, `horizontal overflow: ${JSON.stringify(m)}`);
      check(m.orient === "horizontal", `divider orientation ${m.orient}`);
      await settle(page);
      await shot("phone");
      await page.viewport(1440, 900);
      await sleep(300);
      return m;
    },
  ],
  [
    "network loss shows a notice, retry recovers",
    async ({ page, shot }) => {
      await page.send("Network.enable");
      await page.send("Network.emulateNetworkConditions", {
        offline: true,
        latency: 0,
        downloadThroughput: -1,
        uploadThroughput: -1,
      });
      await cursorAt(page, 0);
      await page.key("s", 2);
      await page.waitFor("!!document.querySelector('[data-key=offline]')", "offline notice");
      await page.waitFor("!!document.querySelector('[data-key=\"error:file.save\"]')", "save error notice");
      await shot("offline");
      await page.send("Network.emulateNetworkConditions", {
        offline: false,
        latency: 0,
        downloadThroughput: -1,
        uploadThroughput: -1,
      });
      await page.waitFor("!document.querySelector('[data-key=offline]')", "the offline notice to clear", 20000);
      // The failed requests logged network errors on purpose.
      page.errors.length = 0;
      await page.eval("[...document.querySelectorAll('[data-key=\"error:file.save\"] button')].find((b) => b.textContent === 'Retry').click()");
      await page.waitFor(`!document.querySelector('[data-key=offline]') && !${A}.dirty()`, "save after recovery", 20000);
      return {};
    },
    { host: "server" },
  ],
  [
    "server shutdown shows the stopped notice",
    async ({ page, shot }) => {
      await settle(page);
      const r = await page.eval(`fetch('/api/shutdown', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: '{"force":true}' }).then((x) => x.json())`);
      check(r.ok, `shutdown: ${JSON.stringify(r)}`);
      await page.waitFor("!!document.querySelector('[data-key=shutdown]')", "stopped notice");
      // Requests in flight when the server stopped log refused connections.
      await sleep(500);
      const unexpected = page.errors.filter((e) => !/^network: .*ERR_CONNECTION_REFUSED/.test(e));
      page.errors.length = 0;
      page.errors.push(...unexpected);
      await shot("stopped");
      return {};
    },
    { host: "server" },
  ],
];
