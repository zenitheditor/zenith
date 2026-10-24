// deck20: a 20-slide board deck. Steps have non-local blast radius: a brand
// color on every slide, every title, an overflow on one slide, a page insert, a
// page swap, every footer, and five edits that name no page or id.
// Usage: node bench/revision/tasks/deck20/gen.mjs
import { kdl, esc, rgb, boxCss, boxZen, positions, writeTask } from "../gen-lib.mjs";

const C = { bg: "#ffffff", ink: "#0f172a", muted: "#475569", brand: "#2563eb", soft: "#eff6ff" };
const W = 960;
const H = 540;
const B = {
  bar: { x: 48, y: 40, w: 64, h: 6 },
  kicker: { x: 48, y: 56, w: 600, h: 20 },
  title: { x: 48, y: 80, w: 864, h: 56 },
  body: { x: 48, y: 168, w: 520, h: 104 },
  panel: { x: 616, y: 168, w: 296, h: 200 },
  stat: { x: 640, y: 196, w: 248, h: 60 },
  statlabel: { x: 640, y: 268, w: 248, h: 84 },
  footer: { x: 48, y: 496, w: 864, h: 20 },
};
const FOOTER = "Northwind Labs · Series B review · Confidential";
const FOOTER2 = "Northwind Labs · Board meeting · March 2027";

// [kicker, title, body, stat, statlabel]
const SLIDES = [
  ["SERIES B REVIEW", "Northwind Labs", "Board update on growth, product and the plan for the next eighteen months.", "2027", "Plan year under review"],
  ["CONTEXT", "Why now", "Freight teams still plan loads in spreadsheets. Our tools cut planning time by more than half.", "52%", "Less time spent planning each load"],
  ["MARKET", "Market overview", "Mid-size carriers in North America and Europe spend over four billion a year on planning software.", "$4.2B", "Yearly spend in our segment"],
  ["PRODUCT", "What we built", "A load planner, a live yard map and a carrier portal that share one data model.", "3", "Products on one platform"],
  ["CUSTOMERS", "Who buys it", "Regional carriers with 50 to 500 trucks. Most start with the planner and add the map within a year.", "212", "Paying carriers today"],
  ["TRACTION", "Growth so far", "Revenue grew every quarter last year as carriers moved from monthly to annual plans.", "$41k", "Average yearly contract"],
  ["TRACTION", "Usage", "Planners open the tool on four out of five working days. Weekly use rose through the winter peak.", "81%", "Weekly active planners"],
  ["FINANCE", "Revenue", "Annual recurring revenue at year end, up from the year before on the back of larger accounts.", "$3.1M", "Annual recurring revenue"],
  ["SALES", "Pipeline", "Qualified pipeline for the next two quarters, weighted by stage and by deal size.", "$6.8M", "Weighted pipeline"],
  ["PLAN", "Roadmap", "Route optimisation ships in spring. A fuel and emissions module follows in the autumn release.", "2", "Major releases this year"],
  ["TEAM", "Hirring plan", "We add engineers first, then account managers once the new modules are live.", "24", "New hires planned"],
  ["TEAM", "Leadership", "The founding team stays in place. We hire a finance lead and a head of sales this year.", "6", "People on the leadership team"],
  ["SUPPORT", "Service quality", "Most tickets close on the first reply. Carriers rate support above every other part of the product.", "94%", "Tickets solved on first reply"],
  ["FINANCE", "Unit economics", "Gross margin is 78% and payback on sales spend is under fourteen months.", "78%", "Gross margin"],
  ["FINANCE", "Burn and runway", "Monthly net burn is steady. The raise extends runway well past the next two releases.", "26", "Months of runway after the raise"],
  ["RISK", "Risks", "Larger platforms could bundle planning tools. Our answer is depth for mid-size fleets.", "3", "Risks we track monthly"],
  ["ASK", "The raise", "We are raising a Series B to fund the roadmap and the sales team.", "$18M", "Series B target"],
  ["USE OF FUNDS", "Use of funds", "Sixty percent goes to product and engineering, the rest to sales, support and finance.", "60%", "Share for product and engineering"],
  ["CONTEXT", "Why now", "Freight teams still plan loads in spreadsheets. Our tools cut planning time by more than half.", "52%", "Less time spent planning each load"],
  ["NEXT STEPS", "Questions", "Thank you. The data room is open to the board from today.", "Q&A", "Open discussion"],
];
// Slide 17's stat label is the one darker label.
const DARK_LABEL = 17;

function zen() {
  const t = (id, type, value) => `    token id="${id}" type="${type}" value=${value}`;
  const style = (id, size, fill, bold) =>
    [`    style id="${id}" {`, `      font-family (token)"font.body"`, `      font-size (token)"${size}"`, ...(bold ? [`      font-weight (token)"weight.bold"`] : []), `      fill (token)"${fill}"`, "    }"].join("\n");
  const out = [
    "zenith version=1 {",
    '  project id="proj.deck20" name="Board deck"',
    '  tokens format="zenith-token-v1" {',
    ...Object.entries(C).map(([k, v]) => t(`color.${k}`, "color", kdl(v))),
    t("font.body", "fontFamily", kdl("Noto Sans")),
    t("weight.bold", "fontWeight", 700),
    ...[["kicker", 14], ["title", 40], ["body", 20], ["stat", 44], ["label", 16], ["footer", 12]].map(([k, v]) => t(`size.${k}`, "dimension", `(px)${v}`)),
    t("radius.panel", "dimension", "(px)12"),
    "  }",
    "  styles {",
    style("deck.kicker", "size.kicker", "color.brand", true),
    style("deck.title", "size.title", "color.ink", true),
    style("deck.body", "size.body", "color.muted", false),
    style("deck.stat", "size.stat", "color.brand", true),
    style("deck.label", "size.label", "color.muted", false),
    style("deck.footer", "size.footer", "color.muted", false),
    "  }",
    "  masters {",
    '    master id="master.slide" {',
    `      rect id="chrome.bar" ${boxZen(B.bar)} fill=(token)"color.brand"`,
    `      text id="chrome.footer" ${boxZen(B.footer)} style="deck.footer" { span ${kdl(FOOTER)} }`,
    "    }",
    "  }",
    '  document id="doc.deck20" title="Board deck" {',
  ];
  SLIDES.forEach(([kicker, title, body, stat, label], i) => {
    const p = `p${i + 1}`;
    const labelFill = i + 1 === DARK_LABEL ? ' fill=(token)"color.ink"' : "";
    out.push(
      `    page id="${p}" w=(px)${W} h=(px)${H} background=(token)"color.bg" master="master.slide" {`,
      `      text id="${p}.kicker" ${boxZen(B.kicker)} style="deck.kicker" { span ${kdl(kicker)} }`,
      `      text id="${p}.title" ${boxZen(B.title)} style="deck.title" { span ${kdl(title)} }`,
      `      text id="${p}.body" ${boxZen(B.body)} style="deck.body" { span ${kdl(body)} }`,
      `      rect id="${p}.panel" ${boxZen(B.panel)} fill=(token)"color.soft" radius=(token)"radius.panel"`,
      `      text id="${p}.stat" ${boxZen(B.stat)} style="deck.stat" { span ${kdl(stat)} }`,
      `      text id="${p}.statlabel" ${boxZen(B.statlabel)} style="deck.label"${labelFill} { span ${kdl(label)} }`,
      "    }",
    );
  });
  out.push("  }", "}", "");
  return out.join("\n");
}

function html() {
  const rel = (b) => ({ ...b, x: b.x - B.panel.x, y: b.y - B.panel.y });
  const out = [
    "<!doctype html>",
    '<html lang="en">',
    "<head>",
    '<meta charset="utf-8">',
    "<title>Board deck</title>",
    "<style>",
    `  :root { ${Object.entries(C).map(([k, v]) => `--${k}: ${v};`).join(" ")} }`,
    "  html, body { margin: 0; }",
    '  body { font-family: "Noto Sans", sans-serif; }',
    `  .page { position: relative; width: ${W}px; height: ${H}px; overflow: hidden; background: var(--bg); }`,
    "  .page *, .page { box-sizing: border-box; }",
    "  .page > *, .panel > * { position: absolute; margin: 0; overflow: hidden; line-height: 1.3; }",
    `  .bar { ${boxCss(B.bar)}; background: var(--brand); }`,
    `  .kicker { ${boxCss(B.kicker)}; font-size: 14px; font-weight: 700; color: var(--brand); }`,
    `  .title { ${boxCss(B.title)}; font-size: 40px; font-weight: 700; color: var(--ink); }`,
    `  .body { ${boxCss(B.body)}; font-size: 20px; color: var(--muted); }`,
    `  .panel { ${boxCss(B.panel)}; background: var(--soft); border-radius: 12px; }`,
    `  .stat { ${boxCss(rel(B.stat))}; font-size: 44px; font-weight: 700; color: var(--brand); }`,
    `  .statlabel { ${boxCss(rel(B.statlabel))}; font-size: 16px; color: var(--muted); }`,
    `  .footer { ${boxCss(B.footer)}; font-size: 12px; color: var(--muted); }`,
    "</style>",
    "</head>",
    "<body>",
  ];
  SLIDES.forEach(([kicker, title, body, stat, label], i) => {
    const dark = i + 1 === DARK_LABEL ? ' style="color: var(--ink);"' : "";
    out.push(
      `<section class="page" id="slide-${i + 1}">`,
      '  <div class="bar"></div>',
      `  <p class="kicker">${esc(kicker)}</p>`,
      `  <h1 class="title">${esc(title)}</h1>`,
      `  <p class="body">${esc(body)}</p>`,
      `  <div class="panel"><p class="stat">${esc(stat)}</p><p class="statlabel"${dark}>${esc(label)}</p></div>`,
      `  <p class="footer">${esc(FOOTER)}</p>`,
      "</section>",
    );
  });
  out.push("</body>", "</html>", "");
  return out.join("\n");
}

const keys = SLIDES.map((_, i) => `p${i + 1}`);
const all = (f) => keys.map(f).flat();
const whole = (...ks) => ks.map((k) => ({ name: `${k}-page`, page: k }));
const region = (k, name) => ({ name: `${k}-${name}`, page: k, ...B[name] });
const PURPLE = rgb("#7c3aed");
const GREEN = rgb("#16a34a");
const OVERFLOW_TEXT =
  "Gross margin is 78% after hosting and support costs, up six points in a year as larger carriers moved to annual plans. Payback on sales and marketing spend is under fourteen months, and the newest cohorts pay back faster than any group before them because they adopt the yard map in their first quarter.";

const steps = [
  {
    instruction: "Change the brand blue (#2563eb) to #7c3aed everywhere it is used.",
    touch: ["*.bar", "*.kicker", "*.stat"],
    frozen: [region("p1", "body"), region("p8", "body"), region("p20", "body"), region("p1", "footer")],
  },
  { instruction: "Make every slide title 34px instead of 40px.", touch: ["*.title"], frozen: [region("p3", "body"), region("p3", "panel"), region("p20", "footer")] },
  {
    instruction: `Replace the paragraph that begins "Gross margin" with this text: "${OVERFLOW_TEXT}" All of the text must stay fully visible inside its box.`,
    touch: ["p14.body"],
    mask: [{ page: "p14", x: 48, y: 168, w: 540, h: 320 }],
    relocate: [{ ids: "p14.body", dh: 216 }],
    frozen: whole("p1", "p13", "p15"),
  },
  {
    instruction:
      'Insert a new slide after slide 5, in the same style as the other slides. Kicker "CUSTOMERS", title "Customer voice", paragraph "Carriers say the planner pays for itself within the first quarter of use.", panel figure "4.8", panel label "Average review score out of 5".',
    pages: { insert: 6, key: "n6" },
    touch: [],
    frozen: whole("p5", "p6", "p20"),
  },
  { instruction: 'Swap the slides titled "Roadmap" and "Risks".', pages: { swap: [11, 17] }, touch: [], frozen: whole("p1", "p10", "p16") },
  { instruction: `Change the footer on every slide to "${FOOTER2}".`, touch: ["*.footer"], frozen: [region("p1", "title"), region("p10", "body"), region("p20", "panel")] },
  { instruction: "One slide still shows last year's revenue figure, $3.1M. Update it to $4.6M.", touch: ["p8.stat"], frozen: [...whole("p7", "p9"), region("p8", "title")] },
  { instruction: "The slide about hiring has a typo in its title. Fix it.", touch: ["p11.title"], frozen: [...whole("p10", "p12"), region("p11", "body")] },
  { instruction: "One panel figure is above 90%. Make that figure green (#16a34a) to flag it.", touch: ["p13.stat"], frozen: [...whole("p12", "p14"), region("p13", "title")] },
  { instruction: "Delete the slide that repeats slide 2.", pages: { delete: 20 }, touch: [], frozen: whole("p2", "p18", "p20") },
  { instruction: "One panel label uses a darker color than the others. Make it match the rest.", touch: ["p17.statlabel"], frozen: [...whole("p16", "p18"), region("p17", "stat")] },
];

// Checks address pages by position after the step.
const pos = positions(keys, steps);
const P = (i, k) => pos[i][k];
const colorIn = (page, b, color, min, extra = {}) => ({ kind: "colorcount", page, ...b, rgb: color, tol: 10, min, ...extra });
const checks = [
  [...all((k) => colorIn(P(0, k), B.bar, PURPLE, 300)), ...["p1", "p10", "p20"].flatMap((k) => [colorIn(P(0, k), B.stat, PURPLE, 40), colorIn(P(0, k), B.kicker, PURPLE, 20)])],
  all((k) => ({ kind: "ink-ratio", page: P(1, k), ref: "start", ref_page: P(1, k), ...B.title, metric: "w", min: 0.78, max: 0.92 })),
  [{ kind: "source-contains", text: "newest cohorts pay back faster" }, { kind: "no-clip", page: P(2, "p14") }, { kind: "ink-ratio", page: P(2, "p14"), ref: "prev", x: 48, y: 168, w: 540, h: 320, metric: "count", min: 1.3 }],
  [
    { kind: "page-count", n: 21 },
    { kind: "source-contains", text: "Customer voice" },
    colorIn(6, B.bar, PURPLE, 300),
    colorIn(6, B.kicker, PURPLE, 20),
    { kind: "ink", page: 6, ...B.title, min: 800 },
    { kind: "ink", page: 6, ...B.footer, min: 150 },
    { kind: "ink-ratio", page: 6, ref: "start", ref_page: 1, ...B.footer, metric: "w", min: 0.98, max: 1.02 },
  ],
  [
    { kind: "same-as", page: 11, ref: "prev", ref_page: 17 },
    { kind: "same-as", page: 17, ref: "prev", ref_page: 11 },
  ],
  [{ kind: "source-absent", text: FOOTER }, { kind: "source-contains", text: FOOTER2 }, ...Array.from({ length: 21 }, (_, i) => ({ kind: "changed", page: i + 1, ref: "prev", ...B.footer, min: 30 }))],
  [{ kind: "source-contains", text: "$4.6M" }, { kind: "source-absent", text: "$3.1M" }],
  [{ kind: "source-contains", text: "Hiring plan" }, { kind: "source-absent", text: "Hirring" }],
  [colorIn(P(8, "p13"), B.stat, GREEN, 40), { kind: "colorcount", page: P(8, "p13"), ...B.stat, rgb: PURPLE, tol: 10, max: 0 }],
  [{ kind: "page-count", n: 20 }, { kind: "source-count", text: "Why now", min: 1, max: 1 }, { kind: "same-as", page: 20, ref: "prev", ref_page: 21 }, { kind: "same-as", page: 2, ref: "prev", ref_page: 2 }],
  [
    { kind: "colorcount", page: P(10, "p17"), ...B.statlabel, rgb: rgb(C.muted), tol: 16, min: 15 },
    { kind: "colorcount", page: P(10, "p17"), ...B.statlabel, rgb: rgb(C.ink), tol: 16, max: 2 },
  ],
];
steps.forEach((s, i) => (s.checks = checks[i]));

const elements = keys.flatMap((k) => Object.entries(B).map(([name, b]) => ({ id: `${k}.${name}`, page: k, ...b })));

writeTask(import.meta.dirname, {
  task: { id: "deck20", canvas: { w: W, h: H }, background: rgb(C.bg), pages: SLIDES.length, maxTurns: 20, maxCalls: 30, elements, steps },
  zen: zen(),
  html: html(),
});
