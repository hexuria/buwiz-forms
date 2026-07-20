import { expect, test, type Locator, type Page } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { PNG } from "pngjs";
import { compareCompleteOfficialPage } from "./official-page-diff";
import { renderEnvelope } from "./support/render-utils";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(HERE, "../../..");
const staticTextInventory = JSON.parse(fs.readFileSync(path.join(
  REPO_ROOT,
  "packages/form-renderer/references/1601c-2018-static-text-inventory.json"
), "utf8")) as {
  coverage_status: string;
  known_gaps: string[];
  official_source_sha256: string;
  regions: Array<{ selector: string; text: string }>;
};
const DEVICE_SCALE_FACTOR = 1.5;
const MAX_CHANGED_PERCENT = 1;
const STRUCTURAL_INK_THRESHOLD = 100;
const STRUCTURAL_LINE_MIN_RUN = 20;
const STRUCTURAL_TOLERANCE_RADIUS = 4;

test("1601C 2018 renders every Rust fixture as two stable unclipped folio pages", async ({ page }) => {
  for (const fixtureName of [
    "1601c-minimum.json",
    "1601c-normal.json",
    "1601c-long-values.json",
    "1601c-validation-edge.json",
    "1601c-3-rows.json"
  ]) {
    const fixture = readFixture(`packages/form-contracts/fixtures/${fixtureName}`);
    await renderEnvelope(page, fixture);
    const pages = page.locator(".form-page");
    await expect(pages, fixtureName).toHaveCount(2);
    expect(await pageHasNoOverflow(pages.nth(0)), `${fixtureName} page 1`).toBe(true);
    expect(await pageHasNoOverflow(pages.nth(1)), `${fixtureName} page 2`).toBe(true);
  }
});

test("1601C 2018 locks every currently represented reviewed static-copy region without hiding known gaps", async ({ page }) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/1601c-minimum.json"));

  expect(staticTextInventory.official_source_sha256).toBe(
    "c8faaa71015337a73b4ceb96bfb265c539589ab5e10eb27899bb81f87f417397"
  );
  expect(staticTextInventory.coverage_status).toBe("partial_reviewed");
  expect(staticTextInventory.known_gaps).toHaveLength(3);
  for (const region of staticTextInventory.regions) {
    const visibleText = await reviewedStaticText(page.locator(region.selector));
    expect(visibleText, region.selector).toBe(region.text);
  }
});

test("1601C 2018 keeps verified page-specific PDF417, caption, and seal geometry", async ({ page }) => {
  const fixture = readFixture("packages/form-contracts/fixtures/1601c-normal.json");
  await renderEnvelope(page, fixture);
  const pages = page.locator(".form-page");
  await expect(pages).toHaveCount(2);

  await expectCriticalRegionGeometry(pages.nth(0), [
    {
      name: "official seal XObject",
      selector: ".government-wordmark-1601c img",
      x: 457,
      y: 20,
      width: 62,
      height: 56
    },
    {
      name: "page 1 official PDF417 XObject",
      selector: '.barcode-1601c[data-barcode-page="1"] .official-pdf417-object-1601c',
      x: 882,
      y: 97,
      width: 302,
      height: 70
    },
    {
      name: "page 1 official PDF417 live caption",
      selector: '.barcode-1601c[data-barcode-page="1"] > small',
      x: 1020,
      y: 167,
      width: 166,
      height: 16
    }
  ]);
  await expectCriticalRegionGeometry(pages.nth(1), [
    {
      name: "page 2 official PDF417 XObject",
      selector: '.barcode-1601c[data-barcode-page="2"] .official-pdf417-object-1601c',
      x: 880,
      y: 69,
      width: 302,
      height: 69
    },
    {
      name: "page 2 official PDF417 live caption",
      selector: '.barcode-1601c[data-barcode-page="2"] > small',
      x: 1019,
      y: 136,
      width: 166,
      height: 16
    }
  ]);

  expect(await page.locator(".official-pdf417-symbol-1601c").evaluateAll(
    (symbols) => symbols.map((symbol) => ({
      preserveAspectRatio: symbol.getAttribute("preserveAspectRatio"),
      shapeRendering: getComputedStyle(symbol).shapeRendering,
      viewBox: symbol.getAttribute("viewBox")
    }))
  )).toEqual(Array.from({ length: 2 }, () => ({
    preserveAspectRatio: "none",
    shapeRendering: "crispedges",
    viewBox: "0 0 120 7"
  })));

  const captions = page.locator(".barcode-1601c > small");
  await expect(captions.nth(0)).toHaveText("1601-C 01/18ENCS P1");
  await expect(captions.nth(1)).toHaveText("1601-C 01/18ENCS P2");
  expect(await captions.evaluateAll((elements) => elements.map((element) => {
    const style = getComputedStyle(element);
    return {
      fontFamily: style.fontFamily,
      fontSize: style.fontSize,
      lineHeight: style.lineHeight,
      textAlign: style.textAlign,
      whiteSpace: style.whiteSpace
    };
  }))).toEqual(Array.from({ length: 2 }, () => ({
    fontFamily: '"eBIRForms Arimo", sans-serif',
    fontSize: "10.72px",
    lineHeight: "10.72px",
    textAlign: "right",
    whiteSpace: "nowrap"
  })));

  expect(await page.locator(".government-wordmark-1601c img").evaluate(
    (image) => ({
      naturalHeight: (image as HTMLImageElement).naturalHeight,
      naturalWidth: (image as HTMLImageElement).naturalWidth
    })
  )).toEqual({ naturalHeight: 78, naturalWidth: 86 });
});

test("1601C 2018 preserves the official gray and white field partitions", async ({ page }) => {
  const fixture = readFixture("packages/form-contracts/fixtures/1601c-normal.json");
  await renderEnvelope(page, fixture);
  const pageOne = page.locator(".form-1601c-page-one");

  const headerOptions = pageOne.locator(".header-option-1601c");
  expect(await headerOptions.locator(":scope > span").evaluateAll((elements) =>
    elements.map((element) => getComputedStyle(element).backgroundColor)
  )).toEqual(Array.from({ length: 5 }, () => "rgb(217, 217, 217)"));

  for (const [optionIndex, expectedWidth] of [[0, 86], [3, 28]] as const) {
    const comb = headerOptions.nth(optionIndex).locator(":scope > span > .comb-value");
    const partition = await comb.evaluate((element) => ({
      backgroundColor: getComputedStyle(element).backgroundColor,
      width: element.getBoundingClientRect().width
    }));
    expect(partition.backgroundColor).toBe("rgb(255, 255, 255)");
    expect(partition.width).toBeCloseTo(expectedWidth * 4 / 3, 1);
  }
  const atcPartition = await headerOptions.nth(4).locator(":scope > span > .atc-plain-1601c")
    .evaluate((element) => ({
      backgroundColor: getComputedStyle(element).backgroundColor,
      width: element.getBoundingClientRect().width
    }));
  expect(atcPartition.backgroundColor).toBe("rgb(255, 255, 255)");
  expect(atcPartition.width).toBeCloseTo(46 * 4 / 3, 1);

  await expectWhiteBackground(pageOne.locator(".address-second-1601c > .comb-value").first());
  await expectGrayBackground(pageOne.locator(".category-choices-1601c"));
  const taxDebitLabel = pageOne.locator(".payment-tax-debit-1601c > span:first-child");
  await expect(taxDebitLabel).toHaveText("39 Tax Debit Memo");
  await expectGrayBackground(taxDebitLabel);
  const taxDebitPartition = await taxDebitLabel.evaluate((element) => ({
    gridColumnEnd: getComputedStyle(element).gridColumnEnd,
    gridColumnStart: getComputedStyle(element).gridColumnStart,
    width: element.getBoundingClientRect().width
  }));
  expect(taxDebitPartition).toMatchObject({
    gridColumnEnd: "span 2",
    gridColumnStart: "1"
  });
  expect(taxDebitPartition.width).toBeCloseTo(173 * 4 / 3, 1);
  await expect(pageOne.locator(".payment-tax-debit-1601c > .comb-value")).toHaveCount(2);
  await expectWhiteBackground(pageOne.locator(".payment-other-1601c > span:first-child"));
});

test("1601C 2018 keeps the official inline Schedule I Tax Paid heading", async ({ page }) => {
  const fixture = readFixture("packages/form-contracts/fixtures/1601c-normal.json");
  await renderEnvelope(page, fixture);
  const label = page.locator(".form-1601c-page-two .schedule-tax-paid-label-1601c");

  await expect(label).toHaveText("Tax Paid (Excluding Penalties for the Month)");
  const layout = await label.evaluate((element) => {
    const style = getComputedStyle(element);
    const note = element.querySelector("em")?.getBoundingClientRect();
    const heading = element.firstChild instanceof Text
      ? document.createRange()
      : null;
    if (!heading || !element.firstChild) throw new Error("missing Tax Paid text node");
    heading.selectNode(element.firstChild);
    const headingBox = heading.getBoundingClientRect();
    return {
      alignItems: style.alignItems,
      noteTop: note?.top,
      headingTop: headingBox.top,
      whiteSpace: style.whiteSpace
    };
  });
  expect(layout).toMatchObject({
    alignItems: "baseline",
    whiteSpace: "nowrap"
  });
  expect(layout.noteTop).toBeDefined();
  expect(Math.abs(layout.headingTop - (layout.noteTop ?? 0))).toBeLessThanOrEqual(2);
});

test("1601C 2018 uses the exact reviewed plain fields and character-guide capacities", async ({ page }) => {
  const fixture = readFixture("packages/form-contracts/fixtures/1601c-normal.json");
  await renderEnvelope(page, fixture);

  const item5 = page.locator(".header-option-1601c").nth(4).locator(":scope > span");
  await expect(item5.locator(":scope > .atc-plain-1601c")).toHaveText("WW010");
  await expect(item5.locator(":scope > .comb-value")).toHaveCount(0);

  for (const line of ["37", "38"] as const) {
    expect(await directCombCapacities(page.locator(`[data-payment-line="${line}"]`)))
      .toEqual([5, 6, 8]);
  }
  expect(await directCombCapacities(page.locator('[data-payment-line="39"]')))
    .toEqual([6, 8]);
  expect(await directCombCapacities(page.locator('[data-payment-line="40"]')))
    .toEqual([7, 5, 6, 8]);

  const blankScheduleRow = page.locator('.schedule-top-row-1601c[data-visual-placeholder="true"]').first();
  expect(await directCombCapacities(blankScheduleRow)).toEqual([6, 8, 5, 6]);
});

test("1601C 2018 switches each reviewed adaptive field only after its official character capacity", async ({ page }) => {
  const minimum = readFixture("packages/form-contracts/fixtures/1601c-minimum.json") as Mutable1601CEnvelope;
  const profileCases: Array<{
    capacity: number;
    exact: string;
    mutate: (fixture: Mutable1601CEnvelope, value: string) => void;
    selector: string;
  }> = [
    {
      capacity: 40,
      exact: "N".repeat(40),
      mutate: (fixture, value) => { fixture.taxpayer.name = value; },
      selector: ".form-1601c-page-one .name-1601c > :nth-child(2)"
    },
    {
      capacity: 40,
      exact: "A".repeat(40),
      mutate: (fixture, value) => { fixture.taxpayer.registered_address = value; },
      selector: ".form-1601c-page-one .address-1601c > :nth-child(2)"
    },
    {
      capacity: 31,
      exact: "B".repeat(31),
      mutate: (fixture, value) => { fixture.fields.registered_address_2.value = value; },
      selector: ".form-1601c-page-one .address-second-1601c > :nth-child(1)"
    },
    {
      capacity: 34,
      exact: "E".repeat(34),
      mutate: (fixture, value) => { fixture.taxpayer.email = value; },
      selector: ".form-1601c-page-one .email-1601c > :nth-child(2)"
    },
    {
      // 17, not 24: official contract p0 candidate[6] measures 16 interior
      // dividers (x=362.11..579.34, pitch 14.48pt) between the field edge at
      // 347.83pt and the page border at 594.48pt. Probe string resized so the
      // exact/capacity+1 boundary still lands on the real capacity.
      capacity: 17,
      exact: "R".repeat(17),
      mutate: (fixture, value) => { fixture.fields.tax_relief_specification.value = value; },
      selector: ".form-1601c-page-one .tax-relief-1601c > :nth-child(4)"
    },
    {
      // 26, not 38: official contract p1 candidate[1] measures 25 interior
      // dividers (x=232.85..579.10, pitch 14.43pt) between the TIN/name
      // boundary at 218.45pt and the right border at 593.51pt. Page 1's name
      // comb is a genuinely wider 40 cells. Probe string resized to keep the
      // exact/capacity+1 boundary exact.
      capacity: 26,
      exact: "C".repeat(26),
      mutate: (fixture, value) => { fixture.taxpayer.name = value; },
      selector: ".form-1601c-page-two .identity-1601c-page-two > :nth-child(4)"
    }
  ];

  for (const boundary of profileCases) {
    await expectAdaptiveCapacityBoundary(page, minimum, boundary);
  }

  const schedule = readFixture("packages/form-contracts/fixtures/1601c-3-rows.json") as Mutable1601CEnvelope;
  for (const boundary of [
    {
      capacity: 5,
      exact: "BANK1",
      mutate: (fixture: Mutable1601CEnvelope, value: string) => {
        fixture.schedules[0].rows[0].cells.drawee_bank_code_or_agency.value = value;
      },
      selector: '.schedule-top-row-1601c[data-row-key="schedule-1-1"] > :nth-child(4)'
    },
    {
      capacity: 6,
      exact: "PAY001",
      mutate: (fixture: Mutable1601CEnvelope, value: string) => {
        fixture.schedules[0].rows[0].cells.payment_number.value = value;
      },
      selector: '.schedule-top-row-1601c[data-row-key="schedule-1-1"] > :nth-child(5)'
    }
  ]) {
    await expectAdaptiveCapacityBoundary(page, schedule, boundary);
  }
});

test("1601C 2018 fits long plain values by measured field width without clipping", async ({ page }) => {
  const fixture = readFixture("packages/form-contracts/fixtures/1601c-long-values.json");
  await renderEnvelope(page, fixture);
  await page.waitForFunction(() => [...document.querySelectorAll<HTMLElement>(".form-document[data-form-code='1601C'] .adaptive-plain-value")]
    .every((element) => element.dataset.adaptiveFitState !== "pending"));

  const fitReport = await page.locator(".form-document[data-form-code='1601C'] .adaptive-plain-value").evaluateAll(
    (elements) => elements.map((element) => ({
      className: element.className,
      fontSize: Number.parseFloat(getComputedStyle(element).fontSize),
      state: (element as HTMLElement).dataset.adaptiveFitState,
      text: element.textContent
    }))
  );
  expect(fitReport.length).toBeGreaterThan(4);
  expect(fitReport.filter((entry) => entry.state !== "fit")).toEqual([]);
  expect(fitReport.filter((entry) => entry.fontSize < 8 || entry.fontSize > 9.6)).toEqual([]);

  const pages = page.locator(".form-page");
  expect(await pageHasNoOverflow(pages.nth(0))).toBe(true);
  expect(await pageHasNoOverflow(pages.nth(1))).toBe(true);
});

test("1601C 2018 matches the complete pinned official pages", async ({ page }, testInfo) => {
  const fixture = readFixture("packages/form-contracts/fixtures/1601c-normal.json");
  await renderEnvelope(page, fixture);
  const pages = page.locator(".form-page");
  await expect(pages).toHaveCount(2);

  await expectCriticalRegionGeometry(pages.nth(0), [
    { name: "government header", selector: ".government-header-1601c", x: 35, y: 21, width: 1155, height: 58 },
    { name: "masthead", selector: ".masthead-1601c", x: 35, y: 79, width: 1155, height: 113 },
    { name: "Items 1-5", selector: ".header-options-1601c", x: 35, y: 192, width: 1155, height: 62 },
    { name: "Part I", selector: ".background-1601c", x: 35, y: 262, width: 1155, height: 299 },
    { name: "Part II", selector: ".computation-1601c", x: 35, y: 566, width: 1155, height: 774 },
    { name: "declaration", selector: ".declaration-1601c", x: 35, y: 1340, width: 1155, height: 179 },
    { name: "Part III", selector: ".payment-1601c", x: 35, y: 1524, width: 1155, height: 306 }
  ]);
  await expectCriticalRegionGeometry(pages.nth(1), [
    { name: "masthead", selector: ".masthead-1601c", x: 35, y: 49, width: 1153, height: 114 },
    { name: "identity", selector: ".identity-1601c-page-two", x: 35, y: 163, width: 1153, height: 55 },
    { name: "Schedule I", selector: ".schedule-1601c", x: 35, y: 222, width: 1153, height: 381 },
    { name: "guidelines", selector: ".guidelines-1601c", x: 35, y: 609, width: 1153, height: 1050 },
    { name: "guideline columns", selector: ".guideline-columns-1601c", x: 35, y: 680, width: 1153, height: 976 }
  ]);
  await expectGuidelineTypographyAndContent(pages.nth(1));

  await page.addStyleTag({
    content: `
      .form-page[data-visual-blank-values="true"] .comb-value > span,
      .form-page[data-visual-blank-values="true"] .adaptive-plain-value,
      .form-page[data-visual-blank-values="true"] .check-box,
      .form-page[data-visual-blank-values="true"] .inline-description-1601c {
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
  for (let index = 0; index < 2; index += 1) {
    const referencePath = path.join(
      REPO_ROOT,
      // Compares against the SAME-RASTERIZER chromium reference, not the Poppler
      // raster. The Poppler comparison carries a cross-rasterizer noise floor - 1.55%
      // to 4.54% depending on the form - which flatters every number by an amount
      // that has nothing to do with this renderer. The chromium reference is built
      // from the same pinned PDF through pdftocairo and rasterized by this exact
      // Chromium build, so the difference it reports is ours. Expect the number to
      // RISE on this change: that is the floor being removed, not a regression.
      `packages/form-renderer/references/1601c-2018-page-${index + 1}-chromium.png`
    );
    const expectedBuffer = fs.readFileSync(referencePath);
    const actualBuffer = await pages.nth(index).screenshot({ animations: "disabled", caret: "hide" });
    const expected = PNG.sync.read(expectedBuffer);
    const actual = PNG.sync.read(actualBuffer);
    expect(actual.width).toBe(expected.width);
    expect(actual.height).toBe(expected.height);
    const { changedPixels, diff } = compareOfficialStructure(expected, actual);
    const completePage = compareCompleteOfficialPage(expected, actual);
    results.push({
      page: index + 1,
      structuralChangedPercent: changedPixels * 100 / (expected.width * expected.height),
      fullPageChangedPercent: completePage.fullPageChangedPercent,
      expectedInkMissingPercent: completePage.expectedInkMissingPercent,
      unexpectedActualInkPercent: completePage.unexpectedActualInkPercent
    });
    fs.writeFileSync(
      testInfo.outputPath(`1601c-page-${index + 1}-actual.png`),
      actualBuffer
    );
    fs.writeFileSync(
      testInfo.outputPath(`1601c-page-${index + 1}-structure-diff.png`),
      PNG.sync.write(diff)
    );
    fs.writeFileSync(
      testInfo.outputPath(`1601c-page-${index + 1}-full-page-diff.png`),
      PNG.sync.write(completePage.diff)
    );
  }
  console.log(`1601C complete-page parity: ${JSON.stringify(results)}`);
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

interface Mutable1601CEnvelope {
  taxpayer: {
    contact_number: string;
    email: string;
    name: string;
    registered_address: string;
  };
  fields: Record<string, {
    type: string;
    value: string | number | boolean;
  }>;
  schedules: Array<{
    rows: Array<{
      cells: Record<string, {
        type: string;
        value: string | number | boolean;
      }>;
    }>;
  }>;
}

async function expectAdaptiveCapacityBoundary(
  page: Page,
  source: Mutable1601CEnvelope,
  boundary: {
    capacity: number;
    exact: string;
    mutate: (fixture: Mutable1601CEnvelope, value: string) => void;
    selector: string;
  }
) {
  const exactFixture = structuredClone(source);
  boundary.mutate(exactFixture, boundary.exact);
  await renderEnvelope(page, exactFixture);
  const guided = page.locator(boundary.selector);
  await expect(guided, `${boundary.selector} at capacity`).toHaveClass(/comb-value/);
  await expect(guided.locator(":scope > span"), `${boundary.selector} comb cells`).toHaveCount(boundary.capacity);

  const overflowValue = `${boundary.exact}X`;
  const overflowFixture = structuredClone(source);
  boundary.mutate(overflowFixture, overflowValue);
  await renderEnvelope(page, overflowFixture);
  const plain = page.locator(boundary.selector);
  await expect(plain, `${boundary.selector} above capacity`).toHaveClass(/adaptive-plain-value/);
  await expect(plain).toHaveAttribute("data-cell-capacity", String(boundary.capacity));
  await expect(plain).toHaveAttribute("data-overflow-mode", "plain");
  await expect(plain).toHaveAttribute("aria-label", overflowValue);
  await expect(plain).toHaveAttribute("data-adaptive-fit-state", "fit");
}

async function reviewedStaticText(locator: Locator) {
  await expect(locator).toHaveCount(1);
  return locator.evaluate((element) => {
    const dynamicValues = [...element.querySelectorAll<HTMLElement>([
      ".comb-value",
      ".adaptive-plain-value",
      ".check-box",
      ".money-1601c",
      ".inline-description-1601c",
      ".schedule-top-row-1601c",
      ".schedule-bottom-row-1601c"
    ].join(", "))];
    const priorDisplays = dynamicValues.map((dynamicValue) => dynamicValue.style.display);
    dynamicValues.forEach((dynamicValue) => { dynamicValue.style.display = "none"; });
    const visibleText = (element as HTMLElement).innerText.replace(/\s+/g, " ").trim();
    dynamicValues.forEach((dynamicValue, index) => { dynamicValue.style.display = priorDisplays[index] ?? ""; });
    return visibleText;
  });
}

async function directCombCapacities(container: Locator): Promise<number[]> {
  return container.locator(":scope > .comb-value").evaluateAll(
    (combs) => combs.map((comb) => comb.children.length)
  );
}

async function expectGuidelineTypographyAndContent(pageTwo: Locator) {
  const columns = pageTwo.locator(".guideline-columns-1601c");
  const typography = await columns.evaluate((element) => {
    const style = getComputedStyle(element);
    return {
      fontFamily: style.fontFamily,
      fontSize: Number.parseFloat(style.fontSize),
      lineHeight: Number.parseFloat(style.lineHeight),
      textAlign: style.textAlign
    };
  });
  expect(typography.fontFamily).toContain("Times New Roman");
  expect(typography.fontSize).toBeCloseTo(8, 2);
  expect(typography.lineHeight).toBeCloseTo(7.84 * 4 / 3, 2);
  expect(typography.textAlign).toBe("justify");

  for (const officialHeading of [
    "Who Shall File",
    "When and Where to File and Pay/Remit",
    "Penalties",
    "Violation of Withholding Tax Provisions",
    "Required Attachments:"
  ]) {
    await expect(columns).toContainText(officialHeading);
  }
}

async function expectGrayBackground(locator: Locator) {
  expect(await locator.evaluate((element) => getComputedStyle(element).backgroundColor))
    .toBe("rgb(217, 217, 217)");
}

async function expectWhiteBackground(locator: Locator) {
  expect(await locator.evaluate((element) => getComputedStyle(element).backgroundColor))
    .toBe("rgb(255, 255, 255)");
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
    const pageIndex = [...document.querySelectorAll(".form-page")].indexOf(element);
    return measurement.pages[pageIndex];
  });
  const valid = report.scroll_height <= report.client_height + 1 &&
    report.scroll_width <= report.client_width + 1 &&
    report.descendant_overflow_x === 0 &&
    report.descendant_overflow_y === 0 &&
    report.descendant_clipped_x === 0 &&
    report.descendant_clipped_y === 0;
  if (!valid) {
    const offenders = await locator.evaluate((element) =>
      [...element.querySelectorAll<HTMLElement>("*")]
        .filter((child) => child.scrollWidth > child.clientWidth + 1.25 || child.scrollHeight > child.clientHeight + 1.25)
        .map((child) => ({
          class_name: child.className,
          client_width: child.clientWidth,
          scroll_width: child.scrollWidth,
          client_height: child.clientHeight,
          scroll_height: child.scrollHeight,
          text: child.textContent?.trim().slice(0, 80)
        }))
    );
    console.warn(`1601C overflow report: ${JSON.stringify({ report, offenders })}`);
  }
  return valid;
}

function readFixture(relativePath: string): unknown {
  return JSON.parse(fs.readFileSync(path.join(REPO_ROOT, relativePath), "utf8")) as unknown;
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
        if (x - start >= STRUCTURAL_LINE_MIN_RUN) {
          for (let fill = start; fill < x; fill += 1) lines[y * image.width + fill] = 1;
        }
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
        if (y - start >= STRUCTURAL_LINE_MIN_RUN) {
          for (let fill = start; fill < y; fill += 1) lines[fill * image.width + x] = 1;
        }
        start = -1;
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
  height: number
) {
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
