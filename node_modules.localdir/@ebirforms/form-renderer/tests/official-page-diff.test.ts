import { describe, expect, it } from "vitest";
import { PNG } from "pngjs";
import { compareCompleteOfficialPage } from "../visual/official-page-diff";

function whitePage(width = 24, height = 24) {
  const page = new PNG({ width, height });
  page.data.fill(255);
  return page;
}

function putBlackPixel(page: PNG, x: number, y: number) {
  const offset = (y * page.width + x) * 4;
  page.data[offset] = 0;
  page.data[offset + 1] = 0;
  page.data[offset + 2] = 0;
  page.data[offset + 3] = 255;
}

describe("complete official-page comparison", () => {
  it("reports an identical page as zero difference", () => {
    const expected = whitePage();
    const actual = PNG.sync.read(PNG.sync.write(expected));
    expect(compareCompleteOfficialPage(expected, actual)).toMatchObject({
      fullPageChangedPixels: 0,
      fullPageChangedPercent: 0,
      expectedInkMissingPercent: 0,
      unexpectedActualInkPercent: 0
    });
  });

  it("detects missing short text-like ink that a long-line mask would ignore", () => {
    const expected = whitePage();
    const actual = whitePage();
    for (let x = 4; x < 10; x += 1) putBlackPixel(expected, x, 8);
    const comparison = compareCompleteOfficialPage(expected, actual, {
      inkToleranceRadius: 0
    });
    expect(comparison.fullPageChangedPixels).toBeGreaterThan(0);
    expect(comparison.expectedInkMissingPercent).toBe(100);
  });

  it("uses the ink radius only for diagnostic registration tolerance", () => {
    const expected = whitePage();
    const actual = whitePage();
    putBlackPixel(expected, 10, 10);
    putBlackPixel(actual, 11, 10);
    const comparison = compareCompleteOfficialPage(expected, actual, {
      inkToleranceRadius: 1
    });
    expect(comparison.fullPageChangedPixels).toBeGreaterThan(0);
    expect(comparison.expectedInkMissingPercent).toBe(0);
    expect(comparison.unexpectedActualInkPercent).toBe(0);
  });
});
