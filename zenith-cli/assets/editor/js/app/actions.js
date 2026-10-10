// Node actions from the keyboard and the inspector: nudge, resize, rotate,
// delete, duplicate, set properties, and clear the selection. Each is one
// engine command; the reply delta reaches the code pane through BufferSync.
//
// Keys on the focused canvas, with one or more nodes selected (several act
// as one selection, in one transaction):
//   Arrows             move 1 page px (Shift: 10)
//   Ctrl/Cmd + arrows  resize from the bottom-right grip (Shift: 10); with
//                      several nodes, of the selection box
//   Alt (with either)  detach a token-bound axis or an anchor
//   [ and ]            rotate 15° counter-clockwise / clockwise (several
//                      nodes turn about the selection centre)
//   Delete, Backspace  remove the selection
//   Escape             stop a drag, else leave fullscreen, else clear the
//                      selection
// Ctrl/Cmd + D anywhere outside the code pane and text fields duplicates
// the selection.
//
// Every action is an intent (see `App.intents`): it runs after the clicks
// and keys before it, and reads the selection when it runs. So a key
// pressed while a click's reply is on the wire acts on the selection that
// click made, and back-to-back keys apply in order, each on the text the
// last one left.

const STEP = 1;
const BIG_STEP = 10;
const TURN = 15;
const ARROWS = {
  ArrowLeft: [-1, 0],
  ArrowRight: [1, 0],
  ArrowUp: [0, -1],
  ArrowDown: [0, 1],
};

export class NodeActions {
  constructor(app) {
    this.app = app;
  }

  /**
   * CanvasView `key`: `true` when the key ran an action. The page has no
   * selection and no intent waits: the key is not an action (arrows pan).
   * While an intent waits the selection can still change, so the key is
   * an action, and it does nothing when the selection is empty by then.
   */
  key(e) {
    const app = this.app;
    if (e.key === "Escape") {
      if (app.gestures.cancel()) return true;
      // In a fullscreen mode Escape restores the layout first.
      if (app.layout.full !== "none" || (app.selectionIds.length === 0 && app.intents.idle())) return false;
      app.selection.selectIds([], "keys");
      return true;
    }
    if (app.selectionIds.length === 0 && app.intents.idle()) return false;
    const mod = e.ctrlKey || e.metaKey;
    const arrow = ARROWS[e.key];
    if (arrow) {
      const step = e.shiftKey ? BIG_STEP : STEP;
      const params = { dx: arrow[0] * step, dy: arrow[1] * step };
      if (mod) params.handle = "se";
      if (e.altKey) {
        params.detach = true;
        params.detach_anchor = true;
      }
      this.gesture(params, mod ? "Resized" : "Moved");
      return true;
    }
    if (mod || e.altKey) return false;
    if (e.key === "[" || e.key === "]") {
      this.gesture({ handle: "rotate", angle: e.key === "]" ? TURN : -TURN }, "Rotated");
      return true;
    }
    if (e.key === "Delete" || e.key === "Backspace") {
      this.remove();
      return true;
    }
    return false;
  }

  /** Run a key gesture on the selection (one node, or several as one). */
  gesture(params, verb) {
    this.run("gesture.commit", (ids) => {
      if (ids.length === 0) return null;
      const target = ids.length === 1 ? { node: ids[0] } : { nodes: ids };
      const what = ids.length === 1 ? ids[0] : `${ids.length} nodes`;
      return { params: { ...target, ...params }, done: `${verb} ${what}.` };
    });
  }

  /** Remove the selected nodes. */
  remove() {
    this.run("node.remove", (ids) => (ids.length ? { params: { ids }, done: `Removed ${ids.join(", ")}.` } : null));
  }

  /** Duplicate the selection in place; the copies become the selection. */
  duplicate() {
    const app = this.app;
    this.run("node.duplicate", (ids) => {
      if (ids.length === 0) {
        app.notices.toast("Select a node to duplicate it.", "info");
        return null;
      }
      const params = ids.length === 1 ? { id: ids[0] } : { ids };
      return { params, done: `Duplicated ${ids.join(", ")}.` };
    });
  }

  /** Inspector write: `node.set` on `id` with `params`. Resolves `true` on success. */
  set(id, params) {
    return this.run("node.set", () => ({ params: { id, ...params }, done: `Updated ${id}.` }));
  }

  /** `false` (and a notice) while the text has errors. */
  ready() {
    const app = this.app;
    if (app.valid && !app.stale) return true;
    app.notices.toast("The source has errors. Fix them to edit nodes.", "warning");
    return false;
  }

  /**
   * Queue an edit with `command` as an intent. When it runs, `build(ids)`
   * gets the selection then and returns `{params, done}` (`done`: what to
   * announce), or `null` to run nothing. Resolves `true` when the edit
   * landed.
   */
  run(command, build) {
    const app = this.app;
    return app.intents.run(async () => {
      if (!this.ready()) return false;
      const edit = build([...app.selectionIds]);
      if (!edit) return false;
      const env = await app.sync.edit((version) => app.engine.run(command, edit.params, { version }), command);
      if (env.ok) {
        app.notices.hide(`error:${command}`);
        app.announce(edit.done);
        return true;
      }
      app.notices.error(command, env, { runOffer: (o) => app.runOffer(o) });
      return false;
    });
  }
}
