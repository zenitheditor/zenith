// Inline canvas text editor: double-clicking a text node on the canvas,
// pressing Enter on a selected text node, or placing a new Text node with the
// T tool opens a floating textarea positioned directly over the node's screen
// bounding box. Cmd/Ctrl+Enter or blur commits via `node.set`; Escape cancels.

import { cssColor } from "../util/color.js";
import { h } from "../util/dom.js";

export class InlineTextEditor {
  constructor(app, viewport) {
    this.app = app;
    this.viewport = viewport;
    this.active = null;
  }

  isOpen() {
    return this.active !== null;
  }

  /**
   * Open inline text editing for `info` (a `node.inspect` reply for a text node).
   * When `selectAll` is true (e.g. newly placed Text node), selects all text so
   * the user can type immediately.
   */
  open(info, { selectAll = false } = {}) {
    this.close({ commit: true });
    if (!info || info.kind !== "text" || !info.edit?.fields?.includes("text") || !info.box) return false;
    if (!this.app.valid || this.app.stale) return false;
    const lock = info.lock ?? {};
    if (lock.locked_by || lock.hidden_by) return false;

    const view = this.app.view;
    const b = info.box;
    const z = view.zoom || 1;
    const left = view.px + b.x * z;
    const top = view.py + b.y * z;
    const width = Math.max(96, b.w * z);
    const height = Math.max(30, b.h * z);

    const fontFamily = info.edit.font_family?.resolved || "Noto Sans";
    const fontSize = Math.max(11, (info.edit.font_size?.resolved ?? 16) * z);
    const fontWeight = info.edit.font_weight?.resolved ?? 400;
    const align = info.edit.align ?? "start";
    const fillHex = cssColor(info.edit.fill?.resolved);
    const original = info.edit.text ?? "";

    const ta = h("textarea", {
      class: "canvas-text-edit",
      spellcheck: "true",
      "aria-label": `Edit text for ${info.id}`,
      value: original,
    });
    ta.value = original;
    ta.style.left = `${Math.round(left)}px`;
    ta.style.top = `${Math.round(top)}px`;
    ta.style.width = `${Math.round(width + 12)}px`;
    ta.style.minHeight = `${Math.round(height + 6)}px`;
    ta.style.fontFamily = `"${fontFamily}", var(--font-ui)`;
    ta.style.fontSize = `${fontSize.toFixed(1)}px`;
    ta.style.fontWeight = String(fontWeight);
    ta.style.textAlign = align === "end" ? "right" : align === "center" ? "center" : "left";
    if (fillHex) {
      ta.style.color = fillHex;
      const lum = hexLuminance(fillHex);
      if (lum !== null) {
        ta.style.background = lum > 0.58 ? "rgb(11 26 46 / 0.92)" : "rgb(255 255 255 / 0.94)";
      }
    }

    let finished = false;
    const finish = async (commit) => {
      if (finished) return;
      finished = true;
      const next = ta.value;
      ta.remove();
      this.active = null;
      if (commit && next !== original) {
        await this.app.actions.set(info.id, { text: next });
      }
      this.viewport.focus();
    };

    ta.addEventListener("keydown", (e) => {
      e.stopPropagation();
      if (e.key === "Escape") {
        e.preventDefault();
        finish(false);
      } else if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
        e.preventDefault();
        finish(true);
      }
    });
    ta.addEventListener("pointerdown", (e) => e.stopPropagation());
    ta.addEventListener("mousedown", (e) => e.stopPropagation());
    ta.addEventListener("blur", () => finish(true));

    this.active = { id: info.id, ta, finish };
    this.viewport.appendChild(ta);
    ta.focus();
    if (selectAll) ta.select();
    else ta.setSelectionRange(ta.value.length, ta.value.length);
    return true;
  }

  close({ commit = false } = {}) {
    if (this.active) this.active.finish(commit);
  }
}

function hexLuminance(hex) {
  const clean = String(hex ?? "").trim().replace(/^#/, "");
  let r, g, b;
  if (clean.length === 3 || clean.length === 4) {
    r = parseInt(clean[0] + clean[0], 16);
    g = parseInt(clean[1] + clean[1], 16);
    b = parseInt(clean[2] + clean[2], 16);
  } else if (clean.length === 6 || clean.length === 8) {
    r = parseInt(clean.slice(0, 2), 16);
    g = parseInt(clean.slice(2, 4), 16);
    b = parseInt(clean.slice(4, 6), 16);
  } else {
    return null;
  }
  return (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
}
