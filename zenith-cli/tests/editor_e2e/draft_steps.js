import { A, check, cursorAt, ready, settle } from "./helpers.js";

const DRAFT_STATUS = "document.getElementById('status-draft').textContent";
const STORE = "await import('./js/app/draft-store.js')";
const list = (page) => page.eval(`(async () => (await new (${STORE}).DraftStore().list()).records)()`);
const edit = (page, text) => page.eval(`${A}.code.view.dispatch({ changes: {
  from: 0, to: ${A}.code.text().length, insert: ${JSON.stringify(text)} } }) ?? true`);

function home(url) {
  const page = new URL(url);
  page.search = "";
  return page;
}

async function welcome(page, url) {
  await page.goto(home(url).href);
  await page.waitFor("!document.getElementById('welcome').hidden", "the welcome screen");
}

async function saved(page) {
  await page.waitFor(`${DRAFT_STATUS} === 'Saved in this browser'`, "the browser draft to commit");
}

async function dialog(page, entry) {
  await page.clickSelector(entry);
  await page.waitFor("document.getElementById('draft-dialog').open", "the browser drafts dialog");
}

async function rowAction(page, action, name) {
  await page.waitFor(`!!document.querySelector(${JSON.stringify(`#draft-list button[aria-label="${action} ${name}"]`)})`,
    `the ${action} action for ${name}`);
  await page.clickSelector(`#draft-list button[aria-label="${action} ${name}"]`);
}

const RAW = "// café 雪\r\nzenith version=1 {\r\n";

export const steps = [
  [
    "draft recovery keeps invalid CRLF text and leaves example links intact",
    async ({ page, url, shot }) => {
      await welcome(page, url);
      await page.viewport(1440, 900);
      await page.media("light");
      await page.clickSelector("#welcome-blank");
      await ready(page);
      // Pause the engine so the storage check cannot pass from its response.
      const pending = await page.eval(`(() => { const app = ${A};
        app.code.view.dispatch({ changes: { from: 0, to: app.code.text().length, insert: ${JSON.stringify(RAW)} } });
        app.sync.schedule.cancel();
        return app.sync.pending() && !app.host.session.text.includes('café'); })()`);
      check(pending, "the engine received source before the draft check");
      await saved(page);
      check(await page.eval("document.getElementById('status-valid').textContent === 'Checking…'"),
        "the editor reported pending input as validated");
      check(await page.eval("getComputedStyle(document.getElementById('diag-list')).visibility === 'hidden'"),
        "diagnostics from the prior source revision remain visible");
      const draft = (await list(page)).find((record) => record.source.kind === "blank");
      check(draft?.text === RAW, "stored source lost invalid input, Unicode or CRLF");
      check(!(await page.eval(`${A}.atRisk()`)), "the saved draft still triggers the exit guard");
      await shot("draft-editor-saved");
      await welcome(page, url);
      await page.waitFor("!document.getElementById('welcome-drafts').hidden", "the welcome recovery entry");
      check(await page.eval("!window.zenithEditor && document.getElementById('app').hidden"), "draft recovery took over startup");
      await shot("draft-welcome");
      await dialog(page, "#welcome-drafts");
      await page.eval(`(() => {
        window.__draftDownload = null;
        URL.createObjectURL = (blob) => { window.__draftDownload = blob; return 'blob:zenith-test'; };
        URL.revokeObjectURL = () => {};
        HTMLAnchorElement.prototype.click = () => {};
        return true;
      })()`);
      await rowAction(page, "Download .zen", draft.name);
      check((await page.eval("window.__draftDownload.text()")) === RAW, "downloaded draft differs from stored text");
      await rowAction(page, "Resume", draft.name);
      await page.waitFor(`document.documentElement.dataset.ready === 'true' && ${A}.diagnostics.tally().error > 0`,
        "validation of the restored invalid source");
      check((await page.eval(`${A}.code.text()`)) === RAW, "restore changed source text");
      check(await page.eval(`${A}.host.handle === null && !${A}.example`), "restore did not open an independent copy");

      const poster = home(url);
      poster.searchParams.set("doc", "starters/poster.zen");
      await page.goto(poster.href);
      await ready(page);
      check(await page.eval(`${A}.example === 'starters/poster.zen' && !document.getElementById('draft-entry').hidden`),
        "the direct starter failed to open alongside existing drafts");
      const workingText = await page.eval(`${A}.code.text()`);
      await edit(page, `// example draft\n${workingText}`);
      await saved(page);
      const example = (await list(page)).find((record) => record.source.kind === "starter");
      check(example?.source.starter === "starters/poster.zen" && example.source.digest.length === 64, "starter metadata is missing");
      await dialog(page, "#draft-entry");
      await rowAction(page, "Discard", draft.name);
      await page.waitFor("document.getElementById('draft-message').textContent === 'Draft discarded.'", "draft discard");
      check(!(await list(page)).some((record) => record.id === draft.id), "discarded draft remains in storage");
      await page.clickSelector("#draft-close");
      return { exactBytes: Buffer.byteLength(RAW), starter: example.source.starter };
    },
  ],
  [
    "device drafts auto-save source only; failed switches preserve drafts",
    async ({ page, url }) => {
      await welcome(page, url);
      const original = await page.eval(`fetch('starters/blank.zen').then((response) => response.text())`);
      const name = "Private source.zen";
      await page.eval(`(() => {
        const name = ${JSON.stringify(name)}, text = ${JSON.stringify(original)};
        window.showOpenFilePicker = async () => [{ kind: 'file', name, async getFile() {
          return { name, lastModified: 1000, async text() { return text; } };
        } }];
        return true;
      })()`);
      await page.eval("document.getElementById('welcome-open').scrollIntoView({ block: 'center' })");
      await page.clickSelector("#welcome-open");
      await ready(page);
      const draftText = `// auto-saved device source\n${original}`;
      await edit(page, draftText);
      await saved(page);
      const record = (await list(page)).find((entry) => entry.name === name);
      check(record?.text === draftText && JSON.stringify(record.source) === '{"kind":"device"}', "device draft holds foreign state");
      check(Object.keys(record).sort().join() === "id,name,revision,schema,source,text,updatedAt", "unexpected device draft fields");
      check(await page.eval("document.getElementById('status-save').textContent === 'File not saved'"), "browser save obscures disk state");

      await cursorAt(page, 2);
      await page.eval(`(() => {
        const app = ${A};
        app.host.callEditorBeforeTest = app.host.callEditor;
        app.host.callEditor = async function(session, request) {
          if (request.command === 'doc.open') {
            await new Promise((resolve) => { window.__finishFailedOpen = resolve; });
            return { ok: false, error: { code: 'test.open_error', message: 'The test denied this open.' } };
          }
          return this.callEditorBeforeTest(session, request);
        };
        window.__attempted = app.files.load({ name: 'cannot-open.zen', text: '', project: { files: { 'foreign.png': 'AAAA' } } });
        return true;
      })()`);
      await page.waitFor("!!document.querySelector('[data-key=open-confirm]')", "the unsaved disk discard prompt");
      await page.eval("[...document.querySelectorAll('[data-key=open-confirm] button')].find((b) => b.textContent.startsWith('Open')).click()");
      await page.waitFor("!!window.__finishFailedOpen && document.getElementById('app').inert", "the switch under input lock");
      await page.type("late input");
      await page.eval("window.__finishFailedOpen()");
      check(!(await page.eval("window.__attempted")), "the injected open error was ignored");
      const unchanged = await page.eval(`(() => { const app = ${A}; return app.code.text() === ${JSON.stringify(draftText)}
        && app.name === ${JSON.stringify(name)} && app.host.handle?.name === app.name
        && !('foreign.png' in app.host.worker.files) && app.drafts.active.id === ${JSON.stringify(record.id)}; })()`);
      check(unchanged, "failed switch changed the current document, project, or draft");

      await welcome(page, url);
      await page.waitFor("!document.getElementById('welcome-drafts').hidden", "device recovery to be listed");
      await dialog(page, "#welcome-drafts");
      await rowAction(page, "Resume", name);
      await ready(page);
      check(await page.eval(`${A}.code.text() === ${JSON.stringify(draftText)} && ${A}.host.handle === null
        && document.querySelector('[data-key=draft-assets]')?.textContent.includes('original folder')`),
      "device recovery lacks source isolation or the asset warning");
      return { draft: name };
    },
  ],
  [
    "draft transactions fork stale edits and reject writes after compare-and-delete",
    async ({ page, url }) => {
      await welcome(page, url);
      const result = await page.eval(`(async () => {
        const { DraftStore, draftId } = ${STORE};
        const store = new DraftStore(), other = new DraftStore();
        const id = draftId(), name = 'CAS.zen', source = { kind: 'blank' };
        const save = (client, text, expected) => client.save({ id, name, source, text }, expected);
        const prior = await save(store, 'base', 0);
        const [update, fork] = await Promise.all([save(store, 'current', prior.revision), save(other, 'stale', prior.revision)]);
        const current = await store.read(id);
        const conflict = (await store.list()).records.find((record) => record.name === name && record.id !== id);
        const changed = await store.discard(id, prior.revision);
        const removed = await other.discard(id, current.revision);
        const missing = await save(store, 'late', current.revision);
        const discarded = await other.discard(conflict.id, conflict.revision);
        let sizeError = '';
        try { await save(store, 'x'.repeat(1024 * 1024), 0); } catch (error) { sizeError = error.message; }
        return { update, fork, current, conflict, changed, removed, missing, discarded, sizeError, remaining: await store.read(id) };
      })()`);
      check(result.current.revision === 2 && result.conflict.revision === 1 && result.current.text !== result.conflict.text,
        "stale update did not fork the exact text");
      check(result.changed === "changed" && result.removed === "discarded" && result.discarded === "discarded"
        && result.missing === null && result.remaining === null, "discard compare or missing write failed");
      check(result.sizeError.includes("draft size limit"), "oversized draft was not rejected");
      return { revisions: [result.current.revision, result.conflict.revision] };
    },
  ],
  [
    "storage errors keep the editor, source download and exit warning available",
    async ({ page, url }) => {
      const { identifier } = await page.send("Page.addScriptToEvaluateOnNewDocument", {
        source: `Object.defineProperty(window, 'BroadcastChannel', { value: undefined });
          Object.defineProperty(window, 'indexedDB', { get() { throw new DOMException('Blocked', 'SecurityError'); } });`,
      });
      try {
        await welcome(page, url);
        await page.clickSelector("#welcome-blank");
        await ready(page);
        const original = await page.eval(`${A}.code.text()`);
        await edit(page, `// retention unavailable\n${original}`);
        await page.waitFor(`${DRAFT_STATUS}.includes('unavailable')`, "browser storage error");
        check(await page.eval(`${A}.atRisk() && !document.getElementById('download').disabled`),
          "blocked storage hid recovery or treated uncommitted text as safe");
        await dialog(page, "#draft-entry");
        await page.waitFor("document.getElementById('draft-message').textContent.includes('unavailable')", "the storage error in the draft dialog");
        await page.clickSelector("#draft-close");
        await page.clickSelector("#export-menu-toggle");
        await page.waitFor("!!document.getElementById('download').getClientRects().length", "source download to stay available");
        return {};
      } finally {
        await page.send("Page.removeScriptToEvaluateOnNewDocument", { identifier });
      }
    },
  ],
  [
    "browser tabs revoke confirmed status when a draft changes or disappears",
    async ({ page, url }) => {
      await welcome(page, url);
      await page.clickSelector("#welcome-blank");
      await ready(page);
      const source = await page.eval(`${A}.code.text()`);
      const first = `// Two tabs' source\n${source}`;
      await edit(page, first);
      await saved(page);
      const draft = (await list(page)).find((record) => record.text === first);
      check(!!draft, "the first tab did not create a draft");

      const other = await page.browser.newPage();
      try {
        await welcome(other, url);
        await other.waitFor("document.getElementById('welcome-drafts').textContent === 'Resume draft'", "the resume action");
        await dialog(other, "#welcome-drafts");
        await rowAction(other, "Resume", draft.name);
        await ready(other);
        await saved(other);
        check(await other.eval(`${A}.drafts.active.id === ${JSON.stringify(draft.id)}`), "the second tab opened a different draft");

        await edit(page, `// Other revision\n${source}`);
        await saved(page);
        await other.waitFor(`${DRAFT_STATUS}.includes('changed in another tab') && ${A}.atRisk()`, "the stale tab to lose confirmed status");
        await dialog(page, "#draft-entry");
        await rowAction(page, "Discard", draft.name);
        await other.waitFor(`${DRAFT_STATUS}.includes('draft removed') && ${A}.atRisk()`, "the stale tab to learn of the deletion");
        check(await other.eval(`${A}.code.text() === ${JSON.stringify(first)}`), "another tab rewrote the open source");

        const next = `// Edit after another tab deleted the draft\n${source}`;
        await edit(other, next);
        await saved(other);
        const records = await list(other);
        check(!records.some((record) => record.id === draft.id) && records.some((record) => record.text === next),
          "the new edit failed to create an independent draft after deletion");
        check(!other.errors.length, `second tab errors: ${other.errors.join(" | ")}`);
      } finally {
        await other.goto("about:blank");
      }
      return {};
    },
  ],
];
