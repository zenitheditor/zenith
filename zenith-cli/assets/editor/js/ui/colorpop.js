// In-app Color & Token Swatch Popover: anchored beside any color swatch in the
// Inspector. Shows all document color tokens as clickable tiles, a hex/alpha
// editor, and a one-click "+ Save as token" action so users never need to hand-
// edit KDL to manage color tokens.

import { cssColor } from "../util/color.js";
import { h } from "../util/dom.js";

let activePop = null;

export function closeColorPopover() {
  if (!activePop) return;
  activePop.cleanup();
  activePop.el.remove();
  activePop = null;
}

function hex6(v) {
  if (typeof v !== "string") return "#0870a6";
  const m = v.trim().match(/^#([0-9a-f]{6})/i);
  return m ? `#${m[1]}` : "#0870a6";
}

function alphaOf(v) {
  if (typeof v !== "string") return 100;
  const m = v.trim().match(/^#[0-9a-f]{6}([0-9a-f]{2})$/i);
  return m ? Math.round((parseInt(m[1], 16) / 255) * 100) : 100;
}

function withAlpha(hex, pct) {
  const base = hex6(hex);
  const p = Math.max(0, Math.min(100, Math.round(pct)));
  if (p >= 100) return base;
  const a = Math.round((p / 100) * 255)
    .toString(16)
    .padStart(2, "0");
  return `${base}${a}`;
}

/**
 * Open the color/token popover anchored near `anchorEl`.
 * `opts`: `{ currentHex, currentToken, tokens, onSelectToken(id), onSelectColor(hex), onCreateToken(hex) }`.
 */
export function openColorPopover(anchorEl, opts) {
  closeColorPopover();
  const { currentHex, currentToken, tokens = [], onSelectToken, onSelectColor, onCreateToken } = opts;

  let curHex = hex6(currentHex || "#0870a6");
  let curAlpha = alphaOf(currentHex || "#0870a6");

  const spectrum = h("input", {
    type: "color",
    class: "cp-spectrum",
    value: curHex,
    "aria-label": "Pick color",
  });
  const hexInput = h("input", {
    type: "text",
    class: "edit-input cp-hex",
    value: withAlpha(curHex, curAlpha),
    spellcheck: "false",
    "aria-label": "Hex color",
  });
  const alphaRange = h("input", {
    type: "range",
    class: "cp-alpha",
    min: "0",
    max: "100",
    value: String(curAlpha),
    "aria-label": "Opacity percent",
  });
  const alphaVal = h("span", { class: "cp-alpha-val", text: `${curAlpha}%` });

  const applyRaw = () => {
    const out = withAlpha(curHex, curAlpha);
    hexInput.value = out;
    onSelectColor?.(out);
  };

  spectrum.addEventListener("input", () => {
    curHex = spectrum.value;
    hexInput.value = withAlpha(curHex, curAlpha);
  });
  spectrum.addEventListener("change", () => {
    curHex = spectrum.value;
    applyRaw();
  });
  alphaRange.addEventListener("input", () => {
    curAlpha = Number(alphaRange.value);
    alphaVal.textContent = `${curAlpha}%`;
    hexInput.value = withAlpha(curHex, curAlpha);
  });
  alphaRange.addEventListener("change", applyRaw);
  hexInput.addEventListener("change", () => {
    const v = hexInput.value.trim();
    if (/^#([0-9a-f]{6}|[0-9a-f]{8})$/i.test(v)) {
      curHex = hex6(v);
      curAlpha = alphaOf(v);
      spectrum.value = curHex;
      alphaRange.value = String(curAlpha);
      alphaVal.textContent = `${curAlpha}%`;
      onSelectColor?.(v);
    }
  });

  const tokenGrid = tokens.length
    ? h(
        "div",
        { class: "cp-tokens" },
        h("div", { class: "cp-sec-title", text: "Document color tokens" }),
        h(
          "div",
          { class: "cp-swatches" },
          tokens.map((t) => {
            const c = cssColor(t.value) || "#888888";
            return h("button", {
              type: "button",
              class: `cp-tile${t.id === currentToken ? " on" : ""}`,
              style: `background:${c}`,
              title: `${t.id} (${t.value})`,
              "aria-label": `Use token ${t.id}`,
              onclick: () => {
                onSelectToken?.(t.id);
                closeColorPopover();
              },
            });
          }),
        ),
      )
    : null;

  const saveTokenBtn = onCreateToken
    ? h("button", {
        type: "button",
        class: "button small cp-save-tok",
        text: "+ Save color as token",
        onclick: async () => {
          await onCreateToken(withAlpha(curHex, curAlpha));
          closeColorPopover();
        },
      })
    : null;

  const el = h(
    "div",
    { class: "popover color-pop", role: "dialog", "aria-label": "Color and token picker" },
    tokenGrid,
    h(
      "div",
      { class: "cp-custom" },
      h("div", { class: "cp-sec-title", text: "Custom color" }),
      h("div", { class: "cp-row" }, spectrum, hexInput),
      h("div", { class: "cp-row" }, h("span", { class: "cp-lbl", text: "Alpha" }), alphaRange, alphaVal),
      saveTokenBtn,
    ),
  );

  document.body.appendChild(el);
  const r = anchorEl.getBoundingClientRect();
  const pw = el.offsetWidth || 236;
  const ph = el.offsetHeight || 220;
  const left = Math.max(8, Math.min(r.left - pw - 8, window.innerWidth - pw - 8));
  const top = Math.max(8, Math.min(r.top, window.innerHeight - ph - 8));
  el.style.left = `${Math.round(left)}px`;
  el.style.top = `${Math.round(top)}px`;

  const onDown = (e) => {
    if (!el.contains(e.target) && !anchorEl.contains(e.target)) closeColorPopover();
  };
  const onKey = (e) => {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      closeColorPopover();
    }
  };
  document.addEventListener("pointerdown", onDown, true);
  document.addEventListener("keydown", onKey, true);
  activePop = {
    el,
    cleanup: () => {
      document.removeEventListener("pointerdown", onDown, true);
      document.removeEventListener("keydown", onKey, true);
    },
  };
}
