import { describe, expect, it } from "vitest";
import { PNG } from "pngjs";
import {
  compareSymmetricGrayscaleEdges,
  LAYERED_EDGE_EVIDENCE_POLICY
} from "../visual/grayscale-edge-match";
import {
  OFFICIAL_2551Q_STATIC_TEXT,
  verifyPageIndexedStaticText,
  type OfficialStaticTextEntry
} from "../visual/official-2551q-static-text";
import { parsePromotionVisualThreshold } from "../visual/release-visual-threshold";

function whitePage(width = 80, height = 60) {
  const page = new PNG({ width, height });
  page.data.fill(255);
  return page;
}

function drawBlackLine(page: PNG, x1: number, y1: number, x2: number, y2: number) {
  if (x1 !== x2 && y1 !== y2) throw new Error("test lines must be axis-aligned");
  for (let y = Math.min(y1, y2); y <= Math.max(y1, y2); y += 1) {
    for (let x = Math.min(x1, x2); x <= Math.max(x1, x2); x += 1) {
      const offset = (y * page.width + x) * 4;
      page.data[offset] = 0;
      page.data[offset + 1] = 0;
      page.data[offset + 2] = 0;
      page.data[offset + 3] = 255;
    }
  }
}

describe("symmetric grayscale edge evidence", () => {
  it("reports identical edge structure as perfect precision, recall, and F1", () => {
    const expected = whitePage();
    drawBlackLine(expected, 8, 12, 70, 12);
    drawBlackLine(expected, 8, 38, 70, 38);
    const actual = PNG.sync.read(PNG.sync.write(expected));

    expect(compareSymmetricGrayscaleEdges(expected, actual)).toMatchObject({
      precision: 1,
      recall: 1,
      f1: 1,
      toleranceRadiusPx: 2
    });
  });

  it("accepts a two-pixel rasterizer registration shift symmetrically", () => {
    const expected = whitePage();
    const actual = whitePage();
    drawBlackLine(expected, 8, 20, 70, 20);
    drawBlackLine(actual, 8, 22, 70, 22);

    const metric = compareSymmetricGrayscaleEdges(expected, actual, {
      toleranceRadiusPx: 2
    });
    expect(metric.precision).toBe(1);
    expect(metric.recall).toBe(1);
    expect(metric.f1).toBe(1);
  });

  it("fails recall evidence when an official line is omitted", () => {
    const expected = whitePage();
    const actual = whitePage();
    drawBlackLine(expected, 8, 12, 70, 12);
    drawBlackLine(expected, 8, 42, 70, 42);
    drawBlackLine(actual, 8, 12, 70, 12);

    const metric = compareSymmetricGrayscaleEdges(expected, actual);
    expect(metric.recall).toBeLessThan(0.75);
    expect(metric.f1).toBeLessThan(0.9);
  });

  it("fails precision evidence when an unverified line is added", () => {
    const expected = whitePage();
    const actual = whitePage();
    drawBlackLine(expected, 8, 12, 70, 12);
    drawBlackLine(actual, 8, 12, 70, 12);
    drawBlackLine(actual, 8, 42, 70, 42);

    const metric = compareSymmetricGrayscaleEdges(expected, actual);
    expect(metric.precision).toBeLessThan(0.75);
    expect(metric.f1).toBeLessThan(0.9);
  });

  it("is additive evidence and cannot replace or relax the raw page gate", () => {
    expect(LAYERED_EDGE_EVIDENCE_POLICY).toEqual({
      promotionEligible: false,
      authoritativeVisualGate: "official-complete-page-v1",
      replacesAuthoritativeGate: false
    });
    expect(parsePromotionVisualThreshold(undefined)).toBe(1);
  });
});

describe("2551Q page-indexed static-text evidence", () => {
  const manifest: readonly OfficialStaticTextEntry[] = [
    { id: "page-one-label", page: 1, kind: "item", text: "6 Taxpayer Identification Number (TIN)" },
    { id: "page-two-heading", page: 2, kind: "table-heading", text: "Schedule 1 – Computation of Tax" }
  ];

  it("reports no violations when exact text is on the reviewed page", () => {
    expect(verifyPageIndexedStaticText([
      "6 Taxpayer Identification Number (TIN)",
      "Schedule 1 – Computation of Tax"
    ], manifest)).toEqual([]);
  });

  it("fails with the omitted label identified", () => {
    expect(verifyPageIndexedStaticText([
      "Part I – Background Information",
      "Schedule 1 – Computation of Tax"
    ], manifest)).toEqual([
      {
        id: "page-one-label",
        expectedPage: 1,
        text: "6 Taxpayer Identification Number (TIN)",
        foundOnPages: []
      }
    ]);
  });

  it("fails when an exact label is present on the wrong page", () => {
    expect(verifyPageIndexedStaticText([
      "Schedule 1 – Computation of Tax",
      "6 Taxpayer Identification Number (TIN)"
    ], manifest)).toEqual([
      {
        id: "page-one-label",
        expectedPage: 1,
        text: "6 Taxpayer Identification Number (TIN)",
        foundOnPages: [2]
      },
      {
        id: "page-two-heading",
        expectedPage: 2,
        text: "Schedule 1 – Computation of Tax",
        foundOnPages: [1]
      }
    ]);
  });

  it("keeps the reviewed manifest page-scoped and uniquely addressable", () => {
    expect(OFFICIAL_2551Q_STATIC_TEXT.length).toBeGreaterThan(100);
    expect(new Set(OFFICIAL_2551Q_STATIC_TEXT.map(({ id }) => id)).size)
      .toBe(OFFICIAL_2551Q_STATIC_TEXT.length);
    expect(OFFICIAL_2551Q_STATIC_TEXT.every(({ page }) => page === 1 || page === 2))
      .toBe(true);
  });

  it("uses selector-scoped evidence for otherwise ambiguous Schedule row numbers", () => {
    const rowNumberEntry: readonly OfficialStaticTextEntry[] = [{
      id: "schedule-row-one",
      page: 2,
      kind: "item",
      text: "1",
      selector: ".official-schedule-row[data-row-slot='1'] > .official-schedule-row-number"
    }];
    expect(verifyPageIndexedStaticText([
      { fullText: "1", selectorText: {} },
      { fullText: "1 elsewhere", selectorText: {} }
    ], rowNumberEntry)).toEqual([
      {
        id: "schedule-row-one",
        expectedPage: 2,
        text: "1",
        foundOnPages: []
      }
    ]);
    expect(verifyPageIndexedStaticText([
      { fullText: "1", selectorText: {} },
      {
        fullText: "1 elsewhere",
        selectorText: {
          ".official-schedule-row[data-row-slot='1'] > .official-schedule-row-number": "1"
        }
      }
    ], rowNumberEntry)).toEqual([]);
  });
});
