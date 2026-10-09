// The inspector's edit form: the `node.set` fields `node.inspect` lists
// under `edit`, as inputs. A field commits on change (Enter or leaving it);
// the engine maps the value and may reject it with offers, which show as a
// notice. The form does no document math.
//
// Token-backed fields (fill, stroke, stroke width, radius, font) take a
// token from a picker or a raw value (hex color with alpha, px, family
// name, weight). The engine binds a raw value through the token that holds
// it, or a new one.

import { h, num } from "../util/dom.js";
import { icon } from "../ui/icons.js";

/** Box and rotate fields: label, accessible name, unit. */
const NUMBERS = [
  ["x", "X", "X position, px"],
  ["y", "Y", "Y position, px"],
  ["w", "W", "Width, px"],
  ["h", "H", "Height, px"],
  ["rotate", "R", "Rotation, degrees"],
  ["opacity", "Op", "Opacity, 0 to 1"],
];

/** Token-backed fields: label, token type, raw value kind, removable. */
const BOUND = [
  ["fill", "Fill", "color", "color", false],
  ["stroke", "Stroke", "color", "color", false],
  ["stroke_width", "Stroke width", "dimension", "px", false],
  ["radius", "Radius", "dimension", "px", true],
  ["font_family", "Font", "fontFamily", "name", true],
  ["font_size", "Font size", "dimension", "px", true],
  ["font_weight", "Weight", "fontWeight", "weight", true],
];

const ALIGNS = ["start", "center", "end", "justify"];

/** What an axis marker says, by the `edit.axes` value. */
const AXES = {
  token: ["link", "Bound to a token. A change asks to detach it."],
  anchor: ["link", "Placed by an anchor. A change moves the anchor gap or asks to detach it."],
  flow: ["frame", "Placed by the layout. A change asks to take it out of the layout."],
  keyword: ["frame", "Computed (hug or fill). A change asks to set a fixed size."],
  absent: ["frame", "Computed from content. A change asks to set a fixed size."],
  unresolved: ["warning", "Not in px. A change asks to replace it with px."],
};

/**
 * The form for `info` (a `node.inspect` reply with `edit`). `tokens`: every
 * token `[{id, type, value}]`. `set(params)` sends `node.set` and resolves
 * `true` on success.
 */
export function editForm(info, tokens, set) {
  const e = info.edit;
  const fields = new Set(e.fields ?? []);
  const out = [];
  const numbers = NUMBERS.filter(([name]) => fields.has(name));
  if (numbers.length) {
    out.push(h("h3", { text: "Edit" }));
    out.push(
      h(
        "div",
        { class: "edit-grid" },
        numbers.map(([name, label, aria]) => numberField(name, label, aria, e[name], e.axes?.[name], set)),
      ),
    );
  }
  const bound = BOUND.filter(([name]) => fields.has(name));
  if (bound.length) {
    out.push(h("h3", { text: "Style" }));
    for (const [name, label, type, raw, removable] of bound) {
      out.push(boundField({ name, label, raw, removable }, e[name] ?? {}, tokens.filter((t) => t.type === type), set));
    }
  }
  if (fields.has("align")) out.push(alignField(e.align ?? "start", set));
  if (fields.has("text")) out.push(textField(e.text ?? "", set));
  if (fields.has("spans")) out.push(spansField(e.spans ?? [], set));
  const flags = ["visible", "locked"].filter((f) => fields.has(f));
  if (flags.length) out.push(flagsField(e, set));
  return out;
}

function numberField(name, label, aria, value, axis, set) {
  const shown = typeof value === "number" ? num(value, 3) : "";
  const input = h("input", {
    class: "edit-input",
    type: "text",
    inputmode: "decimal",
    spellcheck: "false",
    autocomplete: "off",
    "aria-label": aria,
    value: shown,
    dataset: { field: name },
  });
  let sent = shown;
  const commit = async () => {
    const text = input.value.trim();
    const n = Number(text);
    if (text === "" || !Number.isFinite(n)) {
      input.setAttribute("aria-invalid", "true");
      input.title = `${aria}: enter a number`;
      return;
    }
    input.removeAttribute("aria-invalid");
    input.title = "";
    // Enter commits, and the change event that follows must not resend.
    if (sent !== "" && Number(sent) === n) return;
    sent = text;
    const ok = await set({ [name]: n });
    if (!ok) {
      input.value = shown;
      sent = shown;
    }
  };
  input.addEventListener("change", commit);
  input.addEventListener("keydown", (ev) => {
    if (ev.key === "Enter") {
      ev.preventDefault();
      commit();
    } else if (ev.key === "Escape") {
      input.value = shown;
      input.removeAttribute("aria-invalid");
      input.blur();
    }
  });
  const marker = AXES[axis];
  return h(
    "label",
    { class: "edit-field" },
    h("span", { class: "edit-label", text: label }),
    input,
    marker ? h("span", { class: `edit-axis ${axis}`, title: marker[1] }, icon(marker[0]), h("span", { class: "sr-only", text: marker[1] })) : null,
  );
}

/** The value a token-backed field shows: its token's, its own, or its style's. */
function shownValue(bound, tokens) {
  const own = bound.token ? tokens.find((t) => t.id === bound.token)?.value ?? bound.resolved : bound.value;
  const value = own ?? bound.style?.resolved ?? null;
  if (value && typeof value === "object" && "hex" in value) return value.hex;
  return value;
}

/** A dimension token value (`(px)12`) or number as px, else `null`. */
function px(value) {
  if (typeof value === "number") return value;
  const m = /^\((px|pt)\)(-?[0-9.]+)$/.exec(String(value ?? ""));
  if (!m) return null;
  return m[1] === "pt" ? (Number(m[2]) * 96) / 72 : Number(m[2]);
}

/**
 * A token-backed field: a token picker and a raw value input. `spec`:
 * `{name, label, raw: color|px|name|weight, removable}`.
 */
function boundField(spec, bound, tokens, set) {
  const { name, label, raw, removable } = spec;
  const select = h("select", { class: "edit-select", "aria-label": `${label} token`, dataset: { field: name } });
  const current = bound.token ?? "";
  if (!bound.token) {
    const inherited = bound.style?.token ? `From style: ${bound.style.token}` : bound.value ? `${bound.value} (not a token)` : "None";
    select.appendChild(h("option", { value: "", text: inherited, selected: true, disabled: !removable || !bound.value }));
  }
  for (const t of tokens) {
    select.appendChild(h("option", { value: t.id, text: t.id, selected: t.id === current }));
  }
  if (bound.token && !tokens.some((t) => t.id === bound.token)) {
    select.appendChild(h("option", { value: bound.token, text: `${bound.token} (unresolved)`, selected: true }));
  }
  if (removable && bound.token) select.appendChild(h("option", { value: "\u0000clear", text: "Clear (use the style)" }));
  const value = shownValue(bound, tokens);
  let input;
  let shown;
  if (raw === "color") {
    shown = typeof value === "string" ? value : "";
    input = h("input", {
      class: "edit-input hex",
      type: "text",
      spellcheck: "false",
      autocomplete: "off",
      placeholder: "#rrggbbaa",
      "aria-label": `${label} color, hex with optional alpha`,
      value: shown,
      dataset: { field: `${name}-value` },
    });
  } else if (raw === "name") {
    shown = typeof value === "string" ? value : "";
    input = h("input", {
      class: "edit-input",
      type: "text",
      spellcheck: "false",
      autocomplete: "off",
      "aria-label": `${label} family name`,
      value: shown,
      dataset: { field: `${name}-value` },
    });
  } else {
    const n = raw === "px" ? px(value) : typeof value === "number" ? value : null;
    shown = n === null ? "" : num(n, 3);
    input = h("input", {
      class: "edit-input",
      type: "text",
      inputmode: "decimal",
      spellcheck: "false",
      autocomplete: "off",
      "aria-label": raw === "px" ? `${label}, px` : `${label}, 100 to 900`,
      value: shown,
      dataset: { field: `${name}-value` },
    });
  }
  const swatch = raw === "color" ? h("span", { class: "swatch" }) : null;
  const picker = raw === "color"
    ? h("input", { class: "edit-color", type: "color", "aria-label": `${label} color picker`, dataset: { field: `${name}-picker` } })
    : null;
  const paint = (hex) => {
    if (!swatch) return;
    const color = /^#[0-9a-fA-F]{3,8}$/.test(hex ?? "");
    swatch.style.background = color ? hex : "";
    swatch.classList.toggle("none", !color);
    if (picker && /^#[0-9a-fA-F]{6}/.test(hex ?? "")) picker.value = hex.slice(0, 7).toLowerCase();
  };
  paint(shown);
  const send = async (v) => {
    const ok = await set({ [name]: v });
    if (!ok) {
      select.value = current;
      input.value = shown;
      paint(shown);
    }
  };
  select.addEventListener("change", () => {
    if (select.value === "\u0000clear") send(null);
    else if (select.value) send(select.value);
  });
  let sent = shown;
  const commit = () => {
    const text = input.value.trim();
    if (text === sent) return;
    if (text === "" && removable) {
      sent = text;
      send(null);
      return;
    }
    let v;
    if (raw === "color") v = /^#([0-9a-fA-F]{3,4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/.test(text) ? text : null;
    else if (raw === "name") v = text || null;
    else v = text !== "" && Number.isFinite(Number(text)) ? Number(text) : null;
    if (v === null) {
      input.setAttribute("aria-invalid", "true");
      input.title = raw === "color" ? `${label}: enter #rgb, #rrggbb, or #rrggbbaa` : `${label}: enter a value`;
      return;
    }
    input.removeAttribute("aria-invalid");
    input.title = "";
    sent = text;
    paint(text);
    send({ value: v });
  };
  input.addEventListener("change", commit);
  input.addEventListener("keydown", (ev) => {
    if (ev.key === "Enter") {
      ev.preventDefault();
      commit();
    } else if (ev.key === "Escape") {
      input.value = shown;
      input.removeAttribute("aria-invalid");
      input.blur();
    }
  });
  picker?.addEventListener("change", () => {
    // The native picker has no alpha: keep the alpha the field shows.
    const alpha = /^#[0-9a-fA-F]{8}$/.test(input.value.trim()) ? input.value.trim().slice(7) : "";
    input.value = `${picker.value}${alpha}`;
    commit();
  });
  return h(
    "div",
    { class: "edit-bound" },
    h("span", { class: "edit-label wide", text: label }),
    h("div", { class: "edit-bound-row" }, swatch, select),
    h("div", { class: "edit-bound-row" }, picker, input),
  );
}

function alignField(value, set) {
  const select = h("select", { class: "edit-select", "aria-label": "Text alignment", dataset: { field: "align" } });
  for (const a of ALIGNS) select.appendChild(h("option", { value: a, text: a, selected: a === value }));
  select.addEventListener("change", async () => {
    const ok = await set({ align: select.value });
    if (!ok) select.value = value;
  });
  return h("label", { class: "edit-row" }, h("span", { class: "edit-label wide", text: "Align" }), select);
}

function textField(value, set) {
  const area = h("textarea", { class: "edit-text", rows: "2", "aria-label": "Text content", dataset: { field: "text" } });
  area.value = value;
  const commit = async () => {
    if (area.value === value) return;
    const ok = await set({ text: area.value });
    if (!ok) area.value = value;
  };
  area.addEventListener("change", commit);
  area.addEventListener("keydown", (ev) => {
    if (ev.key === "Enter" && (ev.ctrlKey || ev.metaKey)) {
      ev.preventDefault();
      area.blur();
    } else if (ev.key === "Escape") {
      area.value = value;
      area.blur();
    }
  });
  return [h("h3", { text: "Text" }), area, h("p", { class: "edit-tip", text: "Ctrl+Enter or leaving the field applies it." })];
}

/** One text field per span; a styled span keeps its own attributes. */
function spansField(spans, set) {
  const rows = spans.map((span, index) => {
    const area = h("textarea", {
      class: "edit-text",
      rows: "1",
      "aria-label": `Span ${index + 1} text${span.styled ? ", styled" : ""}`,
      dataset: { field: `span-${index}` },
    });
    area.value = span.text;
    const commit = async () => {
      if (area.value === span.text) return;
      const ok = await set({ spans: [{ index, text: area.value }] });
      if (!ok) area.value = span.text;
    };
    area.addEventListener("change", commit);
    area.addEventListener("keydown", (ev) => {
      if (ev.key === "Enter" && (ev.ctrlKey || ev.metaKey)) {
        ev.preventDefault();
        area.blur();
      } else if (ev.key === "Escape") {
        area.value = span.text;
        area.blur();
      }
    });
    return h(
      "div",
      { class: "edit-span" },
      h("span", { class: `edit-span-mark${span.styled ? " styled" : ""}`, text: String(index + 1), title: span.styled ? "Styled span" : "Plain span" }),
      area,
    );
  });
  return [h("h3", { text: "Spans" }), ...rows, h("p", { class: "edit-tip", text: "Each span keeps its own style. Ctrl+Enter or leaving a field applies it." })];
}

function flagsField(e, set) {
  const box = (name, label) => {
    const input = h("input", { type: "checkbox", dataset: { field: name }, checked: !!e[name] });
    input.addEventListener("change", async () => {
      const ok = await set({ [name]: input.checked });
      if (!ok) input.checked = !!e[name];
    });
    return h("label", { class: "edit-check" }, input, h("span", { text: label }));
  };
  return h("div", { class: "edit-flags" }, box("visible", "Visible"), box("locked", "Locked"));
}
