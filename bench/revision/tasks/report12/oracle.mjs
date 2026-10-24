// Reference edits for report12, one function per step and arm. `run.mjs --model oracle`
// applies them to prove that every check can pass in both formats.
const must = (src, from, to) => {
  if (!src.includes(from)) throw new Error(`oracle: missing ${from.slice(0, 60)}`);
  return src.replace(from, to);
};
const zenBlock = (src, id) => src.match(new RegExp(`      frame id="${id}" [\\s\\S]*?\\n      }\\n`))[0];
const LATAM = ["LATAM", "1,140", "$0.17M", "93.8%"];
const APAC = ["APAC", "1,200", "$0.18M", "91.2%"];
const swapX = (s, a, b, xa, xb, tmp) => must(must(must(s, a(xa), a(tmp)), b(xb), b(xa)), a(tmp), a(xb));

export default {
  zenith: [
    (s) => must(s, 'style="r.value" { span "12,480" }', 'style="r.value" { span "12,960" }'),
    (s) => must(s, 'style="r.up" { span "+5.4%" }', 'style="r.down" { span "-1.1%" }'),
    (s) => must(s, "Figure 1. Orders by month, Q1–Q2 2027", "Figure 1. Orders by month, H1 2027"),
    (s) =>
      must(
        must(s, 'rect id="bar.jun" x=(px)770 y=(px)385 w=(px)64 h=(px)215', 'rect id="bar.jun" x=(px)770 y=(px)360 w=(px)64 h=(px)240'),
        'text id="val.jun" x=(px)752 y=(px)361 w=(px)100 h=(px)20 style="r.axis" align="center" { span "2,150" }',
        'text id="val.jun" x=(px)752 y=(px)336 w=(px)100 h=(px)20 style="r.axis" align="center" { span "2,400" }',
      ),
    (s) => must(must(s, '    token id="font.body"', '    token id="color.orange" type="color" value="#f97316"\n    token id="font.body"'), 'rect id="bar.jun" x=(px)770 y=(px)360 w=(px)64 h=(px)240 fill=(token)"color.brand"', 'rect id="bar.jun" x=(px)770 y=(px)360 w=(px)64 h=(px)240 fill=(token)"color.orange"'),
    (s) => {
      const apac = zenBlock(s, "row.apac");
      const latam = APAC.reduce((t, v, i) => t.split(`"${v}"`).join(`"${LATAM[i]}"`), apac.split("row.apac").join("row.latam")).replace("y=(px)884", "y=(px)920");
      return must(s, apac, apac + latam);
    },
    (s) => must(must(s, zenBlock(s, "row.apac"), ""), 'frame id="row.latam" x=(px)48 y=(px)920', 'frame id="row.latam" x=(px)48 y=(px)884'),
    (s) => must(s, "Orders reached 12,480", "Orders reached 12,960"),
    (s) => swapX(s, (x) => `frame id="card3" x=(px)${x}`, (x) => `frame id="card4" x=(px)${x}`, 488, 708, 1),
    (s) => must(s, 'style="r.chart" { span "Monthly orders" }', 'style="r.chart" fill=(token)"color.brand" { span "Monthly orders" }'),
    (s) => must(s, 'style="r.label" { span "On-time rate" }', 'style="r.label" font-weight=(token)"weight.bold" { span "On-time rate" }'),
    (s) => must(s, '{ span "Quarterly operations report" }', '{ span "Q2 operations report" }'),
  ],
  html: [
    (s) => must(s, '<p class="value">12,480</p>', '<p class="value">12,960</p>'),
    (s) => must(s, '<p class="delta up">+5.4%</p>', '<p class="delta down">-1.1%</p>'),
    (s) => must(s, "Figure 1. Orders by month, Q1–Q2 2027", "Figure 1. Orders by month, H1 2027"),
    (s) => must(must(s, "left: 770px; top: 385px; height: 215px;", "left: 770px; top: 360px; height: 240px;"), 'style="left: 752px; top: 361px;">2,150', 'style="left: 752px; top: 336px;">2,400'),
    (s) => must(s, "left: 770px; top: 360px; height: 240px;", "left: 770px; top: 360px; height: 240px; background: #f97316;"),
    (s) => {
      const apac = `  <div class="row" style="top: 884px;">${APAC.map((c) => `<span>${c}</span>`).join("")}</div>\n`;
      return must(s, apac, `${apac}  <div class="row" style="top: 920px;">${LATAM.map((c) => `<span>${c}</span>`).join("")}</div>\n`);
    },
    (s) => must(must(s, `  <div class="row" style="top: 884px;">${APAC.map((c) => `<span>${c}</span>`).join("")}</div>\n`, ""), '<div class="row" style="top: 920px;"><span>LATAM', '<div class="row" style="top: 884px;"><span>LATAM'),
    (s) => must(s, "Orders reached 12,480", "Orders reached 12,960"),
    (s) => swapX(s, (x) => `<div class="card" style="left: ${x}px;"><p class="label">On-time`, (x) => `<div class="card" style="left: ${x}px;"><p class="label">Active`, 488, 708, 1),
    (s) => must(s, '<h2 class="chart-title">', '<h2 class="chart-title" style="color: var(--brand);">'),
    (s) => must(s, '<p class="label">On-time rate</p>', '<p class="label" style="font-weight: 700;">On-time rate</p>'),
    (s) => must(s, ">Quarterly operations report<", ">Q2 operations report<"),
  ],
};
