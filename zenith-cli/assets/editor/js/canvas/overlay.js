// The selection overlay: an SVG above the page image in page coordinates.
//
// It draws the hover outline, the selection outlines from `node.handles`
// corners, the handles (resize grips, the rotate grip on its stem, line
// endpoints, vertices, path anchors and their control handles), a lock
// badge when the engine blocks a plain drag, the ghost outline of a gesture
// in progress, the marquee band, and snap guides. With several nodes
// selected it draws each node's outline and the selection box
// (`node.handles {ids}`) with its grips. Sizes are screen px: the overlay
// redraws on zoom. `handleAt` finds the handle under a page point for the
// gesture layer.

const NS = "http://www.w3.org/2000/svg";
/** Square grip side, screen px. */
const GRIP_PX = 8;
/** Point handle (endpoint, vertex, anchor) radius, screen px. */
const POINT_PX = 4.5;
/** Control handle radius, screen px. */
const CONTROL_PX = 3.5;
/** Rotate grip radius, screen px. */
const ROTATE_PX = 5;
/** Edge grips show only when the edge is at least this long, screen px. */
const MIN_EDGE_PX = 40;
/** Corner grips show only when the box is at least this big, screen px. */
const MIN_BOX_PX = 12;
/** Lock badge side, screen px. */
const BADGE_PX = 14;
/** Guides reach this far past the boxes they join, screen px. */
const GUIDE_REACH_PX = 6;
/** Codes that block a plain move or resize: the lock badge shows. */
const LOCKING = new Set(["tx.token_bound", "tx.anchored", "tx.computed_size", "tx.layout_managed", "editor.locked"]);

export class Overlay {
  constructor(svg, view) {
    this.svg = svg;
    this.view = view;
    this.hover = null;
    this.selection = [];
    /** The selection box of several nodes (`node.handles {ids}`), or `null`. */
    this.group = null;
    /** `{corners, members?, blocked}` of a gesture in progress, or `null`. */
    this.ghost = null;
    /** The marquee band `{x0, y0, x1, y1}` (page px), or `null`. */
    this.marquee = null;
    /** Snap guides `[{axis, at, from, to}]` (page px). */
    this.guides = [];
    /** Handles drawn by the last `draw`, for `handleAt`. */
    this.drawn = [];
  }

  /** Outline `corners` (four `[x, y]` page points) as the hover, or clear. */
  setHover(corners) {
    this.hover = corners;
    this.draw();
  }

  /**
   * Draw `items`: `[{id, corners, center?, handles?: [{id, role, x, y,
   * enabled, reason?}], disabled?: [{action, code}], stale}]`, and with
   * several, `group`: the selection box reply. A new selection ends any
   * ghost and guides.
   */
  setSelection(items, group = null) {
    this.selection = items;
    this.group = items.length > 1 ? group : null;
    this.ghost = null;
    this.guides = [];
    this.draw();
  }

  /** Show the gesture ghost `{corners, members?, blocked}`, or clear it with `null`. */
  setGhost(ghost) {
    this.ghost = ghost;
    if (!ghost) this.guides = [];
    this.draw();
  }

  /** Show the marquee band `{x0, y0, x1, y1}` (page px), or clear it with `null`. */
  setMarquee(band) {
    this.marquee = band;
    this.draw();
  }

  /** Show snap guides `[{axis, at, from, to}]`; `null` or `[]` clears them. */
  setGuides(guides) {
    this.guides = guides ?? [];
    this.draw();
  }

  /** The one selected item, or `null`. */
  single() {
    return this.selection.length === 1 ? this.selection[0] : null;
  }

  /** What the handles belong to: the one selected item, or the selection box. */
  handleItem() {
    return this.single() ?? this.group;
  }

  /**
   * The handle of the single selection (or the selection box) nearest page
   * point `p` within `radiusPx` screen px, or `null`. Only handles drawn
   * now count.
   */
  handleAt(p, radiusPx) {
    const zoom = this.view.zoom || 1;
    let best = null;
    let bestD = radiusPx / zoom;
    // Later handles draw on top: on a tie the later one wins.
    for (const h of this.drawn) {
      const d = Math.hypot(h.x - p.x, h.y - p.y);
      if (d <= bestD) {
        best = h;
        bestD = d;
      }
    }
    return best;
  }

  /** Redraw after a zoom change: handles keep their screen size. */
  draw() {
    const svg = this.svg;
    while (svg.firstChild) svg.removeChild(svg.firstChild);
    this.drawn = [];
    const zoom = this.view.zoom || 1;
    const unit = 1 / zoom;
    const hoverSelected = this.selection.some((s) => sameCorners(s.corners, this.hover));
    if (this.hover && !hoverSelected && !this.ghost && !this.marquee) {
      svg.appendChild(polygon(this.hover, "hover-halo"));
      svg.appendChild(polygon(this.hover, "hover"));
    }
    const single = this.single();
    const many = this.group && this.selection.length > 1;
    for (const item of this.selection) {
      if (!item.corners) continue;
      const g = el("g", { class: item.stale ? "stale" : "" });
      g.appendChild(polygon(item.corners, "selected-halo"));
      g.appendChild(polygon(item.corners, many ? "selected member" : "selected"));
      if (item === single && !this.ghost) this.handles(g, item, zoom, unit);
      svg.appendChild(g);
    }
    if (many && this.group.corners) {
      const g = el("g", { class: this.group.stale ? "stale" : "" });
      g.appendChild(polygon(this.group.corners, "selected-halo"));
      g.appendChild(polygon(this.group.corners, "group"));
      if (!this.ghost) this.handles(g, this.group, zoom, unit);
      svg.appendChild(g);
    }
    if (this.ghost?.corners) {
      const cls = this.ghost.blocked ? "ghost blocked" : "ghost";
      for (const m of this.ghost.members ?? []) {
        if (!m.corners) continue;
        svg.appendChild(polygon(m.corners, "selected-halo"));
        svg.appendChild(polygon(m.corners, `${cls} member`));
      }
      svg.appendChild(polygon(this.ghost.corners, "selected-halo"));
      svg.appendChild(polygon(this.ghost.corners, cls));
    }
    for (const guide of this.guides) this.guide(svg, guide, unit);
    if (this.marquee) {
      const { x0, y0, x1, y1 } = this.marquee;
      const box = { x: Math.min(x0, x1), y: Math.min(y0, y1), width: Math.abs(x1 - x0), height: Math.abs(y1 - y0) };
      svg.appendChild(el("rect", { class: "marquee-halo", ...box }));
      svg.appendChild(el("rect", { class: "marquee", ...box }));
    }
  }

  /** One snap guide line, a little past the boxes it joins. */
  guide(svg, { axis, at, from, to }, unit) {
    const reach = GUIDE_REACH_PX * unit;
    const line = axis === "x"
      ? { x1: at, y1: from - reach, x2: at, y2: to + reach }
      : { x1: from - reach, y1: at, x2: to + reach, y2: at };
    svg.appendChild(el("line", { class: "guide-halo", ...line }));
    svg.appendChild(el("line", { class: "guide", ...line }));
  }

  /** The handles of the single selected `item` (or the selection box) into group `g`. */
  handles(g, item, zoom, unit) {
    const [a, b, c] = item.corners;
    const edgeW = Math.hypot(b[0] - a[0], b[1] - a[1]) * zoom;
    const edgeH = Math.hypot(c[0] - b[0], c[1] - b[1]) * zoom;
    const wide = edgeW >= MIN_EDGE_PX;
    const tall = edgeH >= MIN_EDGE_PX;
    const roomy = Math.max(edgeW, edgeH) >= MIN_BOX_PX;
    const handles = item.handles ?? [];
    const byId = new Map(handles.map((h) => [h.id, h]));
    const shown = handles.filter((h) => {
      if (h.role !== "resize") return true;
      if ((h.id === "n" || h.id === "s") && !wide) return false;
      if ((h.id === "e" || h.id === "w") && !tall) return false;
      // On a tiny node the grips would hide it: keep only the far corner.
      return roomy || h.id === "se";
    });
    // Points draw (and hit) over grips: a grip never hides a vertex or an
    // anchor that shares its place.
    const layer = { resize: 0, rotate: 1 };
    shown.sort((p, q) => (layer[p.role] ?? 2) - (layer[q.role] ?? 2));
    // Stems first, so the handles draw over them.
    for (const h of shown) {
      if (h.role === "rotate") {
        const top = mid(a, b);
        g.appendChild(el("line", { class: "stem", x1: top[0], y1: top[1], x2: h.x, y2: h.y }));
      } else if (h.role === "control") {
        const anchor = byId.get(h.id.replace(/\.(in|out)$/, ""));
        if (anchor) g.appendChild(el("line", { class: "stem", x1: anchor.x, y1: anchor.y, x2: h.x, y2: h.y }));
      }
    }
    for (const h of shown) {
      const cls = `handle ${h.role}${h.enabled ? "" : " off"}`;
      const data = { "data-handle": h.id };
      if (h.role === "resize") {
        const s = GRIP_PX * unit;
        g.appendChild(el("rect", { class: cls, x: h.x - s / 2, y: h.y - s / 2, width: s, height: s, ...data }));
      } else {
        const r = (h.role === "rotate" ? ROTATE_PX : h.role === "control" ? CONTROL_PX : POINT_PX) * unit;
        g.appendChild(el("circle", { class: cls, cx: h.x, cy: h.y, r, ...data }));
      }
      this.drawn.push(h);
    }
    if ((item.disabled ?? []).some((d) => LOCKING.has(d.code) && d.action !== "rotate")) {
      this.badge(g, item.corners, unit);
    }
  }

  /** A lock badge just outside the top-left corner. */
  badge(g, corners, unit) {
    const s = BADGE_PX * unit;
    const [x, y] = corners[0];
    const bx = x - s - 4 * unit;
    const by = y - s - 4 * unit;
    const badge = el("g", { class: "lock-badge", transform: `translate(${bx} ${by}) scale(${s / 16})` });
    badge.appendChild(el("rect", { class: "lock-bg", x: 0, y: 0, width: 16, height: 16, rx: 3 }));
    badge.appendChild(el("rect", { class: "lock-glyph", x: 4.5, y: 7.5, width: 7, height: 5, rx: 1 }));
    badge.appendChild(el("path", { class: "lock-glyph", d: "M6 7.5V6a2 2 0 0 1 4 0v1.5" }));
    g.appendChild(badge);
  }
}

/**
 * The CSS cursor for handle `h` of `item`: a resize arrow along the handle's
 * direction from the centre, `grab` for rotate, `crosshair` for points.
 */
export function handleCursor(h, item) {
  if (!h.enabled && h.role !== "rotate") return "not-allowed";
  if (h.role === "rotate") return "grab";
  if (h.role !== "resize") return "crosshair";
  const c = item.center ?? centroid(item.corners);
  const deg = ((Math.atan2(h.y - c[1], h.x - c[0]) * 180) / Math.PI + 360) % 180;
  const bucket = Math.round(deg / 45) % 4;
  return ["ew-resize", "nwse-resize", "ns-resize", "nesw-resize"][bucket];
}

/** `true` when page point `p` is inside the polygon `corners`. */
export function inside(corners, p) {
  let hit = false;
  for (let i = 0, j = corners.length - 1; i < corners.length; j = i++) {
    const [xi, yi] = corners[i];
    const [xj, yj] = corners[j];
    if (yi > p.y !== yj > p.y && p.x < ((xj - xi) * (p.y - yi)) / (yj - yi) + xi) hit = !hit;
  }
  return hit;
}

function centroid(corners) {
  const x = corners.reduce((s, p) => s + p[0], 0) / corners.length;
  const y = corners.reduce((s, p) => s + p[1], 0) / corners.length;
  return [x, y];
}

function mid(a, b) {
  return [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2];
}

function polygon(corners, cls) {
  return el("polygon", { class: cls, points: corners.map(([x, y]) => `${x},${y}`).join(" ") });
}

function el(tag, attrs) {
  const node = document.createElementNS(NS, tag);
  for (const [k, v] of Object.entries(attrs)) if (v !== "") node.setAttribute(k, String(v));
  return node;
}

function sameCorners(a, b) {
  if (!a || !b) return false;
  return a.every(([x, y], i) => Math.abs(x - b[i][0]) < 1e-6 && Math.abs(y - b[i][1]) < 1e-6);
}
