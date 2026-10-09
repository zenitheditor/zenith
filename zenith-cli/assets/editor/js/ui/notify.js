// Notices (rows under the top bar) and toasts (short confirmations).
//
// Every server error reaches the user here: its code, its message, and
// its next actions (offers, retry). A notice stays until its cause clears
// or the user dismisses it.

import { h } from "../util/dom.js";
import { icon } from "./icons.js";

const TOAST_MS = 3200;

export class Notices {
  constructor(banners, toasts) {
    this.banners = banners;
    this.toasts = toasts;
    this.byKey = new Map();
  }

  /**
   * Show notice `key`, replacing one with the same key.
   * `spec`: `{level: "info"|"warning"|"error", title?, code?, message,
   * detail?, actions?: [{label, kind?: "primary"|"danger", run}],
   * dismiss?: bool = true}`. An action that returns a promise disables the
   * buttons until it settles.
   */
  show(key, spec) {
    const level = spec.level ?? "info";
    const actions = h("div", { class: "notice-actions" });
    const el = h(
      "div",
      {
        class: `notice ${level}`,
        role: level === "error" ? "alert" : "status",
        dataset: { key },
      },
      h("span", { class: "notice-icon" }, icon(level === "info" ? "info" : level)),
      h(
        "div",
        { class: "notice-text" },
        spec.code ? h("span", { class: "notice-code", text: spec.code }) : null,
        spec.title ? h("strong", { text: `${spec.title} ` }) : null,
        spec.message ?? "",
      ),
      actions,
      spec.detail ? h("pre", { class: "notice-detail", tabindex: "0", text: spec.detail }) : null,
    );
    for (const action of spec.actions ?? []) {
      const button = h("button", {
        type: "button",
        class: `button compact ${action.kind ?? ""}`.trim(),
        text: action.label,
        onclick: async () => {
          const buttons = actions.querySelectorAll("button");
          for (const b of buttons) b.disabled = true;
          try {
            await action.run();
          } finally {
            for (const b of buttons) b.disabled = false;
          }
        },
      });
      actions.appendChild(button);
    }
    if (spec.dismiss !== false) {
      actions.appendChild(
        h(
          "button",
          {
            type: "button",
            class: "tool small",
            title: "Dismiss",
            onclick: () => this.hide(key),
          },
          icon("close"),
          h("span", { class: "sr-only", text: "Dismiss" }),
        ),
      );
    }
    const old = this.byKey.get(key);
    if (old) old.replaceWith(el);
    else this.banners.appendChild(el);
    this.byKey.set(key, el);
    return el;
  }

  hide(key) {
    const el = this.byKey.get(key);
    if (!el) return;
    el.remove();
    this.byKey.delete(key);
  }

  /** Remove every notice (another document replaced the one they were about). */
  clear() {
    for (const el of this.byKey.values()) el.remove();
    this.byKey.clear();
  }

  has(key) {
    return this.byKey.has(key);
  }

  /** A short confirmation that fades by itself. */
  toast(message, iconName = "check") {
    const el = h("div", { class: `toast ${iconName}` }, icon(iconName), h("span", { text: message }));
    this.toasts.appendChild(el);
    setTimeout(() => el.remove(), TOAST_MS);
  }

  /**
   * Show the error of envelope or error `err` for `command`. `offers` from
   * the engine become buttons that run `runOffer(offer)`. `retry` adds a
   * Retry button.
   */
  error(command, err, { runOffer, retry } = {}) {
    // The offline notice covers a command that never reached the server.
    if (err?.offline) return null;
    const e = err?.error ?? err ?? {};
    const actions = [];
    for (const offer of e.offers ?? []) {
      if (runOffer) actions.push({ label: offer.label, kind: "primary", run: () => runOffer(offer) });
    }
    if (retry) actions.push({ label: "Retry", run: retry });
    const detail = (e.diagnostics ?? [])
      .map((d) => `${d.severity ?? "error"} ${d.code}: ${d.message}`)
      .join("\n");
    return this.show(`error:${command}`, {
      level: "error",
      code: e.code ?? "edit.error",
      title: `${command} failed.`,
      message: e.message ?? "The engine sent no message. Check the zenith edit terminal.",
      detail: detail || undefined,
      actions,
    });
  }
}
