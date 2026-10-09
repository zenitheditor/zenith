// The diagnostics panel: severity, code, message, and `line:col` of each
// diagnostic. A row with a position jumps to it in the source.

import { clear, h, plural } from "../util/dom.js";
import { icon } from "../ui/icons.js";

const ORDER = { error: 0, warning: 1, advisory: 2 };
const ICON = { error: "error", warning: "warning", advisory: "info" };

export class DiagnosticsPanel {
  /** `jump(diagnostic)` runs when the user picks a row with a position. */
  constructor(list, counts, jump) {
    this.list = list;
    this.counts = counts;
    this.jump = jump;
    this.items = [];
  }

  /** Show `items` (engine diagnostics). */
  set(items) {
    this.items = [...items].sort(
      (a, b) =>
        (ORDER[a.severity] ?? 3) - (ORDER[b.severity] ?? 3) ||
        (a.line ?? Infinity) - (b.line ?? Infinity) ||
        (a.col ?? 0) - (b.col ?? 0),
    );
    this.draw();
  }

  /** `{error, warning, advisory}` counts. */
  tally() {
    const t = { error: 0, warning: 0, advisory: 0 };
    for (const d of this.items) if (d.severity in t) t[d.severity]++;
    return t;
  }

  draw() {
    clear(this.list);
    clear(this.counts);
    const t = this.tally();
    this.counts.append(
      h("span", { class: "sev-error" }, plural(t.error, "error")),
      h("span", {}, plural(t.warning, "warning")),
      h("span", {}, plural(t.advisory, "advisory", "advisories")),
    );
    if (this.items.length === 0) {
      this.list.appendChild(h("li", { class: "empty", text: "No diagnostics. The document checks clean." }));
      return;
    }
    for (const d of this.items) {
      const at = d.line ? `${d.line}:${d.col}` : d.import ? `in ${d.import}` : "";
      const jumpable = typeof d.start === "number";
      const content = [
        icon(ICON[d.severity] ?? "info"),
        h("span", { class: "diag-code", text: d.code }),
        h("span", { class: "diag-msg", text: d.message }),
        at ? h("span", { class: "diag-pos", text: at }) : null,
      ];
      const label = `${d.severity} ${d.code}${at ? ` at ${at}` : ""}: ${d.message}`;
      this.list.appendChild(
        h(
          "li",
          { class: `sev-${d.severity}` },
          jumpable
            ? h(
                "button",
                { type: "button", class: "row", "aria-label": label, onclick: () => this.jump(d) },
                content,
              )
            : h("div", { class: "row", "aria-label": label }, content),
        ),
      );
    }
  }
}
