// mailer: a 4-page brochure with the same card component three times per page.
// Steps: card radius everywhere, one card's text overflows, a photo swap, card
// spacing on two pages only, plus edits that name the card by its content.
// Usage: node bench/revision/tasks/mailer/gen.mjs (writes assets/ too)
import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { encode } from "../../png.mjs";
import { kdl, esc, rgb, boxCss, boxZen, at, writeTask } from "../gen-lib.mjs";

const C = { bg: "#ffffff", ink: "#1c1917", muted: "#57534e", accent: "#0f766e", card: "#e7e5e4", badge: "#c2410c", onaccent: "#ffffff" };
const W = 800;
const H = 1000;
const B = {
  brand: { x: 48, y: 40, w: 400, h: 20 },
  headline: { x: 48, y: 72, w: 704, h: 52 },
  lede: { x: 48, y: 136, w: 704, h: 48 },
  photo: { x: 48, y: 200, w: 704, h: 360 },
  caption: { x: 48, y: 568, w: 704, h: 20 },
  panel: { x: 48, y: 200, w: 704, h: 360 },
  price: { x: 88, y: 320, w: 624, h: 64 },
  priceNote: { x: 88, y: 396, w: 624, h: 24 },
  footer: { x: 48, y: 948, w: 704, h: 20 },
};
const ROW = { x: 48, y: 608, w: 704, h: 220 };
const CARD = { w: 208, h: 220, gap: 24 };
// Card parts, relative to the card.
const PART = { title: { x: 20, y: 20, w: 168, h: 24 }, body: { x: 20, y: 56, w: 168, h: 56 }, badge: { x: 140, y: 16, w: 52, h: 22 } };
const cardBox = (k, gap = CARD.gap) => ({ x: ROW.x + k * (CARD.w + gap), y: ROW.y, w: CARD.w, h: CARD.h });

const PHOTOS = {
  studio: { top: [196, 120, 92], bottom: [120, 64, 48], spots: [[110, 90, 60], [600, 260, 80]] },
  workshop: { top: [148, 163, 184], bottom: [51, 65, 85], spots: [[90, 280, 70], [620, 80, 50]] },
  print: { top: [163, 190, 140], bottom: [63, 98, 18], spots: [[140, 70, 50], [580, 290, 70]] },
  harbor: { top: [125, 211, 252], bottom: [12, 74, 110], spots: [[100, 300, 60], [640, 60, 45]] },
};
const photoPx = (p, x, y) => {
  const t = y / (B.photo.h - 1);
  let c = p.top.map((v, i) => v + (p.bottom[i] - v) * t);
  for (const [sx, sy, r] of p.spots) {
    const d = Math.hypot(x - sx, y - sy);
    if (d < r) c = c.map((v) => v + (255 - v) * 0.35 * (1 - d / r));
  }
  return c.map(Math.round);
};
function photoPng(p) {
  const data = Buffer.alloc(B.photo.w * B.photo.h * 4);
  for (let y = 0; y < B.photo.h; y++) {
    for (let x = 0; x < B.photo.w; x++) {
      const c = photoPx(p, x, y);
      data.set([...c, 255], (y * B.photo.w + x) * 4);
    }
  }
  return encode({ width: B.photo.w, height: B.photo.h, data });
}

// [headline, lede, photo, caption, cards [title, body]]
const PAGES = [
  ["Design that ships on time", "Harbor & Co. builds brand systems for small teams, from the first logo to the last printed page.", "studio", "Our studio in Lisbon",
    [["Fast setup", "Your workspace is ready in a day, with templates for every team."], ["Shared library", "Logos, colors and type live in one place everyone can reach."], ["Live chat", "Talk to a person in under two minutes, every working day."]]],
  ["Built for busy teams", "Every draft, comment and export lives in one place, so nobody waits on an email thread.", "workshop", "Review day with the Harbor team",
    [["Reviews", "Collect comments on every draft without long email threads."], ["Delivery", "Export print and web files in one step."], ["Templates", "Start from layouts made by our studio."]]],
  ["From draft to print", "We check every file before it goes to press and stay with you until the boxes arrive.", "print", "Proofs from our print partner",
    [["Proofing", "See exactly what the printer will see before you send."], ["Print partners", "Coming soon."], ["Archive", "Every version is kept, so nothing is lost."]]],
  ["Plans and pricing", "Pick the plan that fits your team today and change it whenever you need to.", null, null,
    [["Starter", "One brand, three seats and the shared library."], ["Studio", "Five brands, ten seats and print partners."], ["Agency", "Unlimited brands and a dedicated account manager."]]],
];
const BADGE = { page: 2, card: 3 };
const FOOTER = "harbor-co.example · hello@harbor-co.example";

function zen() {
  const t = (id, type, value) => `    token id="${id}" type="${type}" value=${value}`;
  const style = (id, size, fill, bold) =>
    [`    style id="${id}" {`, `      font-family (token)"font.body"`, `      font-size (token)"${size}"`, ...(bold ? [`      font-weight (token)"weight.bold"`] : []), `      fill (token)"${fill}"`, "    }"].join("\n");
  const out = [
    "zenith version=1 {",
    '  project id="proj.mailer" name="Brochure"',
    "  assets {",
    ...["studio", "workshop", "print"].map((n) => `    asset id="asset.${n}" kind="image" src="assets/${n}.png"`),
    "  }",
    '  tokens format="zenith-token-v1" {',
    ...Object.entries(C).map(([k, v]) => t(`color.${k}`, "color", kdl(v))),
    t("font.body", "fontFamily", kdl("Noto Sans")),
    t("weight.bold", "fontWeight", 700),
    ...[["brand", 14], ["headline", 40], ["lede", 18], ["caption", 12], ["card.title", 18], ["card.body", 14], ["badge", 11], ["price", 48], ["note", 18]].map(([k, v]) => t(`size.${k}`, "dimension", `(px)${v}`)),
    t("radius.card", "dimension", "(px)16"),
    t("radius.panel", "dimension", "(px)16"),
    t("radius.pill", "dimension", "(px)11"),
    t("gap.cards", "dimension", `(px)${CARD.gap}`),
    "  }",
    "  styles {",
    style("ui.brand", "size.brand", "color.accent", true),
    style("ui.headline", "size.headline", "color.ink", true),
    style("ui.lede", "size.lede", "color.muted", false),
    style("ui.caption", "size.caption", "color.muted", false),
    style("card.title", "size.card.title", "color.ink", true),
    style("card.body", "size.card.body", "color.muted", false),
    style("card.badge", "size.badge", "color.onaccent", true),
    style("ui.price", "size.price", "color.onaccent", true),
    style("ui.note", "size.note", "color.onaccent", false),
    "  }",
    "  components {",
    '    component id="card" {',
    `      rect id="card.bg" x=(px)0 y=(px)0 w=(px)${CARD.w} h=(px)${CARD.h} fill=(token)"color.card" radius=(token)"radius.card"`,
    `      text id="card.title" ${boxZen(PART.title)} style="card.title" { span "Title" }`,
    `      text id="card.body" ${boxZen(PART.body)} style="card.body" { span "Body" }`,
    `      rect id="card.badge" ${boxZen(PART.badge)} fill=(token)"color.badge" radius=(token)"radius.pill" visible=#false`,
    `      text id="card.badge.label" ${boxZen(PART.badge)} style="card.badge" align="center" v-align="middle" visible=#false { span "NEW" }`,
    "    }",
    "  }",
    '  document id="doc.mailer" title="Brochure" {',
  ];
  PAGES.forEach(([headline, lede, photo, caption, cards], i) => {
    const p = `p${i + 1}`;
    out.push(
      `    page id="${p}" w=(px)${W} h=(px)${H} background=(token)"color.bg" {`,
      `      text id="${p}.brand" ${boxZen(B.brand)} style="ui.brand" { span "HARBOR & CO." }`,
      `      text id="${p}.headline" ${boxZen(B.headline)} style="ui.headline" { span ${kdl(headline)} }`,
      `      text id="${p}.lede" ${boxZen(B.lede)} style="ui.lede" { span ${kdl(lede)} }`,
    );
    if (photo) {
      out.push(`      image id="${p}.photo" asset="asset.${photo}" ${boxZen(B.photo)} fit="cover"`, `      text id="${p}.caption" ${boxZen(B.caption)} style="ui.caption" { span ${kdl(caption)} }`);
    } else {
      out.push(
        `      rect id="${p}.panel" ${boxZen(B.panel)} fill=(token)"color.accent" radius=(token)"radius.panel"`,
        `      text id="${p}.price" ${boxZen(B.price)} style="ui.price" { span "From $49 per month" }`,
        `      text id="${p}.note" ${boxZen(B.priceNote)} style="ui.note" { span "Billed yearly. Cancel any time." }`,
      );
    }
    out.push(`      frame id="${p}.cards" ${boxZen(ROW)} layout="row" gap=(token)"gap.cards" {`);
    cards.forEach(([title, body], k) => {
      out.push(
        `        instance id="${p}.c${k + 1}" component="card" w=(px)${CARD.w} h=(px)${CARD.h} {`,
        `          override ref="card.title" { span ${kdl(title)} }`,
        `          override ref="card.body" { span ${kdl(body)} }`,
      );
      if (i + 1 === BADGE.page && k + 1 === BADGE.card) out.push('          override ref="card.badge" visible=#true', '          override ref="card.badge.label" visible=#true');
      out.push("        }");
    });
    out.push("      }", `      text id="${p}.footer" ${boxZen(B.footer)} style="ui.caption" { span ${kdl(FOOTER)} }`, "    }");
  });
  out.push("  }", "}", "");
  return out.join("\n");
}

function html() {
  const rel = (b, o) => ({ ...b, x: b.x - o.x, y: b.y - o.y });
  const out = [
    "<!doctype html>",
    '<html lang="en">',
    "<head>",
    '<meta charset="utf-8">',
    "<title>Brochure</title>",
    "<style>",
    `  :root { ${Object.entries(C).map(([k, v]) => `--${k}: ${v};`).join(" ")} }`,
    "  html, body { margin: 0; }",
    '  body { font-family: "Noto Sans", sans-serif; }',
    `  .page { position: relative; width: ${W}px; height: ${H}px; overflow: hidden; background: var(--bg); }`,
    "  .page *, .page { box-sizing: border-box; }",
    "  .page > *, .card > *, .panel > * { position: absolute; margin: 0; overflow: hidden; line-height: 1.3; }",
    `  .brand { ${boxCss(B.brand)}; font-size: 14px; font-weight: 700; color: var(--accent); }`,
    `  .headline { ${boxCss(B.headline)}; font-size: 40px; font-weight: 700; color: var(--ink); }`,
    `  .lede { ${boxCss(B.lede)}; font-size: 18px; color: var(--muted); }`,
    `  .photo { ${boxCss(B.photo)}; object-fit: cover; display: block; }`,
    `  .caption { ${boxCss(B.caption)}; font-size: 12px; color: var(--muted); }`,
    `  .panel { ${boxCss(B.panel)}; background: var(--accent); border-radius: 16px; }`,
    `  .price { ${boxCss(rel(B.price, B.panel))}; font-size: 48px; font-weight: 700; color: var(--onaccent); }`,
    `  .note { ${boxCss(rel(B.priceNote, B.panel))}; font-size: 18px; color: var(--onaccent); }`,
    `  .cards { ${boxCss(ROW)}; display: flex; gap: ${CARD.gap}px; overflow: visible; }`,
    `  .card { position: relative; flex: none; width: ${CARD.w}px; height: ${CARD.h}px; background: var(--card); border-radius: 16px; }`,
    `  .card-title { ${boxCss(PART.title)}; font-size: 18px; font-weight: 700; color: var(--ink); }`,
    `  .card-body { ${boxCss(PART.body)}; font-size: 14px; color: var(--muted); }`,
    `  .badge { ${boxCss(PART.badge)}; background: var(--badge); border-radius: 11px; color: var(--onaccent); font-size: 11px; font-weight: 700; line-height: 22px; text-align: center; }`,
    `  .footer { ${boxCss(B.footer)}; font-size: 12px; color: var(--muted); }`,
    "</style>",
    "</head>",
    "<body>",
  ];
  PAGES.forEach(([headline, lede, photo, caption, cards], i) => {
    out.push(
      `<section class="page" id="page-${i + 1}">`,
      '  <p class="brand">HARBOR &amp; CO.</p>',
      `  <h1 class="headline">${esc(headline)}</h1>`,
      `  <p class="lede">${esc(lede)}</p>`,
    );
    if (photo) out.push(`  <img class="photo" src="assets/${photo}.png" alt="">`, `  <p class="caption">${esc(caption)}</p>`);
    else out.push('  <div class="panel">', '    <p class="price">From $49 per month</p>', '    <p class="note">Billed yearly. Cancel any time.</p>', "  </div>");
    out.push('  <div class="cards">');
    cards.forEach(([title, body], k) => {
      const badge = i + 1 === BADGE.page && k + 1 === BADGE.card ? '<span class="badge">NEW</span>' : "";
      out.push(`    <div class="card"><h2 class="card-title">${esc(title)}</h2><p class="card-body">${esc(body)}</p>${badge}</div>`);
    });
    out.push("  </div>", `  <p class="footer">${esc(FOOTER)}</p>`, "</section>");
  });
  out.push("</body>", "</html>", "");
  return out.join("\n");
}

const keys = PAGES.map((_, i) => `p${i + 1}`);
const region = (k, b, name) => ({ name: `${k}-${name}`, page: k, ...b });
const PCARD = [...keys.map((k, i) => [k, i])];
const elements = PCARD.flatMap(([k, i]) => {
  const own = Object.entries(B).filter(([n]) => (PAGES[i][2] ? !["panel", "price", "priceNote"].includes(n) : !["photo", "caption"].includes(n)));
  const cards = [0, 1, 2].flatMap((c) => {
    const box = cardBox(c);
    const parts = [{ id: `${k}.c${c + 1}.bg`, page: k, ...box }, ...["title", "body"].map((n) => ({ id: `${k}.c${c + 1}.${n}`, page: k, ...at(PART[n], box.x, box.y) }))];
    if (i + 1 === BADGE.page && c + 1 === BADGE.card) parts.push({ id: `${k}.c${c + 1}.badge`, page: k, ...at(PART.badge, box.x, box.y) });
    return parts;
  });
  return [...own.map(([n, b]) => ({ id: `${k}.${n}`, page: k, ...b })), ...cards];
});

const CARD_RGB = rgb(C.card);
const WHITE = rgb(C.bg);
const harborAt = photoPx(PHOTOS.harbor, B.photo.w / 2, B.photo.h / 2);
const corner = (page, c) => ({ kind: "pixel", page, x: cardBox(c).x + 2, y: ROW.y + 2, rgb: CARD_RGB, tol: 8 });
const wide = (c) => cardBox(c, 40);
const LONG = "Export print, web and social files in one step, with bleed, crop marks and color profiles set for each printer.";

const steps = [
  {
    instruction: "Make the card corners 4px instead of 16px, on every card.",
    touch: ["*.c*.bg"],
    frozen: [region("p1", B.headline, "headline"), region("p1", B.photo, "photo"), region("p4", B.panel, "panel")],
    checks: [corner(1, 0), corner(2, 2), corner(3, 1), corner(4, 2), corner(4, 0)],
  },
  {
    instruction: `Replace the text of the card about delivery with: "${LONG}" Every word must stay visible.`,
    touch: ["p2.c2.body"],
    mask: [{ page: "p2", ...at(PART.body, cardBox(1).x, ROW.y), h: 150 }],
    relocate: [{ ids: "p2.c2.body", dh: 94 }],
    frozen: [region("p2", cardBox(0), "card-1"), region("p2", cardBox(2), "card-3"), region("p1", ROW, "cards")],
    checks: [{ kind: "source-contains", text: "crop marks" }, { kind: "no-clip", page: 2 }, { kind: "ink-ratio", page: 2, ref: "prev", x: cardBox(1).x + 20, y: ROW.y + 56, w: 168, h: 150, metric: "count", min: 1.3, background: rgb(C.card) }],
  },
  {
    instruction: "Replace the photo on page 2 with assets/harbor.png.",
    touch: ["p2.photo"],
    frozen: [region("p1", B.photo, "photo"), region("p3", B.photo, "photo"), region("p2", B.headline, "headline")],
    checks: [{ kind: "colorcount", page: 2, x: B.photo.x + B.photo.w / 2 - 20, y: B.photo.y + B.photo.h / 2 - 20, w: 40, h: 40, rgb: harborAt, tol: 16, min: 1200 }],
  },
  {
    instruction: "On pages 2 and 3 only, make the space between the cards 40px.",
    touch: ["p2.c*", "p3.c*"],
    mask: [{ page: "p2", ...ROW }, { page: "p3", ...ROW }],
    relocate: [{ ids: "p2.c2.*", dx: 16 }, { ids: "p2.c3.*", dx: 32 }, { ids: "p3.c2.*", dx: 16 }, { ids: "p3.c3.*", dx: 32 }],
    frozen: [region("p1", ROW, "cards"), region("p4", ROW, "cards")],
    checks: [2, 3].flatMap((page) => [
      { kind: "pixel", page, x: 290, y: 800, rgb: WHITE, tol: 4 },
      { kind: "pixel", page, x: 500, y: 800, rgb: CARD_RGB, tol: 8 },
      { kind: "pixel", page, x: 748, y: 800, rgb: CARD_RGB, tol: 8 },
    ]),
  },
  {
    instruction: "Make the page headlines 4px smaller on every page.",
    touch: ["*.headline"],
    frozen: keys.map((k) => region(k, B.lede, "lede")),
    checks: [1, 2, 3, 4].map((page) => ({ kind: "ink-ratio", page, ref: "start", ...B.headline, metric: "w", min: 0.85, max: 0.95 })),
  },
  {
    instruction: "On the pricing page, swap the first two cards.",
    touch: ["p4.c1.*", "p4.c2.*"],
    mask: [{ page: "p4", x: ROW.x, y: ROW.y, w: 440, h: CARD.h }],
    relocate: [{ ids: "p4.c1.*", dx: 232 }, { ids: "p4.c2.*", dx: -232 }],
    frozen: [region("p4", cardBox(2), "card-3"), region("p4", B.panel, "panel"), region("p3", ROW, "cards")],
    checks: [
      { kind: "same-as", page: 4, ...cardBox(0), dx: 232 },
      { kind: "same-as", page: 4, ...cardBox(1), dx: -232 },
    ],
  },
  {
    instruction: "The price on the back page is out of date. It is now $59 per month.",
    touch: ["p4.price"],
    frozen: [region("p4", B.priceNote, "note"), region("p4", ROW, "cards")],
    checks: [{ kind: "source-contains", text: "$59" }, { kind: "source-absent", text: "$49" }],
  },
  {
    instruction: 'Remove the "NEW" badge from the card that has one.',
    touch: ["p2.c3.badge"],
    frozen: [region("p2", cardBox(0), "card-1"), region("p2", B.photo, "photo"), region("p1", ROW, "cards")],
    checks: [{ kind: "colorcount", page: 2, ...at(PART.badge, wide(2).x - 8, ROW.y - 8), w: 68, h: 38, rgb: rgb(C.badge), tol: 24, max: 5 }],
  },
  {
    instruction: "Make the card titles on page 3 the same teal as the brand name at the top of the page.",
    touch: ["p3.c*.title"],
    frozen: [region("p2", ROW, "cards"), region("p4", ROW, "cards"), region("p3", B.brand, "brand")],
    checks: [0, 1, 2].map((c) => ({ kind: "colorcount", page: 3, ...at(PART.title, wide(c).x, ROW.y), rgb: rgb(C.accent), tol: 12, min: 20 })),
  },
  {
    instruction: 'One card still says "Coming soon." Replace that text with "Available now in 14 countries."',
    touch: ["p3.c2.body"],
    frozen: [region("p3", wide(0), "card-1"), region("p3", wide(2), "card-3"), region("p2", ROW, "cards")],
    checks: [{ kind: "source-contains", text: "Available now in 14 countries." }, { kind: "source-absent", text: "Coming soon" }],
  },
  {
    instruction: "The photo caption on page 1 names the wrong city. The studio is in Porto, not Lisbon.",
    touch: ["p1.caption"],
    frozen: [region("p1", B.photo, "photo"), region("p1", ROW, "cards")],
    checks: [{ kind: "source-contains", text: "Our studio in Porto" }, { kind: "source-count", text: "Lisbon", max: 0 }],
  },
];

const dir = import.meta.dirname;
mkdirSync(path.join(dir, "assets"), { recursive: true });
for (const [name, p] of Object.entries(PHOTOS)) writeFileSync(path.join(dir, "assets", `${name}.png`), photoPng(p));
writeTask(dir, {
  task: { id: "mailer", canvas: { w: W, h: H }, background: rgb(C.bg), pages: PAGES.length, maxTurns: 20, maxCalls: 30, elements, steps },
  zen: zen(),
  html: html(),
});
