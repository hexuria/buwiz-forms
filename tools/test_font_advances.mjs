// The print's width tables are the bundled fonts' own: decode each woff2 in
// html-frozen/fonts (brotli, node's zlib), read its cmap and hmtx, and check
// that crates/bir-print/src/frozen_html.rs's ARIMO_ADVANCES and
// TINOS_ADVANCES hold exactly every character the cmap maps from U+0020 up,
// with that glyph's advance width, and that the units per em are 2048
// (tools/font_advances.py writes the tables).
import assert from "node:assert/strict";
import fs from "node:fs";
import zlib from "node:zlib";

const root = new URL("../", import.meta.url);
const KNOWN = ["cmap", "head", "hhea", "hmtx", "maxp", "name", "OS/2", "post", "cvt ", "fpgm", "glyf", "loca", "prep", "CFF ", "VORG", "EBDT", "EBLC", "gasp", "hdmx", "kern", "LTSH", "PCLT", "VDMX", "vhea", "vmtx", "BASE", "GDEF", "GPOS", "GSUB", "EBSC", "JSTF", "MATH", "CBDT", "CBLC", "COLR", "CPAL", "SVG ", "sbix", "acnt", "avar", "bdat", "bloc", "bsln", "cvar", "fdsc", "feat", "fmtx", "fvar", "gvar", "hsty", "just", "lcar", "mort", "morx", "opbd", "prop", "trak", "Zapf", "Silf", "Glat", "Gloc", "Feat", "Sill"];

function tables(file) {
  const buf = fs.readFileSync(new URL(`html-frozen/fonts/${file}`, root));
  assert.equal(buf.toString("latin1", 0, 4), "wOF2", file);
  const numTables = buf.readUInt16BE(12);
  const compressedSize = buf.readUInt32BE(20);
  let at = 48;
  const base128 = () => {
    let value = 0;
    for (let i = 0; i < 5; i++) {
      const byte = buf[at++];
      value = value * 128 + (byte & 0x7f);
      if (!(byte & 0x80)) return value;
    }
    throw new Error("bad UIntBase128");
  };
  const dir = [];
  for (let i = 0; i < numTables; i++) {
    const flags = buf[at++];
    let tag;
    if ((flags & 0x3f) === 63) {
      tag = buf.toString("latin1", at, at + 4);
      at += 4;
    } else tag = KNOWN[flags & 0x3f];
    const version = (flags >> 6) & 3;
    const origLength = base128();
    // glyf and loca are transformed at version 0; every other table at a
    // version other than 0 (hmtx's version 1).
    const transformed = tag === "glyf" || tag === "loca" ? version === 0 : version !== 0;
    const length = transformed ? base128() : origLength;
    dir.push({ tag, length, transformed });
  }
  const data = zlib.brotliDecompressSync(buf.subarray(at, at + compressedSize));
  const out = {};
  let offset = 0;
  for (const t of dir) {
    out[t.tag] = { bytes: data.subarray(offset, offset + t.length), transformed: t.transformed };
    offset += t.length;
  }
  return out;
}

function advances(file) {
  const t = tables(file);
  for (const tag of ["cmap", "head", "hhea", "hmtx"]) assert.ok(t[tag] && !t[tag].transformed, `${file}: ${tag} untransformed`);
  const unitsPerEm = t.head.bytes.readUInt16BE(18);
  const hMetrics = t.hhea.bytes.readUInt16BE(34);
  const hmtx = t.hmtx.bytes;
  const advance = (glyph) => hmtx.readUInt16BE(4 * Math.min(glyph, hMetrics - 1));
  const cmap = t.cmap.bytes;
  const count = cmap.readUInt16BE(2);
  let best = null;
  for (let i = 0; i < count; i++) {
    const platform = cmap.readUInt16BE(4 + 8 * i);
    const encoding = cmap.readUInt16BE(6 + 8 * i);
    const offset = cmap.readUInt32BE(8 + 8 * i);
    const format = cmap.readUInt16BE(offset);
    if (platform === 3 && (encoding === 10 || encoding === 1) && (format === 12 || format === 4)) {
      if (!best || format === 12) best = { offset, format };
    }
  }
  assert.ok(best, `${file}: a Unicode cmap`);
  const map = new Map();
  if (best.format === 12) {
    const groups = cmap.readUInt32BE(best.offset + 12);
    for (let g = 0; g < groups; g++) {
      const p = best.offset + 16 + 12 * g;
      const start = cmap.readUInt32BE(p), end = cmap.readUInt32BE(p + 4), glyph = cmap.readUInt32BE(p + 8);
      for (let cp = start; cp <= end; cp++) map.set(cp, glyph + cp - start);
    }
  } else {
    const o = best.offset;
    const segs = cmap.readUInt16BE(o + 6) / 2;
    const ends = o + 14, starts = ends + 2 * segs + 2, deltas = starts + 2 * segs, ranges = deltas + 2 * segs;
    for (let s = 0; s < segs; s++) {
      const end = cmap.readUInt16BE(ends + 2 * s), start = cmap.readUInt16BE(starts + 2 * s);
      const delta = cmap.readInt16BE(deltas + 2 * s), range = cmap.readUInt16BE(ranges + 2 * s);
      for (let cp = start; cp <= end && cp !== 0xffff; cp++) {
        let glyph;
        if (range === 0) glyph = (cp + delta) & 0xffff;
        else {
          const at = ranges + 2 * s + range + 2 * (cp - start);
          glyph = cmap.readUInt16BE(at);
          if (glyph) glyph = (glyph + delta) & 0xffff;
        }
        if (glyph) map.set(cp, glyph);
      }
    }
  }
  const rows = [...map.entries()].filter(([cp]) => cp >= 0x20).sort((a, b) => a[0] - b[0]).map(([cp, glyph]) => [cp, advance(glyph)]);
  return { unitsPerEm, rows };
}

const src = fs.readFileSync(new URL("crates/bir-print/src/frozen_html.rs", root), "utf8");
function rustTable(name) {
  const m = new RegExp(`const ${name}_ADVANCES: &\\[\\(u32, u16\\)\\] = &\\[([\\s\\S]*?)\\];`).exec(src);
  assert.ok(m, `${name}_ADVANCES in frozen_html.rs`);
  return [...m[1].matchAll(/\(0x([0-9A-Fa-f]+),\s*(\d+)\)/g)].map((r) => [parseInt(r[1], 16), Number(r[2])]);
}
for (const [name, file] of [["ARIMO", "arimo-latin-wght-normal.woff2"], ["TINOS", "tinos-latin-400-normal.woff2"]]) {
  const font = advances(file);
  assert.equal(font.unitsPerEm, 2048, `${file}: units per em`);
  assert.ok(src.includes("const UNITS_PER_EM: f64 = 2048.0;"), "the Rust units per em");
  const table = rustTable(name);
  assert.ok(font.rows.length > 200, `${file}: ${font.rows.length} characters`);
  assert.deepEqual(table, font.rows, `${name}_ADVANCES is ${file}'s cmap and hmtx`);
  console.log(`PASS ${name}_ADVANCES matches ${file}: ${font.rows.length} characters, ${font.unitsPerEm} units per em.`);
}
