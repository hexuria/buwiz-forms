import { expect, test, type Locator, type Page } from "@playwright/test";
import { spawnSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { PNG } from "pngjs";
import { compareCompleteOfficialPage } from "./official-page-diff";
import { parsePromotionVisualThreshold } from "./release-visual-threshold";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(HERE, "../../..");
const MAX_CHANGED_PERCENT = parsePromotionVisualThreshold(
  process.env.FORM_VISUAL_MAX_CHANGED_PERCENT
);
const PIXELMATCH_THRESHOLD = 0.1;
const DEVICE_SCALE_FACTOR = 1.5;
const STRUCTURAL_INK_THRESHOLD = 100;
const STRUCTURAL_LINE_MIN_RUN = 20;
const STRUCTURAL_TOLERANCE_RADIUS = 4;
const VISUAL_EVIDENCE_PRODUCER = "playwright-form-parity-v1";
const VISUAL_EVIDENCE_PRODUCER_PATH =
  "packages/form-renderer/visual/form-parity.spec.ts";
const NON_PROMOTING_ALLOW_DIRTY_SOURCE =
  process.env.FORM_VISUAL_NON_PROMOTING_ALLOW_DIRTY_SOURCE === "1";
// Resolve and, for promotion-capable runs, validate the curated source before
// Playwright loads a page. An explicit dirty-source opt-out is diagnostic only
// and writes a report that the migration audit will refuse as evidence.
const VISUAL_SOURCE_REVISION = curatedSourceRevision();

interface ParityCase {
  code: string;
  name: string;
  revision: string;
  fixture: string;
  references: string[];
  expectedRealRowKeys: number;
}

interface VisualPageMetric {
  form_code: string;
  form_revision: string;
  fixture: string;
  fixture_sha256: string;
  reference: string;
  reference_sha256: string;
  actual: string;
  actual_sha256: string;
  diff: string | null;
  diff_sha256: string | null;
  page: number;
  expected_width: number;
  expected_height: number;
  actual_width: number;
  actual_height: number;
  changed_pixels: number | null;
  changed_percent: number | null;
  max_changed_percent: number;
  pixelmatch_threshold: number;
  comparison: "official-complete-page-v1";
  expected_ink_missing_percent: number | null;
  unexpected_actual_ink_percent: number | null;
  structural_changed_pixels: number | null;
  structural_changed_percent: number | null;
  structural_diff: string | null;
  structural_diff_sha256: string | null;
  structural_ink_threshold: number;
  structural_line_min_run: number;
  structural_tolerance_radius: number;
  passed: boolean;
}

const visualPageMetrics: VisualPageMetric[] = [];

const cases: ParityCase[] = [
  {
    code: "2551Q",
    name: "2551Q 2018",
    revision: "2018",
    fixture: "packages/form-contracts/fixtures/2551q-6-rows.json",
    references: [
      "packages/form-renderer/references/2551q-2018-page-1.png",
      "packages/form-renderer/references/2551q-2018-page-2.png"
    ],
    expectedRealRowKeys: 6
  }
];

for (const parityCase of cases) {
  test(`${parityCase.name} matches the official first two pages`, async ({
    page
  }, testInfo) => {
    visualPageMetrics.length = 0;
    const fixtureBuffer = fs.readFileSync(
      path.join(REPO_ROOT, parityCase.fixture)
    );
    const fixtureSha256 = sha256(fixtureBuffer);
    const envelope = JSON.parse(fixtureBuffer.toString("utf8")) as unknown;

    await renderEnvelope(page, envelope);

    const pages = page.locator(".form-page");
    await expect(pages).toHaveCount(parityCase.references.length);

    const plainName = pages.nth(1).locator('[data-overflow-mode="plain"]');
    await expect(plainName).toHaveCount(1);
    await expect(plainName).toHaveAttribute(
      "aria-label",
      "RENDERER FIXTURE CORPORATION"
    );
    const plainGeometry = await plainName.evaluate((element) => ({
      clientWidth: element.clientWidth,
      clientHeight: element.clientHeight,
      scrollWidth: element.scrollWidth,
      scrollHeight: element.scrollHeight
    }));
    expect(plainGeometry.scrollWidth).toBeLessThanOrEqual(plainGeometry.clientWidth + 1);
    expect(plainGeometry.scrollHeight).toBeLessThanOrEqual(plainGeometry.clientHeight + 1);

    const realRowKeys = await page
      .locator("[data-row-key]")
      .evaluateAll((rows) =>
        rows
          .map((row) => row.getAttribute("data-row-key"))
          .filter(
            (key): key is string =>
              key !== null &&
              !key.includes("-empty-") &&
              !key.startsWith("payment-empty-")
          )
      );
    expect(realRowKeys).toHaveLength(parityCase.expectedRealRowKeys);
    expect(new Set(realRowKeys).size).toBe(realRowKeys.length);

    const officialSchedulePage = pages.nth(1);
    await expect(officialSchedulePage.locator(".official-schedule-row")).toHaveCount(6);
    await expect(officialSchedulePage.locator(".official-atc-table .atc-entry")).toHaveCount(22);
    await expect(officialSchedulePage.locator(".official-atc-table .atc-category")).toHaveCount(3);
    await expect(officialSchedulePage.locator(".official-atc-table")).toContainText(
      "Tax on Banks and Non-Bank Financial Intermediaries Performing Quasi-Banking Functions"
    );
    await expect(officialSchedulePage.locator(".official-atc-table")).toContainText(
      "Agents of Foreign Insurance Companies"
    );
    expect(await pageHasNoOverflow(officialSchedulePage)).toBe(true);

    await expectCriticalRegionGeometry(pages.nth(0), [
      { name: "Items 1-5", selector: ".official-header-options", x: 45, y: 223, width: 1137, height: 67 },
      { name: "Items 1-2 filing basis", selector: ".filing-basis", x: 45, y: 223, width: 398, height: 67 },
      { name: "Item 1 label", selector: ".filing-basis > .option-label:first-child", x: 45, y: 223, width: 114, height: 31 },
      { name: "Item 1 choices", selector: ".filing-basis > .option-choices", x: 159, y: 223, width: 284, height: 31 },
      { name: "Item 2 label", selector: ".filing-basis > .year-label", x: 45, y: 253, width: 226, height: 37 },
      { name: "Item 2 value", selector: ".filing-basis > .comb-value", x: 271, y: 253, width: 172, height: 37 },
      { name: "Item 3 quarter", selector: ".quarter-options", x: 443, y: 223, width: 369, height: 67 },
      { name: "Item 3 label", selector: ".quarter-options > .option-label", x: 443, y: 223, width: 369, height: 30 },
      { name: "Item 3 choices", selector: ".quarter-options > .option-choices", x: 443, y: 253, width: 369, height: 37 },
      { name: "Item 4 amended", selector: ".amended-options", x: 812, y: 223, width: 199, height: 67 },
      { name: "Item 4 label", selector: ".amended-options > .option-label", x: 812, y: 223, width: 199, height: 30 },
      { name: "Item 4 choices", selector: ".amended-options > .option-choices", x: 812, y: 253, width: 199, height: 37 },
      { name: "Item 5 sheets", selector: ".sheets-options", x: 1011, y: 223, width: 168, height: 67 },
      { name: "Item 5 label", selector: ".sheets-options > .option-label", x: 1011, y: 223, width: 168, height: 30 },
      { name: "Item 5 value", selector: ".sheets-options > .sheets-value", x: 1011, y: 253, width: 168, height: 37 },
      { name: "Calendar checkbox", selector: ".filing-basis .check-choice:nth-child(1) .check-box", x: 169, y: 225, width: 28, height: 26 },
      { name: "Fiscal checkbox", selector: ".filing-basis .check-choice:nth-child(2) .check-box", x: 314, y: 226, width: 28, height: 26 },
      { name: "First-quarter checkbox", selector: ".quarter-options .check-choice:nth-child(1) .check-box", x: 488, y: 252, width: 28, height: 26 },
      { name: "Second-quarter checkbox", selector: ".quarter-options .check-choice:nth-child(2) .check-box", x: 573, y: 252, width: 28, height: 26 },
      { name: "Third-quarter checkbox", selector: ".quarter-options .check-choice:nth-child(3) .check-box", x: 658, y: 252, width: 28, height: 26 },
      { name: "Fourth-quarter checkbox", selector: ".quarter-options .check-choice:nth-child(4) .check-box", x: 742, y: 252, width: 28, height: 26 },
      { name: "Amended yes checkbox", selector: ".amended-options .check-choice:nth-child(1) .check-box", x: 851, y: 254, width: 28, height: 26 },
      { name: "Amended no checkbox", selector: ".amended-options .check-choice:nth-child(2) .check-box", x: 934, y: 255, width: 28, height: 26 },
      { name: "Items 6-13", selector: ".background-information", x: 45, y: 295, width: 1137, height: 399 },
      { name: "Items 6-7", selector: ".tin-rdo-row", x: 45, y: 320, width: 1137, height: 36 },
      { name: "Item 6 label", selector: ".tin-rdo-row > .field-label", x: 47, y: 320, width: 394, height: 35 },
      { name: "TIN first group", selector: ".tin-rdo-row > .comb-value:nth-child(2)", x: 441, y: 320, width: 85, height: 35 },
      { name: "TIN first separator", selector: ".tin-rdo-row > .tin-separator:nth-child(3)", x: 526, y: 320, width: 29, height: 35 },
      { name: "TIN second group", selector: ".tin-rdo-row > .comb-value:nth-child(4)", x: 555, y: 320, width: 85, height: 35 },
      { name: "TIN second separator", selector: ".tin-rdo-row > .tin-separator:nth-child(5)", x: 640, y: 320, width: 28, height: 35 },
      { name: "TIN third group", selector: ".tin-rdo-row > .comb-value:nth-child(6)", x: 668, y: 320, width: 86, height: 35 },
      { name: "TIN third separator", selector: ".tin-rdo-row > .tin-separator:nth-child(7)", x: 754, y: 320, width: 28, height: 35 },
      { name: "TIN branch group", selector: ".tin-rdo-row > .comb-value:nth-child(8)", x: 782, y: 320, width: 142, height: 35 },
      { name: "Item 7 label", selector: ".tin-rdo-row > .rdo-label", x: 924, y: 320, width: 170, height: 35 },
      { name: "Item 7 value", selector: ".tin-rdo-row > .comb-value:last-child", x: 1094, y: 320, width: 86, height: 35 },
      { name: "Item 8", selector: ".name-field", x: 45, y: 356, width: 1137, height: 58 },
      { name: "Item 8 label", selector: ".name-field > .field-label", x: 47, y: 357, width: 1133, height: 23 },
      { name: "Item 8 value", selector: ".name-field > .comb-value", x: 47, y: 380, width: 1133, height: 35 },
      { name: "Items 9-9A", selector: ".address-field", x: 45, y: 414, width: 1137, height: 96 },
      { name: "Item 9 label", selector: ".address-field > .field-label", x: 47, y: 415, width: 1133, height: 23 },
      { name: "Item 9 first value row", selector: ".address-field > .comb-value", x: 47, y: 438, width: 1133, height: 35 },
      { name: "Item 9 continuation", selector: ".address-continuation > .comb-value:first-child", x: 47, y: 473, width: 881, height: 38 },
      { name: "Item 9A label", selector: ".address-continuation > .zip-label", x: 928, y: 473, width: 141, height: 38 },
      { name: "Item 9A value", selector: ".address-continuation > .comb-value:last-child", x: 1069, y: 473, width: 111, height: 38 },
      { name: "Items 10-11", selector: ".contact-email-field", x: 45, y: 510, width: 1137, height: 59 },
      { name: "Item 10 label", selector: ".contact-email-field > .field-label:first-child", x: 47, y: 510, width: 337, height: 23 },
      { name: "Item 11 label", selector: ".contact-email-field > .field-label:nth-child(2)", x: 383, y: 510, width: 796, height: 23 },
      { name: "Item 10 value", selector: ".contact-email-field > .comb-value:nth-child(3)", x: 47, y: 533, width: 337, height: 35 },
      { name: "Item 11 value", selector: ".contact-email-field > .comb-value:nth-child(4)", x: 383, y: 533, width: 796, height: 35 },
      { name: "Items 12-12A", selector: ".tax-relief-field", x: 45, y: 569, width: 1137, height: 40 },
      { name: "Item 12 label", selector: ".tax-relief-field > .field-label", x: 47, y: 568, width: 337, height: 38 },
      { name: "Item 12 choices", selector: ".tax-relief-field > .relief-choices", x: 383, y: 568, width: 171, height: 38 },
      { name: "Item 12 Yes box", selector: ".relief-choices .check-choice:first-child .check-box", x: 383, y: 574, width: 28, height: 28 },
      { name: "Item 12 No box", selector: ".relief-choices .check-choice:last-child .check-box", x: 469, y: 574, width: 28, height: 28 },
      { name: "Item 12A label", selector: ".tax-relief-field > .tax-relief-spec", x: 554, y: 568, width: 170, height: 38 },
      { name: "Item 12A value", selector: ".tax-relief-field > .comb-value", x: 724, y: 568, width: 455, height: 38 },
      { name: "Item 13", selector: ".income-rate-field", x: 45, y: 607, width: 1137, height: 87 },
      { name: "Items 14-24 totals", selector: ".tax-payable", x: 45, y: 696, width: 1137, height: 506 },
      { name: "Item 14 total", selector: ".official-tax-line[data-item='14']", x: 45, y: 723, width: 1137, height: 36 },
      { name: "Item 17 specification", selector: ".tax-credit-description", x: 448, y: 862, width: 303, height: 23 },
      { name: "signatures", selector: ".official-declaration", x: 45, y: 1206, width: 1137, height: 232 },
      { name: "declaration copy", selector: ".official-declaration > p", x: 47, y: 1208, width: 1133, height: 56 },
      { name: "signature boxes", selector: ".official-signature-grid", x: 45, y: 1263, width: 1133, height: 134 },
      { name: "individual signature caption", selector: ".official-signature-grid > div:first-child .signature-caption", x: 47, y: 1345, width: 567, height: 53 },
      { name: "non-individual signature caption", selector: ".official-signature-grid > div:last-child .signature-caption", x: 615, y: 1345, width: 565, height: 53 },
      { name: "tax-agent strip", selector: ".tax-agent-strip", x: 47, y: 1398, width: 1133, height: 38 },
      { name: "Part III item 25 decimal cell", selector: ".payment-row-25 .decimal-separator", x: 1097, y: 1502, width: 30, height: 35 },
      { name: "Part III item 25 cents cells", selector: ".payment-row-25 .comb-value:last-child", x: 1127, y: 1502, width: 53, height: 35 },
      { name: "Part III item 26 decimal cell", selector: ".payment-row-26 .decimal-separator", x: 1097, y: 1538, width: 30, height: 35 },
      { name: "Part III item 26 cents cells", selector: ".payment-row-26 .comb-value:last-child", x: 1127, y: 1538, width: 53, height: 35 },
      { name: "Part III item 27 decimal cell", selector: ".payment-row-27 .decimal-separator", x: 1097, y: 1575, width: 30, height: 35 },
      { name: "Part III item 27 cents cells", selector: ".payment-row-27 .comb-value:last-child", x: 1127, y: 1575, width: 53, height: 35 },
      { name: "Part III item 28 continuation decimal cell", selector: ".payment-other-row .decimal-separator", x: 1097, y: 1636, width: 30, height: 35 },
      { name: "Part III item 28 continuation cents cells", selector: ".payment-other-row .comb-value:last-child", x: 1127, y: 1636, width: 53, height: 35 }
    ]);
    await expectHeaderOptionsTopAlignment(pages.nth(0));
    await expectBackgroundInformationParity(pages.nth(0));
    await expectCriticalRegionGeometry(pages.nth(1), [
      { name: "Schedule 1 masthead", selector: ".page-two-masthead", x: 45, y: 78, width: 1137, height: 117 },
      { name: "Schedule 1 identity", selector: ".page-two-identity", x: 45, y: 193, width: 1137, height: 60 },
      { name: "Schedule 1", selector: ".official-schedule", x: 45, y: 256, width: 1137, height: 327 },
      { name: "Schedule 1 ATC table", selector: ".official-atc-table", x: 45, y: 587, width: 1137, height: 677 }
    ]);
    await expectCriticalRegionContent(pages.nth(0), pages.nth(1));

    // The pinned official PDF is an unfilled form while our authoritative
    // fixture exercises all six Schedule 1 rows.  Compare the owned document
    // geometry with only fixture-provided glyphs suppressed: borders, comb
    // cells, check boxes, labels, artwork, fills, and pagination remain in the
    // captured image and therefore remain inside the strict pixel gate.
    await prepareOfficialBlankComparison(page);

    for (const [pageIndex, referencePath] of parityCase.references.entries()) {
      const renderedPage = pages.nth(pageIndex);
      const expectedBuffer = fs.readFileSync(
        path.join(REPO_ROOT, referencePath)
      );
      const expectedImage = PNG.sync.read(expectedBuffer);
      const box = await renderedPage.boundingBox();
      expect(box).not.toBeNull();
      expect(box?.width).toBeCloseTo(
        expectedImage.width / DEVICE_SCALE_FACTOR,
        0
      );
      expect(box?.height).toBeCloseTo(
        expectedImage.height / DEVICE_SCALE_FACTOR,
        0
      );

      const actualBuffer = await renderedPage.screenshot({
        animations: "disabled",
        caret: "hide"
      });
      assertVisualParity({
        parityCase,
        pageNumber: pageIndex + 1,
        fixtureSha256,
        referencePath,
        actualBuffer,
        expectedBuffer,
        artifactStem: `${slug(parityCase.name)}-page-${pageIndex + 1}`,
        outputDir: testInfo.outputDir
      });
    }

    writeVisualEvidence();
    const failedPages = visualPageMetrics
      .filter((metric) => !metric.passed)
      .map((metric) => ({
        page: metric.page,
        changed_percent: metric.changed_percent,
        expected_width: metric.expected_width,
        expected_height: metric.expected_height,
        actual_width: metric.actual_width,
        actual_height: metric.actual_height
      }));
    expect(
      failedPages,
      `Visual parity must be at or below ${MAX_CHANGED_PERCENT}% changed pixels per page`
    ).toEqual([]);
  });
}

test("2551Q PDF417 artwork keeps the reviewed source geometry", async ({ page }) => {
  await renderEnvelope(
    page,
    readFixture("packages/form-contracts/fixtures/2551q-6-rows.json")
  );
  const pages = page.locator(".form-page");
  await expect(pages).toHaveCount(2);

  await expectCriticalRegionGeometry(pages.nth(0), [
    { name: "Page 1 official seal", selector: ".government-seal", x: 490, y: 44, width: 58, height: 50 },
    { name: "Page 1 masthead", selector: ".official-masthead", x: 45, y: 100, width: 1136, height: 124 },
    { name: "Page 1 PDF417 symbol", selector: ".official-barcode > .official-pdf417-symbol", x: 852, y: 119, width: 323, height: 74 },
    { name: "Page 1 PDF417 caption", selector: ".official-barcode > small", x: 1009, y: 196, width: 161, height: 16 }
  ]);
  await expectCriticalRegionGeometry(pages.nth(1), [
    { name: "Page 2 masthead", selector: ".page-two-masthead", x: 45, y: 78, width: 1136, height: 117 },
    { name: "Page 2 PDF417 symbol", selector: ".page-two-barcode > .official-pdf417-symbol", x: 849, y: 97, width: 326, height: 75 },
    { name: "Page 2 PDF417 caption", selector: ".page-two-barcode > small", x: 1009, y: 177, width: 161, height: 16 }
  ]);

  expect(
    await page.locator(".official-pdf417-symbol").evaluateAll((symbols) =>
      symbols.slice(0, 2).map((symbol) => ({
        preserveAspectRatio: symbol.getAttribute("preserveAspectRatio"),
        shapeRendering: getComputedStyle(symbol).shapeRendering,
        viewBox: symbol.getAttribute("viewBox")
      }))
    )
  ).toEqual([
    { preserveAspectRatio: "none", shapeRendering: "crispedges", viewBox: "0 0 120 7" },
    { preserveAspectRatio: "none", shapeRendering: "crispedges", viewBox: "0 0 120 7" }
  ]);

  const captionStyles = await page
    .locator(".official-barcode > small, .page-two-barcode > small")
    .evaluateAll((captions) => captions.slice(0, 2).map((caption) => {
      const style = getComputedStyle(caption);
      return {
        fontFamily: style.fontFamily,
        fontSize: style.fontSize,
        lineHeight: style.lineHeight,
        textAlign: style.textAlign,
        whiteSpace: style.whiteSpace
      };
    }));
  expect(captionStyles).toEqual(Array.from({ length: 2 }, () => ({
    fontFamily: '"eBIRForms Arimo", sans-serif',
    fontSize: "10.6667px",
    lineHeight: "10.6667px",
    textAlign: "right",
    whiteSpace: "nowrap"
  })));

  const gaps = await Promise.all([
    measuredVerticalGap(pages.nth(0), ".official-barcode"),
    measuredVerticalGap(pages.nth(1), ".page-two-barcode")
  ]);
  expect(gaps[0]).toBeCloseTo(3.04, 1);
  expect(gaps[1]).toBeCloseTo(5.02, 1);

  expect(await page.locator(".government-seal").evaluate((image) => ({
    naturalHeight: (image as HTMLImageElement).naturalHeight,
    naturalWidth: (image as HTMLImageElement).naturalWidth
  }))).toEqual({ naturalHeight: 83, naturalWidth: 95 });
});

test("2551Q page-one typography keeps the reviewed bundled-font calibration", async ({
  page
}) => {
  await renderEnvelope(
    page,
    readFixture("packages/form-contracts/fixtures/2551q-6-rows.json")
  );

  const pageOne = page.locator(".form-2551q-page-one");
  const typography = await pageOne.evaluate((formPage) => {
    const read = (selector: string) => {
      const element = formPage.querySelector(selector);
      if (!element) throw new Error(`Missing typography target: ${selector}`);
      const style = getComputedStyle(element);
      return {
        fontFamily: style.fontFamily,
        fontSize: style.fontSize,
        fontWeight: style.fontWeight,
        transform: style.transform
      };
    };
    return {
      formNumber: read(".official-form-number > strong"),
      formRevision: read(".official-form-number > small"),
      formTitle: read(".official-form-title > strong"),
      instructions: read(".official-form-title > em"),
      optionLabel: read(".filing-basis > .option-label:first-child"),
      taxLine: read(".official-tax-line[data-item='14'] .tax-line-label"),
      taxLineNumber: read(".official-tax-line[data-item='14'] .tax-line-label b"),
      taxLineNote: read(".official-tax-line[data-item='14'] .tax-line-label em"),
      paymentLabel: read(".payment-row-25 > span:first-child")
    };
  });

  const bundledFont = '"eBIRForms Arimo", sans-serif';
  expect(typography).toEqual({
    formNumber: {
      fontFamily: bundledFont,
      fontSize: "32px",
      fontWeight: "500",
      transform: "matrix(1, 0, 0, 1, 0, -1.33333)"
    },
    formRevision: {
      fontFamily: bundledFont,
      fontSize: "10.6667px",
      fontWeight: "400",
      transform: "none"
    },
    formTitle: {
      fontFamily: bundledFont,
      fontSize: "24px",
      fontWeight: "500",
      transform: "matrix(1, 0, 0, 1, 0, -2.66667)"
    },
    instructions: {
      fontFamily: bundledFont,
      fontSize: "10.3333px",
      fontWeight: "400",
      transform: "matrix(1, 0, 0, 1, 0, -6.66667)"
    },
    optionLabel: {
      fontFamily: bundledFont,
      fontSize: "12.4444px",
      fontWeight: "400",
      transform: "none"
    },
    taxLine: {
      fontFamily: bundledFont,
      fontSize: "12px",
      fontWeight: "400",
      transform: "none"
    },
    taxLineNumber: {
      fontFamily: bundledFont,
      fontSize: "12px",
      fontWeight: "600",
      transform: "none"
    },
    taxLineNote: {
      fontFamily: bundledFont,
      fontSize: "9.66667px",
      fontWeight: "400",
      transform: "none"
    },
    paymentLabel: {
      fontFamily: bundledFont,
      fontSize: "12.3333px",
      fontWeight: "400",
      transform: "none"
    }
  });

  expect(
    await page.locator(".page-two-form-title > strong").evaluate((element) => {
      const style = getComputedStyle(element);
      return { fontSize: style.fontSize, fontWeight: style.fontWeight };
    })
  ).toEqual({ fontSize: "24px", fontWeight: "400" });
});

test("2551Q Schedule 1 keeps black rules and bottom-anchored comb guides", async ({
  page
}) => {
  await renderEnvelope(
    page,
    readFixture("packages/form-contracts/fixtures/2551q-6-rows.json")
  );
  const pageTwo = page.locator(".form-2551q-page-two");
  await expect(pageTwo).toHaveCount(1);

  const styles = await pageTwo.evaluate((element) => {
    const normalCell = element.querySelector(
      ".page-two-identity > .comb-value > span"
    );
    const majorCell = element.querySelector(
      ".visual-integer-comb-leading-1 > .comb-value > span:nth-child(2)"
    );
    const masthead = element.querySelector(".page-two-masthead");
    const atcCell = element.querySelector(".official-atc-table td");
    if (!normalCell || !majorCell || !masthead || !atcCell) {
      throw new Error("2551Q Schedule 1 calibration targets are missing");
    }
    const normalStyle = getComputedStyle(normalCell);
    const normalGuide = getComputedStyle(normalCell, "::after");
    const majorGuide = getComputedStyle(majorCell, "::after");
    return {
      atcBorder: getComputedStyle(atcCell).borderTopColor,
      majorGuideColor: majorGuide.borderRightColor,
      majorGuideHeight: Number.parseFloat(majorGuide.height),
      mastheadBorder: getComputedStyle(masthead).borderTopColor,
      normalCellBorderWidth: normalStyle.borderRightWidth,
      normalGuideColor: normalGuide.borderRightColor,
      normalGuideHeight: Number.parseFloat(normalGuide.height),
      normalGuidePosition: normalGuide.position
    };
  });

  expect(styles.mastheadBorder).toBe("rgb(0, 0, 0)");
  expect(styles.atcBorder).toBe("rgb(0, 0, 0)");
  expect(styles.normalCellBorderWidth).toBe("0px");
  expect(styles.normalGuidePosition).toBe("absolute");
  expect(styles.normalGuideColor).toBe("rgb(0, 0, 0)");
  expect(styles.normalGuideHeight).toBeCloseTo(9.33, 1);
  expect(styles.majorGuideColor).toBe("rgb(0, 0, 0)");
  expect(styles.majorGuideHeight).toBeGreaterThan(styles.normalGuideHeight);
  expect(await pageHasNoOverflow(pageTwo)).toBe(true);
});

test("2551Q Part III amount cells preserve the official partition", async ({ page }) => {
  await renderEnvelope(
    page,
    readFixture("packages/form-contracts/fixtures/2551q-6-rows.json")
  );
  const pageOne = page.locator(".form-page").first();
  await expectCriticalRegionGeometry(pageOne, [
    { name: "Item 25 Amount", selector: ".payment-row-25 .blank-money-value", x: 757, y: 1502, width: 422, height: 35 },
    { name: "Item 25 decimal cell", selector: ".payment-row-25 .decimal-separator", x: 1097, y: 1502, width: 30, height: 35 },
    { name: "Item 25 cents cells", selector: ".payment-row-25 .comb-value:last-child", x: 1127, y: 1502, width: 53, height: 35 },
    { name: "Item 28 Amount", selector: ".payment-other-row .blank-money-value", x: 757, y: 1636, width: 422, height: 35 },
    { name: "Item 28 decimal cell", selector: ".payment-other-row .decimal-separator", x: 1097, y: 1636, width: 30, height: 35 },
    { name: "Item 28 cents cells", selector: ".payment-other-row .comb-value:last-child", x: 1127, y: 1636, width: 53, height: 35 }
  ]);

  const amountRows = pageOne.locator(
    ".payment-row-25 .blank-money-value, .payment-row-26 .blank-money-value, .payment-row-27 .blank-money-value, .payment-other-row .blank-money-value"
  );
  await expect(amountRows).toHaveCount(4);
  for (let index = 0; index < 4; index += 1) {
    const amount = amountRows.nth(index);
    const integerCells = amount.locator(":scope > .comb-value:first-child > span");
    await expect(integerCells).toHaveCount(12);
    await expect(amount.locator(":scope > .comb-value:last-child > span")).toHaveCount(2);

    const widths = await amount.evaluate((element) => {
      const integer = [...element.querySelectorAll(":scope > .comb-value:first-child > span")]
        .map((cell) => cell.getBoundingClientRect().width);
      const decimal = element.querySelector(":scope > .decimal-separator")?.getBoundingClientRect().width ?? 0;
      const cents = [...element.querySelectorAll(":scope > .comb-value:last-child > span")]
        .map((cell) => cell.getBoundingClientRect().width);
      return { integer, decimal, cents };
    });
    const integerAverage = widths.integer.reduce((sum, width) => sum + width, 0) /
      widths.integer.length;
    expect(Math.max(...widths.integer) - Math.min(...widths.integer)).toBeLessThan(.05);
    expect(widths.decimal / integerAverage).toBeGreaterThan(.95);
    expect(widths.decimal / integerAverage).toBeLessThan(1.1);
    expect(widths.cents.every((width) => width / integerAverage > .9)).toBe(true);
  }
});

test("2551Q Part I labels and writable cells preserve the official rendering", async ({
  page
}) => {
  await renderEnvelope(
    page,
    readFixture("packages/form-contracts/fixtures/2551q-6-rows.json")
  );
  const pageOne = page.locator(".form-page").first();
  await expectCriticalRegionGeometry(pageOne, [
    { name: "Item 6 label", selector: ".tin-rdo-row > .field-label", x: 47, y: 320, width: 394, height: 35 },
    { name: "TIN first group", selector: ".tin-rdo-row > .comb-value:nth-child(2)", x: 441, y: 320, width: 85, height: 35 },
    { name: "TIN first separator", selector: ".tin-rdo-row > .tin-separator:nth-child(3)", x: 526, y: 320, width: 29, height: 35 },
    { name: "TIN branch group", selector: ".tin-rdo-row > .comb-value:nth-child(8)", x: 782, y: 320, width: 142, height: 35 },
    { name: "Item 7 label", selector: ".tin-rdo-row > .rdo-label", x: 924, y: 320, width: 170, height: 35 },
    { name: "Item 8 label", selector: ".name-field > .field-label", x: 47, y: 357, width: 1133, height: 23 },
    { name: "Item 8 value", selector: ".name-field > .comb-value", x: 47, y: 380, width: 1133, height: 35 },
    { name: "Item 9 label", selector: ".address-field > .field-label", x: 47, y: 415, width: 1133, height: 23 },
    { name: "Item 9 continuation", selector: ".address-continuation > .comb-value:first-child", x: 47, y: 473, width: 881, height: 38 },
    { name: "Item 9A label", selector: ".address-continuation > .zip-label", x: 928, y: 473, width: 141, height: 38 },
    { name: "Item 10 label", selector: ".contact-email-field > .field-label:first-child", x: 47, y: 510, width: 337, height: 23 },
    { name: "Item 11 value", selector: ".contact-email-field > .comb-value:nth-child(4)", x: 383, y: 533, width: 796, height: 35 },
    { name: "Item 12 Yes box", selector: ".relief-choices .check-choice:first-child .check-box", x: 383, y: 574, width: 28, height: 28 },
    { name: "Item 12 No box", selector: ".relief-choices .check-choice:last-child .check-box", x: 469, y: 574, width: 28, height: 28 },
    { name: "Item 12A label", selector: ".tax-relief-field > .tax-relief-spec", x: 554, y: 568, width: 170, height: 38 },
    { name: "Item 12A value", selector: ".tax-relief-field > .comb-value", x: 724, y: 568, width: 455, height: 38 }
  ]);
  await expectBackgroundInformationParity(pageOne);
  expect(await pageHasNoOverflow(pageOne)).toBe(true);
});

test("development preview fallback renders without a contract error", async ({ page }) => {
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));

  await page.goto("/");
  await page.locator(".form-document").waitFor();
  await expect(page.locator(".form-page")).toHaveCount(3);
  for (const renderedPage of await page.locator(".form-page").all()) {
    expect(await pageHasNoOverflow(renderedPage)).toBe(true);
  }
  expect(pageErrors).toEqual([]);
});

test("every canonical fixture produces stable, unclipped pages", async ({ page }) => {
  const fixtures = [
    "2551q-10-rows.json",
    "2551q-6-rows.json",
    "2551q-fiscal-period.json",
    "2551q-item13-eight-percent.json",
    "2551q-long-values.json",
    "2551q-minimum.json",
    "2551q-overpayment-refund.json",
    "2551q-overpayment-tcc.json",
    "2551q-tax-relief.json",
    "2551q-validation-edge.json"
  ];

  for (const fixture of fixtures) {
    const envelope = readFixture(`packages/form-contracts/fixtures/${fixture}`) as {
      fields: Record<string, { type: string; value: unknown }>;
      schedules: Array<{ rows: unknown[] }>;
    };
    await renderEnvelope(page, envelope);
    const pages = page.locator(".form-page");
    const expectedPages = envelope.schedules[0].rows.length > 6 ? 3 : 2;
    await expect(pages, fixture).toHaveCount(expectedPages);
    if (fixture === "2551q-long-values.json") {
      const description = envelope.fields.other_tax_credit_description?.value;
      if (typeof description !== "string") {
        throw new Error("2551Q long-value fixture Item 17 description must be text");
      }
      const renderedDescription = page.locator(".tax-credit-description");
      await expect(renderedDescription).toHaveText(description);
      await expect(renderedDescription).toHaveAttribute(
        "aria-label",
        `Item 17 specification: ${description}`
      );
      await expect(renderedDescription).toHaveAttribute("data-overflow-mode", "plain");
      const descriptionGeometry = await renderedDescription.evaluate((element) => ({
        clientHeight: element.clientHeight,
        clientWidth: element.clientWidth,
        scrollHeight: element.scrollHeight,
        scrollWidth: element.scrollWidth
      }));
      expect(descriptionGeometry.scrollWidth).toBeLessThanOrEqual(descriptionGeometry.clientWidth + 1);
      expect(descriptionGeometry.scrollHeight).toBeLessThanOrEqual(descriptionGeometry.clientHeight + 1);
    }
    if (fixture === "2551q-validation-edge.json") {
      await expect(page.locator(".tax-credit-description")).toHaveText("");
      await expect(page.locator(".tax-credit-description")).toHaveAttribute(
        "aria-label",
        "Item 17 specification, blank"
      );
    }
    for (let index = 0; index < expectedPages; index += 1) {
      expect(await pageHasNoOverflow(pages.nth(index)), `${fixture} page ${index + 1}`).toBe(true);
    }
  }

  const capacityEnvelope = readFixture(
    "packages/form-contracts/fixtures/2551q-tax-relief.json"
  ) as {
    fields: Record<string, { type: string; value: unknown }>;
    taxpayer: Record<string, string>;
  };
  capacityEnvelope.taxpayer.name = "N".repeat(40);
  capacityEnvelope.taxpayer.registered_address = "A".repeat(71);
  capacityEnvelope.taxpayer.contact_number = "+63 912 345 6789";
  capacityEnvelope.taxpayer.email = "renderer.2551q@example.com";
  capacityEnvelope.fields.tax_relief_specification.value =
    "Special law reference 1234";
  await renderEnvelope(page, capacityEnvelope);
  await expect(page.locator(".name-field .comb-value")).toContainText("N".repeat(40));
  const contactAndEmail = page.locator(".contact-email-field > .comb-value");
  expect((await contactAndEmail.nth(0).locator("> span").allTextContents()).join("").trimEnd()).toBe("639123456789");
  expect((await contactAndEmail.nth(1).locator("> span").allTextContents()).join("").trimEnd()).toBe("RENDERER.2551Q@EXAMPLE.COM");
  expect((await page.locator(".tax-relief-field > .comb-value > span").allTextContents()).join("").trimEnd()).toBe("SPECIAL LAW REFERENCE 1234");
  const addressCharacters = await page
    .locator(".address-field > .comb-value > span, .address-continuation > .comb-value:first-child > span")
    .allTextContents();
  expect(addressCharacters.join("").trimEnd()).toBe("A".repeat(71));
  for (const renderedPage of await page.locator(".form-page").all()) {
    expect(await pageHasNoOverflow(renderedPage)).toBe(true);
  }
});

test("geometry readiness sees overflow hidden descendants and clipping", async ({ page }) => {
  await renderEnvelope(
    page,
    readFixture("packages/form-contracts/fixtures/2551q-6-rows.json")
  );
  const firstPage = page.locator(".form-page").first();
  expect(await pageHasNoOverflow(firstPage)).toBe(true);

  await firstPage.evaluate((element) => {
    const clipped = document.createElement("div");
    clipped.dataset.geometryProbe = "clipped";
    Object.assign(clipped.style, {
      height: "10px",
      left: "30px",
      overflow: "hidden",
      position: "absolute",
      top: "30px",
      width: "20px"
    });
    const oversized = document.createElement("div");
    Object.assign(oversized.style, { height: "40px", width: "80px" });
    clipped.append(oversized);
    element.append(clipped);
  });

  const geometry = await measuredPage(firstPage);
  expect(geometry.descendant_overflow_x).toBeGreaterThan(0);
  expect(geometry.descendant_overflow_y).toBeGreaterThan(0);
  expect(geometry.descendant_clipped_x).toBeGreaterThan(0);
  expect(geometry.descendant_clipped_y).toBeGreaterThan(0);
});

function writeVisualEvidence() {
  const expectedPageCount = cases.reduce(
    (count, parityCase) => count + parityCase.references.length,
    0
  );
  const evidencePath = process.env.FORM_VISUAL_EVIDENCE_PATH
    ? path.resolve(process.env.FORM_VISUAL_EVIDENCE_PATH)
    : path.join(
        REPO_ROOT,
        "test-results/form-renderer/visual-evidence.json"
      );
  const referenceManifest = path.join(
    REPO_ROOT,
    "packages/form-renderer/references/manifest.json"
  );
  const report = {
    schema_version: 1,
    gate: "visual_parity",
    producer: VISUAL_EVIDENCE_PRODUCER,
    producer_path: VISUAL_EVIDENCE_PRODUCER_PATH,
    producer_sha256: sha256(
      fs.readFileSync(path.join(REPO_ROOT, VISUAL_EVIDENCE_PRODUCER_PATH))
    ),
    promotion_eligible: !NON_PROMOTING_ALLOW_DIRTY_SOURCE,
    source_worktree_clean: !NON_PROMOTING_ALLOW_DIRTY_SOURCE,
    generated_at: new Date().toISOString(),
    source_revision: VISUAL_SOURCE_REVISION,
    ci_run_id: process.env.GITHUB_RUN_ID ?? null,
    platform: process.platform,
    architecture: process.arch,
    browser: "chromium",
    device_scale_factor: DEVICE_SCALE_FACTOR,
    expected_page_count: expectedPageCount,
    measured_page_count: visualPageMetrics.length,
    references_manifest:
      "packages/form-renderer/references/manifest.json",
    references_manifest_sha256: sha256(
      fs.readFileSync(referenceManifest)
    ),
    passed:
      !NON_PROMOTING_ALLOW_DIRTY_SOURCE &&
      visualPageMetrics.length === expectedPageCount &&
      visualPageMetrics.every((metric) => metric.passed),
    pages: visualPageMetrics
  };

  fs.mkdirSync(path.dirname(evidencePath), { recursive: true });
  fs.writeFileSync(evidencePath, `${JSON.stringify(report, null, 2)}\n`);
}

function curatedSourceRevision(): string {
  const arguments_ = [
    path.join(REPO_ROOT, "scripts/run_python.mjs"),
    path.join(REPO_ROOT, "scripts/audit_html_form_migration.py"),
    "--root",
    REPO_ROOT,
    "--print-source-revision"
  ];
  if (!NON_PROMOTING_ALLOW_DIRTY_SOURCE) {
    arguments_.push("--require-clean-source");
  }
  const result = spawnSync(
    process.execPath,
    arguments_,
    { encoding: "utf8" }
  );
  if (result.status !== 0) {
    throw new Error(
      `Unable to resolve curated renderer source revision: ${result.stderr.trim()}`
    );
  }
  const revision = result.stdout.trim();
  if (!/^[0-9a-f]{40}(?:[0-9a-f]{24})?$/.test(revision)) {
    throw new Error(`Invalid curated renderer source revision: ${revision}`);
  }
  return revision;
}

test("2551Q continuation pages preserve every row and move totals to the final page", async ({
  page
}) => {
  const envelope = readFixture(
    "packages/form-contracts/fixtures/2551q-10-rows.json"
  );

  await renderEnvelope(page, envelope);

  const pages = page.locator(".form-page");
  await expect(pages).toHaveCount(3);
  await expect(pages.nth(1).locator("[data-row-key]")).toHaveCount(6);
  await expect(pages.nth(2).locator("[data-row-key]")).toHaveCount(4);
  await expect(pages.nth(1).locator(".official-schedule-row")).toHaveCount(6);
  await expect(pages.nth(2).locator(".official-schedule-row")).toHaveCount(12);
  await expect(pages.nth(2).locator("[data-visual-placeholder='true']")).toHaveCount(8);
  await expect(pages.nth(1).locator("[data-final-total='true']")).toHaveCount(0);
  await expect(pages.nth(1).locator("[data-final-total='false']")).toHaveCount(1);
  await expect(pages.nth(1).locator("[data-summary-kind='page_2_subtotal']")).toHaveCount(1);
  await expect(pages.nth(1).locator(".official-schedule-total")).toContainText(
    "Subtotal carried to Schedule 1 continuation"
  );
  await expect(pages.nth(1).locator(".official-schedule-total")).toHaveAttribute(
    "data-summary-value",
    "6300"
  );
  await expect(pages.nth(1).locator(".official-schedule-total")).not.toContainText(
    "To Part II Item 14"
  );
  await expect(pages.nth(2).locator("[data-final-total='true']")).toHaveCount(1);
  await expect(pages.nth(2).locator("[data-summary-kind='final_total']")).toHaveCount(1);
  await expect(pages.nth(2).locator(".official-schedule-total")).toHaveAttribute(
    "data-summary-value",
    "16500"
  );
  await expect(pages.nth(2).locator(".official-schedule-total")).toContainText(
    "To Part II Item 14"
  );
  await expect(pages.nth(2).locator(".official-schedule-head > span").first()).toContainText(
    /Alphanumeric Tax\s*Code/
  );
  await expect(pages.nth(2).locator(".page-two-identity")).toContainText(
    "Taxpayer’s Last Name"
  );
  await expect(pages.nth(2).locator(".official-atc-table")).toHaveCount(0);
  await expect(pages.nth(2).locator(".declaration, .official-declaration")).toHaveCount(0);
  expect(await pageHasNoOverflow(pages.nth(1))).toBe(true);
  expect(await pageHasNoOverflow(pages.nth(2))).toBe(true);
  await expectRealRowKeys(
    page,
    ["PT010", "PT040", "PT041", "PT060", "PT070", "PT090", "PT140", "PT150", "PT160", "PT170"]
      .map((atc, index) => `schedule-1-${index + 1}-${atc}`)
  );
});

async function pageHasNoOverflow(locator: Locator) {
  const geometry = await measuredPage(locator);
  const fits =
    geometry.scroll_height <= geometry.client_height + 1 &&
    geometry.scroll_width <= geometry.client_width + 1 &&
    geometry.descendant_overflow_x === 0 &&
    geometry.descendant_overflow_y === 0 &&
    geometry.descendant_clipped_x === 0 &&
    geometry.descendant_clipped_y === 0;
  if (!fits) {
    const offenders = await locator.evaluate((element) =>
      [...element.querySelectorAll<HTMLElement>("*")]
        .filter((child) =>
          child.scrollWidth > child.clientWidth + 1.25 ||
          child.scrollHeight > child.clientHeight + 1.25
        )
        .slice(0, 20)
        .map((child) => ({
          className: child.className,
          clientHeight: child.clientHeight,
          clientWidth: child.clientWidth,
          overflowX: getComputedStyle(child).overflowX,
          overflowY: getComputedStyle(child).overflowY,
          scrollHeight: child.scrollHeight,
          scrollWidth: child.scrollWidth,
          tagName: child.tagName,
          text: child.textContent?.trim().slice(0, 80)
        }))
    );
    console.error(`Form page overflow: ${JSON.stringify({ geometry, offenders })}`);
  }
  return fits;
}

async function measuredPage(locator: Locator) {
  return locator.evaluate((element) => {
    const hostWindow = window as Window & {
      measureEbirFormGeometry?: () => {
        pages: Array<{
          client_height: number;
          client_width: number;
          descendant_clipped_x: number;
          descendant_clipped_y: number;
          descendant_overflow_x: number;
          descendant_overflow_y: number;
          scroll_height: number;
          scroll_width: number;
        }>;
      } | null;
    };
    const report = hostWindow.measureEbirFormGeometry?.();
    if (!report) throw new Error("renderer geometry measurement is unavailable");
    const pageIndex = [...document.querySelectorAll(".form-page")].indexOf(element);
    const page = report.pages[pageIndex];
    if (!page) throw new Error(`renderer geometry omitted page ${pageIndex + 1}`);
    return page;
  });
}

function readFixture(relativePath: string): unknown {
  return JSON.parse(
    fs.readFileSync(path.join(REPO_ROOT, relativePath), "utf8")
  ) as unknown;
}

async function renderEnvelope(page: Page, envelope: unknown) {
  await page.goto("/");
  await page.waitForFunction(
    () =>
      typeof (
        window as Window & {
          renderEbirForm?: (value: unknown) => void;
        }
      ).renderEbirForm === "function"
  );
  await page.evaluate((value) => {
    const render = (
      window as Window & {
        renderEbirForm?: (input: unknown) => void;
      }
    ).renderEbirForm;
    if (!render) throw new Error("renderEbirForm is not installed");
    render(value);
  }, envelope);
  await page.locator(".form-document").waitFor();
  await page.evaluate(() =>
    new Promise<void>((resolve) => {
      requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
    })
  );
  await page.evaluate(() => document.fonts.ready);
}

async function prepareOfficialBlankComparison(page: Page) {
  await page.addStyleTag({
    content: `
      .form-page[data-visual-blank-values="true"] .comb-value > span,
      .form-page[data-visual-blank-values="true"] .adaptive-plain-value,
      .form-page[data-visual-blank-values="true"] .check-box,
      .form-page[data-visual-blank-values="true"] .tax-credit-description {
        color: transparent !important;
        text-shadow: none !important;
      }
    `
  });
  await page.locator(".form-page").evaluateAll((pages) => {
    for (const formPage of pages) {
      formPage.setAttribute("data-visual-blank-values", "true");
    }
  });
}

interface CriticalRegion {
  name: string;
  selector: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

async function expectCriticalRegionGeometry(
  formPage: Locator,
  regions: readonly CriticalRegion[]
) {
  const pageBox = await formPage.boundingBox();
  expect(pageBox, "critical-region page must have geometry").not.toBeNull();
  if (!pageBox) return;

  const failures: Array<{ region: string; dimension: string; difference_css_px: number }> = [];
  for (const region of regions) {
    const box = await formPage.locator(region.selector).boundingBox();
    expect(box, `${region.name} must render`).not.toBeNull();
    if (!box) continue;
    const actual = {
      x: box.x - pageBox.x,
      y: box.y - pageBox.y,
      width: box.width,
      height: box.height
    };
    const expected = {
      x: region.x / DEVICE_SCALE_FACTOR,
      y: region.y / DEVICE_SCALE_FACTOR,
      width: region.width / DEVICE_SCALE_FACTOR,
      height: region.height / DEVICE_SCALE_FACTOR
    };
    for (const key of ["x", "y", "width", "height"] as const) {
      const difference = Math.abs(actual[key] - expected[key]);
      if (difference > 2 / DEVICE_SCALE_FACTOR) {
        failures.push({
          region: region.name,
          dimension: key,
          difference_css_px: difference
        });
      }
    }
  }
  expect(
    failures,
    "critical regions must match the pinned 144 DPI reference within two pixels"
  ).toEqual([]);
}

async function expectHeaderOptionsTopAlignment(pageOne: Locator) {
  const labels = pageOne.locator(".official-header-options .option-label");
  await expect(labels).toHaveCount(5);
  expect(
    await labels.evaluateAll((elements) =>
      elements.map((element) => getComputedStyle(element).alignItems)
    )
  ).toEqual(["flex-start", "flex-start", "flex-start", "flex-start", "flex-start"]);

  for (const selector of [
    ".filing-basis > .option-label:first-child",
    ".filing-basis > .option-choices"
  ]) {
    const style = await pageOne.locator(selector).evaluate((element) => {
      const computed = getComputedStyle(element);
      return {
        borderBottomStyle: computed.borderBottomStyle,
        borderBottomWidth: Number.parseFloat(computed.borderBottomWidth)
      };
    });
    expect(style.borderBottomStyle).toBe("solid");
    expect(style.borderBottomWidth).toBeGreaterThan(0);
  }

  expect(
    await pageOne.locator(".official-header-options .check-box").evaluateAll((elements) =>
      elements.map((element) => getComputedStyle(element).backgroundColor)
    )
  ).toEqual(Array.from({ length: 8 }, () => "rgb(255, 255, 255)"));
}

async function expectBackgroundInformationParity(pageOne: Locator) {
  const labels = [
    [".tin-rdo-row > .field-label", "6 Taxpayer Identification Number (TIN)"],
    [".name-field > .field-label", "8 Taxpayer’s Name (Last Name, First Name, Middle Name for Individual OR Registered Name for Non-Individual)"],
    [".address-field > .field-label", "9 Registered Address (Indicate complete address. If branch, indicate the branch address. If the registered address is different from the current address, go to the RDO to update registered address by using BIR Form No. 1905)"],
    [".contact-email-field > .field-label:first-child", "10 Contact Number (Landline/Cellphone No.)"],
    [".contact-email-field > .field-label:nth-child(2)", "11 Email Address"],
    [".tax-relief-field > .field-label", "12 Are you availing of tax relief under Special Law or International Tax Treaty?"],
    [".tax-relief-spec", "12A If yes, specify"]
  ] as const;
  for (const [selector, wording] of labels) {
    await expect(pageOne.locator(selector)).toHaveText(wording);
  }

  const styles = await pageOne.locator(
    ".tin-rdo-row > .field-label, .name-field > .field-label, .address-field > .field-label, .contact-email-field > .field-label, .tax-relief-spec"
  ).evaluateAll((elements) => elements.map((element) => {
    const style = getComputedStyle(element);
    return {
      backgroundColor: style.backgroundColor,
      color: style.color,
      fontSize: style.fontSize
    };
  }));
  expect(styles).toEqual(Array.from({ length: 6 }, () => ({
    backgroundColor: "rgb(217, 217, 217)",
    color: "rgb(0, 0, 0)",
    fontSize: "12.8px"
  })));

  const noteStyles = await pageOne.locator(
    ".name-field > .field-label em, .address-field > .field-label em, .contact-email-field > .field-label em"
  ).evaluateAll((elements) => elements.map((element) => {
    const style = getComputedStyle(element);
    return {
      fontSize: style.fontSize,
      fontStyle: style.fontStyle,
      letterSpacing: style.letterSpacing,
      transform: style.transform
    };
  }));
  expect(noteStyles).toEqual([
    { fontSize: "10.6667px", fontStyle: "italic", letterSpacing: "normal", transform: "none" },
    { fontSize: "7.06667px", fontStyle: "italic", letterSpacing: "-0.106667px", transform: "none" },
    { fontSize: "9.93333px", fontStyle: "italic", letterSpacing: "normal", transform: "none" }
  ]);

  const separatorStyles = await pageOne.locator(".tin-separator").evaluateAll(
    (elements) => elements.map((element) => ({
      backgroundColor: getComputedStyle(element).backgroundColor,
      leftRule: getComputedStyle(element, "::before").borderLeftColor,
      rightRule: getComputedStyle(element, "::after").borderRightColor
    }))
  );
  expect(separatorStyles).toEqual(Array.from({ length: 3 }, () => ({
    backgroundColor: "rgb(166, 166, 166)",
    leftRule: "rgb(0, 0, 0)",
    rightRule: "rgb(0, 0, 0)"
  })));

  const guide = await pageOne.locator(".name-field .comb-value > span").first().evaluate(
    (element) => ({
      color: getComputedStyle(element, "::after").borderRightColor,
      height: Number.parseFloat(getComputedStyle(element, "::after").height)
    })
  );
  expect(guide.color).toBe("rgb(17, 17, 17)");
  expect(guide.height).toBeCloseTo(9.33, 1);

  expect(await pageOne.locator(".tin-rdo-row .comb-value > span").count()).toBe(17);
  expect(await pageOne.locator(".name-field .comb-value > span").count()).toBe(40);
  expect(await pageOne.locator(".address-field .comb-value > span").count()).toBe(75);
  expect(await pageOne.locator(".contact-email-field .comb-value > span").count()).toBe(40);
  expect(await pageOne.locator(".tax-relief-field .comb-value > span").count()).toBe(26);

  expect(
    await pageOne.locator(".relief-choices .check-box").evaluateAll((elements) =>
      elements.map((element) => getComputedStyle(element).backgroundColor)
    )
  ).toEqual(["rgb(255, 255, 255)", "rgb(255, 255, 255)"]);
}

async function measuredVerticalGap(formPage: Locator, barcodeSelector: string) {
  const barcode = formPage.locator(barcodeSelector);
  const symbol = await barcode.locator(".official-pdf417-symbol").boundingBox();
  const caption = await barcode.locator("small").boundingBox();
  expect(symbol, "PDF417 symbol must have geometry").not.toBeNull();
  expect(caption, "PDF417 caption must have geometry").not.toBeNull();
  if (!symbol || !caption) return Number.NaN;
  return (caption.y - symbol.y - symbol.height) * DEVICE_SCALE_FACTOR;
}

async function expectCriticalRegionContent(pageOne: Locator, pageTwo: Locator) {
  for (const label of [
    "For the",
    "Year Ended",
    "Quarter",
    "Amended Return?",
    "Number of Sheet/s"
  ]) {
    await expect(pageOne.locator(".official-header-options")).toContainText(label);
  }
  for (const label of [
    "Taxpayer Identification Number (TIN)",
    "RDO Code",
    "Taxpayer’s Name",
    "Registered Address",
    "Contact Number",
    "Email Address",
    "Special Law or International Tax Treaty?",
    "What income tax rates are you availing?"
  ]) {
    await expect(pageOne.locator(".background-information")).toContainText(label);
  }
  const taxLines = pageOne.locator(".official-tax-line");
  await expect(taxLines).toHaveCount(11);
  expect(
    await taxLines.evaluateAll((lines) =>
      lines.map((line) => line.getAttribute("data-item"))
    )
  ).toEqual(
    ["14", "15", "16", "17", "18", "19", "20", "21", "22", "23", "24"]
  );
  await expect(pageOne.locator(".tax-credit-description")).toHaveText(
    "Validated prior payment"
  );
  await expect(pageOne.locator(".tax-credit-description")).toHaveAttribute(
    "aria-label",
    "Item 17 specification: Validated prior payment"
  );
  await expect(pageOne.locator(".official-declaration")).toContainText(
    "Signature over Printed Name of Taxpayer/Authorized Representative/Tax Agent"
  );
  await expect(pageOne.locator(".official-declaration > p")).toContainText(
    "I/We declare under the penalties of perjury that this return, and all its attachments, have been made in good faith"
  );
  await expect(
    pageOne.locator(".official-signature-grid > div:first-child .signature-caption b")
  ).toHaveText(
    "Signature over Printed Name of Taxpayer/Authorized Representative/Tax Agent"
  );
  await expect(
    pageOne.locator(".official-signature-grid > div:last-child .signature-caption b")
  ).toHaveText(
    "Signature over Printed Name of President/Vice President/Authorized Officer or Representative/Tax Agent"
  );
  await expect(pageOne.locator(".tax-agent-strip")).toContainText(
    "Tax Agent Accreditation No./Attorney’s Roll No."
  );
  await expect(pageOne.locator(".official-barcode > small")).toHaveText(
    "2551Q 01/18ENCS P1"
  );
  await expect(pageOne.locator(".payment-row .decimal-separator")).toHaveCount(3);
  await expect(pageOne.locator(".payment-other-row .decimal-separator")).toHaveCount(1);
  await expect(pageOne.locator(".blank-money-value")).toHaveCount(4);
  await expect(
    pageOne.locator(".blank-money-value > .comb-value:first-child > span")
  ).toHaveCount(48);
  await expect(
    pageOne.locator(".blank-money-value > .comb-value:last-child > span")
  ).toHaveCount(8);
  await expect(pageTwo.locator(".page-two-barcode > small")).toHaveText(
    "2551Q 01/18ENCS P2"
  );
  await expect(pageTwo.locator(".official-schedule > h2")).toContainText(
    "Schedule 1 – Computation of Tax"
  );
  await expect(pageTwo.locator(".official-schedule-total")).toContainText(
    "Total Tax Due"
  );
}

async function expectRealRowKeys(page: Page, expected: string[]) {
  const keys = await page
    .locator("[data-row-key]")
    .evaluateAll((rows) =>
      rows
        .map((row) => row.getAttribute("data-row-key"))
        .filter(
          (key): key is string =>
            key !== null &&
            !key.includes("-empty-") &&
            !key.startsWith("payment-empty-")
        )
    );
  expect(keys).toEqual(expected);
  expect(new Set(keys).size).toBe(keys.length);
}

function assertVisualParity({
  parityCase,
  pageNumber,
  fixtureSha256,
  referencePath,
  actualBuffer,
  expectedBuffer,
  artifactStem,
  outputDir
}: {
  parityCase: ParityCase;
  pageNumber: number;
  fixtureSha256: string;
  referencePath: string;
  actualBuffer: Buffer;
  expectedBuffer: Buffer;
  artifactStem: string;
  outputDir: string;
}) {
  const actual = PNG.sync.read(actualBuffer);
  const expected = PNG.sync.read(expectedBuffer);
  const dimensionsMatch =
    actual.width === expected.width && actual.height === expected.height;

  if (!dimensionsMatch) {
    const artifacts = writeVisualArtifacts({
      actualBuffer,
      expectedBuffer,
      artifactStem,
      outputDir
    });
    visualPageMetrics.push({
      form_code: parityCase.code,
      form_revision: parityCase.revision,
      fixture: parityCase.fixture,
      fixture_sha256: fixtureSha256,
      reference: referencePath,
      reference_sha256: sha256(expectedBuffer),
      actual: artifacts.actual.path,
      actual_sha256: artifacts.actual.sha256,
      diff: null,
      diff_sha256: null,
      page: pageNumber,
      expected_width: expected.width,
      expected_height: expected.height,
      actual_width: actual.width,
      actual_height: actual.height,
      changed_pixels: null,
      changed_percent: null,
      max_changed_percent: MAX_CHANGED_PERCENT,
      pixelmatch_threshold: PIXELMATCH_THRESHOLD,
      comparison: "official-complete-page-v1",
      expected_ink_missing_percent: null,
      unexpected_actual_ink_percent: null,
      structural_changed_pixels: null,
      structural_changed_percent: null,
      structural_diff: null,
      structural_diff_sha256: null,
      structural_ink_threshold: STRUCTURAL_INK_THRESHOLD,
      structural_line_min_run: STRUCTURAL_LINE_MIN_RUN,
      structural_tolerance_radius: STRUCTURAL_TOLERANCE_RADIUS,
      passed: false
    });
    return;
  }

  const structural = compareOfficialStructure(expected, actual);
  const structuralChangedPercent =
    (structural.changedPixels / (expected.width * expected.height)) * 100;
  const completePage = compareCompleteOfficialPage(expected, actual, {
    pixelThreshold: PIXELMATCH_THRESHOLD
  });
  const passed = completePage.fullPageChangedPercent <= MAX_CHANGED_PERCENT;

  const diffBuffer = PNG.sync.write(completePage.diff);
  const artifacts = writeVisualArtifacts({
    actualBuffer,
    expectedBuffer,
    diffBuffer,
    artifactStem,
    outputDir
  });
  const structuralDiffBuffer = PNG.sync.write(structural.diff);
  const structuralDiffPath = path.join(
    outputDir,
    `${artifactStem}-structure-diff.png`
  );
  fs.writeFileSync(structuralDiffPath, structuralDiffBuffer);

  visualPageMetrics.push({
    form_code: parityCase.code,
    form_revision: parityCase.revision,
    fixture: parityCase.fixture,
    fixture_sha256: fixtureSha256,
    reference: referencePath,
    reference_sha256: sha256(expectedBuffer),
    actual: artifacts.actual.path,
    actual_sha256: artifacts.actual.sha256,
    diff: artifacts.diff?.path ?? null,
    diff_sha256: artifacts.diff?.sha256 ?? null,
    page: pageNumber,
    expected_width: expected.width,
    expected_height: expected.height,
    actual_width: actual.width,
    actual_height: actual.height,
    changed_pixels: completePage.fullPageChangedPixels,
    changed_percent: completePage.fullPageChangedPercent,
    max_changed_percent: MAX_CHANGED_PERCENT,
    pixelmatch_threshold: PIXELMATCH_THRESHOLD,
    comparison: "official-complete-page-v1",
    expected_ink_missing_percent: completePage.expectedInkMissingPercent,
    unexpected_actual_ink_percent: completePage.unexpectedActualInkPercent,
    structural_changed_pixels: structural.changedPixels,
    structural_changed_percent: structuralChangedPercent,
    structural_diff: repositoryRelativePath(structuralDiffPath),
    structural_diff_sha256: sha256(structuralDiffBuffer),
    structural_ink_threshold: STRUCTURAL_INK_THRESHOLD,
    structural_line_min_run: STRUCTURAL_LINE_MIN_RUN,
    structural_tolerance_radius: STRUCTURAL_TOLERANCE_RADIUS,
    passed
  });
}

/**
 * Compare the semantic form's ruled structure while tolerating the rasterizer
 * differences between Poppler's pinned-PDF output and Chromium's HTML output.
 * Only continuous rule segments participate; text and discrete artwork have
 * independent content/hash assertions and fixture-provided glyphs are blanked
 * before capture.  A four-pixel radius is two typographic points at 144 DPI.
 */
function compareOfficialStructure(expected: PNG, actual: PNG) {
  const expectedLines = structuralLineMask(expected);
  const actualLines = structuralLineMask(actual);
  const width = expected.width;
  const height = expected.height;
  const changed = new Uint8Array(width * height);

  markUnmatchedStructure(
    expectedLines,
    actualLines,
    changed,
    width,
    height,
    STRUCTURAL_TOLERANCE_RADIUS
  );
  markUnmatchedStructure(
    actualLines,
    expectedLines,
    changed,
    width,
    height,
    STRUCTURAL_TOLERANCE_RADIUS
  );

  const diff = new PNG({ width, height });
  let changedPixels = 0;
  for (let index = 0; index < changed.length; index += 1) {
    const offset = index * 4;
    if (changed[index] === 1) {
      changedPixels += 1;
      diff.data[offset] = 255;
      diff.data[offset + 1] = 0;
      diff.data[offset + 2] = 0;
      diff.data[offset + 3] = 255;
    } else {
      diff.data[offset] = 0;
      diff.data[offset + 1] = 0;
      diff.data[offset + 2] = 0;
      diff.data[offset + 3] = 0;
    }
  }
  return { changedPixels, diff };
}

function structuralLineMask(image: PNG) {
  const { width, height } = image;
  const dark = new Uint8Array(width * height);
  const lines = new Uint8Array(width * height);
  for (let index = 0; index < dark.length; index += 1) {
    const offset = index * 4;
    dark[index] =
      image.data[offset] < STRUCTURAL_INK_THRESHOLD &&
      image.data[offset + 1] < STRUCTURAL_INK_THRESHOLD &&
      image.data[offset + 2] < STRUCTURAL_INK_THRESHOLD
        ? 1
        : 0;
  }

  for (let y = 0; y < height; y += 1) {
    let runStart = -1;
    for (let x = 0; x <= width; x += 1) {
      const isDark = x < width && dark[y * width + x] === 1;
      if (isDark && runStart < 0) runStart = x;
      if (!isDark && runStart >= 0) {
        if (x - runStart >= STRUCTURAL_LINE_MIN_RUN) {
          for (let fillX = runStart; fillX < x; fillX += 1) {
            lines[y * width + fillX] = 1;
          }
        }
        runStart = -1;
      }
    }
  }

  for (let x = 0; x < width; x += 1) {
    let runStart = -1;
    for (let y = 0; y <= height; y += 1) {
      const isDark = y < height && dark[y * width + x] === 1;
      if (isDark && runStart < 0) runStart = y;
      if (!isDark && runStart >= 0) {
        if (y - runStart >= STRUCTURAL_LINE_MIN_RUN) {
          for (let fillY = runStart; fillY < y; fillY += 1) {
            lines[fillY * width + x] = 1;
          }
        }
        runStart = -1;
      }
    }
  }
  return lines;
}

function markUnmatchedStructure(
  source: Uint8Array,
  target: Uint8Array,
  changed: Uint8Array,
  width: number,
  height: number,
  radius: number
) {
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const index = y * width + x;
      if (source[index] !== 1) continue;
      let matched = false;
      for (let targetY = Math.max(0, y - radius); targetY <= Math.min(height - 1, y + radius) && !matched; targetY += 1) {
        for (let targetX = Math.max(0, x - radius); targetX <= Math.min(width - 1, x + radius); targetX += 1) {
          if (target[targetY * width + targetX] === 1) {
            matched = true;
            break;
          }
        }
      }
      if (!matched) changed[index] = 1;
    }
  }
}

function writeVisualArtifacts({
  actualBuffer,
  expectedBuffer,
  diffBuffer,
  artifactStem,
  outputDir
}: {
  actualBuffer: Buffer;
  expectedBuffer: Buffer;
  diffBuffer?: Buffer;
  artifactStem: string;
  outputDir: string;
}) {
  fs.mkdirSync(outputDir, { recursive: true });
  const actualPath = path.join(outputDir, `${artifactStem}-actual.png`);
  fs.writeFileSync(actualPath, actualBuffer);
  fs.writeFileSync(
    path.join(outputDir, `${artifactStem}-expected.png`),
    expectedBuffer
  );
  let diffArtifact: { path: string; sha256: string } | null = null;
  if (diffBuffer) {
    const diffPath = path.join(outputDir, `${artifactStem}-full-page-diff.png`);
    fs.writeFileSync(diffPath, diffBuffer);
    diffArtifact = {
      path: repositoryRelativePath(diffPath),
      sha256: sha256(diffBuffer)
    };
  }
  return {
    actual: {
      path: repositoryRelativePath(actualPath),
      sha256: sha256(actualBuffer)
    },
    diff: diffArtifact
  };
}

function repositoryRelativePath(artifactPath: string) {
  const relative = path.relative(REPO_ROOT, artifactPath);
  if (!relative || relative.startsWith("..") || path.isAbsolute(relative)) {
    throw new Error(`Visual evidence artifact is outside the repository: ${artifactPath}`);
  }
  return relative.split(path.sep).join("/");
}

function sha256(value: Buffer) {
  return crypto.createHash("sha256").update(value).digest("hex");
}

function slug(value: string) {
  return value.toLowerCase().replaceAll(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
}
