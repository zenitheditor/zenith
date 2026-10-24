// Reference edits for mailer, one function per step and arm. `run.mjs --model oracle`
// applies them to prove that every check can pass in both formats.
const LONG = "Export print, web and social files in one step, with bleed, crop marks and color profiles set for each printer.";
const SHORT = "Export print and web files in one step.";

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
const zenInstance = (src, id) => src.match(new RegExp(`        instance id="${id}" [\\s\\S]*?\\n        }\\n`))[0];
const htmlPage = (src, n) => src.match(new RegExp(`<section class="page" id="page-${n}">[\\s\\S]*?</section>\\n`))[0];
const onPage = (src, n, f) => {
  const p = htmlPage(src, n);
  return must(src, p, f(p));
};

export default {
  zenith: [
    (s) => must(s, 'token id="radius.card" type="dimension" value=(px)16', 'token id="radius.card" type="dimension" value=(px)4'),
    (s) => must(must(s, `override ref="card.body" { span "${SHORT}" }`, `override ref="card.body" { span "${LONG}" }`), 'text id="card.body" x=(px)20 y=(px)56 w=(px)168 h=(px)56', 'text id="card.body" x=(px)20 y=(px)56 w=(px)168 h=(px)150'),
    (s) => must(must(s, '    asset id="asset.print"', '    asset id="asset.harbor" kind="image" src="assets/harbor.png"\n    asset id="asset.print"'), 'image id="p2.photo" asset="asset.workshop"', 'image id="p2.photo" asset="asset.harbor"'),
    (s) => ["p2", "p3"].reduce((t, p) => must(t, `frame id="${p}.cards" x=(px)48 y=(px)608 w=(px)704 h=(px)220 layout="row" gap=(token)"gap.cards"`, `frame id="${p}.cards" x=(px)48 y=(px)608 w=(px)704 h=(px)220 layout="row" gap=(px)40`), s),
    (s) => must(s, 'token id="size.headline" type="dimension" value=(px)40', 'token id="size.headline" type="dimension" value=(px)36'),
    (s) => swap(s, zenInstance(s, "p4.c1"), zenInstance(s, "p4.c2")),
    (s) => must(s, "$49", "$59"),
    (s) => must(s, '          override ref="card.badge" visible=#true\n          override ref="card.badge.label" visible=#true\n', ""),
    (s) => ["p3.c1", "p3.c2", "p3.c3"].reduce((t, id) => {
      const block = zenInstance(t, id);
      return must(t, block, block.replace('override ref="card.title" {', 'override ref="card.title" fill=(token)"color.accent" {'));
    }, s),
    (s) => must(s, "Coming soon.", "Available now in 14 countries."),
    (s) => must(s, "Our studio in Lisbon", "Our studio in Porto"),
  ],
  html: [
    (s) => must(s, "background: var(--card); border-radius: 16px;", "background: var(--card); border-radius: 4px;"),
    (s) => must(s, `<p class="card-body">${SHORT}</p>`, `<p class="card-body" style="height: 150px;">${LONG}</p>`),
    (s) => must(s, 'src="assets/workshop.png"', 'src="assets/harbor.png"'),
    (s) => [2, 3].reduce((t, n) => onPage(t, n, (p) => must(p, '<div class="cards">', '<div class="cards" style="gap: 40px;">')), s),
    (s) => must(s, "font-size: 40px; font-weight: 700; color: var(--ink);", "font-size: 36px; font-weight: 700; color: var(--ink);"),
    (s) =>
      onPage(s, 4, (p) => {
        const cards = p.match(/    <div class="card">.*\n/g);
        return swap(p, cards[0], cards[1]);
      }),
    (s) => must(s, "$49", "$59"),
    (s) => must(s, '<span class="badge">NEW</span>', ""),
    (s) => onPage(s, 3, (p) => p.split('<h2 class="card-title">').join('<h2 class="card-title" style="color: var(--accent);">')),
    (s) => must(s, "Coming soon.", "Available now in 14 countries."),
    (s) => must(s, "Our studio in Lisbon", "Our studio in Porto"),
  ],
};
