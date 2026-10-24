// Reference edits for deck20, one function per step and arm. `run.mjs --model oracle`
// applies them to prove that every check can pass in both formats.
const NEW = ["CUSTOMERS", "Customer voice", "Carriers say the planner pays for itself within the first quarter of use.", "4.8", "Average review score out of 5"];
const OLD5 = ["CUSTOMERS", "Who buys it", "Regional carriers with 50 to 500 trucks. Most start with the planner and add the map within a year.", "212", "Paying carriers today"];
const LONG =
  "Gross margin is 78% after hosting and support costs, up six points in a year as larger carriers moved to annual plans. Payback on sales and marketing spend is under fourteen months, and the newest cohorts pay back faster than any group before them because they adopt the yard map in their first quarter.";
const SHORT = "Gross margin is 78% and payback on sales spend is under fourteen months.";
const F1 = "Northwind Labs · Series B review · Confidential";
const F2 = "Northwind Labs · Board meeting · March 2027";

const must = (src, from, to) => {
  if (!src.includes(from)) throw new Error(`oracle: missing ${from.slice(0, 60)}`);
  return src.replace(from, to);
};
const swap = (src, a, b) => {
  if (src.indexOf(a) > src.indexOf(b)) [a, b] = [b, a];
  const i = src.indexOf(a);
  const j = src.indexOf(b);
  return src.slice(0, i) + b + src.slice(i + a.length, j) + a + src.slice(j + b.length);
};
const zenPage = (src, id) => src.match(new RegExp(`    page id="${id}" [\\s\\S]*?\\n    }\\n`))[0];
const htmlPage = (src, n) => src.match(new RegExp(`<section class="page" id="slide-${n}">[\\s\\S]*?</section>\\n`))[0];
const fill = (text, from, to) => from.reduce((t, f, i) => t.split(f).join(to[i]), text);

export default {
  zenith: [
    (s) => must(s, 'value="#2563eb"', 'value="#7c3aed"'),
    (s) => must(s, 'token id="size.title" type="dimension" value=(px)40', 'token id="size.title" type="dimension" value=(px)34'),
    (s) => must(s, `text id="p14.body" x=(px)48 y=(px)168 w=(px)520 h=(px)104 style="deck.body" { span "${SHORT}" }`, `text id="p14.body" x=(px)48 y=(px)168 w=(px)520 h=(px)300 style="deck.body" { span "${LONG}" }`),
    (s) => {
      const p5 = zenPage(s, "p5");
      return must(s, p5, p5 + fill(p5.split('"p5').join('"n6'), OLD5, NEW));
    },
    (s) => swap(s, zenPage(s, "p10"), zenPage(s, "p16")),
    (s) => must(s, F1, F2),
    (s) => must(s, "$3.1M", "$4.6M"),
    (s) => must(s, "Hirring plan", "Hiring plan"),
    (s) => must(must(s, '    token id="font.body"', '    token id="color.green" type="color" value="#16a34a"\n    token id="font.body"'), 'style="deck.stat" { span "94%" }', 'style="deck.stat" fill=(token)"color.green" { span "94%" }'),
    (s) => must(s, zenPage(s, "p19"), ""),
    (s) => must(s, 'style="deck.label" fill=(token)"color.ink"', 'style="deck.label"'),
  ],
  html: [
    (s) => must(s, "--brand: #2563eb;", "--brand: #7c3aed;"),
    (s) => must(s, "font-size: 40px; font-weight: 700; color: var(--ink);", "font-size: 34px; font-weight: 700; color: var(--ink);"),
    (s) => must(s, `<p class="body">${SHORT}</p>`, `<p class="body" style="height: 300px;">${LONG}</p>`),
    (s) => {
      const p5 = htmlPage(s, 5);
      return must(s, p5, p5 + fill(p5.replace('id="slide-5"', 'id="slide-5b"'), OLD5, NEW));
    },
    (s) => swap(s, htmlPage(s, 10), htmlPage(s, 16)),
    (s) => s.split(F1).join(F2),
    (s) => must(s, "$3.1M", "$4.6M"),
    (s) => must(s, "Hirring plan", "Hiring plan"),
    (s) => must(s, '<p class="stat">94%</p>', '<p class="stat" style="color: #16a34a;">94%</p>'),
    (s) => must(s, htmlPage(s, 19), ""),
    (s) => must(s, ' style="color: var(--ink);"', ""),
  ],
};
