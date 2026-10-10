// Unit tests for the page modules that run without a browser: the KDL
// stream grammar, the UTF-16 / byte offset conversion, and the canvas render
// window and placement math, and the keystroke batch split. The async tests
// of `unit_page.js` (buffer sync, the engine Worker client, file writes,
// swatch colors) run after these.
//
// Run: node zenith-cli/tests/editor_e2e/unit.js
// Exit code 0 when every check passes. Zero dependencies.

import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import path from "node:path";

const here = path.dirname(fileURLToPath(import.meta.url));
const assets = path.join(here, "..", "..", "assets", "editor");
const { StringStream } = await import(path.join(assets, "vendor", "codemirror.js"));
const { kdl } = await import(path.join(assets, "js", "code", "kdl.js"));
const text = await import(path.join(assets, "js", "util", "text.js"));
const { lineDiff } = await import(path.join(assets, "js", "util", "diff.js"));
const region = await import(path.join(assets, "js", "canvas", "region.js"));
const { placement } = await import(path.join(assets, "js", "canvas", "paint.js"));
const { Drag, ROTATE_SNAP } = await import(path.join(assets, "js", "app", "drag.js"));
const { handleCursor } = await import(path.join(assets, "js", "canvas", "overlay.js"));
const { splitBatch, batchDiagnostics } = await import(path.join(assets, "js", "engine", "batch.js"));

let passed = 0;
const failures = [];

function test(name, fn) {
  try {
    fn();
    passed++;
  } catch (err) {
    failures.push(`${name}: ${err.message}`);
  }
}

/** Tokenize `source`: `[[type, text], …]` per line, nulls dropped. */
function tokens(source) {
  const state = kdl.startState(2);
  const lines = [];
  for (const line of source.split("\n")) {
    const stream = new StringStream(line, 2, 2);
    const out = [];
    if (line === "") {
      lines.push(out);
      continue;
    }
    let guard = 0;
    while (!stream.eol()) {
      const type = kdl.token(stream, state);
      const piece = stream.current();
      if (stream.pos === stream.start) throw new Error(`no progress at ${stream.pos} in ${line}`);
      if (type) out.push([type, piece]);
      stream.start = stream.pos;
      if (++guard > 1000) throw new Error("tokenizer loop");
    }
    lines.push(out);
  }
  return lines;
}

function typeOf(source, piece, line = 0) {
  const found = tokens(source)[line].find(([, t]) => t === piece);
  return found ? found[0] : null;
}

test("node names, properties, units, token annotations", () => {
  const src = 'rect id="a" x=(px)20 fill=(token)"color.ink" rotate=(deg)-12.5';
  const t = tokens(src)[0];
  assert.deepEqual(t, [
    ["keyword", "rect"],
    ["propertyName", "id"],
    ["operator", "="],
    ["string", '"a"'],
    ["propertyName", "x"],
    ["operator", "="],
    ["unit", "(px)"],
    ["number", "20"],
    ["propertyName", "fill"],
    ["operator", "="],
    ["tokenTag", "(token)"],
    ["tokenRef", '"color.ink"'],
    ["propertyName", "rotate"],
    ["operator", "="],
    ["unit", "(deg)"],
    ["number", "-12.5"],
  ]);
});

test("keywords, hex and exponent numbers, bare words", () => {
  assert.equal(typeOf("n a=#true", "#true"), "bool");
  assert.equal(typeOf("n a=#false", "#false"), "bool");
  assert.equal(typeOf("n a=#null", "#null"), "null");
  assert.equal(typeOf("n a=#inf", "#inf"), "number");
  assert.equal(typeOf("n a=0xff_00", "0xff_00"), "number");
  assert.equal(typeOf("n a=1.5e-3", "1.5e-3"), "number");
  assert.equal(typeOf("n align=center", "center"), "string");
  assert.equal(typeOf("n a=12px", "12px"), "string", "a number glued to letters is a word");
});

test("comments: line, block, nested block across lines, slashdash", () => {
  assert.deepEqual(tokens("// note")[0], [["comment", "// note"]]);
  assert.equal(typeOf("rect /* x */ id=1", "/* x */"), "comment");
  const lines = tokens("a /* one /* two */\nstill */ b=1");
  assert.equal(lines[0][1][0], "comment");
  assert.deepEqual(lines[1][0], ["comment", "still */"]);
  assert.equal(typeOf("a /* one /* two */\nstill */ b=1", "b", 1), "propertyName");
  assert.equal(typeOf("/-rect id=1", "/-"), "comment");
});

test("raw and multi-line strings", () => {
  assert.equal(typeOf('n s=#"a "quoted" b"#', '#"a "quoted" b"#'), "string");
  assert.equal(typeOf('n s=r#"x"#', 'r#"x"#'), "string");
  const lines = tokens('n s="""\nbody "x"\n"""\nnext');
  assert.deepEqual(lines[1], [["string", 'body "x"']]);
  assert.deepEqual(lines[2], [["string", '"""']]);
  assert.deepEqual(lines[3], [["keyword", "next"]]);
  assert.equal(typeOf('n s="a \\" b"', '"a \\" b"'), "string", "escaped quote stays inside");
});

test("children braces, semicolons, and node position", () => {
  const lines = tokens('page id="p" {\n  span "hi"; text\n}');
  assert.deepEqual(lines[0].at(-1), ["brace", "{"]);
  assert.deepEqual(lines[1][0], ["keyword", "span"]);
  assert.deepEqual(lines[1].at(-1), ["keyword", "text"]);
  assert.deepEqual(lines[2], [["brace", "}"]]);
  const inline = tokens('text { span "a" }')[0];
  assert.deepEqual(inline[2], ["keyword", "span"]);
});

test("line continuation keeps the node going", () => {
  const lines = tokens("rect id=1 \\\n  w=2");
  assert.deepEqual(lines[1][0], ["propertyName", "w"]);
});

test("unclosed quote ends at the line end", () => {
  const lines = tokens('n s="open\nnext a=1');
  assert.deepEqual(lines[1][0], ["keyword", "next"]);
});

test("indent follows brace depth", () => {
  const state = kdl.startState(2);
  for (const line of ["a {", "  b {"]) {
    const s = new StringStream(line, 2, 2);
    while (!s.eol()) {
      kdl.token(s, state);
      s.start = s.pos;
    }
  }
  const cx = { unit: 2 };
  assert.equal(kdl.indent(state, "c", cx), 4);
  assert.equal(kdl.indent(state, "}", cx), 2);
});

test("utf16 to byte and back over ASCII, 2-, 3-, and 4-byte characters", () => {
  const s = "aé—😀b\n";
  // a=1 byte, é=2, —=3, 😀=4 (2 code units), b=1, \n=1.
  const pairs = [
    [0, 0],
    [1, 1],
    [2, 3],
    [3, 6],
    [5, 10],
    [6, 11],
    [7, 12],
  ];
  for (const [u, b] of pairs) {
    assert.equal(text.utf16ToByte(s, u), b, `u${u}`);
    assert.equal(text.byteToUtf16(s, b), u, `b${b}`);
  }
  assert.equal(text.utf8Length(s), Buffer.byteLength(s, "utf8"));
  // A byte inside a character maps to the characters before it.
  assert.equal(text.byteToUtf16(s, 2), 1);
  assert.equal(text.byteToUtf16(s, 8), 3);
  // A unit inside a surrogate pair maps to the pair start.
  assert.equal(text.utf16ToByte(s, 4), 6);
  // Out of range clamps.
  assert.equal(text.byteToUtf16(s, 999), s.length);
  assert.equal(text.utf16ToByte(s, -5), 0);
});

test("OffsetIndex agrees with Buffer over a large mixed text", () => {
  let s = "";
  for (let i = 0; i < 4000; i++) s += i % 7 === 0 ? "—😀" : i % 3 === 0 ? "é" : "x\n";
  const index = new text.OffsetIndex(s);
  assert.equal(index.byteLength, Buffer.byteLength(s, "utf8"));
  for (let u = 0; u <= s.length; u += 37) {
    // Skip positions inside a surrogate pair.
    const c = s.charCodeAt(u);
    if (c >= 0xdc00 && c <= 0xdfff) continue;
    const b = Buffer.byteLength(s.slice(0, u), "utf8");
    assert.equal(index.toByte(u), b, `toByte ${u}`);
    assert.equal(index.toUtf16(b), u, `toUtf16 ${b}`);
  }
});

test("diffRange finds the single replaced range and keeps surrogate pairs", () => {
  assert.equal(text.diffRange("abc", "abc"), null);
  assert.deepEqual(text.diffRange("abcdef", "abXYef"), { from: 2, to: 4, insert: "XY" });
  assert.deepEqual(text.diffRange("ab", "abc"), { from: 2, to: 2, insert: "c" });
  const r = text.diffRange("x😀y", "x😃y");
  assert.equal(r.from, 1);
  assert.equal(r.to, 3);
  assert.equal(r.insert, "😃");
});

test("lineDiff shows the changed block with context", () => {
  assert.equal(lineDiff("a\nb", "a\nb"), "");
  const d = lineDiff("1\n2\n3\n4\n5\n6", "1\n2\n3\nX\n5\n6");
  assert.equal(d, "@@ line 4\n  2\n  3\n- 4\n+ X\n  5\n  6");
  assert.equal(lineDiff("a", "a\nb"), "@@ line 2\n  a\n+ b");
});

test("render window: visible part plus half a viewport, clamped to the page", () => {
  const page = { w: 1000, h: 800 };
  const p = region.plan({ visible: { x: 300, y: 200, w: 200, h: 100 }, page, scale: 2 });
  assert.equal(p.scale, 2);
  assert.deepEqual(p.visible, { x: 300, y: 200, w: 200, h: 100 });
  assert.deepEqual(p.viewport, { x: 200, y: 150, w: 400, h: 200 });
  const edge = region.plan({ visible: { x: -50, y: 700, w: 200, h: 200 }, page, scale: 1 });
  assert.deepEqual(edge.visible, { x: 0, y: 700, w: 150, h: 100 });
  assert.equal(edge.viewport.x, 0);
  assert.equal(edge.viewport.y + edge.viewport.h, 800);
  assert.equal(region.plan({ visible: { x: 2000, y: 0, w: 10, h: 10 }, page, scale: 1 }), null);
  // A window clamped to the rounded device page still covers the page.
  const view = { x: 0, y: 0, w: 480, h: 583 / 1.62 };
  assert.equal(region.contains(view, { x: 0, y: 0, w: 480, h: 360 }), false);
  assert.equal(region.contains(view, { x: 0, y: 0, w: 480, h: 360 }, 1 / 1.62), true);
});

test("render window: margins shrink and the scale drops only past the limits", () => {
  const page = { w: 20000, h: 12000 };
  // 1440 × 900 CSS px at 800 %: the margin must shrink to fit 8191 px.
  const p = region.plan({ visible: { x: 9000, y: 5000, w: 180, h: 112.5 }, page, scale: 8 });
  assert.equal(p.scale, 8);
  assert.ok(region.fits(p.viewport, p.scale));
  assert.ok(region.contains(p.viewport, p.visible));
  // A visible part too large for the limits lowers the scale.
  const big = region.plan({ visible: { x: 0, y: 0, w: 5000, h: 5000 }, page, scale: 4 });
  assert.ok(big.scale < 4);
  assert.ok(region.fits(big.viewport, big.scale));
  // The page extent caps the scale: 20000 px × 64 passes 2^20.
  const deep = region.plan({ visible: { x: 0, y: 0, w: 10, h: 10 }, page, scale: 64 });
  assert.ok(deep.scale * 20000 < region.LIMITS.extent);
});

test("placement: 1:1 at whole device px at the render scale, scaled otherwise", () => {
  const shown = { rect: { x: 734, y: 290, w: 367, h: 240 }, scale: 2.5 };
  const exact = placement(shown, { zoom: 1.25, px: -300.3, py: 10 }, 0.3, 0, 2);
  assert.equal(exact.exact, true);
  assert.deepEqual([exact.x, exact.y, exact.w, exact.h], [734 - 600, 310, 367, 240]);
  const scaled = placement(shown, { zoom: 2.5, px: 0, py: 0 }, 0, 0, 2);
  assert.equal(scaled.exact, false);
  assert.equal(scaled.w, 734);
  assert.equal(scaled.x, 1468);
});

test("drag params: page deltas and modifier flags, no document math", () => {
  const drag = new Drag(null, { start: { x: 10, y: 20 }, version: 3, node: "r" });
  drag.point = { x: 15, y: 18 };
  assert.deepEqual(drag.params(), { node: "r", dx: 5, dy: -2 });
  drag.mods = { shift: true, alt: true, mod: false };
  assert.deepEqual(drag.params(), { node: "r", dx: 5, dy: -2, constrain: true, detach: true, detach_anchor: true });
  const grip = new Drag(null, { start: { x: 0, y: 0 }, version: 3, node: "r", handle: { id: "nw", role: "resize" } });
  grip.point = { x: -4, y: -4 };
  grip.mods = { shift: false, alt: false, mod: true };
  assert.deepEqual(grip.params(), { node: "r", handle: "nw", dx: -4, dy: -4, from_center: true });
  assert.equal(grip.action, "resize");
  const still = new Drag(null, { start: { x: 1, y: 1 }, version: 1, node: "r" });
  assert.equal(still.still(), true);
});

test("drag params: several nodes, snap reach, and no snap with Ctrl on a move", () => {
  const ctl = { snapReach: () => 3 };
  const many = new Drag(ctl, { start: { x: 0, y: 0 }, version: 1, nodes: ["a", "b"] });
  many.point = { x: 7, y: 1 };
  assert.deepEqual(many.params(), { nodes: ["a", "b"], dx: 7, dy: 1, snap_distance: 3 });
  many.mods = { shift: false, alt: false, mod: true };
  assert.deepEqual(many.params(), { nodes: ["a", "b"], dx: 7, dy: 1 });
  const one = new Drag(ctl, { start: { x: 0, y: 0 }, version: 1, nodes: ["a"] });
  one.point = { x: 1, y: 0 };
  assert.equal(one.params().node, "a");
  const grip = new Drag(ctl, { start: { x: 0, y: 0 }, version: 1, node: "r", handle: { id: "se", role: "resize" } });
  grip.point = { x: 2, y: 2 };
  grip.mods = { shift: false, alt: false, mod: true };
  assert.deepEqual(grip.params(), { node: "r", handle: "se", dx: 2, dy: 2, from_center: true, snap_distance: 3 });
  const off = new Drag({ snapReach: () => 0 }, { start: { x: 0, y: 0 }, version: 1, node: "r" });
  off.point = { x: 2, y: 0 };
  assert.equal(off.params().snap_distance, undefined);
  const band = new Drag(ctl, { start: { x: 5, y: 6 }, version: 1 });
  band.marquee = true;
  band.point = { x: 1, y: 9 };
  assert.equal(band.action, "marquee");
  assert.deepEqual(band.band(), { x0: 5, y0: 6, x1: 1, y1: 9 });
});

test("drag params: rotate angle about the centre, clockwise, in (-180, 180]", () => {
  const item = { center: [0, 0] };
  const rot = new Drag(null, { start: { x: 0, y: -10 }, version: 1, node: "r", item, handle: { id: "rotate", role: "rotate" } });
  rot.point = { x: 10, y: 0 };
  assert.equal(rot.params().angle, 90);
  rot.point = { x: -10, y: 0 };
  assert.equal(rot.params().angle, -90);
  rot.point = { x: -0.0001, y: 10 };
  assert.ok(Math.abs(Math.abs(rot.params().angle) - 180) < 0.01);
  rot.mods = { shift: true, alt: false, mod: false };
  assert.equal(rot.params().snap, ROTATE_SNAP);
  assert.equal(rot.params().dx, undefined);
});

test("handle cursors follow the handle direction from the centre", () => {
  const item = { center: [50, 50], corners: [[0, 0], [100, 0], [100, 100], [0, 100]] };
  const at = (id, x, y, role = "resize", enabled = true) => handleCursor({ id, role, x, y, enabled }, item);
  assert.equal(at("e", 100, 50), "ew-resize");
  assert.equal(at("s", 50, 100), "ns-resize");
  assert.equal(at("se", 100, 100), "nwse-resize");
  assert.equal(at("ne", 100, 0), "nesw-resize");
  assert.equal(at("rotate", 50, -24, "rotate"), "grab");
  assert.equal(at("p0", 10, 10, "vertex"), "crosshair");
  assert.equal(at("w", 0, 50, "resize", false), "not-allowed");
});

test("batch split: one envelope per step, the image on the step that rendered", () => {
  const steps = [
    { command: "buffer.set", params: { text: "t" }, version: 3 },
    { command: "doc.render", params: { scale: 1 } },
    { command: "select.at_offset", params: { offset: 99 } },
  ];
  const image = { url: "image/a.png", sha256: "a" };
  const env = {
    ok: true, command: "commands.batch", version: 4, dirty: true, image,
    result: {
      image_step: 1,
      steps: [
        { command: "buffer.set", ok: true, result: { diagnostics: [{ code: "font.unresolved" }] }, work: { parses: 1 } },
        { command: "doc.render", ok: true, result: { page: 1, diagnostics: [] }, work: { parses: 0 } },
        { command: "select.at_offset", ok: false, error: { code: "editor.invalid_params", message: "m", diagnostics: [{ code: "x" }] } },
      ],
    },
  };
  const [set, render, at] = splitBatch(env, steps);
  assert.deepEqual(set, { version: 4, dirty: true, ok: true, command: "buffer.set", work: { parses: 1 }, result: { diagnostics: [{ code: "font.unresolved" }] } });
  assert.equal(render.image, image);
  assert.equal(set.image, undefined);
  assert.equal(at.ok, false);
  assert.equal(at.error.code, "editor.invalid_params");
  assert.deepEqual(batchDiagnostics(env.result).map((d) => d.code), ["font.unresolved", "x"]);
  // A batch that did not run: every step carries its error and the offline mark.
  const off = splitBatch({ ok: false, offline: true, error: { code: "edit.offline", message: "down" } }, steps);
  assert.deepEqual(off.map((e) => [e.ok, e.offline, e.error.code]), [[false, true, "edit.offline"], [false, true, "edit.offline"], [false, true, "edit.offline"]]);
});

const { tests: pageTests } = await import("./unit_page.js");
const TEST_TIMEOUT_MS = 10000;
for (const [name, fn] of pageTests) {
  let timer;
  const timeout = new Promise((_, reject) => {
    timer = setTimeout(() => reject(new Error(`did not finish within ${TEST_TIMEOUT_MS} ms`)), TEST_TIMEOUT_MS);
  });
  try {
    await Promise.race([fn(), timeout]);
    passed++;
  } catch (err) {
    failures.push(`${name}: ${err.stack ?? err.message}`);
  } finally {
    clearTimeout(timer);
  }
}

for (const f of failures) console.error(`FAIL ${f}`);
console.log(JSON.stringify({ suite: "editor-unit", passed, failed: failures.length }));
process.exit(failures.length ? 1 : 0);
