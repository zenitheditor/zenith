// Buffer sync: keeps the code pane and the engine session on one text.
//
// The code pane owns the text. Typing goes to the engine as `buffer.set`
// after a 150 ms pause, in one `commands.batch` with the follow-up reads
// the page needs (the `frame` hook). Engine changes (other clients, disk reloads, undo,
// save stamps) come back as deltas against a known version.
//
// Model:
//   base     the engine text at `version`, as this page last saw it
//   inflight the `buffer.set` on the wire: { changes over base, text }
//   unsent   local changes not sent yet, over inflight.text (else base)
//
// A foreign change F over `base` merges with local changes L over `base`
// by rebasing: the pane applies F mapped through L, and L becomes L mapped
// through F. Keystrokes are never dropped. When the version does not match
// (`editor.stale_version`, a missed event, a reconnect), the page reads
// `GET /api/state?text=1` and merges the difference the same way.

import { ChangeSet, Text } from "../../vendor/codemirror.js";
import { splitBatch } from "../engine/batch.js";
import { diffRange } from "../util/text.js";
import { debounce, sleep } from "../util/timing.js";

const TYPING_MS = 150;
const RETRY_MS = 1000;

export class BufferSync {
  /**
   * `hooks`: `changed({source, reply?, valid?, stale?, frame?})` after the
   * engine text moved; `frame(text)` the `{steps, ...}` to send with a
   * `buffer.set` of `text` (optional); `dirty()` when the local-pending
   * state flips; `error(err, retry)` for a failure the user must see;
   * `offline(err)` when the engine is out of reach.
   */
  constructor(engine, editor, summary, hooks) {
    this.engine = engine;
    this.editor = editor;
    this.hooks = hooks;
    this.base = summary.text;
    this.version = summary.version;
    this.inflight = null;
    this.unsent = ChangeSet.empty(this.base.length);
    this.queued = [];
    this.needResync = false;
    this.resyncing = null;
    this.idle = [];
    this.schedule = debounce(() => this.send(), TYPING_MS);
  }

  /** `true` while local text has not reached the engine. */
  pending() {
    return this.inflight !== null || !this.unsent.empty;
  }

  /** `true` when the pane text equals the engine text at `version`. */
  synced() {
    return !this.pending() && this.resyncing === null;
  }

  /** A local edit from the pane. */
  localChange(changes) {
    const was = this.pending();
    this.unsent = this.unsent.compose(changes);
    this.schedule();
    if (!was) this.hooks.dirty();
  }

  /** Send unsent text now and wait until the engine holds the pane text. */
  async flush() {
    this.schedule.cancel();
    for (let round = 0; round < 8 && !this.synced(); round++) {
      if (this.resyncing) await this.resyncing;
      else if (this.inflight) await this.inflight.done;
      else if (!this.unsent.empty) await this.send();
    }
    if (!this.synced()) {
      throw new Error("the pane text did not reach the engine; check the connection and retry");
    }
  }

  /** `flush`, as `true` on success and `false` when the text is still out. */
  async tryFlush() {
    try {
      await this.flush();
      return true;
    } catch {
      return false;
    }
  }

  /** Send the pane text as `buffer.set`. */
  async send() {
    if (this.inflight || this.resyncing || this.unsent.empty) return;
    const text = this.editor.text();
    let resolve;
    const done = new Promise((r) => (resolve = r));
    this.inflight = { changes: this.unsent, text, done };
    this.unsent = ChangeSet.empty(text.length);
    const { env, frame } = await this.sendText(text);
    if (env.offline) {
      // The offline notice shows; the text goes out when the server answers.
      this.restoreInflight();
      resolve();
      await sleep(RETRY_MS);
      this.schedule();
      return;
    }
    if (env.ok) {
      this.base = text;
      this.version = env.version;
      this.inflight = null;
      resolve();
      this.hooks.changed({ source: "typing", reply: env.result, dirty: env.dirty, frame });
    } else {
      this.restoreInflight();
      resolve();
      if (env.error?.code === "editor.stale_version") this.needResync = true;
      else this.hooks.error(env, () => this.schedule());
    }
    await this.drain();
    if (!this.unsent.empty && !this.resyncing) this.schedule();
    if (!this.pending()) this.hooks.dirty();
  }

  /**
   * Send `text` as `buffer.set`. With a `frame` hook the follow-up commands
   * (render, outline, tokens, cursor) ride in the same `commands.batch`, so
   * a keystroke costs one engine call. Returns the `buffer.set` envelope
   * and `frame: {steps, envs, ...}` (the hook's steps, their envelopes, and
   * the hook's extra fields), or `frame: null`.
   */
  async sendText(text) {
    const set = { command: "buffer.set", params: { text }, version: this.version };
    const extra = this.hooks.frame?.(text);
    if (!extra?.steps?.length) {
      return { env: await this.engine.run("buffer.set", { text }, { version: this.version }), frame: null };
    }
    const steps = [set, ...extra.steps];
    const batch = await this.engine.run("commands.batch", { steps });
    const [env, ...envs] = splitBatch(batch, steps);
    if (batch.offline || !env.ok) return { env, frame: null };
    return { env, frame: { ...extra, envs } };
  }

  /** Put the inflight changes back in front of the unsent ones. */
  restoreInflight() {
    if (!this.inflight) return;
    this.unsent = this.inflight.changes.compose(this.unsent);
    this.inflight = null;
  }

  /** Handle events queued while a request was on the wire. */
  async drain() {
    const queued = this.queued;
    this.queued = [];
    for (const item of queued) this.remote(item);
    if (this.needResync) {
      this.needResync = false;
      await this.resync();
    }
  }

  /**
   * An engine change from an event: `delta` (UTF-16 `from`/`to`, `insert`)
   * turns the text at `baseVersion` into the text at `version`.
   * Returns `true` when it applied, `false` when a resync started.
   */
  remote({ delta, baseVersion, version, source, extra }) {
    if (this.inflight || this.resyncing) {
      this.queued.push({ delta, baseVersion, version, source, extra });
      return true;
    }
    if (version === this.version) return true;
    if (baseVersion !== this.version) {
      this.resync();
      return false;
    }
    if (delta) this.merge(ChangeSet.of([delta], this.base.length));
    this.version = version;
    this.hooks.changed({ source, ...extra });
    return true;
  }

  /**
   * Run an engine command that changes the text (`history.undo`,
   * `doc.format`, `file.reload`, offers, …). Local text reaches the engine
   * first, so the reply delta applies to `base`. Returns the envelope.
   */
  async edit(run, source) {
    if (!(await this.tryFlush())) return unsynced();
    let env = await run(this.version);
    if (!env.ok && env.error?.code === "editor.stale_version") {
      await this.resync();
      if (!(await this.tryFlush())) return unsynced();
      env = await run(this.version);
    }
    this.landed(env, source);
    return env;
  }

  /**
   * Run an edit made against text `version` (a gesture). Unlike `edit` it
   * never retries: when the text moved on since `version` (typing, another
   * client, a disk reload) it returns an `editor.stale_version` envelope and
   * runs nothing.
   */
  async editAt(version, run, source) {
    if (!(await this.tryFlush())) return unsynced();
    if (this.version !== version) return stale(version, this.version);
    const env = await run(version);
    if (!env.ok && env.error?.code === "editor.stale_version") {
      await this.resync();
      return env;
    }
    this.landed(env, source);
    return env;
  }

  /** Apply the delta of a successful edit envelope `env`. */
  landed(env, source) {
    if (!env.ok) return;
    const before = this.version;
    const delta = env.result?.delta;
    if (env.version !== before || delta) {
      if (delta) this.merge(ChangeSet.of([delta], this.base.length));
      this.version = env.version;
      this.hooks.changed({ source, reply: env.result, dirty: env.dirty });
    }
  }

  /** Read the engine text and merge the difference into the pane. */
  resync() {
    if (this.inflight) {
      this.needResync = true;
      return this.inflight.done;
    }
    if (this.resyncing) return this.resyncing;
    this.schedule.cancel();
    this.resyncing = (async () => {
      let summary = null;
      while (summary === null) {
        try {
          summary = await this.engine.state({ text: true });
        } catch (err) {
          this.hooks.offline(err);
          await sleep(RETRY_MS);
        }
      }
      const range = diffRange(this.base, summary.text);
      if (range) this.merge(ChangeSet.of([range], this.base.length));
      this.base = summary.text;
      this.version = summary.version;
      return summary;
    })();
    return this.resyncing.then((summary) => {
      this.resyncing = null;
      this.queued = this.queued.filter((q) => q.version > this.version);
      this.hooks.changed({ source: "resync", summary, dirty: summary.dirty });
      if (!this.unsent.empty) this.schedule();
      this.drain();
      return summary;
    });
  }

  /** Merge foreign change set `foreign` (over `base`) with local edits. */
  merge(foreign) {
    const local = this.unsent;
    this.editor.applyRemote(foreign.map(local));
    this.unsent = local.map(foreign, true);
    this.base = foreign.apply(textOf(this.base)).toString();
  }

  /** Map engine position `pos` (UTF-16, in `base`) into the pane text. */
  toPane(pos) {
    let p = Math.min(pos, this.base.length);
    if (this.inflight) p = this.inflight.changes.mapPos(p, 1);
    return this.unsent.mapPos(Math.min(p, this.unsent.length), 1);
  }
}

/** The envelope of an edit that could not run: local text is not on the server. */
function unsynced() {
  return {
    ok: false,
    command: "buffer.set",
    error: {
      code: "edit.unsynced",
      message: "the editor text has not reached zenith edit yet; check the connection and retry",
    },
  };
}

/** The envelope of a gesture made against `was` while the text is at `now`. */
function stale(was, now) {
  return {
    ok: false,
    command: "gesture.commit",
    error: {
      code: "editor.stale_version",
      message: `the gesture was made against version ${was}, and the text is at version ${now} now`,
    },
  };
}

/** A CodeMirror `Text` of `s`. The pane splits lines on `\n` only. */
function textOf(s) {
  return Text.of(s.split("\n"));
}
