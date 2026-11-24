// Canvas Rulers (Shift+R): 20px top & left DPR-aware pixel rulers with
// zoom-adaptive tick intervals (1/2/5 × 10^n), rotated vertical labels, and
// accent-tinted selection range highlight bands.

const SIZE = 20;

function pickStep(zoom) {
  const minPx = 48;
  const raw = minPx / Math.max(zoom, 0.01);
  const pow = Math.pow(10, Math.floor(Math.log10(raw)));
  for (const m of [1, 2, 5, 10]) {
    if (m * pow >= raw) return m * pow;
  }
  return 10 * pow;
}

export class CanvasRulers {
  constructor(viewport, view, { on = false } = {}) {
    this.viewport = viewport;
    this.view = view;
    this.on = Boolean(on);
    this.selBox = null;
    this.canvas = document.createElement("canvas");
    this.canvas.className = "canvas-rulers";
    this.canvas.setAttribute("aria-hidden", "true");
    this.canvas.hidden = !this.on;
    this.viewport.appendChild(this.canvas);
    this.ctx = this.canvas.getContext("2d");
  }

  setVisible(on) {
    this.on = Boolean(on);
    this.canvas.hidden = !this.on;
    if (this.on) this.draw();
  }

  setSelectionBox(box) {
    this.selBox = box || null;
    if (this.on) this.draw();
  }

  draw() {
    if (!this.on || !this.ctx) return;
    const rect = this.viewport.getBoundingClientRect();
    const w = Math.max(0, Math.round(rect.width));
    const h = Math.max(0, Math.round(rect.height));
    if (w < SIZE || h < SIZE) return;

    const dpr = Math.max(1, window.devicePixelRatio || 1);
    const cw = Math.round(w * dpr);
    const ch = Math.round(h * dpr);
    if (this.canvas.width !== cw || this.canvas.height !== ch) {
      this.canvas.width = cw;
      this.canvas.height = ch;
    }

    const ctx = this.ctx;
    ctx.save();
    ctx.scale(dpr, dpr);
    ctx.clearRect(0, 0, w, h);

    const cs = getComputedStyle(document.documentElement);
    const bg = cs.getPropertyValue("--surface").trim() || "#ffffff";
    const border = cs.getPropertyValue("--border").trim() || "#e2e8f0";
    const tick = cs.getPropertyValue("--text-faint").trim() || "#5d6a7c";
    const accent = cs.getPropertyValue("--accent").trim() || "#0870a6";
    const accentSoft = cs.getPropertyValue("--accent-soft").trim() || "rgba(8,112,166,0.12)";

    const z = this.view.zoom || 1;
    const px = this.view.px || 0;
    const py = this.view.py || 0;
    const step = pickStep(z);
    const subStep = step / 5;

    // Ruler bars
    ctx.fillStyle = bg;
    ctx.fillRect(0, 0, w, SIZE);
    ctx.fillRect(0, 0, SIZE, h);

    // Selection range highlight bands
    if (this.selBox) {
      const x0 = Math.max(SIZE, px + this.selBox.x * z);
      const x1 = Math.min(w, px + (this.selBox.x + this.selBox.w) * z);
      const y0 = Math.max(SIZE, py + this.selBox.y * z);
      const y1 = Math.min(h, py + (this.selBox.y + this.selBox.h) * z);
      ctx.fillStyle = accentSoft;
      if (x1 > x0) ctx.fillRect(x0, 0, x1 - x0, SIZE);
      if (y1 > y0) ctx.fillRect(0, y0, SIZE, y1 - y0);
      ctx.fillStyle = accent;
      if (x1 > x0) ctx.fillRect(x0, SIZE - 2, x1 - x0, 2);
      if (y1 > y0) ctx.fillRect(SIZE - 2, y0, 2, y1 - y0);
    }

    // Horizontal ticks & labels
    ctx.strokeStyle = border;
    ctx.fillStyle = tick;
    ctx.font = "500 9.5px ui-monospace, SFMono-Regular, Menlo, monospace";
    ctx.textBaseline = "top";
    ctx.textAlign = "left";

    const startX = Math.floor((SIZE - px) / z / step) * step;
    const endX = Math.ceil((w - px) / z / step) * step;
    ctx.beginPath();
    for (let v = startX; v <= endX; v += step) {
      const sx = Math.round(px + v * z) + 0.5;
      if (sx >= SIZE && sx <= w) {
        ctx.moveTo(sx, SIZE - 8);
        ctx.lineTo(sx, SIZE);
        ctx.fillText(String(Math.round(v)), sx + 3, 2);
      }
      for (let i = 1; i < 5; i++) {
        const subX = Math.round(px + (v + i * subStep) * z) + 0.5;
        if (subX >= SIZE && subX <= w) {
          ctx.moveTo(subX, SIZE - 4);
          ctx.lineTo(subX, SIZE);
        }
      }
    }

    // Vertical ticks & rotated labels
    const startY = Math.floor((SIZE - py) / z / step) * step;
    const endY = Math.ceil((h - py) / z / step) * step;
    for (let v = startY; v <= endY; v += step) {
      const sy = Math.round(py + v * z) + 0.5;
      if (sy >= SIZE && sy <= h) {
        ctx.moveTo(SIZE - 8, sy);
        ctx.lineTo(SIZE, sy);
        ctx.save();
        ctx.translate(2, sy + 3);
        ctx.rotate(Math.PI / 2);
        ctx.fillText(String(Math.round(v)), 0, -10);
        ctx.restore();
      }
      for (let i = 1; i < 5; i++) {
        const subY = Math.round(py + (v + i * subStep) * z) + 0.5;
        if (subY >= SIZE && subY <= h) {
          ctx.moveTo(SIZE - 4, subY);
          ctx.lineTo(SIZE, subY);
        }
      }
    }
    ctx.stroke();

    // Borders + corner square
    ctx.fillStyle = bg;
    ctx.fillRect(0, 0, SIZE, SIZE);
    ctx.strokeStyle = border;
    ctx.beginPath();
    ctx.moveTo(0, SIZE + 0.5);
    ctx.lineTo(w, SIZE + 0.5);
    ctx.moveTo(SIZE + 0.5, 0);
    ctx.lineTo(SIZE + 0.5, h);
    ctx.stroke();

    ctx.restore();
  }
}
