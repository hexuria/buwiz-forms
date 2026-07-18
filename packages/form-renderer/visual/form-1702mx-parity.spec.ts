import { expect, test, type Locator, type Page } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { PNG } from "pngjs";
import { compareCompleteOfficialPage } from "./official-page-diff";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(HERE, "../../..");
const DEVICE_SCALE_FACTOR = 1.5;
const MAX_CHANGED_PERCENT = 1;
const STRUCTURAL_INK_THRESHOLD = 100;
const STRUCTURAL_LINE_MIN_RUN = 20;
const STRUCTURAL_TOLERANCE_RADIUS = 4;

test("1702MX January 2018C renders every Rust fixture as four stable unclipped 612x936 pages", async ({ page }) => {
  for (const fixtureName of [
    "1702mx-minimum.json",
    "1702mx-normal.json",
    "1702mx-long-values.json",
    "1702mx-validation-edge.json",
    "1702mx-fixed-capacity.json"
  ]) {
    await renderEnvelope(page, readFixture(`packages/form-contracts/fixtures/${fixtureName}`));
    const pages = page.locator(".form-page");
    await expect(pages, fixtureName).toHaveCount(4);
    for (let pageIndex = 0; pageIndex < 4; pageIndex += 1) {
      await expect(pages.nth(pageIndex)).toHaveAttribute("data-paper", "folio");
      expect(await pageHasNoOverflow(pages.nth(pageIndex)), `${fixtureName} page ${pageIndex + 1}`).toBe(true);
    }
  }
});

test("1702MX January 2018C keeps verified page-specific PDF417, caption, and seal geometry", async ({ page }) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/1702mx-normal.json"));
  const pages = page.locator(".form-page");
  await expect(pages).toHaveCount(4);

  await expectCriticalRegionGeometry(pages.nth(0), [
    {
      name: "official seal XObject",
      selector: ".government-wordmark-1702mx img",
      x: 457.04,
      y: 57.936,
      width: 67.364,
      height: 56.304
    },
    {
      name: "page 1 official PDF417 XObject",
      selector: '.barcode-1702mx[data-barcode-page="1"] .official-pdf417-object-1702mx',
      x: 864.72,
      y: 134.64,
      width: 316.08,
      height: 90.72
    },
    {
      name: "page 1 official PDF417 live caption",
      selector: '.barcode-1702mx[data-barcode-page="1"] > small',
      x: 1004.12,
      y: 224.5276,
      width: 178.1404,
      height: 16.08
    }
  ]);
  await expectCriticalRegionGeometry(pages.nth(1), [
    {
      name: "page 2 official PDF417 XObject",
      selector: '.barcode-1702mx[data-barcode-page="2"] .official-pdf417-object-1702mx',
      x: 862.8,
      y: 62.4,
      width: 321.6,
      height: 79.2
    },
    {
      name: "page 2 official PDF417 live caption",
      selector: '.barcode-1702mx[data-barcode-page="2"] > small',
      x: 1003.88,
      y: 145.2876,
      width: 178.1404,
      height: 16.08
    }
  ]);
  await expectCriticalRegionGeometry(pages.nth(2), [
    {
      name: "page 3 official PDF417 XObject",
      selector: '.barcode-1702mx[data-barcode-page="3"] .official-pdf417-object-1702mx',
      x: 861.12,
      y: 97.44,
      width: 322.08,
      height: 85.44
    },
    {
      name: "page 3 official PDF417 live caption",
      selector: '.barcode-1702mx[data-barcode-page="3"] > small',
      x: 1005.56,
      y: 182.0476,
      width: 178.1404,
      height: 16.08
    }
  ]);
  await expectCriticalRegionGeometry(pages.nth(3), [
    {
      name: "page 4 official PDF417 XObject",
      selector: '.barcode-1702mx[data-barcode-page="4"] .official-pdf417-object-1702mx',
      x: 862.8,
      y: 66.24,
      width: 320.4,
      height: 83.28
    },
    {
      name: "page 4 official PDF417 live caption",
      selector: '.barcode-1702mx[data-barcode-page="4"] > small',
      x: 1004.12,
      y: 149.8476,
      width: 178.1404,
      height: 16.08
    }
  ]);

  expect(await page.locator(".official-pdf417-symbol-1702mx").evaluateAll(
    (symbols) => symbols.map((symbol) => ({
      preserveAspectRatio: symbol.getAttribute("preserveAspectRatio"),
      shapeRendering: getComputedStyle(symbol).shapeRendering,
      viewBox: symbol.getAttribute("viewBox")
    }))
  )).toEqual(Array.from({ length: 4 }, () => ({
    preserveAspectRatio: "none",
    shapeRendering: "crispedges",
    viewBox: "0 0 120.5 8"
  })));

  const captions = page.locator(".barcode-1702mx > small");
  for (const pageNumber of [1, 2, 3, 4]) {
    await expect(captions.nth(pageNumber - 1)).toHaveText(`1702-MX 01/18ENCS P${pageNumber}`);
  }
  expect(await captions.evaluateAll((elements) => elements.map((element) => {
    const style = getComputedStyle(element);
    return {
      fontFamily: style.fontFamily,
      fontSize: style.fontSize,
      lineHeight: style.lineHeight,
      textAlign: style.textAlign,
      whiteSpace: style.whiteSpace
    };
  }))).toEqual(Array.from({ length: 4 }, () => ({
    fontFamily: '"eBIRForms Arimo", sans-serif',
    fontSize: "10.72px",
    lineHeight: "10.72px",
    textAlign: "right",
    whiteSpace: "nowrap"
  })));

  expect(await page.locator(".government-wordmark-1702mx img").evaluate(
    (image) => ({
      naturalHeight: (image as HTMLImageElement).naturalHeight,
      naturalWidth: (image as HTMLImageElement).naturalWidth
    })
  )).toEqual({ naturalHeight: 102, naturalWidth: 119 });
});

test("1702MX January 2018C preserves the official Schedule 2 group rows", async ({ page }) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/1702mx-normal.json"));
  const pageTwo = page.locator(".form-page").nth(1);
  const scheduleTwoGroups = pageTwo.locator(".schedule-two-1702mx .regime-group-row-1702mx");

  await expect(scheduleTwoGroups).toHaveCount(2);
  await expect(scheduleTwoGroups.nth(0)).toHaveText("Less: Deductions Allowable under Existing Law");
  await expect(scheduleTwoGroups.nth(1)).toHaveText("OR [in case taxable under Sec 27(A) & 28(A)(1)]");
  expect(await scheduleTwoGroups.evaluateAll((rows) => rows.map((row) => ({
    backgroundColor: getComputedStyle(row).backgroundColor,
    borderBottomStyle: getComputedStyle(row).borderBottomStyle
  })))).toEqual(Array.from({ length: 2 }, () => ({
    backgroundColor: "rgb(217, 217, 217)",
    borderBottomStyle: "solid"
  })));
  expect(await pageHasNoOverflow(pageTwo)).toBe(true);
});

test("1702MX January 2018C preserves the official Schedule 1 guides and unavailable cells", async ({ page }) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/1702mx-normal.json"));
  const pageTwo = page.locator(".form-page").nth(1);
  const scheduleOne = pageTwo.locator(".schedule-one-1702mx");
  const rows = scheduleOne.locator(":scope > .relief-row-1702mx");

  await expectCriticalRegionGeometry(pageTwo, [
    { name: "Schedule 1", selector: ".schedule-one-1702mx", x: 36, y: 312, width: 1152, height: 262 }
  ]);
  await expect(rows).toHaveCount(6);
  expect(await rows.evaluateAll((elements) => elements.map(
    (element) => Math.round(element.getBoundingClientRect().height * 1.5)
  ))).toEqual([35, 32, 32, 32, 35, 33]);

  await expect(rows.locator(":scope > span:first-child")).toHaveText([
    "1 Investment Promotion Agency (IPA)/Implementing Government Entity",
    "2 Legal Basis",
    "3 Registered Activity/Program (Reg. No.)",
    "4 Special Tax Rate",
    "5 Effectivity Date of Tax Relief/Exemption FROM (MM/DD/YYYY)",
    "6 Expiration Date of Tax Relief/Exemption TO (MM/DD/YYYY)"
  ]);

  for (const rowIndex of [0, 1, 2]) {
    const fields = rows.nth(rowIndex).locator(":scope > .relief-character-field-1702mx");
    await expect(fields).toHaveCount(3);
    await expect(fields.nth(0)).toHaveAttribute("data-guide-cells", "10");
    await expect(fields.nth(1)).toHaveAttribute("data-guide-cells", "10");
    await expect(fields.nth(2)).toHaveAttribute("data-guide-cells", "9");
    await expect(fields.nth(0).locator(":scope > i")).toHaveCount(10);
    await expect(fields.nth(1).locator(":scope > i")).toHaveCount(10);
    await expect(fields.nth(2).locator(":scope > i")).toHaveCount(9);
  }

  const rateRow = rows.nth(3);
  await expect(rateRow.locator(":scope > .relief-unavailable-field-1702mx")).toHaveCount(2);
  const rate = rateRow.locator(":scope > .relief-rate-field-1702mx");
  await expect(rate).toHaveAttribute("data-rate-format", "00.0%");
  await expect(rate).toHaveAttribute("data-unavailable-cells", "5");
  await expect(rate).toHaveAttribute("data-applicable-character-cells", "3");
  await expect(rate).toHaveAttribute("data-static-cells", "2");
  await expect(rate).toHaveText(".%");

  for (const rowIndex of [4, 5]) {
    const fields = rows.nth(rowIndex).locator(":scope > .relief-date-field-1702mx");
    await expect(fields).toHaveCount(3);
    await expect(fields.nth(0)).toHaveAttribute("data-guide-cells", "10");
    await expect(fields.nth(1)).toHaveAttribute("data-guide-cells", "10");
    await expect(fields.nth(2)).toHaveAttribute("data-guide-cells", "9");
    await expect(fields.nth(0).locator(":scope > .relief-cell-unavailable-1702mx")).toHaveCount(2);
    await expect(fields.nth(1).locator(":scope > .relief-cell-unavailable-1702mx")).toHaveCount(2);
    await expect(fields.nth(2).locator(":scope > .relief-cell-unavailable-1702mx")).toHaveCount(1);
  }

  expect(await rateRow.locator(".relief-unavailable-field-1702mx").evaluateAll((fields) => fields.map(
    (field) => getComputedStyle(field).backgroundColor
  ))).toEqual(["rgb(217, 217, 217)", "rgb(217, 217, 217)"]);
  expect(await pageHasNoOverflow(pageTwo)).toBe(true);
});

test("1702MX January 2018C preserves the official Part II penalty heading", async ({ page }) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/1702mx-normal.json"));
  const pageOne = page.locator(".form-page").nth(0);
  const partTwo = pageOne.locator(".part-two-1702mx");
  const penaltyHeading = partTwo.locator(":scope > .penalty-heading-1702mx");

  await expect(penaltyHeading).toHaveCount(1);
  await expect(penaltyHeading).toHaveText("Add: Penalties");
  await expect(partTwo.locator(":scope > .amount-row-1702mx")).toHaveCount(8);
  expect(await penaltyHeading.evaluate((element) => ({
    backgroundColor: getComputedStyle(element).backgroundColor,
    borderBottomStyle: getComputedStyle(element).borderBottomStyle
  }))).toEqual({
    backgroundColor: "rgb(217, 217, 217)",
    borderBottomStyle: "solid"
  });
  expect(await pageHasNoOverflow(pageOne)).toBe(true);
});

test("1702MX January 2018C preserves the official Schedule 5 Item 17 band and subrows", async ({ page }) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/1702mx-normal.json"));
  const pageThree = page.locator(".form-page").nth(2);
  const scheduleFive = pageThree.locator(".schedule-five-1702mx");

  await expect(scheduleFive.locator(":scope > .regime-header-1702mx")).toHaveCount(0);
  await expect(scheduleFive.locator(":scope > .schedule-five-row-1702mx")).toHaveCount(26);
  await expect(scheduleFive.locator(":scope > .schedule-five-group-row-1702mx")).toHaveText(
    "17 Others (Deductions Subject to Withholding Tax and Other Expenses) [Specify below; Add additional sheet(s), if necessary]"
  );
  await expect(scheduleFive.locator(":scope > .item-17-1702mx > span:first-child")).toHaveText(
    "a. Janitorial and Messengerial Services"
  );
  await expect(scheduleFive.locator(":scope > .item-25-1702mx > span:first-child")).toHaveText("i.");
  await expect(scheduleFive.locator(":scope > .item-26-1702mx > span:first-child")).toContainText(
    "18 Total Ordinary Allowable Itemized Deductions"
  );
  expect(await pageHasNoOverflow(pageThree)).toBe(true);
});

test("1702MX January 2018C preserves the official Schedule 10 reconciliation groups", async ({ page }) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/1702mx-normal.json"));
  const pageFour = page.locator(".form-page").nth(3);
  const scheduleTen = pageFour.locator(".schedule-ten-1702mx");

  await expect(scheduleTen.locator(":scope > .schedule-ten-row-1702mx")).toHaveCount(10);
  const groups = scheduleTen.locator(":scope > .schedule-ten-group-row-1702mx");
  await expect(groups).toHaveCount(3);
  await expect(groups.nth(0)).toHaveText("Add: Non-Deductible Expenses/Taxable Other Income (specify below)");
  await expect(groups.nth(1)).toHaveText("Less: A) Non-Taxable Income and Income Subjected to Final Tax (specify below)");
  await expect(groups.nth(2)).toHaveText("B) Special Deductions (specify below)");
  await expect(scheduleTen.locator(":scope > .item-2-1702mx .schedule-ten-description-1702mx")).toHaveCount(1);
  await expect(scheduleTen.locator(":scope > .item-8-1702mx .schedule-ten-description-1702mx")).toHaveCount(1);
  await expect(scheduleTen.locator(":scope > .item-1-1702mx > span:first-child")).toHaveText("1 Net Income/(Loss) per Books");
  await expect(scheduleTen.locator(":scope > .item-4-1702mx > span:first-child")).toHaveText("4 Total (Sum of Items 1 to 3)");
  await expect(scheduleTen.locator(":scope > .item-9-1702mx > span:first-child")).toHaveText("9 Total (Sum of Items 5 to 8)");
  await expect(scheduleTen.locator(":scope > .item-10-1702mx > span:first-child")).toHaveText("10 Net Taxable Income/(Loss) (Item 4 Less Item 9)");
  expect(await scheduleTen.locator(".amount-cell-1702mx").first().evaluate(
    (cell) => getComputedStyle(cell, "::after").display
  )).toBe("none");
  expect(await pageHasNoOverflow(pageFour)).toBe(true);
});

test("1702MX January 2018C preserves the official page 4 NOLCO and MCIT hierarchy", async ({ page }) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/1702mx-normal.json"));
  const pageFour = page.locator(".form-page").nth(3);

  for (const selector of [".schedule-seven-one-1702mx", ".schedule-eight-one-1702mx"]) {
    const schedule = pageFour.locator(selector);
    await expect(schedule.locator(":scope > .nolco-header-1702mx")).toHaveCount(1);
    await expect(schedule.locator(":scope > .nolco-data-row-1702mx")).toHaveCount(4);
    await expect(schedule.locator(":scope > .nolco-total-row-1702mx")).toHaveCount(1);
    await expect(schedule.locator(":scope > .nolco-data-row-1702mx .nolco-year-cell-1702mx > b")).toHaveText(["4", "5", "6", "7"]);
    expect(await schedule.locator(".amount-cell-1702mx").first().evaluate(
      (cell) => getComputedStyle(cell, "::after").display
    )).toBe("none");
  }

  const scheduleNine = pageFour.locator(".schedule-nine-1702mx");
  await expect(scheduleNine.locator(":scope > .mcit-data-row-1702mx")).toHaveCount(3);
  await expect(scheduleNine.locator(":scope > .mcit-continuation-row-1702mx")).toHaveCount(3);
  await expect(scheduleNine.locator(":scope > .mcit-total-row-1702mx")).toHaveCount(1);
  await expect(scheduleNine.locator(":scope > .mcit-data-row-1702mx .mcit-year-cell-1702mx > b")).toHaveText(["1", "2", "3"]);
  await expect(scheduleNine.locator(":scope > .mcit-continuation-row-1702mx .mcit-numbered-amount-1702mx > b")).toHaveText(["1", "2", "3"]);
  expect(await pageHasNoOverflow(pageFour)).toBe(true);
});

test("1702MX January 2018C matches the complete official pages", async ({ page }, testInfo) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/1702mx-normal.json"));
  const pages = page.locator(".form-page");
  await expect(pages).toHaveCount(4);

  await expectCriticalRegionGeometry(pages.nth(0), [
    { name: "masthead", selector: ".masthead-1702mx", x: 36, y: 121, width: 1152, height: 137 },
    { name: "Items 1-5", selector: ".header-options-1702mx", x: 36, y: 258, width: 1152, height: 88 },
    { name: "Part I", selector: ".part-one-1702mx", x: 36, y: 350, width: 1152, height: 456 },
    { name: "Part II", selector: ".part-two-1702mx", x: 36, y: 810, width: 1152, height: 411 },
    { name: "declaration", selector: ".declaration-1702mx", x: 36, y: 1225, width: 1152, height: 200 },
    { name: "Part III", selector: ".part-three-1702mx", x: 36, y: 1429, width: 1152, height: 386 }
  ]);
  await expectCriticalRegionGeometry(pages.nth(1), [
    { name: "masthead", selector: ".masthead-1702mx", x: 36, y: 55, width: 1152, height: 109 },
    { name: "identity", selector: ".page-identity-1702mx", x: 36, y: 164, width: 1152, height: 59 },
    { name: "Part IV instructions", selector: ".part-four-heading-1702mx", x: 36, y: 228, width: 1152, height: 84 },
    { name: "Schedule 2", selector: ".schedule-two-1702mx", x: 36, y: 578, width: 1152, height: 807 },
    { name: "Schedule 3", selector: ".schedule-three-1702mx", x: 36, y: 1393, width: 1152, height: 369 }
  ]);
  await expectCriticalRegionGeometry(pages.nth(2), [
    { name: "masthead", selector: ".masthead-1702mx", x: 36, y: 89, width: 1152, height: 115 },
    { name: "identity", selector: ".page-identity-1702mx", x: 36, y: 204, width: 1152, height: 63 },
    { name: "Schedule 4", selector: ".schedule-four-1702mx", x: 36, y: 273, width: 1152, height: 303 },
    { name: "Schedule 5", selector: ".schedule-five-1702mx", x: 36, y: 582, width: 1152, height: 772 },
    { name: "Schedule 6", selector: ".schedule-six-1702mx", x: 36, y: 1360, width: 1152, height: 214 },
    { name: "Schedule 7", selector: ".schedule-seven-1702mx", x: 36, y: 1579, width: 1152, height: 146 }
  ]);
  await expectCriticalRegionGeometry(pages.nth(3), [
    { name: "masthead", selector: ".masthead-1702mx", x: 36, y: 57, width: 1152, height: 115 },
    { name: "identity", selector: ".page-identity-1702mx", x: 36, y: 172, width: 1152, height: 63 },
    { name: "Schedule 7.1", selector: ".schedule-seven-one-1702mx", x: 36, y: 238, width: 1152, height: 242 },
    { name: "Schedule 8", selector: ".schedule-eight-1702mx", x: 36, y: 482, width: 1152, height: 164 },
    { name: "Schedule 8.1", selector: ".schedule-eight-one-1702mx", x: 36, y: 646, width: 1152, height: 240 },
    { name: "Schedule 9", selector: ".schedule-nine-1702mx", x: 36, y: 888, width: 1152, height: 419 },
    { name: "Schedule 10", selector: ".schedule-ten-1702mx", x: 36, y: 1309, width: 1152, height: 443 }
  ]);

  expect(await pages.nth(0).locator("img").count()).toBe(1);
  for (const pageIndex of [1, 2, 3]) expect(await pages.nth(pageIndex).locator("img").count()).toBe(0);
  for (const pageIndex of [0, 1, 2, 3]) {
    await expect(pages.nth(pageIndex).locator(".official-pdf417-symbol-1702mx")).toHaveCount(1);
  }

  await page.addStyleTag({
    content: `
      .form-page[data-visual-blank-values="true"] .comb-value > span,
      .form-page[data-visual-blank-values="true"] .adaptive-plain-value,
      .form-page[data-visual-blank-values="true"] .plain-value-1702mx,
      .form-page[data-visual-blank-values="true"] .check-box,
      .form-page[data-visual-blank-values="true"] .amount-cell-1702mx {
        color: transparent !important;
        text-shadow: none !important;
      }
    `
  });
  await pages.evaluateAll((elements) => {
    for (const element of elements) element.setAttribute("data-visual-blank-values", "true");
  });

  const results: Array<{
    page: number;
    structuralChangedPercent: number;
    fullPageChangedPercent: number;
    expectedInkMissingPercent: number;
    unexpectedActualInkPercent: number;
  }> = [];
  for (let pageIndex = 0; pageIndex < 4; pageIndex += 1) {
    const expectedBuffer = fs.readFileSync(path.join(
      REPO_ROOT,
      `packages/form-renderer/references/1702mx-2018c-page-${pageIndex + 1}.png`
    ));
    const actualBuffer = await pages.nth(pageIndex).screenshot({ animations: "disabled", caret: "hide" });
    const expected = PNG.sync.read(expectedBuffer);
    const actual = PNG.sync.read(actualBuffer);
    expect(actual.width).toBe(expected.width);
    expect(actual.height).toBe(expected.height);
    const { changedPixels, diff } = compareOfficialStructure(expected, actual);
    const completePage = compareCompleteOfficialPage(expected, actual);
    results.push({
      page: pageIndex + 1,
      structuralChangedPercent: changedPixels * 100 / (expected.width * expected.height),
      fullPageChangedPercent: completePage.fullPageChangedPercent,
      expectedInkMissingPercent: completePage.expectedInkMissingPercent,
      unexpectedActualInkPercent: completePage.unexpectedActualInkPercent
    });
    fs.writeFileSync(testInfo.outputPath(`1702mx-page-${pageIndex + 1}-actual.png`), actualBuffer);
    fs.writeFileSync(testInfo.outputPath(`1702mx-page-${pageIndex + 1}-structure-diff.png`), PNG.sync.write(diff));
    fs.writeFileSync(
      testInfo.outputPath(`1702mx-page-${pageIndex + 1}-full-page-diff.png`),
      PNG.sync.write(completePage.diff)
    );
  }
  console.log(`1702MX complete-page parity: ${JSON.stringify(results)}`);
  expect(
    results.filter((result) => result.fullPageChangedPercent > MAX_CHANGED_PERCENT),
    "complete page pixels, including all static labels, instructions, fields, signatures, and artwork"
  ).toEqual([]);
});

interface CriticalRegion {
  name: string;
  selector: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

async function expectCriticalRegionGeometry(page: Locator, regions: CriticalRegion[]) {
  const pageBox = await page.boundingBox();
  expect(pageBox).not.toBeNull();
  if (!pageBox) return;
  const failures: Array<{ region: string; dimension: string; difference: number }> = [];
  for (const region of regions) {
    const box = await page.locator(region.selector).boundingBox();
    expect(box, region.name).not.toBeNull();
    if (!box) continue;
    const actual = { x: box.x - pageBox.x, y: box.y - pageBox.y, width: box.width, height: box.height };
    const expected = {
      x: region.x / DEVICE_SCALE_FACTOR,
      y: region.y / DEVICE_SCALE_FACTOR,
      width: region.width / DEVICE_SCALE_FACTOR,
      height: region.height / DEVICE_SCALE_FACTOR
    };
    console.log(`1702MX geometry ${region.name}: ${JSON.stringify({ actual, expected })}`);
    for (const key of ["x", "y", "width", "height"] as const) {
      const difference = Math.abs(actual[key] - expected[key]);
      if (difference > 2 / DEVICE_SCALE_FACTOR) failures.push({ region: region.name, dimension: key, difference });
    }
  }
  expect(failures).toEqual([]);
}

async function pageHasNoOverflow(locator: Locator) {
  const report = await locator.evaluate((element) => {
    const measurement = (window as Window & {
      measureEbirFormGeometry?: () => { pages: Array<{
        client_height: number;
        client_width: number;
        descendant_clipped_x: number;
        descendant_clipped_y: number;
        descendant_overflow_x: number;
        descendant_overflow_y: number;
        scroll_height: number;
        scroll_width: number;
      }> } | null;
    }).measureEbirFormGeometry?.();
    if (!measurement) throw new Error("renderer measurement unavailable");
    return measurement.pages[[...document.querySelectorAll(".form-page")].indexOf(element)];
  });
  const valid = report.scroll_height <= report.client_height + 1 &&
    report.scroll_width <= report.client_width + 1 &&
    report.descendant_overflow_x === 0 && report.descendant_overflow_y === 0 &&
    report.descendant_clipped_x === 0 && report.descendant_clipped_y === 0;
  if (!valid) {
    const offenders = await locator.evaluate((element) => [...element.querySelectorAll<HTMLElement>("*")]
      .filter((child) => child.scrollWidth > child.clientWidth + 1.25 || child.scrollHeight > child.clientHeight + 1.25)
      .map((child) => ({
        class_name: child.className,
        parent_class_name: child.parentElement?.className,
        client_width: child.clientWidth,
        scroll_width: child.scrollWidth,
        client_height: child.clientHeight,
        scroll_height: child.scrollHeight,
        text: child.textContent?.trim().slice(0, 100)
      })));
    console.warn(`1702MX overflow report: ${JSON.stringify({ report, offenders })}`);
  }
  return valid;
}

function readFixture(relativePath: string): unknown {
  return JSON.parse(fs.readFileSync(path.join(REPO_ROOT, relativePath), "utf8")) as unknown;
}

async function renderEnvelope(page: Page, envelope: unknown) {
  await page.goto("/");
  await page.waitForFunction(() => typeof (window as Window & { renderEbirForm?: unknown }).renderEbirForm === "function");
  await page.evaluate((value) => {
    const render = (window as Window & { renderEbirForm?: (input: unknown) => void }).renderEbirForm;
    if (!render) throw new Error("renderEbirForm is unavailable");
    render(value);
  }, envelope);
  await page.locator(".form-document").waitFor();
  await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));
  await page.evaluate(() => document.fonts.ready);
}

function compareOfficialStructure(expected: PNG, actual: PNG) {
  const expectedLines = structuralLineMask(expected);
  const actualLines = structuralLineMask(actual);
  const changed = new Uint8Array(expected.width * expected.height);
  markUnmatchedStructure(expectedLines, actualLines, changed, expected.width, expected.height);
  markUnmatchedStructure(actualLines, expectedLines, changed, expected.width, expected.height);
  const diff = new PNG({ width: expected.width, height: expected.height });
  let changedPixels = 0;
  for (let index = 0; index < changed.length; index += 1) {
    const offset = index * 4;
    if (changed[index] === 1) {
      changedPixels += 1;
      diff.data[offset] = 255;
      diff.data[offset + 3] = 255;
    }
  }
  return { changedPixels, diff };
}

function structuralLineMask(image: PNG) {
  const dark = new Uint8Array(image.width * image.height);
  const lines = new Uint8Array(dark.length);
  for (let index = 0; index < dark.length; index += 1) {
    const offset = index * 4;
    dark[index] = image.data[offset] < STRUCTURAL_INK_THRESHOLD &&
      image.data[offset + 1] < STRUCTURAL_INK_THRESHOLD &&
      image.data[offset + 2] < STRUCTURAL_INK_THRESHOLD ? 1 : 0;
  }
  for (let y = 0; y < image.height; y += 1) {
    let start = -1;
    for (let x = 0; x <= image.width; x += 1) {
      const ink = x < image.width && dark[y * image.width + x] === 1;
      if (ink && start < 0) start = x;
      if (!ink && start >= 0) {
        if (x - start >= STRUCTURAL_LINE_MIN_RUN) for (let fill = start; fill < x; fill += 1) lines[y * image.width + fill] = 1;
        start = -1;
      }
    }
  }
  for (let x = 0; x < image.width; x += 1) {
    let start = -1;
    for (let y = 0; y <= image.height; y += 1) {
      const ink = y < image.height && dark[y * image.width + x] === 1;
      if (ink && start < 0) start = y;
      if (!ink && start >= 0) {
        if (y - start >= STRUCTURAL_LINE_MIN_RUN) for (let fill = start; fill < y; fill += 1) lines[fill * image.width + x] = 1;
        start = -1;
      }
    }
  }
  return lines;
}

function markUnmatchedStructure(source: Uint8Array, target: Uint8Array, changed: Uint8Array, width: number, height: number) {
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const index = y * width + x;
      if (source[index] !== 1) continue;
      let matched = false;
      for (let ty = Math.max(0, y - STRUCTURAL_TOLERANCE_RADIUS); ty <= Math.min(height - 1, y + STRUCTURAL_TOLERANCE_RADIUS) && !matched; ty += 1) {
        for (let tx = Math.max(0, x - STRUCTURAL_TOLERANCE_RADIUS); tx <= Math.min(width - 1, x + STRUCTURAL_TOLERANCE_RADIUS); tx += 1) {
          if (target[ty * width + tx] === 1) { matched = true; break; }
        }
      }
      if (!matched) changed[index] = 1;
    }
  }
}
