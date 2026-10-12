// Minimal PNG codec: decode 8-bit non-interlaced PNGs to RGBA, encode RGBA.
import zlib from "node:zlib";

const SIG = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

function crc32(buf) {
  return zlib.crc32(buf);
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const td = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(td));
  return Buffer.concat([len, td, crc]);
}

export function encode({ width, height, data }) {
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8;
  ihdr[9] = 6;
  const raw = Buffer.alloc((width * 4 + 1) * height);
  for (let y = 0; y < height; y++) {
    raw[y * (width * 4 + 1)] = 0;
    data.copy(raw, y * (width * 4 + 1) + 1, y * width * 4, (y + 1) * width * 4);
  }
  return Buffer.concat([SIG, chunk("IHDR", ihdr), chunk("IDAT", zlib.deflateSync(raw)), chunk("IEND", Buffer.alloc(0))]);
}

export function decode(buf) {
  if (!buf.subarray(0, 8).equals(SIG)) throw new Error("not a PNG");
  let off = 8;
  let hdr;
  const idat = [];
  while (off < buf.length) {
    const len = buf.readUInt32BE(off);
    const type = buf.toString("ascii", off + 4, off + 8);
    const body = buf.subarray(off + 8, off + 8 + len);
    if (type === "IHDR") hdr = body;
    else if (type === "IDAT") idat.push(body);
    off += 12 + len;
  }
  const width = hdr.readUInt32BE(0);
  const height = hdr.readUInt32BE(4);
  const depth = hdr[8];
  const color = hdr[9];
  if (depth !== 8 || hdr[12] !== 0) throw new Error("only 8-bit non-interlaced PNG supported");
  const ch = { 0: 1, 2: 3, 4: 2, 6: 4 }[color];
  if (!ch) throw new Error(`unsupported PNG color type ${color}`);
  const raw = zlib.inflateSync(Buffer.concat(idat));
  const stride = width * ch;
  const out = Buffer.alloc(width * height * 4);
  let prev = Buffer.alloc(stride);
  for (let y = 0; y < height; y++) {
    const f = raw[y * (stride + 1)];
    const line = Buffer.from(raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1)));
    for (let i = 0; i < stride; i++) {
      const a = i >= ch ? line[i - ch] : 0;
      const b = prev[i];
      const c = i >= ch ? prev[i - ch] : 0;
      let v = line[i];
      if (f === 1) v += a;
      else if (f === 2) v += b;
      else if (f === 3) v += (a + b) >> 1;
      else if (f === 4) {
        const p = a + b - c;
        const pa = Math.abs(p - a);
        const pb = Math.abs(p - b);
        const pc = Math.abs(p - c);
        v += pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
      }
      line[i] = v & 255;
    }
    for (let x = 0; x < width; x++) {
      const s = x * ch;
      const d = (y * width + x) * 4;
      if (ch === 1) out.fill(line[s], d, d + 3), (out[d + 3] = 255);
      else if (ch === 2) out.fill(line[s], d, d + 3), (out[d + 3] = line[s + 1]);
      else {
        out[d] = line[s];
        out[d + 1] = line[s + 1];
        out[d + 2] = line[s + 2];
        out[d + 3] = ch === 4 ? line[s + 3] : 255;
      }
    }
    prev = line;
  }
  return { width, height, data: out };
}

/** Crop an RGBA image `{width,height,data}` to the given rectangle. */
export function crop(img, x, y, w, h) {
  const data = Buffer.alloc(w * h * 4);
  for (let row = 0; row < h; row++) {
    img.data.copy(data, row * w * 4, ((y + row) * img.width + x) * 4, ((y + row) * img.width + x + w) * 4);
  }
  return { width: w, height: h, data };
}

/** Resize an RGBA image `{width,height,data}` to `w` x `h` by averaging the source pixels under each target pixel. */
function resize(img, w, h) {
  const data = Buffer.alloc(w * h * 4);
  for (let y = 0; y < h; y++) {
    const y0 = Math.floor((y * img.height) / h);
    const y1 = Math.max(y0 + 1, Math.floor(((y + 1) * img.height) / h));
    for (let x = 0; x < w; x++) {
      const x0 = Math.floor((x * img.width) / w);
      const x1 = Math.max(x0 + 1, Math.floor(((x + 1) * img.width) / w));
      let r = 0, g = 0, b = 0, a = 0, n = 0;
      for (let sy = y0; sy < y1; sy++) {
        for (let sx = x0; sx < x1; sx++) {
          const i = (sy * img.width + sx) * 4;
          r += img.data[i]; g += img.data[i + 1]; b += img.data[i + 2]; a += img.data[i + 3];
          n++;
        }
      }
      const o = (y * w + x) * 4;
      data[o] = r / n; data[o + 1] = g / n; data[o + 2] = b / n; data[o + 3] = a / n;
    }
  }
  return { width: w, height: h, data };
}

/**
 * One image with every page of `images` (decoded RGBA) tiled in a grid of
 * ceil(sqrt(n)) columns on a white ground, at most `maxWidth` px wide and never
 * above scale 1. Both arms use this for `view` with `all`.
 */
export function contactSheet(images, maxWidth = 2048) {
  const gutter = 16;
  const cols = Math.ceil(Math.sqrt(images.length));
  const rows = Math.ceil(images.length / cols);
  const cellW = Math.max(...images.map((i) => i.width));
  const cellH = Math.max(...images.map((i) => i.height));
  const scale = Math.min(1, (maxWidth - gutter * (cols + 1)) / (cols * cellW));
  const w = Math.round(cellW * scale);
  const h = Math.round(cellH * scale);
  const width = gutter + cols * (w + gutter);
  const height = gutter + rows * (h + gutter);
  const data = Buffer.alloc(width * height * 4, 255);
  images.forEach((img, i) => {
    const small = resize(img, Math.max(1, Math.round(img.width * scale)), Math.max(1, Math.round(img.height * scale)));
    const ox = gutter + (i % cols) * (w + gutter);
    const oy = gutter + Math.floor(i / cols) * (h + gutter);
    for (let y = 0; y < small.height; y++) {
      small.data.copy(data, ((oy + y) * width + ox) * 4, y * small.width * 4, (y + 1) * small.width * 4);
    }
  });
  return { width, height, data };
}
