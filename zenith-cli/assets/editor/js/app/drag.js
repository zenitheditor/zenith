// One pointer drag on the canvas: a move of a node (or of the selection),
// a drag of one of its handles (resize grip, rotate grip, endpoint,
// vertex, anchor), or a marquee band on empty canvas.
//
// The drag turns the pointer into `gesture.*` params: `dx`/`dy` page px
// from the press point, or `angle` degrees about the node (or selection)
// centre for the rotate grip, plus modifier flags. With several nodes
// selected it sends `nodes`. The engine maps them to ops; the drag does no
// document math. Modifiers:
//   Shift      move: keep to one axis; resize: keep the aspect ratio;
//              endpoint/vertex: 45° steps; rotate: 15° steps;
//              marquee: add to the selection
//   Alt        detach a token-bound axis or an anchor (`detach`,
//              `detach_anchor`); marquee: only nodes wholly inside
//   Ctrl/Cmd   resize about the centre (`from_center`); move: no snapping
// Snapping (`snap_distance`) follows the canvas Snap toggle.

/** Rotate snap with Shift, degrees. */
export const ROTATE_SNAP = 15;
/** Snap reach, screen px. */
export const SNAP_PX = 6;

export class Drag {
  /**
   * `ctl`: the gesture controller. `spec`: `{start, version, node?, nodes?,
   * item?, handle?, blocked?}`. A body drag without `node` / `nodes` asks
   * `ctl.target(start)` on its first move; with no node there it becomes a
   * marquee. `blocked` (a message) makes a node drag inert: it shows the
   * message and does nothing.
   */
  constructor(ctl, spec) {
    this.ctl = ctl;
    this.start = spec.start;
    this.version = spec.version;
    this.node = spec.node ?? null;
    this.nodes = spec.nodes ?? null;
    this.item = spec.item ?? null;
    this.handle = spec.handle ?? null;
    this.blocked = spec.blocked ?? null;
    this.point = spec.start;
    this.mods = { shift: false, alt: false, mod: false };
    this.done = false;
    this.inert = false;
    /** `true` once the drag turned out to be a marquee band. */
    this.marquee = false;
    this.resolving = null;
  }

  /** What the drag does, for hints: `move`, `rotate`, `resize`, `point`, or `marquee`. */
  get action() {
    if (this.marquee) return "marquee";
    if (!this.handle) return "move";
    if (this.handle.role === "rotate") return "rotate";
    if (this.handle.role === "resize") return "resize";
    return "point";
  }

  /** `true` once the drag knows what it moves. */
  get bound() {
    return !!(this.node || this.nodes);
  }

  move(p, e) {
    if (this.done) return;
    this.point = p;
    this.mods = modifiers(e);
    if (this.marquee) {
      this.ctl.band(this);
      return;
    }
    if (this.blocked && this.bound) {
      this.ctl.hint(this.blocked, p, "blocked");
      return;
    }
    if (this.inert) return;
    if (!this.bound) {
      this.resolve();
      return;
    }
    this.ctl.update(this);
  }

  /** A modifier key went down or up while dragging. */
  modifiers(e) {
    if (this.done || this.inert) return;
    this.mods = modifiers(e);
    if (this.marquee) this.ctl.band(this);
    else if (this.bound && !this.blocked) this.ctl.update(this);
  }

  /** Find the node under the press point (body drags); none starts a marquee. */
  resolve() {
    if (this.resolving) return;
    // A release before the hit returns still commits: `finish` waits.
    this.resolving = this.ctl.target(this.start).then((target) => {
      if (!target) {
        this.marquee = true;
        if (!this.done) this.ctl.band(this);
        return;
      }
      if (this.blocked) {
        this.node = target.node ?? null;
        this.nodes = target.nodes ?? null;
        if (!this.done) this.ctl.hint(this.blocked, this.point, "blocked");
        return;
      }
      this.node = target.node ?? null;
      this.nodes = target.nodes ?? null;
      this.item = target.item;
      if (!this.done) this.ctl.update(this);
    });
  }

  end(p, e) {
    if (this.done) return;
    this.point = p;
    this.mods = modifiers(e);
    this.done = true;
    this.ctl.finish(this, !this.inert);
  }

  cancel() {
    if (this.done) return;
    this.done = true;
    this.ctl.finish(this, false);
  }

  /** `true` when the pointer has not moved the node. */
  still() {
    const p = this.params();
    return !p.dx && !p.dy && !p.angle;
  }

  /** The node or nodes the params name. */
  target() {
    return this.nodes && this.nodes.length > 1 ? { nodes: this.nodes } : { node: this.node ?? this.nodes?.[0] };
  }

  /** The `gesture.*` params for the pointer now. */
  params() {
    const out = this.target();
    if (this.handle) out.handle = this.handle.id;
    if (this.action === "rotate") {
      out.angle = this.angle();
      if (this.mods.shift) out.snap = ROTATE_SNAP;
    } else {
      out.dx = this.point.x - this.start.x;
      out.dy = this.point.y - this.start.y;
      if (this.mods.shift) out.constrain = true;
      if (this.mods.mod && this.action === "resize") out.from_center = true;
      const snaps = this.action === "resize" || (this.action === "move" && !this.mods.mod);
      const reach = this.ctl?.snapReach() ?? 0;
      if (snaps && reach) out.snap_distance = reach;
    }
    if (this.mods.alt) {
      out.detach = true;
      out.detach_anchor = true;
    }
    return out;
  }

  /** The marquee band in page px. */
  band() {
    return { x0: this.start.x, y0: this.start.y, x1: this.point.x, y1: this.point.y };
  }

  /** Degrees clockwise the pointer turned about the node centre. */
  angle() {
    const c = this.item?.center;
    if (!c) return 0;
    const a0 = Math.atan2(this.start.y - c[1], this.start.x - c[0]);
    const a1 = Math.atan2(this.point.y - c[1], this.point.x - c[0]);
    let deg = ((a1 - a0) * 180) / Math.PI;
    if (deg > 180) deg -= 360;
    if (deg <= -180) deg += 360;
    return deg;
  }
}

/** The modifier flags of pointer or key event `e`. */
export function modifiers(e) {
  return { shift: !!e?.shiftKey, alt: !!e?.altKey, mod: !!(e?.ctrlKey || e?.metaKey) };
}
