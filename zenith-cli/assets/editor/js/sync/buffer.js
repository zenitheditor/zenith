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
// Every exchange that moves the text (a `buffer.set`, an engine edit, a
// state read) is a task. Tasks run one at a time, in call order, so no
// reply ever lands over a text another reply moved. Typing never waits:
// keystrokes made while a task runs collect in `unsent`. Events that arrive
// while a task runs wait in `queued` and apply after it.
//
// A foreign change F over `base` merges with local changes L over `base`
// by rebasing: the pane applies F mapped through L, and L becomes L mapped
// through F. Keystrokes are never dropped. When the version does not match
// (`editor.stale_version`, a missed event, a reconnect), the page reads
// `GET /api/state?text=1` and merges the difference the same way.
//
// Positions are UTF-16 code units, as in the engine deltas. The pane splits
// lines on `\n` only (see code/editor.js), so every change set built here
// does the same: a `\r` stays text and counts one unit.

import { ChangeSet, Text } from "../../vendor/codemirror.js";
import { splitBatch } from "../engine/batch.js";
import { diffRange } from "../util/text.js";
import { debounce, sleep } from "../util/timing.js";

const TYPING_MS = 150;
const RETRY_MS = 1000;
/** The line separator of the pane and of every change set here. */
const LINE_SEP = "\n";
/** Rounds of send and resync one flush tries before it gives up. */
const FLUSH_ROUNDS = 8;

export class BufferSync {
  /**
   * `hooks`: `changed({source, reply?, valid?, stale?, frame?})` after the
   * engine text moved; `frame(text)` the `{steps, ...}` to send with a
   * `buffer.set` of `text` (optional); `dirty()` when the local-pending
   * state flips; `error(err, retry)` for an error the user must see;
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
    /** `true` while an engine edit or a state read is on the wire. */
    this.waiting = false;
    /** `true` while a resync is due: the engine text may differ from `base`. */
    this.stale = false;
    /** `true` while a task runs. */
    this.running = false;
    /** `true` after a failed send: only a keystroke or a retry sends again. */
    this.held = false;
    this.tail = Promise.resolve();
    this.wasPending = false;
    this.schedule = debounce(() => this.send(), TYPING_MS);
  }

  /** `true` while local text has not reached the engine. */
  pending() {
    return this.inflight !== null || !this.unsent.empty;
  }

  /** `true` when the pane text equals the engine text at `version`. */
  synced() {
    return !this.pending() && !this.waiting && !this.stale;
  }

  /** A local edit from the pane. */
  localChange(changes) {
    this.unsent = this.unsent.compose(changes);
    this.schedule();
    this.notifyDirty();
  }

  /** Call the `dirty` hook when `pending()` flipped since the last call. */
  notifyDirty() {
    const now = this.pending();
    if (now === this.wasPending) return;
    this.wasPending = now;
    this.hooks.dirty();
  }

  /**
   * Run `task` alone: after every task queued before it, and before any
   * queued after it. Queued events apply when it ends. Resolves what
   * `task` resolves.
   */
  exclusive(task) {
    const run = this.tail.then(async () => {
      this.running = true;
      try {
        return await task();
      } finally {
        try {
          await this.drainNow();
        } finally {
          this.running = false;
          if (!this.unsent.empty && !this.held && !this.schedule.pending()) this.schedule();
          this.notifyDirty();
        }
      }
    });
    this.tail = run.then(
      () => {},
      () => {},
    );
    return run;
  }

  /** Send unsent text now and wait until the engine holds the pane text. */
  async flush() {
    this.schedule.cancel();
    if (!(await this.exclusive(() => this.flushNow()))) {
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

  /** Send the pane text as `buffer.set` (a task). */
  send() {
    return this.exclusive(() => this.sendNow());
  }

  /**
   * Inside a task: send and resync until the engine holds the pane text.
   * `true` on success, `false` when a send failed (offline, an engine error).
   */
  async flushNow() {
    this.schedule.cancel();
    for (let round = 0; round < FLUSH_ROUNDS && !this.synced(); round++) {
      if (this.stale) await this.resyncNow();
      else if (!(await this.sendNow()) && !this.stale) return false;
    }
    return this.synced();
  }

  /**
   * Inside a task: send the pane text as `buffer.set`. `true` when the
   * engine took it (or there was nothing to send), `false` otherwise. An
   * `editor.stale_version` answer sets `stale`.
   */
  async sendNow() {
    this.held = false;
    if (this.unsent.empty) return true;
    const text = this.editor.text();
    this.inflight = { changes: this.unsent, text };
    this.unsent = ChangeSet.empty(text.length);
    let reply;
    try {
      reply = await this.sendText(text);
    } catch (err) {
      this.restoreInflight();
      throw err;
    }
    const { env, frame } = reply;
    if (env.offline) {
      // The offline notice shows; the text goes out when the engine answers.
      this.restoreInflight();
      this.held = true;
      sleep(RETRY_MS).then(() => {
        this.held = false;
        this.schedule();
      });
      return false;
    }
    if (env.ok) {
      this.base = text;
      this.version = env.version;
      this.inflight = null;
      this.hooks.changed({ source: "typing", reply: env.result, dirty: env.dirty, frame });
      return true;
    }
    this.restoreInflight();
    if (env.error?.code === "editor.stale_version") {
      this.stale = true;
    } else {
      this.held = true;
      this.hooks.error(env, () => this.schedule());
    }
    return false;
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

  /** Inside a task: apply the queued events, and resync when one is due. */
  async drainNow() {
    while (this.queued.length || this.stale) {
      if (this.stale) {
        await this.resyncNow();
        continue;
      }
      const item = this.queued.shift();
      if (!this.applyEvent(item)) this.stale = true;
    }
  }

  /**
   * An engine change from an event: `delta` (UTF-16 `from`/`to`, `insert`)
   * turns the text at `baseVersion` into the text at `version`. It applies
   * now, or after the running task. A version gap starts a resync.
   */
  remote(item) {
    if (this.running) {
      this.queued.push(item);
      return;
    }
    if (!this.applyEvent(item)) this.resync();
  }

  /**
   * Apply event `item` over `base` (no task may hold changes on the wire).
   * `false` when it does not follow `version`: a resync is due.
   */
  applyEvent({ delta, baseVersion, version, source, extra }) {
    // Versions only grow: an event at or before `version` is already in.
    if (version <= this.version) return true;
    if (baseVersion !== this.version) return false;
    if (delta) this.merge(changeOf(delta, this.base.length));
    this.version = version;
    this.hooks.changed({ source, ...extra });
    return true;
  }

  /**
   * Run an engine command that changes the text (`history.undo`,
   * `doc.format`, `file.reload`, offers, …). Local text reaches the engine
   * first, so the reply delta applies to `base`. Keystrokes made while the
   * command runs rebase over its delta. Returns the envelope.
   */
  edit(run, source) {
    return this.exclusive(async () => {
      if (!(await this.flushNow())) return unsynced();
      let env = await this.engineEdit(run);
      if (!env.ok && env.error?.code === "editor.stale_version") {
        await this.resyncNow();
        if (!(await this.flushNow())) return unsynced();
        env = await this.engineEdit(run);
      }
      this.landed(env, source);
      return env;
    });
  }

  /**
   * Run an edit made against text `version` (a gesture). Unlike `edit` it
   * never retries: when the text moved on since `version` (typing, another
   * client, a disk reload) it returns an `editor.stale_version` envelope and
   * runs nothing.
   */
  editAt(version, run, source) {
    return this.exclusive(async () => {
      if (!(await this.flushNow())) return unsynced();
      if (this.version !== version) return stale(version, this.version);
      const env = await this.engineEdit(run);
      if (!env.ok && env.error?.code === "editor.stale_version") {
        await this.resyncNow();
        return env;
      }
      this.landed(env, source);
      return env;
    });
  }

  /** Inside a task: `run(version)`, marked as on the wire. */
  async engineEdit(run) {
    this.waiting = true;
    try {
      return await run(this.version);
    } finally {
      this.waiting = false;
    }
  }

  /** Apply the delta of a successful edit envelope `env`. */
  landed(env, source) {
    if (!env.ok) return;
    const delta = env.result?.delta;
    if (env.version !== this.version || delta) {
      if (delta) this.merge(changeOf(delta, this.base.length));
      this.version = env.version;
      this.hooks.changed({ source, reply: env.result, dirty: env.dirty });
    }
  }

  /**
   * Read the engine text and merge the difference into the pane (a task).
   * Resolves the state summary, always: a failed read retries until the
   * engine answers.
   */
  resync() {
    this.stale = true;
    return this.exclusive(() => this.resyncNow());
  }

  /** Inside a task: `resync`. */
  async resyncNow() {
    this.stale = true;
    this.waiting = true;
    let summary = null;
    try {
      while (summary === null) {
        try {
          summary = await this.engine.state({ text: true });
        } catch (err) {
          this.hooks.offline(err);
          await sleep(RETRY_MS);
        }
      }
    } finally {
      this.waiting = false;
    }
    const range = diffRange(this.base, summary.text);
    if (range) this.merge(changeOf(range, this.base.length));
    this.base = summary.text;
    this.version = summary.version;
    this.stale = false;
    this.queued = this.queued.filter((q) => q.version > this.version);
    this.hooks.changed({ source: "resync", summary, dirty: summary.dirty });
    return summary;
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

/**
 * The change set of `{from, to, insert}` over a text of `length`. The
 * insert splits on `\n` only, as the pane does; the default would also
 * split on `\r\n` and `\r` and drop every `\r`.
 */
export function changeOf(change, length) {
  return ChangeSet.of([change], length, LINE_SEP);
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
  return Text.of(s.split(LINE_SEP));
}
