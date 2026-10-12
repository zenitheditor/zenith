// Generates the merge corpus: 200 attendee rows, 12 banner photos, and the hidden
// ground truth. The PRNG seed is fixed, so a rerun writes identical bytes.
// Usage: node bench/revision/merge/gen.mjs
import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { encode } from "../png.mjs";

const SEED = 20261012;
const ROWS = 200;
const out = path.join(import.meta.dirname, "data");

function mulberry32(a) {
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
const rand = mulberry32(SEED);
const pick = (list) => list[Math.floor(rand() * list.length)];
function shuffle(list) {
  const a = [...list];
  for (let i = a.length - 1; i > 0; i--) {
    const j = Math.floor(rand() * (i + 1));
    [a[i], a[j]] = [a[j], a[i]];
  }
  return a;
}

const FIRST = ["Ana", "Ben", "Chloe", "Daniel", "Elena", "Farid", "Grace", "Hugo", "Isla", "Jonas", "Keiko", "Liam", "Maya", "Noah", "Olivia", "Pedro", "Quinn", "Rosa", "Samir", "Tara", "Umar", "Vera", "Wes", "Ximena", "Yusuf", "Zoe"];
const LAST = ["Lima", "Okafor", "Martin", "Novak", "Fischer", "Haddad", "Kim", "Silva", "Brennan", "Larsen", "Tanaka", "Murphy", "Chen", "Patel", "Rossi", "Garcia", "Walsh", "Moreau", "Ibrahim", "Kowalski"];
const LONG_NAMES = [
  "Maximiliana Alexandrovna Konstantinopoulou-Wright",
  "Bartholomew Fitzgerald Montgomery-Vanderbilt III",
  "Anastasia Evangelina Papadopoulos-Richardson",
  "Guadalupe Esperanza Villanueva de los Santos",
  "Christopher Jonathan Abernathy-Worthington",
  "Oluwaseun Adebayo Oyelaran-Babatunde",
  "Siddharth Venkataraman Ramachandran-Iyer",
  "Marie-Antoinette Josephine de Beauharnais",
  "Thelonious Archibald Wolfensberger-Hughes",
  "Kaleikaumaka Leilani Kahananui-Akana",
  "Ingrid Margarethe Schwarzenegger-Lindqvist",
  "Rhiannon Gwendolyn Llewellyn-Pritchard",
  "Fernando Augusto Albuquerque de Magalhaes",
  "Nkechinyere Chiamaka Okonkwo-Adeyemi",
  "Hieronymus Leopold Wittgenstein-Habsburg",
];
// One word each: nothing to wrap at.
const UNBREAKABLE = ["Wolfeschlegelsteinhausenbergerdorff", "Venkatanarasimharajuvaripeta", "Taumatawhakatangihangakoauauotamatea"];
// No character here is drawn as a closed empty box (such as 口 or Hangul ㅁ), so a real glyph never looks like a missing-glyph box.
const CJK = ["张伟", "王芳", "李娜", "刘洋", "陈静", "杨磊", "黄敏", "赵强", "佐藤 健", "鈴木 花子", "이서연", "박지훈"];
const EMOJI = ["Maya 🌻 Chen", "Leo 🚀 Martin", "Ana 🌸 Souza", "Sam 😀 Park", "Priya ✨ Nair", "Jo 🎉 Walsh", "Kai 🌊 Tanaka", "Lena 🦊 Novak", "Omar 🔥 Haddad", "Ivy 🍀 Murphy"];
const SHORT = ["Bo", "Al", "Ed", "Jo", "Vi", "Su", "Ty", "Li", "Mo", "Cy", "Ra", "Ox"];
const TITLES = ["Software Engineer", "Product Manager", "Designer", "Data Scientist", "Head of Sales", "CTO", "Research Lead", "Marketing Director", "Developer Advocate", "Founder", "Staff Engineer", "UX Researcher"];
const LONG_TITLES = [
  "Senior Principal Staff Software Engineering Manager, Platform Reliability and Developer Experience",
  "Vice President of Global Strategic Partnerships, Alliances, and Ecosystem Development (EMEA and APAC)",
  "Associate Director of Clinical Data Management, Biostatistics, and Regulatory Submissions",
  "Chief Executive Officer, Co-Founder, and Head of Research for Applied Machine Learning Systems",
  "Distinguished Engineer and Technical Fellow, Distributed Storage Infrastructure Group",
];
const COMPANIES = ["Contoso", "Fabrikam", "Northwind Traders", "Tailspin Toys", "Wide World Importers", "Adventure Works", "Litware", "Proseware", "Woodgrove Bank", "Lucerne Publishing"];

// Banner photos: smooth two-colour gradients. Luminance class decides which text colours stay readable.
const PHOTOS = [
  ["mid", [96, 112, 132], [128, 104, 96]], ["mid", [84, 120, 108], [120, 120, 90]],
  ["mid", [110, 96, 130], [90, 118, 140]], ["mid", [120, 110, 92], [100, 124, 120]],
  ["mid", [104, 116, 100], [126, 100, 112]], ["mid", [92, 108, 140], [118, 118, 104]],
  ["light", [246, 242, 232], [226, 236, 244]], ["light", [255, 250, 240], [236, 240, 226]],
  ["light", [232, 240, 250], [250, 236, 230]], ["dark", [12, 14, 26], [34, 20, 30]],
  ["dark", [8, 20, 18], [26, 30, 44]], ["dark", [20, 12, 10], [16, 18, 36]],
];
const PW = 360, PH = 88;

function photo([, a, b], k) {
  const data = Buffer.alloc(PW * PH * 4);
  for (let y = 0; y < PH; y++) {
    for (let x = 0; x < PW; x++) {
      const t = (x / PW) * 0.75 + (y / PH) * 0.25;
      const wave = 0.06 * Math.sin((x + k * 17) / 23) * Math.cos(y / 11);
      const o = (y * PW + x) * 4;
      for (let c = 0; c < 3; c++) data[o + c] = Math.max(0, Math.min(255, Math.round(a[c] + (b[c] - a[c]) * t + wave * 40)));
      data[o + 3] = 255;
    }
  }
  return encode({ width: PW, height: PH, data });
}

const csvCell = (s) => (/[",\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s);

function main() {
  mkdirSync(path.join(out, "photos"), { recursive: true });
  PHOTOS.forEach((p, i) => writeFileSync(path.join(out, "photos", `p${String(i + 1).padStart(2, "0")}.png`), photo(p, i)));

  const idx = shuffle([...Array(ROWS).keys()]);
  const nameTag = new Map();
  let at = 0;
  const take = (n, tag) => idx.slice(at, (at += n)).forEach((i) => nameTag.set(i, tag));
  take(LONG_NAMES.length, "long_name");
  take(UNBREAKABLE.length, "unbreakable_name");
  take(CJK.length, "cjk_name");
  take(EMOJI.length, "emoji_name");
  take(SHORT.length, "short_name");
  const pools = { long_name: [...LONG_NAMES], unbreakable_name: [...UNBREAKABLE], cjk_name: [...CJK], emoji_name: [...EMOJI], short_name: [...SHORT] };

  const tIdx = shuffle([...Array(ROWS).keys()]);
  const titleTag = new Map([...tIdx.slice(0, 20).map((i) => [i, "long_title"]), ...tIdx.slice(20, 30).map((i) => [i, "empty_title"])]);
  const cIdx = shuffle([...Array(ROWS).keys()]);
  const companyTag = new Map(cIdx.slice(0, 10).map((i) => [i, "empty_company"]));
  const pIdx = shuffle([...Array(ROWS).keys()]);
  const photoOf = new Map();
  pIdx.forEach((row, k) => {
    // 30 light, 30 dark, 140 mid.
    const want = k < 30 ? "light" : k < 60 ? "dark" : "mid";
    const choices = PHOTOS.map((p, i) => [p[0], i]).filter(([c]) => c === want).map(([, i]) => i);
    photoOf.set(row, pick(choices));
  });

  const rows = [];
  const truth = [];
  for (let i = 0; i < ROWS; i++) {
    const id = `A${String(i + 1).padStart(3, "0")}`;
    const tags = [];
    const nt = nameTag.get(i);
    const name = nt ? pools[nt].shift() : `${pick(FIRST)} ${pick(LAST)}`;
    if (nt) tags.push(nt);
    const tt = titleTag.get(i);
    const title = tt === "long_title" ? pick(LONG_TITLES) : tt === "empty_title" ? "" : pick(TITLES);
    if (tt) tags.push(tt);
    const company = companyTag.has(i) ? "" : pick(COMPANIES);
    if (companyTag.has(i)) tags.push("empty_company");
    const p = photoOf.get(i);
    if (PHOTOS[p][0] !== "mid") tags.push(`photo_${PHOTOS[p][0]}`);
    const photoPath = `photos/p${String(p + 1).padStart(2, "0")}.png`;
    rows.push([id, name, title, company, photoPath]);
    // `short_name` is a layout stress, not a reason to withhold; every other tag makes a row hostile.
    truth.push({ id, tags, hostile: tags.some((t) => t !== "short_name") });
  }
  const csv = ["id,name,title,company,photo", ...rows.map((r) => r.map(csvCell).join(","))].join("\n") + "\n";
  writeFileSync(path.join(out, "attendees.csv"), csv);
  writeFileSync(path.join(out, "truth.json"), `${JSON.stringify({ seed: SEED, rows: truth }, null, 1)}\n`);
  const hostile = truth.filter((t) => t.hostile).length;
  console.log(`wrote ${ROWS} rows (${hostile} hostile), ${PHOTOS.length} photos to ${out}`);
}

main();
