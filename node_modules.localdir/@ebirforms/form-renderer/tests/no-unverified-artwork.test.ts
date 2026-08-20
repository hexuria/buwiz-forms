import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { PNG } from "pngjs";
import { describe, expect, it } from "vitest";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const FORMS_ROOT = path.resolve(HERE, "../src/forms");
const ASSETS_ROOT = path.join(FORMS_ROOT, "assets");

describe("verified runtime artwork only", () => {
  it("does not retain raster barcode or QR assets", () => {
    const machineReadableRaster = fs.readdirSync(ASSETS_ROOT)
      .filter((name) => /(?:barcode|pdf417|qr)/i.test(name));
    expect(machineReadableRaster).toEqual([]);
  });

  it("does not expose the removed fake gradient masthead", () => {
    const components = fs.readFileSync(
      path.resolve(HERE, "../src/components.tsx"),
      "utf8"
    );
    const printCss = fs.readFileSync(
      path.resolve(HERE, "../src/print.css"),
      "utf8"
    );
    expect(components).not.toContain("FormMasthead");
    expect(printCss).not.toContain("repeating-linear-gradient");
  });

  it("preserves every reviewed official seal as monochrome source pixels", () => {
    const sealAssets = fs.readdirSync(ASSETS_ROOT)
      .filter((name) => /-seal\.png$/i.test(name))
      .sort();

    expect(sealAssets).toHaveLength(10);
    for (const name of sealAssets) {
      const png = PNG.sync.read(fs.readFileSync(path.join(ASSETS_ROOT, name)));
      for (let offset = 0; offset < png.data.length; offset += 4) {
        const red = png.data[offset];
        const green = png.data[offset + 1];
        const blue = png.data[offset + 2];
        expect([green, blue], `${name} pixel ${offset / 4}`).toEqual([red, red]);
      }
    }
  });
});
