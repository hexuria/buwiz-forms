import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
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
});
