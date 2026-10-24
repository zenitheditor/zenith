// report12: one dense report page (stat cards, bar chart, table, summary) and a
// 12-step chain. Later steps depend on earlier ones, and most steps name their
// target by description ("the second stat card", "the label under the chart").
// Usage: node bench/revision/tasks/report12/gen.mjs
import { kdl, esc, rgb, boxCss, boxZen, at, writeTask } from "../gen-lib.mjs";

const C = { bg: "#ffffff", ink: "#111827", muted: "#6b7280", brand: "#4f46e5", soft: "#f3f4f6", line: "#e5e7eb", up: "#047857", down: "#dc2626", head: "#1f2937", onhead: "#ffffff" };
const W = 960;
const H = 1280;
const B = {
  title: { x: 48, y: 40, w: 864, h: 44 },
  subtitle: { x: 48, y: 88, w: 864, h: 22 },
  panel: { x: 48, y: 272, w: 864, h: 400 },
  chartTitle: { x: 72, y: 292, w: 600, h: 26 },
  baseline: { x: 96, y: 600, w: 768, h: 2 },
  caption: { x: 72, y: 640, w: 816, h: 20 },
  head: { x: 48, y: 704, w: 864, h: 36 },
  paragraph: { x: 48, y: 984, w: 864, h: 104 },
  footer: { x: 48, y: 1228, w: 864, h: 20 },
};
const CARDS = [
  ["Orders", "12,480", "+8.2%", "up"],
  ["Revenue", "$1.92M", "+5.4%", "up"],
  ["On-time rate", "96.1%", "-0.6%", "down"],
  ["Active sites", "38", "+3", "up"],
];
const cardBox = (k) => ({ x: 48 + k * 220, y: 136, w: 204, h: 112 });
const PART = { label: { x: 16, y: 14, w: 172, h: 20 }, value: { x: 16, y: 37, w: 172, h: 42 }, delta: { x: 16, y: 82, w: 172, h: 20 } };
const MONTHS = [["jan", "Jan", 1820], ["feb", "Feb", 1960], ["mar", "Mar", 2050], ["apr", "Apr", 1890], ["may", "May", 2210], ["jun", "Jun", 2150]];
const bar = (i, v) => ({ x: 120 + i * 130, y: 600 - v / 10, w: 64, h: v / 10 });
const valBox = (i, v) => ({ x: 102 + i * 130, y: 600 - v / 10 - 24, w: 100, h: 20 });
const monBox = (i) => ({ x: 102 + i * 130, y: 608, w: 100, h: 20 });
const COLS = [{ x: 16, w: 264 }, { x: 300, w: 180 }, { x: 500, w: 180 }, { x: 700, w: 148 }];
const HEAD = ["Region", "Orders", "Revenue", "On-time"];
const ROWS = [
  ["North", "3,420", "$0.52M", "97.2%"],
  ["South", "2,980", "$0.46M", "96.4%"],
  ["East", "2,610", "$0.41M", "95.8%"],
  ["West", "2,270", "$0.35M", "96.9%"],
  ["APAC", "1,200", "$0.18M", "91.2%"],
];
const rowBox = (r) => ({ x: 48, y: 740 + r * 36, w: 864, h: 36 });
const TEXT = {
  title: "Quarterly operations report",
  subtitle: "Q2 2027 · Prepared by the operations team",
  chartTitle: "Monthly orders",
  caption: "Figure 1. Orders by month, Q1–Q2 2027",
  paragraph:
    "Orders reached 12,480 in the first half, up 8.2% on last year. Revenue grew to $1.92M. On-time delivery slipped slightly to 96.1% as Asia-Pacific volumes doubled. The East region needs a second depot before the autumn peak.",
  footer: "Internal · Generated for the operations review",
};
const fmt = (n) => n.toLocaleString("en-US");

function zen() {
  const t = (id, type, value) => `    token id="${id}" type="${type}" value=${value}`;
  const style = (id, size, fill, bold) =>
    [`    style id="${id}" {`, `      font-family (token)"font.body"`, `      font-size (token)"${size}"`, ...(bold ? [`      font-weight (token)"weight.bold"`] : []), `      fill (token)"${fill}"`, "    }"].join("\n");
  const txt = (id, b, st, s, extra = "") => `      text id="${id}" ${boxZen(b)} style="${st}"${extra} { span ${kdl(s)} }`;
  const out = [
    "zenith version=1 {",
    '  project id="proj.report12" name="Operations report"',
    '  tokens format="zenith-token-v1" {',
    ...Object.entries(C).map(([k, v]) => t(`color.${k}`, "color", kdl(v))),
    t("font.body", "fontFamily", kdl("Noto Sans")),
    t("weight.bold", "fontWeight", 700),
    ...[["title", 32], ["subtitle", 16], ["label", 14], ["value", 32], ["chart", 20], ["cell", 14], ["paragraph", 16], ["small", 12]].map(([k, v]) => t(`size.${k}`, "dimension", `(px)${v}`)),
    t("radius.card", "dimension", "(px)12"),
    t("stroke.thin", "dimension", "(px)1"),
    "  }",
    "  styles {",
    style("r.title", "size.title", "color.ink", true),
    style("r.subtitle", "size.subtitle", "color.muted", false),
    style("r.label", "size.label", "color.muted", false),
    style("r.value", "size.value", "color.ink", true),
    style("r.up", "size.label", "color.up", true),
    style("r.down", "size.label", "color.down", true),
    style("r.chart", "size.chart", "color.ink", true),
    style("r.axis", "size.label", "color.muted", false),
    style("r.caption", "size.small", "color.muted", false),
    style("r.head", "size.cell", "color.onhead", true),
    style("r.cell", "size.cell", "color.ink", false),
    style("r.paragraph", "size.paragraph", "color.ink", false),
    style("r.footer", "size.small", "color.muted", false),
    "  }",
    '  document id="doc.report12" title="Operations report" {',
    `    page id="p1" w=(px)${W} h=(px)${H} background=(token)"color.bg" {`,
    txt("title", B.title, "r.title", TEXT.title),
    txt("subtitle", B.subtitle, "r.subtitle", TEXT.subtitle),
  ];
  CARDS.forEach(([label, value, delta, dir], k) => {
    const id = `card${k + 1}`;
    out.push(
      `      frame id="${id}" ${boxZen(cardBox(k))} fill=(token)"color.soft" radius=(token)"radius.card" {`,
      `  ${txt(`${id}.label`, PART.label, "r.label", label)}`,
      `  ${txt(`${id}.value`, PART.value, "r.value", value)}`,
      `  ${txt(`${id}.delta`, PART.delta, `r.${dir}`, delta)}`,
      "      }",
    );
  });
  out.push(
    `      rect id="chart.panel" ${boxZen(B.panel)} fill=(token)"color.bg" stroke=(token)"color.line" stroke-width=(token)"stroke.thin" radius=(token)"radius.card"`,
    txt("chart.title", B.chartTitle, "r.chart", TEXT.chartTitle),
  );
  MONTHS.forEach(([id, name, v], i) => {
    out.push(
      `      rect id="bar.${id}" ${boxZen(bar(i, v))} fill=(token)"color.brand"`,
      txt(`val.${id}`, valBox(i, v), "r.axis", fmt(v), ' align="center"'),
      txt(`mon.${id}`, monBox(i), "r.axis", name, ' align="center"'),
    );
  });
  out.push(`      rect id="chart.baseline" ${boxZen(B.baseline)} fill=(token)"color.line"`, txt("chart.caption", B.caption, "r.caption", TEXT.caption));
  const row = (id, b, cells, st, fill) => {
    out.push(`      frame id="${id}" ${boxZen(b)}${fill ? ` fill=(token)"${fill}"` : ""} {`);
    if (!fill) out.push(`        rect id="${id}.rule" x=(px)0 y=(px)35 w=(px)864 h=(px)1 fill=(token)"color.line"`);
    cells.forEach((c, i) => out.push(`  ${txt(`${id}.c${i + 1}`, { x: COLS[i].x, y: 8, w: COLS[i].w, h: 20 }, st, c)}`));
    out.push("      }");
  };
  row("table.head", B.head, HEAD, "r.head", "color.head");
  ROWS.forEach((cells, r) => row(`row.${cells[0].toLowerCase()}`, rowBox(r), cells, "r.cell"));
  out.push(txt("paragraph", B.paragraph, "r.paragraph", TEXT.paragraph), txt("footer", B.footer, "r.footer", TEXT.footer), "    }", "  }", "}", "");
  return out.join("\n");
}

function html() {
  const out = [
    "<!doctype html>",
    '<html lang="en">',
    "<head>",
    '<meta charset="utf-8">',
    "<title>Operations report</title>",
    "<style>",
    `  :root { ${Object.entries(C).map(([k, v]) => `--${k}: ${v};`).join(" ")} }`,
    "  html, body { margin: 0; }",
    '  body { font-family: "Noto Sans", sans-serif; }',
    `  .page { position: relative; width: ${W}px; height: ${H}px; overflow: hidden; background: var(--bg); }`,
    "  .page *, .page { box-sizing: border-box; }",
    "  .page > *, .card > * { position: absolute; margin: 0; overflow: hidden; line-height: 1.3; }",
    `  .title { ${boxCss(B.title)}; font-size: 32px; font-weight: 700; color: var(--ink); }`,
    `  .subtitle { ${boxCss(B.subtitle)}; font-size: 16px; color: var(--muted); }`,
    "  .card { top: 136px; width: 204px; height: 112px; background: var(--soft); border-radius: 12px; }",
    `  .label { ${boxCss(PART.label)}; font-size: 14px; color: var(--muted); }`,
    `  .value { ${boxCss(PART.value)}; font-size: 32px; font-weight: 700; color: var(--ink); }`,
    `  .delta { ${boxCss(PART.delta)}; font-size: 14px; font-weight: 700; }`,
    "  .up { color: var(--up); }",
    "  .down { color: var(--down); }",
    `  .chart-panel { ${boxCss(B.panel)}; background: var(--bg); border: 1px solid var(--line); border-radius: 12px; }`,
    `  .chart-title { ${boxCss(B.chartTitle)}; font-size: 20px; font-weight: 700; color: var(--ink); }`,
    "  .bar { width: 64px; background: var(--brand); }",
    "  .axis { width: 100px; height: 20px; font-size: 14px; color: var(--muted); text-align: center; }",
    `  .baseline { ${boxCss(B.baseline)}; background: var(--line); }`,
    `  .caption { ${boxCss(B.caption)}; font-size: 12px; color: var(--muted); }`,
    "  .row { left: 48px; width: 864px; height: 36px; display: grid; grid-template-columns: 284px 200px 200px 180px; align-items: center; padding-left: 16px; font-size: 14px; color: var(--ink); border-bottom: 1px solid var(--line); }",
    "  .row span { overflow: hidden; white-space: nowrap; }",
    `  .row.head { top: ${B.head.y}px; background: var(--head); color: var(--onhead); font-weight: 700; border-bottom: none; }`,
    `  .paragraph { ${boxCss(B.paragraph)}; font-size: 16px; color: var(--ink); }`,
    `  .footer { ${boxCss(B.footer)}; font-size: 12px; color: var(--muted); }`,
    "</style>",
    "</head>",
    "<body>",
    '<section class="page">',
    `  <h1 class="title">${esc(TEXT.title)}</h1>`,
    `  <p class="subtitle">${esc(TEXT.subtitle)}</p>`,
    ...CARDS.map(
      ([label, value, delta, dir], k) =>
        `  <div class="card" style="left: ${cardBox(k).x}px;"><p class="label">${esc(label)}</p><p class="value">${esc(value)}</p><p class="delta ${dir}">${esc(delta)}</p></div>`,
    ),
    '  <div class="chart-panel"></div>',
    `  <h2 class="chart-title">${esc(TEXT.chartTitle)}</h2>`,
    ...MONTHS.flatMap(([id, name, v], i) => {
      const b = bar(i, v);
      const l = valBox(i, v);
      return [
        `  <div class="bar" id="bar-${id}" style="left: ${b.x}px; top: ${b.y}px; height: ${b.h}px;"></div>`,
        `  <p class="axis" style="left: ${l.x}px; top: ${l.y}px;">${fmt(v)}</p>`,
        `  <p class="axis" style="left: ${monBox(i).x}px; top: ${monBox(i).y}px;">${name}</p>`,
      ];
    }),
    '  <div class="baseline"></div>',
    `  <p class="caption">${esc(TEXT.caption)}</p>`,
    `  <div class="row head">${HEAD.map((c) => `<span>${c}</span>`).join("")}</div>`,
    ...ROWS.map((cells, r) => `  <div class="row" style="top: ${rowBox(r).y}px;">${cells.map((c) => `<span>${esc(c)}</span>`).join("")}</div>`),
    `  <p class="paragraph">${esc(TEXT.paragraph)}</p>`,
    `  <p class="footer">${esc(TEXT.footer)}</p>`,
    "</section>",
    "</body>",
    "</html>",
    "",
  ];
  return out.join("\n");
}

const el = (id, b) => ({ id, page: "p1", ...b });
const elements = [
  el("title", B.title),
  el("subtitle", B.subtitle),
  ...CARDS.flatMap((_, k) => [el(`card${k + 1}.bg`, cardBox(k)), ...Object.entries(PART).map(([n, b]) => el(`card${k + 1}.${n}`, at(b, cardBox(k).x, cardBox(k).y)))]),
  el("chart.panel", B.panel),
  el("chart.title", B.chartTitle),
  ...MONTHS.flatMap(([id, , v], i) => [el(`bar.${id}`, bar(i, v)), el(`val.${id}`, valBox(i, v)), el(`mon.${id}`, monBox(i))]),
  el("chart.baseline", B.baseline),
  el("chart.caption", B.caption),
  el("table.head", B.head),
  ...ROWS.map((cells, r) => el(`row.${cells[0].toLowerCase()}`, rowBox(r))),
  el("paragraph", B.paragraph),
  el("footer", B.footer),
];

const R = (name, b) => ({ name, page: "p1", ...b });
const HEADER = R("header", { x: 48, y: 40, w: 864, h: 70 });
const CHART = R("chart", B.panel);
const TABLE = R("table", { x: 48, y: 704, w: 864, h: 216 });
const PARA = R("paragraph", B.paragraph);
const BARS = R("bars", { x: 96, y: 330, w: 768, h: 300 });
const CARDROW = R("cards", { x: 48, y: 136, w: 864, h: 112 });
const card = (k, b) => at(b, cardBox(k).x, cardBox(k).y);
const BRAND = rgb(C.brand);

const steps = [
  {
    instruction: "Orders came in higher than first reported. Change the value in the first stat card to 12,960.",
    touch: ["card1.value"],
    frozen: [HEADER, CHART, TABLE, PARA],
    checks: [{ kind: "source-contains", text: "12,960" }, { kind: "changed", ref: "prev", ...card(0, PART.value), min: 30 }],
  },
  {
    instruction: "Revenue fell instead of rising. Set the change in the second stat card to -1.1% and give it the same red as the other falling figure.",
    touch: ["card2.delta"],
    frozen: [R("card1", cardBox(0)), CHART, TABLE],
    checks: [{ kind: "source-contains", text: "-1.1%" }, { kind: "colorcount", ...card(1, PART.delta), rgb: rgb(C.down), tol: 16, min: 15 }, { kind: "colorcount", ...card(1, PART.delta), rgb: rgb(C.up), tol: 16, max: 0 }],
  },
  {
    instruction: 'Update the label under the chart so it reads "Figure 1. Orders by month, H1 2027".',
    touch: ["chart.caption"],
    frozen: [BARS, CARDROW, TABLE],
    checks: [{ kind: "source-contains", text: "Figure 1. Orders by month, H1 2027" }],
  },
  {
    instruction: "June's figure was wrong: it is 2,400 orders. Update its bar and the number above it. Bars are drawn at 0.1 px per order from the shared baseline.",
    touch: ["bar.jun", "val.jun"],
    mask: [{ page: "p1", x: 752, y: 300, w: 100, h: 302 }],
    relocate: [{ ids: "bar.jun", dy: -25, dh: 25 }, { ids: "val.jun", dy: -25 }],
    frozen: [R("bars-jan-may", { x: 96, y: 300, w: 640, h: 302 }), CARDROW, TABLE],
    checks: [
      { kind: "pixel", x: 802, y: 365, rgb: BRAND, tol: 4 },
      { kind: "pixel", x: 802, y: 355, rgb: rgb(C.bg), tol: 4 },
      { kind: "source-contains", text: "2,400" },
      { kind: "ink", x: 752, y: 334, w: 100, h: 22, min: 40 },
    ],
  },
  {
    instruction: "Highlight the tallest bar in the chart in orange, #f97316.",
    touch: ["bar.jun"],
    frozen: [R("bars-jan-may", { x: 96, y: 300, w: 640, h: 302 }), R("caption", B.caption), CARDROW],
    checks: [{ kind: "colorcount", x: 770, y: 400, w: 64, h: 150, rgb: rgb("#f97316"), tol: 12, min: 5000 }, { kind: "pixel", x: 672, y: 500, rgb: BRAND, tol: 4 }],
  },
  {
    instruction: "Add a row at the bottom of the table for LATAM: 1,140 orders, $0.17M revenue, 93.8% on time. Match the other rows.",
    touch: [],
    frozen: [TABLE, PARA, CHART],
    checks: [{ kind: "source-contains", text: "LATAM" }, { kind: "source-contains", text: "93.8%" }, { kind: "ink", x: 48, y: 920, w: 864, h: 36, min: 300 }],
  },
  {
    instruction: "Remove the table row with the lowest on-time rate. The rows below it move up.",
    touch: ["row.apac"],
    mask: [{ page: "p1", x: 48, y: 884, w: 864, h: 72 }],
    frozen: [R("rows-1-4", { x: 48, y: 704, w: 864, h: 180 }), PARA],
    checks: [{ kind: "source-absent", text: "APAC" }, { kind: "source-contains", text: "LATAM" }, { kind: "ink", x: 48, y: 920, w: 864, h: 36, max: 20 }, { kind: "ink", x: 48, y: 884, w: 864, h: 36, min: 300 }],
  },
  {
    instruction: "The summary paragraph still quotes the old order total. Make it match the first stat card.",
    touch: ["paragraph"],
    frozen: [CARDROW, CHART, R("table", { x: 48, y: 704, w: 864, h: 216 })],
    checks: [{ kind: "source-absent", text: "12,480" }, { kind: "source-count", text: "12,960", min: 2 }],
  },
  {
    instruction: "Swap the third and fourth stat cards.",
    touch: ["card3.*", "card4.*"],
    mask: [{ page: "p1", x: 488, y: 136, w: 424, h: 112 }],
    relocate: [{ ids: "card3.*", dx: 220 }, { ids: "card4.*", dx: -220 }],
    frozen: [R("cards-1-2", { x: 48, y: 136, w: 424, h: 112 }), CHART, TABLE],
    checks: [
      { kind: "same-as", ref: "prev", ...cardBox(2), dx: 220 },
      { kind: "same-as", ref: "prev", ...cardBox(3), dx: -220 },
    ],
  },
  {
    instruction: "Make the chart title the same color as the bars that are not highlighted.",
    touch: ["chart.title"],
    frozen: [BARS, CARDROW],
    checks: [{ kind: "colorcount", ...B.chartTitle, rgb: BRAND, tol: 12, min: 30 }],
  },
  {
    instruction: "Make the label of the stat card that is now fourth in the row bold.",
    touch: ["card3.label"],
    frozen: [R("cards-1-3", { x: 48, y: 136, w: 644, h: 112 }), CHART, TABLE],
    checks: [{ kind: "ink-ratio", ref: "prev", ...card(3, PART.label), metric: "count", min: 1.12 }],
  },
  {
    instruction: 'Shorten the report title to "Q2 operations report". Keep its position and style.',
    touch: ["title"],
    frozen: [R("subtitle", B.subtitle), CARDROW, CHART],
    checks: [{ kind: "source-contains", text: "Q2 operations report" }, { kind: "source-absent", text: "Quarterly operations report" }, { kind: "ink-ratio", ref: "prev", ...B.title, metric: "w", min: 0.5, max: 0.8 }],
  },
];

writeTask(import.meta.dirname, {
  task: { id: "report12", canvas: { w: W, h: H }, background: rgb(C.bg), pages: 1, maxTurns: 20, maxCalls: 30, elements, steps },
  zen: zen(),
  html: html(),
});
