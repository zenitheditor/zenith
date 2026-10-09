// Live events and the connection: `state`, `session`, `external_change`,
// `saved`, `shutdown`; the offline notice; the disk conflict and deleted
// notices.

import { EngineUnavailable } from "../engine/index.js";
import { lineDiff } from "../util/diff.js";
import { sleep } from "../util/timing.js";

const DISK = "disk";
const OFFLINE = "offline";
const PROBE_MIN_MS = 1000;
const PROBE_MAX_MS = 8000;

export class EventRouter {
  constructor(app) {
    this.app = app;
    this.raw = null;
    this.probing = false;
    this.connected = false;
    this.everConnected = false;
    this.stopped = false;
  }

  /**
   * `engine` with connection tracking: a call that cannot reach the server
   * shows the offline notice, the next call that gets through hides it.
   */
  track(engine) {
    const router = this;
    this.raw = engine;
    const wrap =
      (fn) =>
      async (...args) => {
        try {
          const out = await fn(...args);
          router.reachable();
          return out;
        } catch (err) {
          if (err instanceof EngineUnavailable) router.offline(err);
          throw err;
        }
      };
    // A command that cannot reach the server resolves with an envelope
    // marked `offline`, so no caller leaves a promise rejected. The offline
    // notice covers it, and the probe refreshes the page on recovery.
    const command =
      (fn, name) =>
      async (...args) => {
        try {
          return await wrap(fn)(...args);
        } catch (err) {
          if (!(err instanceof EngineUnavailable)) throw err;
          return {
            ok: false,
            offline: true,
            command: name(args),
            error: { code: err.code, message: err.message },
          };
        }
      };
    return {
      kind: engine.kind,
      clientId: engine.clientId,
      run: command((...a) => engine.run(...a), (a) => a[0]),
      save: command((...a) => engine.save(...a), () => "file.save"),
      state: wrap((...a) => engine.state(...a)),
      image: wrap((...a) => engine.image(...a)),
      subscribe: (...a) => engine.subscribe(...a),
    };
  }

  subscribe() {
    this.app.engine.subscribe({
      open: () => this.open(),
      event: (name, data) => this.event(name, data),
      lost: (ms) => this.lost(ms),
      stopped: () => this.stop(),
    });
  }

  open() {
    this.connected = true;
    this.app.notices.hide(OFFLINE);
    this.app.status.connection(this.connectedLabel());
    if (this.everConnected) {
      // Events do not resume: read the state with text and merge it.
      this.app.sync.resync().then((summary) => this.summary(summary));
    }
    this.everConnected = true;
  }

  /** What the status bar says while the engine answers. */
  connectedLabel() {
    return this.raw?.kind === "wasm" ? "In browser" : "Connected";
  }

  lost(ms) {
    if (this.stopped) return;
    this.connected = false;
    this.app.status.connection("Reconnecting");
    this.app.notices.show(OFFLINE, {
      level: "error",
      code: "edit.unreachable",
      title: "Connection lost.",
      message: `Cannot reach zenith edit. Retrying in ${Math.max(1, Math.round(ms / 1000))} s. Edits stay in the editor and go out on reconnect.`,
      dismiss: false,
    });
  }

  offline(err) {
    if (this.stopped) return;
    this.app.status.connection("Offline");
    this.app.notices.show(OFFLINE, {
      level: "error",
      code: err.code ?? "edit.unreachable",
      title: this.raw?.kind === "wasm" ? "The engine stopped." : "Connection lost.",
      message: `${err.message}. Retrying. Edits stay in the editor and go out on reconnect.`,
      dismiss: false,
    });
    this.probe();
  }

  /** Retry the server until it answers, then resync and refresh. */
  async probe() {
    if (this.probing) return;
    this.probing = true;
    let delay = PROBE_MIN_MS;
    while (!this.stopped) {
      await sleep(delay);
      try {
        await this.raw.state();
        break;
      } catch {
        delay = Math.min(delay * 2, PROBE_MAX_MS);
      }
    }
    this.probing = false;
    if (this.stopped) return;
    this.app.notices.hide(OFFLINE);
    this.app.status.connection(this.connected ? this.connectedLabel() : "Reconnecting");
    const summary = await this.app.sync.resync();
    this.summary(summary);
    this.app.refreshLater();
  }

  reachable() {
    if (this.stopped || !this.app.notices.has(OFFLINE)) return;
    if (this.connected) {
      this.app.notices.hide(OFFLINE);
      this.app.status.connection(this.connectedLabel());
    }
  }

  stop() {
    this.stopped = true;
    this.app.status.connection("Stopped");
    this.app.notices.hide(OFFLINE);
    this.app.notices.show("shutdown", {
      level: "error",
      code: "edit.stopped",
      title: "zenith edit stopped.",
      message: "Saves no longer reach the disk. Copy unsaved text before you close this tab, or run zenith edit again.",
      dismiss: false,
    });
  }

  event(name, data) {
    const app = this.app;
    switch (name) {
      case "state":
        this.summary(data);
        if (data.version !== app.sync.version && app.sync.synced()) app.sync.resync();
        break;
      case "session":
        this.session(data);
        break;
      case "external_change":
        this.external(data);
        break;
      case "opened":
        app.documentOpened(data);
        break;
      case "saved":
        if (data.version === app.sync.version) app.serverDirty = false;
        if (data.client !== app.engine.clientId) {
          app.notices.toast(`Saved ${app.name} from another client.`);
        }
        this.cleared();
        app.status.update();
        break;
      case "shutdown":
        this.stop();
        break;
      default:
        break;
    }
  }

  /** Flags from a state summary: dirty, conflict, missing. */
  summary(s) {
    const app = this.app;
    app.serverDirty = s.dirty;
    if (s.conflict && typeof s.disk_text === "string") this.conflict(s.disk_text);
    else if (s.missing) this.deleted();
    else if (!s.conflict) app.notices.hide(DISK);
    app.status.update();
  }

  session(data) {
    const app = this.app;
    if (data.client === app.engine.clientId) return;
    app.serverDirty = data.dirty;
    const moved = data.version !== data.base_version;
    if (moved) {
      app.sync.remote({
        delta: data.delta,
        baseVersion: data.base_version,
        version: data.version,
        source: "remote",
        extra: { dirty: data.dirty },
      });
    }
    if (data.page && data.page !== app.page) {
      app.page = data.page;
      app.layers.setPage(data.page);
      app.renderer.request({ fit: true });
    }
    const same =
      data.selection.length === app.selectionIds.length &&
      data.selection.every((id, i) => id === app.selectionIds[i]);
    if (!same) {
      app.selectionIds = data.selection;
      app.selection.refresh("remote");
    }
    if (!data.conflict) app.notices.hide(DISK);
    app.status.update();
  }

  external(data) {
    const app = this.app;
    if (typeof data.dirty === "boolean") app.serverDirty = data.dirty;
    if (data.deleted) {
      this.deleted();
    } else if (data.conflict) {
      this.conflict(data.text ?? "");
    } else {
      app.notices.hide(DISK);
      if (data.reloaded) {
        app.sync.remote({
          delta: data.delta,
          baseVersion: data.base_version,
          version: data.version,
          source: "disk",
          extra: { dirty: data.dirty },
        });
        app.notices.toast(`Reloaded ${app.name}: it changed on disk.`, "info");
      }
    }
    app.status.update();
  }

  deleted() {
    const app = this.app;
    app.notices.show(DISK, {
      level: "warning",
      code: "edit.deleted",
      title: `${app.name} was removed from disk.`,
      message: "The editor keeps the text. Save writes the file again.",
      actions: [{ label: "Save", kind: "primary", run: () => app.save() }],
    });
  }

  /** The disk changed while the editor has unsaved edits. */
  conflict(diskText) {
    const app = this.app;
    // What the disk has that the editor does not: `-` editor, `+` disk.
    const shown = lineDiff(app.code.text(), diskText);
    app.notices.show(DISK, {
      level: "error",
      code: "edit.conflict",
      title: `${app.name} changed on disk.`,
      message:
        "The editor also has unsaved edits. Reload takes the disk text (undo brings yours back). Overwrite writes the editor text. Below: - editor, + disk.",
      detail: shown || undefined,
      dismiss: false,
      actions: [
        { label: "Reload", kind: "primary", run: () => this.reload() },
        { label: "Overwrite", kind: "danger", run: () => app.save({ overwrite: true }) },
      ],
    });
  }

  async conflictFromSave(env) {
    const summary = await this.app.engine.state({ text: true });
    if (typeof summary.disk_text === "string") this.conflict(summary.disk_text);
    else this.app.notices.error("file.save", env, { runOffer: (o) => this.app.runOffer(o) });
  }

  async reload() {
    const app = this.app;
    const env = await app.sync.edit(
      (version) => app.engine.run("file.reload", {}, { version }),
      "disk",
    );
    if (env.ok) {
      app.serverDirty = env.dirty;
      this.cleared();
      app.notices.toast(`Reloaded ${app.name} from disk.`, "info");
      app.status.update();
    } else {
      app.notices.error("file.reload", env, { runOffer: (o) => app.runOffer(o), retry: () => this.reload() });
    }
  }

  /** The disk and the session agree again: drop the disk notices. */
  cleared() {
    this.app.notices.hide(DISK);
  }
}
