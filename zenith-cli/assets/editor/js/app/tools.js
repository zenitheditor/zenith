// Tool state machine: Move (V), Text (T), Rectangle (R), Ellipse (O), Line (L), Hand (H).
// In drawing modes, dragging on the canvas previews a creation box and commits `node.add`,
// then returns to Move with the newly created node selected.

import { byId } from "../util/dom.js";

const VALID = new Set(["move", "text", "rect", "ellipse", "line", "hand"]);
const DRAW = new Set(["text", "rect", "ellipse", "line"]);
const DEFAULTS = {
  rect: { w: 120, h: 80 },
  ellipse: { w: 100, h: 100 },
  line: { w: 120, h: 0 },
  text: { w: 180, h: 32 },
};

export class ToolController {
  constructor(app) {
    this.app = app;
    this.mode = "move";
    this.buttons = [...document.querySelectorAll("[data-tool]")];
    this.bind();
  }

  bind() {
    for (const btn of this.buttons) {
      btn.addEventListener("click", () => this.set(btn.dataset.tool));
    }
  }

  isDrawing() {
    return DRAW.has(this.mode);
  }

  isHand() {
    return this.mode === "hand";
  }

  set(mode) {
    if (!VALID.has(mode)) return;
    this.mode = mode;
    for (const btn of this.buttons) {
      const active = btn.dataset.tool === mode;
      btn.classList.toggle("on", active);
      btn.setAttribute("aria-pressed", String(active));
    }
    const vp = byId("viewport");
    if (vp) {
      vp.classList.toggle("tool-hand", mode === "hand");
      vp.classList.toggle("tool-add", DRAW.has(mode));
    }
    this.app.gestures?.cursor();
  }

  /** Begin a creation drag at page point `start`. Returns a Drag-compatible handler. */
  beginDraw(start) {
    const kind = this.mode;
    const app = this.app;
    let cur = { ...start };
    let moved = false;
    const boxOf = () => {
      if (kind === "line") {
        return { x0: start.x, y0: start.y, x1: cur.x, y1: cur.y };
      }
      return {
        x0: Math.min(start.x, cur.x),
        y0: Math.min(start.y, cur.y),
        x1: Math.max(start.x, cur.x),
        y1: Math.max(start.y, cur.y),
      };
    };
    return {
      move: (p) => {
        cur = { ...p };
        if (Math.hypot(cur.x - start.x, cur.y - start.y) > 3) moved = true;
        const b = boxOf();
        app.overlay.setMarquee({
          x0: Math.min(b.x0, b.x1),
          y0: Math.min(b.y0, b.y1),
          x1: Math.max(b.x0, b.x1),
          y1: Math.max(b.y0, b.y1),
        });
      },
      end: async () => {
        app.overlay.setMarquee(null);
        const b = boxOf();
        const def = DEFAULTS[kind];
        let params;
        if (kind === "line") {
          params = moved
            ? { kind, x: start.x, y: start.y, x2: cur.x, y2: cur.y, page: app.page }
            : { kind, x: start.x, y: start.y, x2: start.x + def.w, y2: start.y, page: app.page };
        } else {
          const w = moved ? Math.max(8, b.x1 - b.x0) : def.w;
          const h = moved ? Math.max(8, b.y1 - b.y0) : def.h;
          params = { kind, x: b.x0, y: b.y0, w, h, page: app.page };
        }
        this.set("move");
        await app.actions.run("node.add", () => ({ params, done: `Added ${kind}.` }));
      },
      cancel: () => {
        app.overlay.setMarquee(null);
      },
    };
  }

  /** Click-to-place when a drawing tool is active and the pointer didn't drag. */
  async clickDraw(p) {
    if (!this.isDrawing()) return false;
    const kind = this.mode;
    const def = DEFAULTS[kind];
    const params =
      kind === "line"
        ? { kind, x: p.x, y: p.y, x2: p.x + def.w, y2: p.y, page: this.app.page }
        : { kind, x: p.x, y: p.y, w: def.w, h: def.h, page: this.app.page };
    this.set("move");
    await this.app.actions.run("node.add", () => ({ params, done: `Added ${kind}.` }));
    return true;
  }
}
