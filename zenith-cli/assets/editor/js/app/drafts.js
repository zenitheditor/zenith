import { download } from "../engine/wasm/files.js";
import { byId, h } from "../util/dom.js";
import { DraftStore, draftId } from "./draft-store.js";
import { curatedExample, loadSample } from "./samples.js";

export async function starterSource(starter, spec) {
  const files = Object.entries(spec.project?.files ?? {}).sort(([a], [b]) => a < b ? -1 : Number(a > b));
  const bytes = new TextEncoder().encode(JSON.stringify([spec.text, files]));
  const digest = await crypto.subtle?.digest("SHA-256", bytes);
  return {
    kind: "starter", starter,
    digest: digest ? Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("") : "",
  };
}

export class Drafts {
  constructor(open) {
    this.store = new DraftStore();
    this.active = null;
    this.open = open;
    this.dialog = byId("draft-dialog");
    this.entry = byId("draft-entry");
    this.welcome = byId("welcome-drafts");
    this.list = byId("draft-list");
    this.message = byId("draft-message");
    this.optIn = byId("draft-opt-in");
    this.hasDrafts = false;
    this.error = false;
    this.reading = 0;
    this.writes = new Map();
    try {
      this.channel = new BroadcastChannel("zenith-editor-drafts");
      this.channel.onmessage = ({ data }) => {
        if (this.active?.id === data?.id) void this.checkActive();
        void this.refresh();
      };
    } catch {
      this.channel = null;
    }
    this.entry.addEventListener("click", () => this.show());
    this.welcome.addEventListener("click", () => this.show());
    this.optIn.addEventListener("click", () => {
      const scope = this.active;
      if (scope?.source?.kind !== "device" || scope.enabled) return;
      scope.name = this.app.name;
      scope.enabled = true;
      scope.pending = true;
      this.render();
      this.write(scope);
    });
    byId("draft-close").addEventListener("click", () => this.dialog.close());
    this.dialog.addEventListener("cancel", (event) => {
      if (this.busy) event.preventDefault();
    });
    const check = () => {
      void this.refresh();
      void this.checkActive();
    };
    window.addEventListener("focus", check);
    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState === "hidden") {
        if (this.active?.pending) this.write(this.active);
      } else {
        check();
      }
    });
    void this.refresh();
  }

  attach(app) {
    this.app = app;
    this.render();
  }

  bind(name, text, source, record) {
    this.active = {
      id: record?.id ?? null,
      revision: record?.revision ?? 0,
      name,
      text,
      confirmed: record?.text ?? null,
      recovered: !!record,
      source,
      enabled: !!record || source?.kind === "starter" || source?.kind === "blank",
      pending: false,
      writing: false,
      message: "",
    };
    this.render();
    if (record) void this.checkActive();
  }

  confirmed(text) {
    return this.active?.confirmed === text;
  }

  needsCopy(text) {
    return this.active?.recovered === true && !this.confirmed(text);
  }

  copied() {
    if (this.active) this.active.recovered = false;
  }

  capture(text, name) {
    const scope = this.active;
    if (!scope || text === scope.text) return;
    scope.text = text;
    scope.name = name;
    if (!scope.enabled) return;
    scope.pending = true;
    scope.message = "";
    this.render();
    this.write(scope);
  }

  async write(scope) {
    if (scope.writing || !scope.pending) return;
    scope.writing = true;
    try {
      while (scope.pending) {
        scope.pending = false;
        const { text, name, source } = scope;
        if (scope.confirmed === text) continue;
        scope.id ??= draftId();
        const id = scope.id;
        const committing = this.store.save({ id, name, text, source }, scope.revision);
        this.writes.set(id, committing);
        let record;
        try {
          record = await committing;
        } finally {
          this.writes.delete(id);
        }
        if (!record) {
          if (scope.id === id) this.removed(scope);
          continue;
        }
        if (scope.id === id) {
          scope.id = record.id;
          scope.revision = record.revision;
          scope.confirmed = record.text;
          scope.message = record.id === id ? "" : "Saved as a separate browser draft.";
        }
        this.channel?.postMessage({ id: record.id });
        void this.refresh();
      }
    } catch (error) {
      scope.message = this.storageError(error);
      scope.pending = true;
    } finally {
      scope.writing = false;
      if (scope === this.active) {
        this.render();
        if (!scope.pending) void this.checkActive();
      }
    }
  }

  removed(scope) {
    scope.id = null;
    scope.revision = 0;
    scope.confirmed = null;
    scope.pending = false;
    scope.message = "Browser draft removed. Download .zen to keep this copy.";
    if (scope === this.active) this.render();
  }

  async checkActive() {
    const scope = this.active;
    if (!scope?.revision || scope.writing) return;
    const { id, revision } = scope;
    try {
      const record = await this.store.read(id);
      if (scope.id !== id || scope.revision !== revision || scope.writing) return;
      if (!record) this.removed(scope);
      else if (record.revision !== revision) {
        scope.confirmed = null;
        scope.message = "Draft changed in another tab. Edit to save a separate copy.";
        this.render();
      }
    } catch (error) {
      if (scope.id !== id || scope.revision !== revision || scope.writing) return;
      scope.confirmed = null;
      scope.message = this.storageError(error);
      this.render();
    }
  }

  render() {
    const scope = this.active;
    this.entry.hidden = !this.app || !(this.hasDrafts || this.error || scope?.message
      || (scope?.source?.kind === "device" && !scope.enabled));
    this.welcome.hidden = !(this.hasDrafts || this.error);
    byId("welcome-drafts-label").textContent = this.hasDrafts ? "Resume draft" : "Browser drafts";
    this.optIn.hidden = !(scope?.source?.kind === "device" && !scope.enabled);
    const status = byId("status-draft");
    const label = scope?.message || (scope?.writing || scope?.pending ? "Saving browser draft…"
      : scope?.confirmed === scope?.text && scope?.revision ? "Saved in this browser" : "");
    if (status.textContent !== label) status.textContent = label;
    status.hidden = !label;
    status.title = label;
    if (this.app) this.app.status.fileSave(this.app.dirty());
  }

  storageError(error) {
    if (error.name === "Error") return error.message;
    if (error.name === "QuotaExceededError") return "Browser storage is full. Download .zen or discard a draft.";
    return "Browser drafts are unavailable. Download .zen to keep a copy.";
  }

  async show() {
    this.message.textContent = this.active?.message ?? "";
    await this.refresh(true);
    if (!this.dialog.open) this.dialog.showModal();
  }

  async refresh(opening = false) {
    const reading = ++this.reading;
    const populate = opening || this.dialog.open;
    try {
      const { records, unreadable } = await this.store.list();
      if (reading !== this.reading) return;
      this.hasDrafts = records.length > 0;
      this.error = unreadable;
      if (populate && !this.busy) {
        this.list.replaceChildren(...records.map((record) => this.row(record)));
        if (unreadable) this.message.textContent = "Some browser drafts are unreadable.";
        else if (!records.length && !this.message.textContent) this.message.textContent = "No browser drafts.";
        else if (records.length && this.message.textContent === "No browser drafts.") this.message.textContent = "";
      }
    } catch (error) {
      if (reading !== this.reading) return;
      this.error = true;
      if (populate) this.message.textContent = this.storageError(error);
    }
    this.render();
  }

  row(record) {
    const { id, revision, name, text, updatedAt } = record;
    const row = h("li");
    const heading = h("strong", { text: name });
    const date = new Date(updatedAt);
    const stamp = h("span", { class: "muted", text: date.toLocaleString() });
    const controls = h("div", { class: "draft-actions" });
    const action = (label, run) => {
      const button = h("button", { type: "button", class: "button", text: label, "aria-label": label + " " + name });
      button.addEventListener("click", run);
      controls.append(button);
    };
    action("Resume", () => this.resume(record));
    action("Download .zen", () => download(name, text));
    action("Discard", () => this.act(async () => {
      if (!window.confirm(`Discard the browser draft for ${name}?`)) return;
      const result = await this.store.discard(id, revision);
      if (result === "changed") throw new Error("This draft changed in another tab. Review it before discarding.");
      if (this.active?.id === id) this.removed(this.active);
      this.channel?.postMessage({ id });
      this.message.textContent = "Draft discarded.";
    }));
    row.append(heading, stamp, controls);
    return row;
  }

  resume(record) {
    return this.act(async () => {
      await this.writes.get(record.id)?.catch(() => {});
      const current = await this.store.read(record.id);
      if (current?.revision !== record.revision) throw new Error("This draft changed. Select it again.");
      const { name, text, source } = current;
      const spec = { name, text, draftSource: source, draftRecord: current };
      if (source.kind === "device") {
        spec.draftWarning = "Recovered source only. Reopen the original folder to access its assets.";
      } else if (source.kind === "starter") {
        try {
          if (!curatedExample(source.starter)) throw new Error();
          const starter = await loadSample(source.starter);
          if (!source.digest || (await starterSource(source.starter, starter)).digest !== source.digest) throw new Error();
          spec.project = starter.project;
          spec.path = starter.name;
        } catch {
          spec.draftWarning = "The starter changed or is unavailable. Check its assets before export.";
        }
      }
      this.dialog.close();
      await this.open(spec);
    });
  }

  async act(run) {
    if (this.busy) return;
    this.busy = true;
    this.list.inert = true;
    this.optIn.disabled = true;
    byId("draft-close").disabled = true;
    this.message.textContent = "";
    try {
      await run();
    } catch (error) {
      if (!this.dialog.open) this.dialog.showModal();
      this.message.textContent = this.storageError(error);
    } finally {
      this.busy = false;
      this.list.inert = false;
      this.optIn.disabled = false;
      byId("draft-close").disabled = false;
      await this.refresh();
      if (this.dialog.open && !this.dialog.contains(document.activeElement)) byId("draft-close").focus();
    }
  }
}
