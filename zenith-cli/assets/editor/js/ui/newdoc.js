// New Document dialog: choose filename, page size preset or custom W×H,
// orientation swap, and background color. Shared by Welcome and the Editor.

import { createBlankDoc, PAGE_PRESETS } from "../app/samples.js";
import { byId, clear, h } from "../util/dom.js";
import { hydrateIcons } from "./icons.js";

const BG_SWATCHES = [
  { label: "White", hex: "#ffffff" },
  { label: "Warm paper", hex: "#f8f7f4" },
  { label: "Slate", hex: "#f8fafc" },
  { label: "Dark", hex: "#0c1118" },
];

let bound = false;
let currentOnCreate = null;

function ensureBound() {
  if (bound) return;
  const dlg = byId("new-doc-dlg");
  if (!dlg) return;
  bound = true;

  const form = byId("new-doc-form");
  const nameInput = byId("new-doc-name");
  const wInput = byId("new-doc-w");
  const hInput = byId("new-doc-h");
  const bgInput = byId("new-doc-bg");
  const presetsWrap = byId("new-doc-presets");
  const swatchesWrap = byId("new-doc-swatches");

  const syncPresetHighlight = () => {
    const w = Math.round(Number(wInput.value) || 0);
    const ht = Math.round(Number(hInput.value) || 0);
    for (const btn of presetsWrap.querySelectorAll(".fmt-pill")) {
      const match = Number(btn.dataset.w) === w && Number(btn.dataset.h) === ht;
      btn.classList.toggle("on", match);
      btn.setAttribute("aria-pressed", String(match));
    }
  };

  clear(presetsWrap);
  for (const p of PAGE_PRESETS) {
    presetsWrap.appendChild(
      h("button", {
        type: "button",
        class: "fmt-pill",
        text: p.label,
        title: `${p.label} (${p.detail} px)`,
        dataset: { preset: p.key, w: String(p.w), h: String(p.h) },
        onclick: () => {
          wInput.value = String(p.w);
          hInput.value = String(p.h);
          syncPresetHighlight();
        },
      }),
    );
  }

  clear(swatchesWrap);
  for (const sw of BG_SWATCHES) {
    const btn = h("button", {
      type: "button",
      class: "bg-swatch-btn",
      title: `${sw.label} (${sw.hex})`,
      "aria-label": `${sw.label} background`,
      dataset: { hex: sw.hex },
      onclick: () => {
        bgInput.value = sw.hex;
      },
    });
    btn.style.backgroundColor = sw.hex;
    swatchesWrap.appendChild(btn);
  }

  wInput.addEventListener("input", syncPresetHighlight);
  hInput.addEventListener("input", syncPresetHighlight);

  byId("new-doc-swap")?.addEventListener("click", () => {
    const prevW = wInput.value;
    wInput.value = hInput.value;
    hInput.value = prevW;
    syncPresetHighlight();
  });

  byId("new-doc-close")?.addEventListener("click", () => dlg.close());
  byId("new-doc-cancel")?.addEventListener("click", () => dlg.close());
  dlg.addEventListener("click", (e) => {
    if (e.target === dlg) dlg.close();
  });

  form.addEventListener("submit", (e) => {
    e.preventDefault();
    const spec = createBlankDoc({
      name: nameInput.value,
      w: Number(wInput.value),
      h: Number(hInput.value),
      background: bgInput.value,
    });
    const cb = currentOnCreate;
    dlg.close();
    cb?.(spec);
  });
}

/** Open the New Document modal and invoke `onCreate(spec)` on submit. */
export function openNewDocDialog(onCreate, { preset = "custom", name = "untitled.zen", w = 800, h = 600, background = "#ffffff" } = {}) {
  const dlg = byId("new-doc-dlg");
  if (!dlg) return;
  ensureBound();
  hydrateIcons(dlg);
  currentOnCreate = onCreate;

  const found = PAGE_PRESETS.find((p) => p.key === preset);
  const nameInput = byId("new-doc-name");
  const wInput = byId("new-doc-w");
  const hInput = byId("new-doc-h");
  const bgInput = byId("new-doc-bg");

  nameInput.value = found ? found.name : name;
  wInput.value = String(found ? found.w : w);
  hInput.value = String(found ? found.h : h);
  bgInput.value = background;
  wInput.dispatchEvent(new Event("input"));

  if (!dlg.open) dlg.showModal();
  nameInput.focus();
  const dot = nameInput.value.lastIndexOf(".");
  nameInput.setSelectionRange(0, dot > 0 ? dot : nameInput.value.length);
}
