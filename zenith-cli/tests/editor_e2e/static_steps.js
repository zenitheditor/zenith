// Checks only the static host has: the wasm engine contract, file open and
// save through the File System Access API (mocked in the page, since a
// headless browser has no picker), the upload and download fallbacks, lazy
// fonts, the project folder, and the unsaved-change guard.
//
// `run.js` appends these to `steps.js` when it runs against the static site.

import { writeFileSync } from "node:fs";
import path from "node:path";
import { tmpdir } from "node:os";
import { A, STATE, check, cursorAt, engineState, ready, settle, state } from "./helpers.js";
import { crlfEdits, editWhileTyping, paneIsEngine } from "./sync_checks.js";

/** Install a fake `FileSystemFileHandle` store and pickers in the page. */
const MOCKS = `(() => {
  if (window.__mock) return window.__mock;
  const mock = { files: new Map(), picks: [], writes: [], clock: 1000, readonly: new Set(), saveAsName: null };
  const handle = (name) => ({
    kind: 'file', name,
    async getFile() {
      const f = mock.files.get(name);
      return { name, size: f.text.length, lastModified: f.mtime, async text() { return f.text; }, async arrayBuffer() { return new TextEncoder().encode(f.text).buffer; } };
    },
    async createWritable() {
      if (mock.readonly.has(name)) throw new DOMException(name + ' is read-only', 'NoModificationAllowedError');
      let buf = '';
      return { async write(t) { buf += t; }, async close() { mock.files.set(name, { text: buf, mtime: ++mock.clock }); mock.writes.push({ name, text: buf }); } };
    },
  });
  mock.handle = handle;
  mock.put = (name, text) => { mock.files.set(name, { text, mtime: ++mock.clock }); };
  window.showOpenFilePicker = async () => { const n = mock.next; mock.picks.push(n); return [handle(n)]; };
  window.showSaveFilePicker = async ({ suggestedName }) => { const n = mock.saveAsName ?? suggestedName; mock.saveAs = n; mock.files.set(n, { text: '', mtime: ++mock.clock }); return handle(n); };
  window.__mock = mock;
  return mock;
})()`;

const DOC = (title, body = "") => `zenith version=1 {
  project id="p" name="P"
  tokens format="zenith-token-v1" {
    token id="c" type="color" value="#102030"
    token id="ink" type="color" value="#ffffff"
  }
  styles {}
  document id="d" title="${title}" {
    page id="pg" w=(px)200 h=(px)100 {
      rect id="r" x=(px)10 y=(px)10 w=(px)80 h=(px)40 fill=(token)"c"
      ${body}
    }
  }
}
`;

const FONT_DOC = `zenith version=1 {
  project id="p" name="P"
  tokens format="zenith-token-v1" {
    token id="ink" type="color" value="#101010"
    token id="font.serif" type="fontFamily" value="Noto Serif"
    token id="font.mono" type="fontFamily" value="Noto Sans Mono"
  }
  styles {}
  document id="d" title="Fonts" {
    page id="pg" w=(px)300 h=(px)120 {
      text id="serif" x=(px)10 y=(px)10 w=(px)280 h=(px)40 font-family=(token)"font.serif" fill=(token)"ink" { span "Serif face" }
      text id="mono" x=(px)10 y=(px)60 w=(px)280 h=(px)40 font-family=(token)"font.mono" fill=(token)"ink" { span "Mono face" }
    }
  }
}
`;

/** Open `text` through the mocked picker as file `name` and wait for it. */
async function openMocked(page, name, text, { render = true } = {}) {
  await page.eval(`(() => { const m = ${MOCKS}; m.put(${JSON.stringify(name)}, ${JSON.stringify(text)}); m.next = ${JSON.stringify(name)}; return true; })()`);
  await page.clickSelector("#open-file");
  await page.waitFor(`${A}.name === ${JSON.stringify(name)} && ${A}.sync.version === (window.zenithEditor.host.session.version) && ${A}.sync.synced()`, `${name} to open`);
  if (render) await page.waitFor(`${A}.renderer.shown && ${A}.renderer.shown.gen === ${A}.renderer.gen`, "the render of the opened document");
  await settle(page);
}

export const steps = [
  [
    "wasm engine: the page runs on the wasm host with a clean status",
    async ({ page, url }) => {
      // A fresh load: the shared steps leave a dark phone-width viewport.
      await page.viewport(1440, 900);
      await page.media("light");
      await page.goto(url);
      await ready(page);
      const info = await page.eval(`({
        host: document.documentElement.dataset.host, kind: ${A}.host.kind,
        conn: document.getElementById('status-conn').textContent,
        tools: !document.getElementById('file-tools').hidden,
        base: document.baseURI, wasm: ${A}.host.wasmUrl, fonts: ${A}.host.fontsUrl,
        compileMs: ${A}.host.worker.compileMs,
      })`);
      check(info.host === "wasm" && info.kind === "wasm", `host ${info.host}/${info.kind}`);
      check(info.conn === "In browser", `status says ${info.conn}`);
      check(info.tools, "file tools hidden");
      check(info.wasm.startsWith(info.base.replace(/index\.html.*$|\?.*$/, "")), `wasm URL ${info.wasm} is not under the page ${info.base}`);
      return info;
    },
  ],
  [
    "a stale version is rejected with editor.stale_version, no state change",
    async ({ page }) => {
      await settle(page);
      const before = await engineState(page);
      const env = await page.eval(`${A}.host.run('buffer.set', { text: 'x' }, { version: ${before.version - 1} })`);
      check(!env.ok && env.error.code === "editor.stale_version", `got ${JSON.stringify(env.error)}`);
      check(env.version === before.version, `envelope version ${env.version}`);
      const after = await engineState(page);
      check(after.text === before.text && after.version === before.version, "the stale call changed the session");
      const save = await page.eval(`${A}.host.save({ version: ${before.version - 1} })`);
      check(!save.ok && save.error.code === "editor.stale_version", `save: ${JSON.stringify(save.error)}`);
      return { code: env.error.code };
    },
  ],
  [
    "a keystroke is one engine call: a commands.batch that also renders",
    async ({ page }) => {
      await settle(page);
      const before = await state(page);
      const at = before.text.indexOf('span "') + 'span "'.length;
      check(at > 'span "'.length, "no span to type into");
      await cursorAt(page, at);
      await settle(page);
      await page.eval(`${A}.host.metrics({ clear: true }); true`);
      const renders = await page.eval(`${A}.renderer.renders`);
      await page.type("q");
      await page.waitFor(`${A}.renderer.renders > ${renders}`, "the render of the keystroke");
      await settle(page);
      const calls = await page.eval(`${A}.host.metrics({ clear: true }).map((e) => e.command)`);
      check(JSON.stringify(calls) === '["commands.batch"]', `engine calls for one keystroke: ${JSON.stringify(calls)}`);
      const after = await state(page);
      check(after.text === before.text.slice(0, at) + "q" + before.text.slice(at), "the keystroke did not land");
      check(after.sha !== before.sha, "the preview did not change");
      const shown = await page.eval(`${A}.renderer.shown.gen === ${A}.renderer.gen`);
      check(shown, "the shown image is not of the new text");
      await page.key("Backspace");
      await page.waitFor(`(${STATE}).text === ${JSON.stringify(before.text)}`, "the text back");
      await settle(page);
      return { calls };
    },
  ],
  [
    "envelope shape and image blobs, preview included",
    async ({ page }) => {
      const out = await page.eval(`(async () => {
        const h = ${A}.host;
        const r = await h.run('doc.render', { scale: 0.5 });
        const blob = await h.image(r.image);
        const sig = new Uint8Array(await blob.arrayBuffer()).slice(0, 4);
        const st = await h.state();
        const sel = await h.run('select.set', { ids: ['card.b'] });
        const p = await h.run('gesture.preview', { dx: 5, dy: 3, scale: 0.5 });
        const pblob = p.image ? await h.image(p.image) : null;
        const psig = pblob ? new Uint8Array(await pblob.arrayBuffer()).slice(0, 4) : [];
        const missing = await h.image({ sha256: 'nope' }).then(() => 'resolved', (e) => e.code);
        return {
          keys: Object.keys(r).sort(), imageKeys: Object.keys(r.image).sort(), mime: blob.type, sig: [...sig],
          sha: r.image.sha256 === r.result.sha256, w: r.image.width, rw: r.result.width,
          previewOk: p.ok, previewImage: !!p.image, previewMime: pblob && pblob.type, psig: [...psig], previewRect: p.result.rect,
          stateKeys: Object.keys(st).sort(), missing,
        };
      })()`);
      const want = ["command", "dirty", "image", "ok", "result", "version", "work"];
      check(JSON.stringify(out.keys) === JSON.stringify(want), `envelope keys ${out.keys}`);
      check(out.imageKeys.includes("sha256") && out.imageKeys.includes("width") && out.imageKeys.includes("page") && out.imageKeys.includes("url"), `image keys ${out.imageKeys}`);
      check(out.mime === "image/png" && out.sig.join() === "137,80,78,71", `png signature ${out.sig}`);
      check(out.sha && out.w === out.rw, "image meta does not match the result");
      check(out.previewOk && out.previewImage && out.previewMime === "image/png" && out.psig.join() === "137,80,78,71", `preview ${JSON.stringify(out)}`);
      check(out.previewRect === undefined || typeof out.previewRect === "object", "preview rect");
      check(out.missing === "edit.image_expired", `unknown image: ${out.missing}`);
      for (const k of ["ok", "path", "version", "dirty", "valid", "stale", "conflict", "page", "selection"]) {
        check(out.stateKeys.includes(k), `state lacks ${k}`);
      }
      await page.eval(`${A}.host.run('select.set', { ids: [] })`);
      return { image: out.imageKeys };
    },
  ],
  [
    "unsaved-change guard follows the dirty state",
    async ({ page }) => {
      await settle(page);
      const guard = () => page.eval(`(() => { const e = new Event('beforeunload', { cancelable: true }); window.dispatchEvent(e); return e.defaultPrevented; })()`);
      check((await guard()) === false, "guard fires on a clean document");
      await cursorAt(page, 0);
      await page.type("// note\n");
      await page.waitFor(`${A}.dirty()`, "dirty after typing");
      check((await guard()) === true, "guard does not fire on a dirty document");
      await page.key("z", 2);
      await settle(page);
      return {};
    },
  ],
  [
    "Open through the file picker replaces the document; the dirty guard asks first",
    async ({ page, shot }) => {
      await settle(page);
      // Dirty first: the open asks.
      await cursorAt(page, 0);
      await page.type("// unsaved\n");
      await page.waitFor(`${A}.dirty()`, "dirty");
      await page.eval(`(() => { const m = ${MOCKS}; m.put('first.zen', ${JSON.stringify(DOC("First"))}); m.next = 'first.zen'; return true; })()`);
      await page.clickSelector("#open-file");
      await page.waitFor("!!document.querySelector('[data-key=open-confirm]')", "the discard confirmation");
      await page.eval("[...document.querySelectorAll('[data-key=open-confirm] button')].find((b) => b.textContent === 'Cancel').click()");
      await page.waitFor("!document.querySelector('[data-key=open-confirm]')", "the confirmation to close");
      await settle(page);
      const kept = await state(page);
      check(kept.text.includes("// unsaved") && (await page.eval(`${A}.name`)) === "stack.zen", "Cancel did not keep the document");
      await page.clickSelector("#open-file");
      await page.waitFor("!!document.querySelector('[data-key=open-confirm]')", "the discard confirmation again");
      await page.eval("[...document.querySelectorAll('[data-key=open-confirm] button')].find((b) => b.textContent.startsWith('Open')).click()");
      await page.waitFor(`${A}.name === 'first.zen' && ${A}.sync.synced() && ${A}.code.text().includes('title="First"')`, "first.zen to open");
      await page.waitFor(`${A}.renderer.shown && ${A}.renderer.shown.gen === ${A}.renderer.gen && ${A}.renderer.shown.page_size.w === 200`, "the new page render");
      await settle(page);
      const s = await state(page);
      check(s.text === DOC("First"), "pane text is not the opened file");
      check(!s.dirty, "the opened document is dirty");
      check(s.sel.length === 0 && s.page === 1, `selection ${s.sel} page ${s.page}`);
      check(s.errors === 0, `diagnostics: ${JSON.stringify(s.diags)}`);
      const title = await page.eval("document.getElementById('doc-name').textContent");
      check(title === "first.zen", `title ${title}`);
      const notices = await page.eval("[...document.querySelectorAll('#banners .notice')].map((n) => n.dataset.key)");
      check(notices.length === 0, `notices about the old document remain: ${notices}`);
      const layers = await page.eval("[...document.querySelectorAll('#layers [role=treeitem]')].map((e) => e.textContent.trim())");
      check(layers.length === 1 && layers[0].includes("r"), `layers ${layers}`);
      await shot("static-open");
      return { name: title };
    },
  ],
  [
    "Save writes back to the opened file; edit, Ctrl+S, file equals the pane",
    async ({ page }) => {
      await cursorAt(page, 0);
      await page.type("// saved note\n");
      await page.waitFor(`${A}.dirty()`, "dirty");
      await page.key("s", 2);
      await page.waitFor(`!${A}.dirty()`, "save to finish");
      await settle(page);
      const r = await page.eval(`(() => { const m = window.__mock; const f = m.files.get('first.zen'); return { text: f.text, writes: m.writes.length }; })()`);
      const s = await state(page);
      check(r.text === s.text, "the file text differs from the pane text");
      check(r.text.startsWith("// saved note\n"), "the edit is not in the file");
      check(r.writes === 1, `${r.writes} writes`);
      const status = await page.eval("document.getElementById('status-save').textContent");
      check(status === "Saved", `status ${status}`);
      return { writes: r.writes };
    },
  ],
  [
    "disk change on a clean buffer reloads when the page gets focus",
    async ({ page }) => {
      const before = await state(page);
      const next = before.text.replace("// saved note", "// changed on disk");
      await page.eval(`(() => { window.__mock.put('first.zen', ${JSON.stringify(next)}); window.dispatchEvent(new Event('focus')); return true; })()`);
      await page.waitFor(`(${STATE}).text.includes('changed on disk')`, "the external change to reach the pane");
      await settle(page);
      const s = await state(page);
      check(s.text === next && !s.dirty, "reload left the pane differing or dirty");
      check(s.version > before.version, "version did not move");
      return { version: s.version };
    },
  ],
  [
    "conflict banner: Reload takes the disk text, undo brings the edit back",
    async ({ page, shot }) => {
      const s0 = await state(page);
      await cursorAt(page, 0);
      await page.type("// local edit\n");
      await settle(page);
      const disk = s0.text.replace("// changed on disk", "// theirs");
      await page.eval(`(() => { window.__mock.put('first.zen', ${JSON.stringify(disk)}); window.dispatchEvent(new Event('focus')); return true; })()`);
      await page.waitFor("!!document.querySelector('[data-key=disk]')", "the conflict banner");
      const banner = await page.eval("document.querySelector('[data-key=disk]').textContent");
      check(banner.includes("edit.conflict"), `banner: ${banner}`);
      await shot("static-conflict");
      await page.eval("[...document.querySelectorAll('[data-key=disk] button')].find((b) => b.textContent === 'Reload').click()");
      await page.waitFor(`(${STATE}).text === ${JSON.stringify(disk)}`, "the disk text in the pane");
      await settle(page);
      const s = await state(page);
      check(!s.dirty, "dirty after Reload");
      check(!(await page.eval("!!document.querySelector('[data-key=disk]')")), "banner still shown");
      await page.key("z", 2);
      await page.waitFor(`(${STATE}).text.includes('// local edit')`, "undo to bring the local edit back");
      await settle(page);
      return {};
    },
  ],
  [
    "conflict banner: Overwrite writes the editor text",
    async ({ page }) => {
      const s = await state(page);
      const disk = s.text.replace("// local edit", "// rival");
      await page.eval(`(() => { window.__mock.put('first.zen', ${JSON.stringify(disk)}); window.dispatchEvent(new Event('focus')); return true; })()`);
      await page.waitFor("!!document.querySelector('[data-key=disk]')", "the conflict banner");
      await page.eval("[...document.querySelectorAll('[data-key=disk] button')].find((b) => b.textContent === 'Overwrite').click()");
      await page.waitFor(`!${A}.dirty() && !document.querySelector('[data-key=disk]')`, "Overwrite to finish");
      const text = await page.eval("window.__mock.files.get('first.zen').text");
      check(text === s.text, "the file is not the editor text after Overwrite");
      return {};
    },
  ],
  [
    "Save of a file-less document picks a target (Save As)",
    async ({ page }) => {
      // The sample has no handle: reload to get it back is not needed, open
      // a document with no handle by clearing it on the engine.
      await page.eval(`(() => { ${A}.host.handle = null; window.__mock.saveAs = null; return true; })()`);
      await cursorAt(page, 0);
      await page.type("// as new\n");
      await page.waitFor(`${A}.dirty()`, "dirty");
      await page.key("s", 2);
      await page.waitFor(`!${A}.dirty()`, "save to finish");
      const r = await page.eval("({ as: window.__mock.saveAs, text: (window.__mock.files.get(window.__mock.saveAs) || {}).text })");
      const s = await state(page);
      check(r.as === "first.zen" && r.text === s.text, `Save As ${JSON.stringify({ as: r.as })}`);
      return {};
    },
  ],
  [
    "a conflict clears when the file returns to the saved text; save then works",
    async ({ page }) => {
      await settle(page);
      const saved = await page.eval("window.__mock.files.get('first.zen').text");
      await cursorAt(page, 0);
      await page.type("// mine again\n");
      await settle(page);
      const theirs = saved.replace("// as new", "// theirs again");
      check(theirs !== saved, "the disk edit changed nothing");
      await page.eval(`(() => { window.__mock.put('first.zen', ${JSON.stringify(theirs)}); window.dispatchEvent(new Event('focus')); return true; })()`);
      await page.waitFor("!!document.querySelector('[data-key=disk]')", "the conflict banner");
      // `git checkout`: the file holds the saved text again.
      await page.eval(`(() => { window.__mock.put('first.zen', ${JSON.stringify(saved)}); window.dispatchEvent(new Event('focus')); return true; })()`);
      await page.waitFor("!document.querySelector('[data-key=disk]')", "the conflict banner to clear");
      check((await engineState(page)).conflict === false, "the engine still reports a conflict");
      await page.key("s", 2);
      await page.waitFor(`!${A}.dirty()`, "save to finish");
      const text = await page.eval("window.__mock.files.get('first.zen').text");
      check(text === (await state(page)).text, "the file is not the editor text after the save");
      return {};
    },
  ],
  [
    "a read-only file: save fails with edit.readonly, Save is off, Save As writes a copy",
    async ({ page }) => {
      await page.eval("(() => { window.__mock.readonly.add('first.zen'); window.__mock.saveAsName = 'copy.zen'; return true; })()");
      await cursorAt(page, 0);
      await page.type("// read-only edit\n");
      await settle(page);
      await page.key("s", 2);
      await page.waitFor("!!document.querySelector('[data-key=readonly]')", "the read-only notice");
      check(await page.eval("document.getElementById('save').disabled"), "Save is still on");
      check((await engineState(page)).readonly === true, "the summary is not read-only");
      check(await page.eval(`${A}.dirty()`), "the failed save cleared the dirty mark");
      await page.eval("[...document.querySelectorAll('[data-key=readonly] button')].find((b) => b.textContent === 'Save As').click()");
      await page.waitFor(`!${A}.dirty() && !document.querySelector('[data-key=readonly]')`, "Save As to finish");
      const r = await page.eval("({ copy: window.__mock.files.get('copy.zen').text, name: window.zenithEditor.name })");
      check(r.copy === (await state(page)).text, "the copy is not the editor text");
      check(r.name === "copy.zen", `the page names ${r.name}`);
      check(!(await page.eval("document.getElementById('save').disabled")), "Save is still off");
      await page.eval("(() => { window.__mock.readonly.clear(); window.__mock.saveAsName = null; return true; })()");
      return {};
    },
  ],
  [
    "CRLF document: open, duplicate, undo, redo, typing, an edit while typing, and save keep every \\r",
    async ({ page }) => {
      const crlf = DOC("CRLF", "// note").replace(/\n/g, "\r\n");
      await openMocked(page, "crlf.zen", crlf);
      check((await state(page)).text === crlf, "the pane text is not the CRLF file");
      await crlfEdits(page, "r");
      await page.eval(`${A}.selection.selectIds(['r'], 'layers')`);
      await editWhileTyping(page, { command: "node.duplicate", trigger: `${A}.actions.duplicate()`, anchor: "// crlf", typed: " y" });
      await page.key("s", 2);
      await page.waitFor(`!${A}.dirty()`, "save to finish");
      const text = await paneIsEngine(page, "after save");
      const file = await page.eval("window.__mock.files.get('crlf.zen').text");
      check(file === text, "the saved file differs from the engine text");
      check(file.startsWith('zenith version=1 {\r\n'), "the saved file lost its CRLF");
      return { bytes: file.length };
    },
  ],
  [
    "a second Open while the discard question shows ends the first; Cancel keeps the document",
    async ({ page }) => {
      await settle(page);
      const before = await state(page);
      const at = before.text.indexOf("// ") + 3;
      await cursorAt(page, at);
      await page.type("keep ");
      await page.waitFor(`${A}.dirty()`, "dirty");
      await page.eval(`(() => { const m = ${MOCKS}; m.put('other.zen', ${JSON.stringify(DOC("Other"))}); m.next = 'other.zen';
        window.__opens = [];
        for (const n of [1, 2]) ${A}.files.openFile().then(() => window.__opens.push(n), (e) => window.__opens.push('error ' + e.message));
        return true; })()`);
      await page.waitFor("window.__opens.includes(1) && !!document.querySelector('[data-key=open-confirm]')", "the first open to end when the second asks");
      await page.eval("[...document.querySelectorAll('[data-key=open-confirm] button')].find((b) => b.textContent === 'Cancel').click()");
      await page.waitFor("window.__opens.length === 2 && !document.querySelector('[data-key=open-confirm]')", "the second open to end on Cancel");
      const opens = await page.eval("window.__opens");
      check(JSON.stringify(opens) === "[1,2]", `opens ended as ${JSON.stringify(opens)}`);
      await settle(page);
      const s = await state(page);
      check(s.text === before.text.slice(0, at) + "keep " + before.text.slice(at), "Cancel did not keep the document");
      check((await page.eval(`${A}.name`)) === "crlf.zen", "another document opened");
      await page.key("s", 2);
      await page.waitFor(`!${A}.dirty()`, "save to finish");
      return { opens };
    },
  ],
  [
    "lazy fonts: serif and mono faces load by name and render",
    async ({ page, shot }) => {
      await openMocked(page, "fonts.zen", FONT_DOC);
      const out = await page.eval(`(async () => {
        const h = ${A}.host;
        const f = await h.run('fonts.required');
        const d = await h.run('doc.diagnose');
        return { faces: f.result.faces, loaded: [...h.loadedFonts].sort(),
          unresolved: d.result.diagnostics.filter((x) => x.code === 'font.unresolved').length };
      })()`);
      check(out.faces.length === 0, `faces still missing: ${JSON.stringify(out.faces)}`);
      check(out.loaded.includes("NotoSerif-Regular.ttf") && out.loaded.includes("NotoSansMono-Regular.ttf"), `loaded ${out.loaded}`);
      check(out.unresolved === 0, `${out.unresolved} font.unresolved left`);
      const diags = await page.eval(`${A}.diagnosticList.filter((d) => d.code === 'font.unresolved').length`);
      check(diags === 0, `the diagnostics panel shows ${diags} font.unresolved`);
      await shot("static-fonts");
      return out.loaded;
    },
  ],
  [
    "open a project folder: the document finds its asset; a lone file does not",
    async ({ page, shot }) => {
      const img = await page.eval(`(async () => {
        const text = await (await fetch(new URL('samples/image.zen', document.baseURI))).text();
        const man = await (await fetch(new URL('samples/manifest.json', document.baseURI))).json();
        const files = {};
        for (const rel of man.files) {
          const b = new Uint8Array(await (await fetch(new URL('samples/' + rel, document.baseURI))).arrayBuffer());
          let s = ''; for (const c of b) s += String.fromCharCode(c);
          files[rel] = btoa(s);
        }
        return { text, files: Object.keys(files) };
      })()`).catch(() => null);
      check(img && img.files.length > 0, "the sample manifest lists no project files");
      // A directory handle over the sample site: `values()` yields file handles.
      await page.eval(`(async () => {
        const m = ${MOCKS};
        const man = await (await fetch(new URL('samples/manifest.json', document.baseURI))).json();
        const names = [...man.documents.filter((d) => d === 'image.zen'), ...man.files];
        const entries = new Map();
        for (const rel of names) {
          const res = await fetch(new URL('samples/' + rel, document.baseURI));
          const bytes = new Uint8Array(await res.arrayBuffer());
          entries.set(rel, bytes);
        }
        const dirOf = (prefix) => ({
          kind: 'directory', name: prefix.split('/').filter(Boolean).pop() || 'examples',
          async *values() {
            const seen = new Set();
            for (const rel of entries.keys()) {
              if (!rel.startsWith(prefix)) continue;
              const rest = rel.slice(prefix.length);
              const head = rest.split('/')[0];
              if (seen.has(head)) continue;
              seen.add(head);
              if (rest.includes('/')) yield dirOf(prefix + head + '/');
              else yield fileOf(rel);
            }
          },
        });
        const fileOf = (rel) => ({
          kind: 'file', name: rel.split('/').pop(),
          async getFile() {
            const bytes = entries.get(rel);
            return new File([bytes], rel.split('/').pop(), { lastModified: 5 });
          },
          async createWritable() { return { async write() {}, async close() {} }; },
        });
        window.showDirectoryPicker = async () => dirOf('');
        return true;
      })()`);
      await page.clickSelector("#open-folder");
      await page.waitFor("document.getElementById('doc-dialog').open", "the document chooser");
      const listed = await page.eval("[...document.querySelectorAll('#doc-dialog-list button')].map((b) => b.textContent)");
      check(listed.includes("image.zen") && listed.includes("library/poster.zen"), `chooser lists ${listed}`);
      await shot("static-chooser");
      await page.eval("[...document.querySelectorAll('#doc-dialog-list button')].find((b) => b.textContent === 'image.zen').click()");
      await page.waitFor(`${A}.name === 'image.zen' && ${A}.sync.synced() && ${A}.renderer.shown && ${A}.renderer.shown.gen === ${A}.renderer.gen`, "image.zen from the folder");
      await settle(page);
      const withFolder = await state(page);
      check(withFolder.errors === 0, `errors with the folder: ${JSON.stringify(withFolder.diags)}`);
      const missing = withFolder.diags.filter((d) => /asset/.test(d.code));
      check(missing.length === 0, `asset diagnostics with the folder: ${JSON.stringify(missing)}`);
      await shot("static-project");
      // The same text with no project files: the asset is missing.
      await page.eval(`(() => { const m = ${MOCKS}; return true; })()`);
      await openMocked(page, "lone.zen", img.text, { render: false });
      const lone = await state(page);
      const loneMissing = lone.diags.filter((d) => d.severity === "error" || /asset/.test(d.code));
      check(loneMissing.length > 0, "the document without its folder shows no asset diagnostic");
      return { folderErrors: withFolder.errors, loneDiagnostics: loneMissing.map((d) => d.code) };
    },
  ],
  [
    "fallback: no File System Access API, Open reads an <input type=file>, Save downloads",
    async ({ page, shot }) => {
      // A fresh load without the picker APIs.
      const url = await page.eval("location.href");
      await page.send("Page.addScriptToEvaluateOnNewDocument", {
        source: "delete window.showOpenFilePicker; delete window.showSaveFilePicker; delete window.showDirectoryPicker; window.__downloads = []; const mk = URL.createObjectURL.bind(URL); URL.createObjectURL = (b) => { window.__lastBlob = b; return mk(b); }; const click = HTMLAnchorElement.prototype.click; HTMLAnchorElement.prototype.click = function () { if (this.download) window.__downloads.push(this.download); };",
      });
      await page.goto(url);
      await ready(page);
      const file = path.join(tmpdir(), `zenith-e2e-upload-${process.pid}.zen`);
      const text = DOC("Uploaded");
      writeFileSync(file, text);
      await page.send("Page.setInterceptFileChooserDialog", { enabled: true });
      const chosen = new Promise((resolve) => page.browser.on(page.sessionId, "Page.fileChooserOpened", resolve));
      await page.clickSelector("#open-file");
      const opened = await chosen;
      await page.send("DOM.setFileInputFiles", { files: [file], backendNodeId: opened.backendNodeId });
      await page.waitFor(`${A}.code.text().includes('title="Uploaded"') && ${A}.sync.synced()`, "the uploaded document");
      await settle(page);
      const upload = await state(page);
      check(upload.text === text && !upload.dirty, "uploaded text differs or is dirty");
      await cursorAt(page, 0);
      await page.type("// downloaded\n");
      await page.waitFor(`${A}.dirty()`, "dirty");
      await page.key("s", 2);
      await page.waitFor(`!${A}.dirty()`, "the download save to finish");
      const dl = await page.eval(`(async () => ({ names: window.__downloads, text: window.__lastBlob ? await window.__lastBlob.text() : null,
        warn: !!document.querySelector('[data-key=save-warning]') }))()`);
      const s = await state(page);
      check(dl.names.length === 1 && dl.names[0].endsWith(".zen"), `downloads ${dl.names}`);
      check(dl.text === s.text, "the downloaded text is not the pane text");
      check(dl.warn, "no warning that the browser cannot write the file back");
      await shot("static-download");
      return { downloads: dl.names };
    },
  ],
];
