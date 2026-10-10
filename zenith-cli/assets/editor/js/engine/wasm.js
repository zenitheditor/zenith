// WasmEngine: the Engine interface (see `index.js`) over the wasm module,
// for the static host. The engine runs in a Worker, the module keeps no
// state, and this class holds the session: every command sends the session
// JSON in and takes the next session out.
//
// It also plays the part of the `zenith edit` host. `file.save`,
// `file.reload`, and `file.state` run here, over the File System Access API
// (a download when the browser has none). The envelope and the events have
// the shape the server sends, so the page needs no branching. There is one
// client, so a `session` event only echoes its own commands.
//
// Commands run one at a time in order, as the server runs them under its
// document lock.
//
// Extra methods for the static page: `openDocument`, `pickDocument`,
// `pickProject`, `metrics`, `close`. One extra host command:
// `file.save_as` writes the text to a new file the user picks (the offer of
// a save that met a read-only file).

import { diffRange } from "../util/text.js";
import { lineDiff } from "../util/diff.js";
import { batchDiagnostics } from "./batch.js";
import { EngineError, EngineUnavailable } from "./errors.js";
import * as browser from "./wasm/files.js";
import { FontCache, fontAdvisoryKey } from "./wasm/fonts.js";
import { WorkerClient } from "./wasm/rpc.js";

const IMAGES_KEPT = 32;
const MAX_FONT_ROUNDS = 6;
const NO_WORK = { parses: 0, validations: 0, tx_runs: 0, patches: 0, compiles: 0, rasters: 0 };

/** A random client id: 16 characters of `[A-Za-z0-9]`. */
function newClientId() {
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);
  return Array.from(bytes, (b) => alphabet[b % alphabet.length]).join("");
}

/** SHA-256 of `text` as hex, or `null` where `crypto.subtle` is missing (plain http). */
async function sha256Hex(text) {
  if (!crypto.subtle) return null;
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(text));
  return Array.from(new Uint8Array(digest), (b) => b.toString(16).padStart(2, "0")).join("");
}

const utf8Length = (text) => new TextEncoder().encode(text).length;

/** The diagnostics of outcome `out` of `command`: every step's for a batch. */
function replyDiagnostics(command, out) {
  if (!out.ok) return out.error?.diagnostics;
  return command === "commands.batch" ? batchDiagnostics(out.result) : out.result.diagnostics;
}

const FILE_COMMANDS = [
  {
    id: "file.save",
    label: "Save to disk",
    params: "{overwrite?=false}",
    result: "{saved, path, version, bytes, sha256, mtime_ms, stamped, delta?, warning?, dirty}",
    mutates: true,
    needs_version: false,
  },
  {
    id: "file.reload",
    label: "Reload from disk",
    params: "{}",
    result: "{reloaded, changed, version, delta?, valid, stale, diagnostics}",
    mutates: true,
    needs_version: true,
  },
  {
    id: "file.save_as",
    label: "Save to a new file",
    params: "{}",
    result: "{saved, path, version, bytes, sha256, mtime_ms, stamped, dirty}",
    mutates: true,
    needs_version: false,
  },
  {
    id: "file.state",
    label: "File state",
    params: "{}",
    result: "{path, root, version, dirty, valid, stale, conflict, missing, page, selection, mtime_ms, saved_sha256}",
    mutates: false,
    needs_version: false,
  },
];

export class WasmEngine {
  /**
   * `wasmUrl`, `fontsUrl`, `workerUrl` default to `zenith-editor-wasm.wasm`
   * and `fonts/` next to the page and `wasm/worker.js` next to this module.
   * Relative values resolve against the page address, so a site can live in
   * a subdirectory.
   */
  constructor({ wasmUrl, fontsUrl, workerUrl } = {}) {
    this.kind = "wasm";
    this.clientId = `page-${newClientId()}`;
    const page = document.baseURI;
    this.wasmUrl = String(new URL(wasmUrl ?? "zenith-editor-wasm.wasm", page));
    this.fontsUrl = String(new URL(fontsUrl ?? "fonts/", page));
    this.worker = new WorkerClient(workerUrl ?? new URL("./wasm/worker.js", import.meta.url), this.wasmUrl);
    this.fonts = new FontCache(this.fontsUrl);
    /** Bundled font files the Worker holds. */
    this.loadedFonts = new Set();
    this.fontKey = null;
    this.session = null;
    this.path = "document.zen";
    this.name = "document.zen";
    this.root = "";
    this.handle = null;
    /** Project file handles by path (a picked folder), to save into. */
    this.handles = new Map();
    this.savedText = "";
    this.mtimeMs = null;
    this.conflict = null;
    /** The last write met a read-only file. */
    this.readonly = false;
    this.images = new Map();
    this.imageCount = 0;
    this.listeners = new Set();
    this.tail = Promise.resolve();
  }

  /** Start the Worker and load the module. Rejects with an `EngineError`. */
  async ready() {
    await this.worker.ready();
    return { compileMs: this.worker.compileMs };
  }

  /** Stop the engine Worker (see `WorkerClient.close`). */
  close() {
    this.worker.close();
  }

  /** Per-call timings so far, newest last. `clear` empties the list. */
  metrics({ clear = false } = {}) {
    const out = this.worker.timings.slice();
    if (clear) this.worker.timings.length = 0;
    return out;
  }

  /** `true` while the session text differs from the saved text. */
  dirty() {
    return !!this.session && this.session.text !== this.savedText;
  }

  enqueue(fn) {
    const run = this.tail.then(fn, fn);
    this.tail = run.then(
      () => {},
      () => {},
    );
    return run;
  }

  emit(name, data) {
    for (const l of this.listeners) l.event?.(name, data);
  }

  // ---- Engine interface -------------------------------------------------

  /**
   * `client` names the sender in the `session` event (default: this page).
   * A test uses it to play another client.
   */
  async run(command, params = {}, { version, diff = false, client = this.clientId } = {}) {
    return this.enqueue(() => this.runNow(command, params ?? {}, version, diff, client));
  }

  async save({ version, overwrite = false } = {}) {
    return this.enqueue(() => this.runNow("file.save", { overwrite }, version, false, this.clientId));
  }

  async state({ text = false } = {}) {
    return this.enqueue(async () => ({ ...this.summary(text), saved_sha256: await sha256Hex(this.savedText) }));
  }

  async image(image) {
    const blob = this.images.get(image.sha256);
    if (!blob) {
      throw new EngineError(
        "edit.image_expired",
        `image ${image.sha256} is not in the cache (it keeps the last ${IMAGES_KEPT} renders); send doc.render again`,
      );
    }
    return blob;
  }

  subscribe({ open, event, lost, stopped }) {
    const listener = { open, event, lost, stopped };
    this.listeners.add(listener);
    queueMicrotask(() => {
      if (!this.listeners.has(listener)) return;
      open?.();
      event?.("state", this.summary(false));
    });
    // A browser file has no change notification: look at it when the page
    // gets focus back.
    const look = () => {
      if (document.visibilityState === "visible") this.enqueue(() => this.pollDisk());
    };
    window.addEventListener("focus", look);
    document.addEventListener("visibilitychange", look);
    return () => {
      this.listeners.delete(listener);
      window.removeEventListener("focus", look);
      document.removeEventListener("visibilitychange", look);
    };
  }

  // ---- Static host extras -----------------------------------------------

  /** Ask the user for a `.zen` file: `{name, text, handle}` or `null`. */
  pickDocument() {
    return browser.pickDocument();
  }

  /** Ask the user for a project folder: see `files.js` `pickProject`. */
  pickProject() {
    return browser.pickProject();
  }

  /**
   * Make `text` the document. `project` is a picked folder (`pickProject`)
   * and `path` the document's path in it; without them the document has no
   * project files. `handle` is the file to save back to. Emits `opened`.
   * The page text must equal the engine text first (the page flushes).
   */
  openDocument({ name, text, path = null, handle = null, project = null, mtimeMs = null }) {
    return this.enqueue(async () => {
      await this.worker.ready();
      const before = this.session;
      this.name = name;
      this.path = path ?? name;
      this.root = project?.root ?? "";
      this.handle = handle ?? project?.handles?.get(this.path) ?? null;
      this.handles = project?.handles ?? new Map();
      await this.worker.project({ files: project?.files ?? {}, resetFonts: true });
      this.loadedFonts.clear();
      this.fontKey = null;
      const out = await this.exec({ command: "doc.open", params: { text } });
      if (!out.ok) throw new EngineError(out.error.code, out.error.message);
      this.savedText = text;
      this.mtimeMs = this.handle ? (mtimeMs ?? (await browser.readHandle(this.handle)).mtimeMs) : null;
      this.conflict = null;
      this.readonly = false;
      this.emit("opened", {
        name: this.name,
        path: this.path,
        base_version: before?.version ?? 0,
        version: this.session.version,
        delta: diffRange(before?.text ?? "", text),
        text,
        selection: [],
        page: 1,
        valid: this.session.valid,
        stale: !this.session.valid,
        dirty: false,
      });
      return this.summary(true);
    });
  }

  // ---- Commands ---------------------------------------------------------

  async runNow(command, params, version, wantDiff, client) {
    if (command === "file.save") return this.fileSave(params, version, client);
    if (command === "file.save_as") return this.fileSaveAs(version, client);
    if (command === "file.reload") return this.fileReload(version, client);
    if (command === "file.state") return this.envelope(command, { ok: true, result: this.summary(false) });
    const request = { command, params };
    if (version !== undefined && version !== null) request.version = version;
    const before = this.session;
    const out = await this.exec(request);
    if (out.ok && command === "commands.list") this.appendFileCommands(out.result);
    const env = this.envelope(command, out);
    if (out.ok && this.session !== before && before) {
      if (wantDiff && before.text !== this.session.text) {
        env.diff = lineDiff(before.text, this.session.text);
      }
      if (before.version !== this.session.version) this.sessionEvent(before, command, client);
    }
    return env;
  }

  /**
   * Run `request` on the session. A reply that reports an unresolved bundled
   * font loads the named files and runs the request again from the same
   * session (the module keeps no state, so the first run leaves no trace).
   */
  async exec(request) {
    const before = this.session;
    let out = await this.callEditor(before, request);
    const key = fontAdvisoryKey(replyDiagnostics(request.command, out));
    if (key !== null && key !== this.fontKey) {
      this.fontKey = key;
      const after = out.ok ? out.session : before;
      if (after && (await this.loadFonts(after))) out = await this.callEditor(before, request);
    }
    if (out.ok) this.session = out.session;
    return out;
  }

  /** One `editor` module call. Never rejects: a module error is an error outcome. */
  async callEditor(session, request) {
    let reply;
    try {
      reply = await this.worker.call("editor", { session, command: request, path: this.path }, { command: request.command });
    } catch (err) {
      if (err.code === "wasm.worker_failed" || err.code === "wasm.load_failed") throw new EngineUnavailable(err.message);
      return { ok: false, error: { code: err.code ?? "wasm.engine_failed", message: err.message }, work: NO_WORK };
    }
    const { response, png } = reply;
    if (!response.ok) return { ok: false, error: response.error, work: NO_WORK };
    const r = response.result;
    return { ok: true, session: r.session, result: r.result, work: r.work, png };
  }

  /**
   * Fetch the bundled font files `session`'s document needs, and give them
   * to the Worker. Asks `fonts.required` again after each batch (a face can
   * name a further one). Resolves `true` when it loaded any file.
   */
  async loadFonts(session) {
    let loaded = false;
    for (let round = 0; round < MAX_FONT_ROUNDS; round++) {
      const out = await this.callEditor(session, { command: "fonts.required", params: {} });
      if (!out.ok) break;
      const wanted = [...new Set(out.result.faces.map((f) => f.file).filter((f) => f && !this.loadedFonts.has(f)))];
      if (!wanted.length) break;
      const got = {};
      await Promise.all(
        wanted.map(async (file) => {
          try {
            got[file] = await this.fonts.get(file);
          } catch (err) {
            // Leave the face to the `font.unresolved` advisory, and do not retry.
            console.warn(`zenith: cannot fetch font ${file}: ${err.message}`);
          }
          this.loadedFonts.add(file);
        }),
      );
      if (!Object.keys(got).length) break;
      await this.worker.project({ fonts: got });
      loaded = true;
    }
    return loaded;
  }

  /** The reply envelope for an outcome `out`. */
  envelope(command, out) {
    const env = {
      ok: out.ok,
      command,
      version: this.session?.version ?? 0,
      dirty: this.dirty(),
      work: out.work ?? NO_WORK,
    };
    if (out.ok) env.result = out.result;
    else env.error = out.error;
    if (out.ok && out.png) env.image = this.storeImage(out.result, out.png);
    return env;
  }

  /**
   * Keep the PNG of a render and describe it as the server does. For a
   * batch, `result` is the reply of the step that rendered.
   */
  storeImage(reply, png) {
    const result = Array.isArray(reply.steps) ? (reply.steps[reply.image_step]?.result ?? {}) : reply;
    const view = new DataView(png);
    const meta = {
      sha256: result.sha256 ?? `png-${++this.imageCount}`,
      width: view.getUint32(16),
      height: view.getUint32(20),
      page: result.page ?? this.session?.page ?? 1,
    };
    for (const key of ["rect", "scale", "device_size", "page_size"]) {
      if (result[key] !== undefined) meta[key] = result[key];
    }
    meta.url = `image/${meta.sha256}.png`;
    this.images.delete(meta.sha256);
    this.images.set(meta.sha256, new Blob([png], { type: "image/png" }));
    while (this.images.size > IMAGES_KEPT) this.images.delete(this.images.keys().next().value);
    return meta;
  }

  appendFileCommands(result) {
    for (const spec of FILE_COMMANDS) {
      const enabled = spec.id !== "file.reload" || this.handle !== null;
      result.commands.push({ ...spec, enabled, disabled: enabled ? null : "the document was not opened from a file" });
    }
    result.conflict = this.conflict !== null;
  }

  sessionEvent(before, command, client) {
    const s = this.session;
    this.emit("session", {
      client,
      command,
      base_version: before.version,
      version: s.version,
      delta: diffRange(before.text, s.text),
      selection: s.selection,
      page: s.page,
      valid: s.valid,
      stale: !s.valid,
      dirty: this.dirty(),
      conflict: this.conflict !== null,
    });
  }

  summary(withText) {
    const s = this.session;
    const out = {
      ok: true,
      path: this.path,
      root: this.root,
      version: s?.version ?? 0,
      dirty: this.dirty(),
      valid: s?.valid ?? false,
      stale: s ? !s.valid : false,
      conflict: this.conflict !== null,
      missing: false,
      readonly: this.readonly,
      page: s?.page ?? 1,
      selection: s?.selection ?? [],
      mtime_ms: this.mtimeMs,
    };
    if (withText) {
      out.text = s?.text ?? "";
      if (this.conflict) out.disk_text = this.conflict.text;
    }
    return out;
  }

  // ---- File commands ----------------------------------------------------

  /** An error envelope for a stale or missing `version`, in the engine's words. */
  versionError(command, version, required) {
    const now = this.session?.version ?? 0;
    if (version !== undefined && version !== null && version !== now) {
      return this.failure(command, "editor.stale_version", `'${command}' was made at version ${version}, but the session is at version ${now}; apply the latest delta (or reload the text) and resend`);
    }
    if (required && (version === undefined || version === null)) {
      return this.failure(command, "editor.missing_version", `'${command}' changes the text, so it needs the session version the sender saw; add "version": ${now}`);
    }
    return null;
  }

  failure(command, code, message, extra = {}) {
    return this.envelope(command, { ok: false, error: { code, message, ...extra } });
  }

  conflictError() {
    return {
      code: "edit.conflict",
      message:
        "the file changed on disk while the session has unsaved edits; send file.reload to take the disk text, or file.save with overwrite=true to replace it",
      offers: [
        { id: "reload", label: "Reload from disk", command: "file.reload", params: {} },
        { id: "overwrite", label: "Overwrite the file", command: "file.save", params: { overwrite: true } },
      ],
    };
  }

  async fileSave(params, version, client) {
    const bad = this.versionError("file.save", version, false);
    if (bad) return bad;
    const overwrite = params.overwrite === true;
    if (this.handle && !overwrite) {
      const seen = await this.pollDisk();
      if (this.conflict) return this.envelope("file.save", { ok: false, error: this.conflictError() });
      if (seen === "reloaded") {
        return this.failure(
          "file.save",
          "edit.reloaded",
          "the file changed on disk and the clean session reloaded it; review the text and save again",
        );
      }
    }
    const text = this.session.text;
    let warning = null;
    try {
      if (!this.handle) await this.chooseTarget(text);
      if (this.handle) {
        this.mtimeMs = await browser.writeHandle(this.handle, text);
      } else {
        browser.download(this.name, text);
        warning = `This browser cannot write files back. Downloaded a copy of ${this.name} instead.`;
      }
    } catch (err) {
      if (browser.isReadOnlyError(err)) return this.readOnlyFailure("file.save");
      return this.failure("file.save", "edit.write_failed", `cannot write ${this.name}: ${err.message}`);
    }
    return this.saved(text, warning, client, "file.save");
  }

  /** The failure of a write that met a read-only file. Flags the file and tells the page. */
  readOnlyFailure(command) {
    if (!this.readonly) {
      this.readonly = true;
      this.emit("state", this.summary(false));
    }
    return this.failure(command, "edit.readonly", `${this.name} is read-only, so the browser cannot write it; save a copy with Save As, or make the file writable and save again`, {
      offers: [{ id: "save_as", label: "Save As", command: "file.save_as", params: {} }],
    });
  }

  /** `file.save_as`: ask for a new file and write the text there. Resolves `saved: false` when the user cancels. */
  async fileSaveAs(version, client) {
    const bad = this.versionError("file.save_as", version, false);
    if (bad) return bad;
    const text = this.session.text;
    let handle;
    try {
      handle = await browser.saveAs(this.name, text);
    } catch (err) {
      return this.failure("file.save_as", "edit.write_failed", `cannot write a copy of ${this.name}: ${err.message}`);
    }
    if (!handle) {
      if (typeof window.showSaveFilePicker === "function") {
        return this.envelope("file.save_as", { ok: true, result: { saved: false, path: this.path, version: this.session.version, dirty: this.dirty() } });
      }
      browser.download(this.name, text);
      return this.saved(text, `This browser cannot write files. Downloaded a copy of ${this.name} instead.`, client, "file.save_as");
    }
    this.handle = handle;
    this.name = handle.name;
    this.path = handle.name;
    this.mtimeMs = (await browser.readHandle(handle)).mtimeMs;
    return this.saved(text, null, client, "file.save_as");
  }

  /** Record a write of `text` that landed: clear the conflict and read-only flags, send `saved`. */
  async saved(text, warning, client, command) {
    this.savedText = text;
    this.conflict = null;
    if (this.readonly) {
      this.readonly = false;
      this.emit("state", this.summary(false));
    }
    const sha = await sha256Hex(text);
    const result = {
      saved: true,
      path: this.path,
      version: this.session.version,
      bytes: utf8Length(text),
      sha256: sha,
      mtime_ms: this.mtimeMs,
      stamped: false,
      delta: null,
      warning,
      dirty: false,
    };
    this.emit("saved", {
      client,
      path: this.path,
      version: this.session.version,
      sha256: sha,
      mtime_ms: this.mtimeMs,
      stamped: false,
    });
    return this.envelope(command, { ok: true, result });
  }

  /** Pick where an unsaved document goes, when the browser can. */
  async chooseTarget(text) {
    const folderHandle = this.handles.get(this.path);
    if (folderHandle) {
      this.handle = folderHandle;
      return;
    }
    // `saveAs` writes the file it picked.
    const handle = await browser.saveAs(this.name, text);
    if (handle) {
      this.handle = handle;
      this.name = handle.name;
      this.mtimeMs = (await browser.readHandle(handle)).mtimeMs;
    }
  }

  async fileReload(version, client) {
    const bad = this.versionError("file.reload", version, true);
    if (bad) return bad;
    if (!this.handle) {
      return this.failure("file.reload", "edit.missing_file", `${this.name} was not opened from a file, so there is nothing to reload; open it again`);
    }
    let disk;
    try {
      disk = await browser.readHandle(this.handle);
    } catch (err) {
      return this.failure("file.reload", "edit.read_failed", `cannot read ${this.name}: ${err.message}`);
    }
    const before = this.session;
    let result = { reloaded: true, changed: false, version: before.version, delta: null, valid: before.valid, stale: !before.valid, diagnostics: [] };
    if (disk.text !== before.text) {
      const out = await this.exec({ command: "buffer.set", params: { text: disk.text, coalesce: false }, version: before.version });
      if (!out.ok) return this.envelope("file.reload", out);
      result = {
        reloaded: true,
        changed: true,
        version: this.session.version,
        delta: diffRange(before.text, this.session.text),
        valid: this.session.valid,
        stale: !this.session.valid,
        diagnostics: out.result.diagnostics ?? [],
      };
    }
    this.savedText = disk.text;
    this.mtimeMs = disk.mtimeMs;
    this.conflict = null;
    const env = this.envelope("file.reload", { ok: true, result });
    if (result.changed) this.sessionEvent(before, "file.reload", client);
    return env;
  }

  /**
   * Compare the file with the last text read or written. A clean session
   * takes a changed file (`external_change` with `reloaded`). A dirty one
   * records a conflict. Resolves `"none"`, `"synced"` (the file equals the
   * session), `"reloaded"`, or `"conflict"`.
   */
  async pollDisk() {
    if (!this.handle || !this.session) return "none";
    let disk;
    try {
      const file = await this.handle.getFile();
      if (file.lastModified === this.mtimeMs && !this.conflict) return "none";
      disk = { text: await file.text(), mtimeMs: file.lastModified };
    } catch {
      return "none";
    }
    if (disk.text === this.savedText && this.conflict) {
      // The file went back to the saved text (`git checkout`): no conflict is left.
      this.mtimeMs = disk.mtimeMs;
      this.conflict = null;
      const s = this.session;
      this.emit("external_change", {
        path: this.path,
        deleted: false,
        mtime_ms: disk.mtimeMs,
        text: disk.text,
        base_version: s.version,
        version: s.version,
        valid: s.valid,
        stale: !s.valid,
        conflict: false,
        reloaded: false,
        delta: null,
        dirty: this.dirty(),
      });
      return "synced";
    }
    if (disk.text === this.savedText || this.conflict?.text === disk.text) {
      this.mtimeMs = disk.mtimeMs;
      return this.conflict ? "conflict" : "none";
    }
    const before = this.session;
    const base = {
      path: this.path,
      deleted: false,
      mtime_ms: disk.mtimeMs,
      text: disk.text,
      base_version: before.version,
      version: before.version,
      valid: before.valid,
      stale: !before.valid,
    };
    this.mtimeMs = disk.mtimeMs;
    if (before.text === disk.text) {
      this.savedText = disk.text;
      this.conflict = null;
      this.emit("external_change", { ...base, conflict: false, reloaded: false, delta: null, dirty: false });
      return "synced";
    }
    if (!this.dirty()) {
      const out = await this.exec({ command: "buffer.set", params: { text: disk.text, coalesce: false }, version: before.version });
      if (out.ok) {
        this.savedText = disk.text;
        this.conflict = null;
        this.emit("external_change", {
          ...base,
          version: this.session.version,
          conflict: false,
          reloaded: true,
          delta: diffRange(before.text, this.session.text),
          valid: this.session.valid,
          stale: !this.session.valid,
          dirty: false,
        });
        return "reloaded";
      }
    }
    this.conflict = { text: disk.text, mtimeMs: disk.mtimeMs };
    this.emit("external_change", { ...base, conflict: true, reloaded: false, delta: null, dirty: true });
    return "conflict";
  }
}
