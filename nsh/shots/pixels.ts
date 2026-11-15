// Pixels of a real-window screenshot, for the shots tests that check what a picture shows (an underline that must be there, a box
// that must not be). Headless runs have no picture (app.screenshot rejects), so a test that reads pixels runs only in a real window.
// Decodes the PNG the host writes (8-bit RGB or RGBA, no interlace), as many rows as asked for from the top.
import type { App } from "nsh/test";
import { inflateSync } from "zlib";

/** Takes `<name>.png` of the main window and hands its bytes back; null (and no file) when the run is headless. */
export async function picture(app: App, name: string): Promise<Uint8Array | null> {
  try {
    return await app.screenshot(`${name}.png`);
  } catch (error) {
    if (String(error).includes("needs a real platform")) return null;
    throw error;
  }
}

/** A decoded top part of a picture: 2x pixels on a Retina window (1440 points wide is 2880 pixels). */
export interface Raster {
  width: number;
  /** How many rows were decoded. */
  rows: number;
  /** The colour of pixel (x, y) as [r, g, b]. */
  at(x: number, y: number): [number, number, number];
}

function u32(b: Uint8Array, at: number): number {
  return ((b[at] << 24) | (b[at + 1] << 16) | (b[at + 2] << 8) | b[at + 3]) >>> 0;
}

/** The first `rows` rows of a PNG (all when omitted). */
export function decodePng(png: Uint8Array, rows?: number): Raster {
  let width = 0;
  let height = 0;
  let channels = 4;
  const parts: Uint8Array[] = [];
  let at = 8;
  while (at < png.length) {
    const length = u32(png, at);
    const kind = String.fromCharCode(png[at + 4], png[at + 5], png[at + 6], png[at + 7]);
    const body = png.subarray(at + 8, at + 8 + length);
    if (kind === "IHDR") {
      width = u32(body, 0);
      height = u32(body, 4);
      channels = body[9] === 2 ? 3 : 4;
    } else if (kind === "IDAT") parts.push(body);
    at += 12 + length;
  }
  const joined = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let offset = 0;
  for (const p of parts) {
    joined.set(p, offset);
    offset += p.length;
  }
  const raw = new Uint8Array(inflateSync(joined));
  const stride = width * channels;
  const count = Math.min(height, rows ?? height);
  const lines: Uint8Array[] = [];
  let prev = new Uint8Array(stride);
  for (let y = 0; y < count; y++) {
    const filter = raw[y * (stride + 1)];
    const line = raw.slice(y * (stride + 1) + 1, (y + 1) * (stride + 1));
    for (let i = 0; i < stride; i++) {
      const a = i >= channels ? line[i - channels] : 0;
      const b = prev[i];
      const c = i >= channels ? prev[i - channels] : 0;
      let add = 0;
      if (filter === 1) add = a;
      else if (filter === 2) add = b;
      else if (filter === 3) add = (a + b) >> 1;
      else if (filter === 4) {
        const p = a + b - c;
        const pa = Math.abs(p - a);
        const pb = Math.abs(p - b);
        const pc = Math.abs(p - c);
        add = pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
      }
      line[i] = (line[i] + add) & 255;
    }
    lines.push(line);
    prev = line;
  }
  return {
    width,
    rows: count,
    at: (x, y) => {
      const line = lines[y];
      const i = x * channels;
      return [line[i], line[i + 1], line[i + 2]];
    },
  };
}

/** The longest run of pixels in row `y`, between x0 and x1, that `test` accepts. */
export function longestRun(raster: Raster, y: number, x0: number, x1: number, test: (rgb: [number, number, number]) => boolean): number {
  let best = 0;
  let run = 0;
  for (let x = x0; x < x1; x++) {
    if (test(raster.at(x, y))) {
      run += 1;
      if (run > best) best = run;
    } else run = 0;
  }
  return best;
}
