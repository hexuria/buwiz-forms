import { expect, test, type Locator, type Page } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { PNG } from "pngjs";
import { compareCompleteOfficialPage } from "./official-page-diff";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(HERE, "../../..");
const staticTextInventory = JSON.parse(fs.readFileSync(path.join(
  REPO_ROOT,
  "packages/form-renderer/references/1701-2018-static-text-inventory.json"
), "utf8")) as {
  coverage_status: string;
  known_gaps: string[];
  official_source_sha256: string;
  regions: Array<{ selector: string; text: string }>;
};
const MAX_CHANGED_PERCENT = 1;
const DEVICE_SCALE_FACTOR = 1.5;
const STRUCTURAL_INK_THRESHOLD = 100;
const STRUCTURAL_LINE_MIN_RUN = 20;
const STRUCTURAL_TOLERANCE_RADIUS = 4;

test("1701 2018 renders every Rust fixture as four stable unclipped 612x936 pages", async ({ page }) => {
  for (const fixtureName of [
    "1701-minimum.json",
    "1701-normal.json",
    "1701-long-values.json",
    "1701-validation-edge.json",
    "1701-fixed-capacity.json"
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

test("1701 2018 locks every currently represented reviewed static-copy region without hiding known gaps", async ({ page }) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/1701-minimum.json"));

  expect(staticTextInventory.official_source_sha256).toBe(
    "19be91d78258eb7c255f2615610db2739f10c378f8ac97adc0887c1bf40d1b2e"
  );
  expect(staticTextInventory.coverage_status).toBe("partial_reviewed");
  expect(staticTextInventory.known_gaps).toHaveLength(8);
  for (const region of staticTextInventory.regions) {
    const visibleText = await reviewedStaticText(page.locator(region.selector));
    expect(visibleText, region.selector).toBe(region.text);
  }
});

test("1701 2018 uses only the reviewed guided capacities and official plain cells", async ({ page }) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/1701-minimum.json"));

  const firstPage = page.locator(".page-one-1701");
  for (const [item, capacity] of [
    ["8", 40], ["9", 71], ["10", 8], ["11", 32],
    ["12", 16], ["14", 16], ["15", 13]
  ] as const) {
    const locator = firstPage.locator(`.labeled-comb-1701[data-item-number="${item}"]`);
    await expect(locator, `page 1 item ${item}`).toHaveAttribute("data-field-mode", "guided");
    await expect(locator, `page 1 item ${item}`).toHaveAttribute("data-cell-capacity", String(capacity));
  }
  await expect(firstPage.locator('.labeled-comb-1701[data-item-number="8"] > .comb-value > span')).toHaveCount(40);
  await expect(firstPage.locator('.labeled-comb-1701[data-item-number="9"] > .comb-value > span')).toHaveCount(40);
  await expect(firstPage.locator('.labeled-comb-1701[data-item-number="9"] .address-second-1701 > .comb-value > span')).toHaveCount(31);
  await expect(firstPage.locator('.labeled-comb-1701[data-item-number="9"] .address-second-1701 > span .comb-value > span')).toHaveCount(4);

  const secondPage = page.locator(".page-two-1701");
  for (const [item, capacity] of [
    ["5", 40], ["6", 11], ["7", 19], ["9", 17]
  ] as const) {
    const locator = secondPage.locator(`.labeled-comb-1701[data-item-number="${item}"]`);
    await expect(locator, `page 2 item ${item}`).toHaveAttribute("data-field-mode", "guided");
    await expect(locator, `page 2 item ${item}`).toHaveAttribute("data-cell-capacity", String(capacity));
    await expect(locator.locator(":scope > .comb-value > span"), `page 2 item ${item}`).toHaveCount(capacity);
  }

  for (const tin of await page.locator(".tin-value-1701").all()) {
    expect(await tin.locator(":scope > .comb-value").evaluateAll((groups) =>
      groups.map((group) => group.children.length)
    )).toEqual([3, 3, 3, 5]);
  }
  for (const continuationName of await page.locator('[data-field-name="taxpayer_continuation_name"]').all()) {
    await expect(continuationName).toHaveAttribute("data-field-mode", "guided");
    await expect(continuationName).toHaveAttribute("data-cell-capacity", "26");
    await expect(continuationName.locator(":scope > .comb-value > span")).toHaveCount(26);
  }

  for (const [fieldKey, capacity] of [
    ["payment_34_bank", 6], ["payment_34_number", 10], ["payment_34_date", 8],
    ["payment_35_bank", 6], ["payment_35_number", 10], ["payment_35_date", 8],
    ["payment_36_number", 10], ["payment_36_date", 8],
    ["payment_37_description", 7], ["payment_37_bank", 6],
    ["payment_37_number", 10], ["payment_37_date", 8]
  ] as const) {
    const locator = page.locator(`.guided-field-1701[data-field-key="${fieldKey}"]`);
    await expect(locator, fieldKey).toHaveAttribute("data-field-mode", "guided");
    await expect(locator, fieldKey).toHaveAttribute("data-cell-capacity", String(capacity));
    await expect(locator.locator(":scope > .comb-value > span"), fieldKey).toHaveCount(capacity);
  }
  await expect(page.locator('[data-field-key="payment_36_bank"]')).toHaveCount(0);

  for (const plainField of await page.locator(".nolco-plain-value-1701, .plain-amount-1701, .row-description-1701, .inline-description-1701").all()) {
    await expect(plainField).toHaveAttribute("data-field-mode", "plain");
    await expect(plainField.locator(".comb-value"), "invented comb on reviewed plain field").toHaveCount(0);
  }
});

test("1701 2018 switches at capacity plus one without dropping valid characters", async ({ page }) => {
  const fixture = readFixture("packages/form-contracts/fixtures/1701-minimum.json") as MutableEnvelope;

  fixture.taxpayer.name = "N".repeat(26);
  fixture.taxpayer.registered_address = "R".repeat(71);
  fixture.taxpayer.email = "E".repeat(32);
  fixture.fields.payment_34_bank = { type: "text", value: "A B.C!" };
  await renderEnvelope(page, fixture);
  await expect(page.locator('.page-one-1701 [data-item-number="11"]')).toHaveAttribute("data-field-mode", "guided");
  await expect(page.locator('.page-one-1701 [data-item-number="11"] > .comb-value > span')).toHaveCount(32);
  await expect(page.locator('.page-one-1701 [data-item-number="9"]')).toHaveAttribute("data-field-mode", "guided");
  await expect(page.locator('.page-two-1701 [data-field-name="taxpayer_continuation_name"]')).toHaveAttribute("data-field-mode", "guided");
  await expect(page.locator('.page-two-1701 [data-field-name="taxpayer_continuation_name"] > .comb-value > span')).toHaveCount(26);
  await expect(page.locator('[data-field-key="payment_34_bank"]')).toHaveAttribute("data-field-mode", "guided");
  await expect(page.locator('[data-field-key="payment_34_bank"] > .comb-value > span')).toHaveCount(6);

  fixture.taxpayer.name += "N";
  fixture.taxpayer.registered_address += "R";
  fixture.taxpayer.email += "E";
  fixture.fields.payment_34_bank = { type: "text", value: "A B.C!D" };
  await renderEnvelope(page, fixture);
  const email = page.locator('.page-one-1701 [data-item-number="11"]');
  await expect(email).toHaveAttribute("data-field-mode", "plain");
  await expect(email.locator(":scope > .comb-value")).toHaveCount(0);
  await expect(email.locator(":scope > .adaptive-plain-value")).toHaveText(fixture.taxpayer.email);
  const address = page.locator('.page-one-1701 [data-item-number="9"]');
  await expect(address).toHaveAttribute("data-field-mode", "plain");
  await expect(address.locator(":scope > .comb-value")).toHaveCount(0);
  await expect(address.locator(":scope > .adaptive-plain-value")).toHaveText(fixture.taxpayer.registered_address);
  const continuationName = page.locator('.page-two-1701 [data-field-name="taxpayer_continuation_name"]');
  await expect(continuationName).toHaveAttribute("data-field-mode", "plain");
  await expect(continuationName.locator(":scope > .comb-value")).toHaveCount(0);
  await expect(continuationName.locator(":scope > .adaptive-plain-value")).toHaveText(fixture.taxpayer.name);
  const paymentBank = page.locator('[data-field-key="payment_34_bank"]');
  await expect(paymentBank).toHaveAttribute("data-field-mode", "plain");
  await expect(paymentBank.locator(":scope > .comb-value")).toHaveCount(0);
  await expect(paymentBank.locator(":scope > .adaptive-plain-value")).toHaveText("A B.C!D");

  fixture.taxpayer.name = "T".repeat(41);
  await renderEnvelope(page, fixture);
  const primaryName = page.locator('.page-one-1701 [data-item-number="8"]');
  await expect(primaryName).toHaveAttribute("data-field-mode", "plain");
  await expect(primaryName.locator(":scope > .comb-value")).toHaveCount(0);
  await expect(primaryName.locator(":scope > .adaptive-plain-value")).toHaveText(fixture.taxpayer.name);

  const longFixture = readFixture("packages/form-contracts/fixtures/1701-long-values.json") as MutableEnvelope;
  await renderEnvelope(page, longFixture);
  await expect(page.locator('.page-one-1701 [data-item-number="8"] > .adaptive-plain-value')).toHaveText(longFixture.taxpayer.name);
  await expect(page.locator('.page-one-1701 [data-item-number="9"] > .adaptive-plain-value')).toHaveText(longFixture.taxpayer.registered_address);
  await expect(page.locator('.page-one-1701 [data-item-number="11"] > .adaptive-plain-value')).toHaveText(longFixture.taxpayer.email);
});

test("1701 2018 keeps verified page-specific PDF417, caption, and seal geometry", async ({ page }) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/1701-normal.json"));
  const pages = page.locator(".form-page");
  await expect(pages).toHaveCount(4);

  await expectCriticalRegionGeometry(pages.nth(0), [
    {
      name: "official seal XObject",
      selector: ".government-wordmark-1701 img",
      x: 480.32,
      y: 28.176,
      width: 67.364,
      height: 56.304
    },
    {
      name: "page 1 official PDF417 active matrix",
      selector: '.barcode-1701[data-barcode-page="1"] .official-pdf417-object-1701',
      x: 877.68,
      y: 98.88,
      width: 316.8,
      height: 82.8
    },
    {
      name: "page 1 official PDF417 live caption",
      selector: '.barcode-1701[data-barcode-page="1"] > small',
      x: 1044.92,
      y: 180.8476,
      width: 148.86044,
      height: 17.9292
    },
    {
      name: "page 1 Item 1 taxable-year guide",
      selector: ".header-options-1701 > div:first-child .comb-value",
      x: 256,
      y: 201,
      width: 110,
      height: 37
    }
  ]);

  const continuationRegions = [
    {
      page: 2,
      matrix: { x: 872.64, y: 42.24, width: 323.28, height: 70.8 },
      caption: { x: 1044.68, y: 113.6076, width: 148.86044, height: 17.9292 }
    },
    {
      page: 3,
      matrix: { x: 867.6, y: 42.96, width: 326.4, height: 71.76 },
      caption: { x: 1045.16, y: 113.8476, width: 148.79688, height: 17.9292 }
    },
    {
      page: 4,
      matrix: { x: 870.24, y: 42.24, width: 323.52, height: 68.64 },
      caption: { x: 1044.68, y: 110.4876, width: 148.79688, height: 17.9292 }
    }
  ] as const;
  for (const region of continuationRegions) {
    await expectCriticalRegionGeometry(pages.nth(region.page - 1), [
      {
        name: `page ${region.page} official PDF417 active matrix`,
        selector: `.barcode-1701[data-barcode-page="${region.page}"] .official-pdf417-object-1701`,
        ...region.matrix
      },
      {
        name: `page ${region.page} official PDF417 live caption`,
        selector: `.barcode-1701[data-barcode-page="${region.page}"] > small`,
        ...region.caption
      }
    ]);
  }

  expect(await page.locator(".official-pdf417-symbol-1701").evaluateAll(
    (symbols) => symbols.map((symbol) => ({
      preserveAspectRatio: symbol.getAttribute("preserveAspectRatio"),
      shapeRendering: getComputedStyle(symbol).shapeRendering,
      viewBox: symbol.getAttribute("viewBox")
    }))
  )).toEqual(Array.from({ length: 4 }, () => ({
    preserveAspectRatio: "none",
    shapeRendering: "crispedges",
    viewBox: "0 0 120 7"
  })));

  const captions = page.locator(".barcode-1701 > small");
  for (let pageNumber = 1; pageNumber <= 4; pageNumber += 1) {
    await expect(captions.nth(pageNumber - 1)).toHaveText(`1701 01/18ENCS P${pageNumber}`);
  }
  expect(await captions.evaluateAll((elements) => elements.map((element) => {
    const style = getComputedStyle(element);
    return {
      fontFamily: style.fontFamily,
      fontSize: style.fontSize,
      fontWeight: style.fontWeight,
      whiteSpace: style.whiteSpace
    };
  }))).toEqual(Array.from({ length: 4 }, () => ({
    fontFamily: '"eBIRForms Arimo", Arial, sans-serif',
    fontSize: "10.72px",
    fontWeight: "400",
    whiteSpace: "nowrap"
  })));

  expect(await page.locator(".government-wordmark-1701 img").evaluate(
    (image) => ({
      naturalHeight: (image as HTMLImageElement).naturalHeight,
      naturalWidth: (image as HTMLImageElement).naturalWidth
    })
  )).toEqual({ naturalHeight: 102, naturalWidth: 119 });
});

test("1701 2018 keeps the official Item 21 and 21A choice hierarchy", async ({ page }) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/1701-normal.json"));
  const firstPage = page.locator(".form-page").nth(0);

  await expectCriticalRegionGeometry(firstPage, [
    { name: "Item 21 graduated box", selector: ".taxpayer-election-1701 .graduated-choice-1701 .check-box", x: 124, y: 792, width: 28, height: 25 },
    { name: "Item 21A itemized box", selector: ".taxpayer-election-1701 .itemized-choice-1701 .check-box", x: 456, y: 793, width: 28, height: 25 },
    { name: "Item 21A OSD box", selector: ".taxpayer-election-1701 .osd-choice-1701 .check-box", x: 719, y: 793, width: 28, height: 25 },
    { name: "Item 21 eight-percent box", selector: ".taxpayer-election-1701 .eight-percent-choice-1701 .check-box", x: 124, y: 836, width: 28, height: 24 }
  ]);

  await expect(firstPage.locator(".taxpayer-election-1701 .deduction-label-1701")).toHaveText("21A Method of Deduction (choose one)");
  await expect(firstPage.locator(".taxpayer-election-1701 .graduated-choice-1701 > small")).toHaveText("(Choose Method of Deduction in Item 21A)");
  await expect(firstPage.locator(".taxpayer-election-1701 .itemized-choice-1701 > small")).toHaveText("[Sec. 34(A-J), NIRC]");
  await expect(firstPage.locator(".taxpayer-election-1701 .osd-choice-1701 > small")).toHaveText("[40% of Gross Sales/Receipts/Revenues/Fees [Sec. 34(L), NIRC]]");
  await expect(firstPage.locator(".taxpayer-election-1701 .eight-percent-choice-1701 > small")).toHaveText("(available if gross sales/receipts and other non-operating income do not exceed Three million pesos (P3M))");

  const spouseElection = page.locator(".spouse-election-1701");
  await expect(spouseElection.locator(".deduction-label-1701")).toHaveText("12A Method of Deduction (choose one)");
  await expect(spouseElection.locator(".graduated-choice-1701 > small")).toHaveText("(Choose Method of Deduction in Item 12A)");
  await expect(spouseElection.locator(".itemized-choice-1701 > small")).toHaveText("[Sec. 34(A-J), NIRC]");
  await expect(spouseElection.locator(".osd-choice-1701 > small")).toHaveText("[40% of Gross Sales/Receipts/Revenues/Fees [Sec. 34(L), NIRC]]");
  await expect(spouseElection.locator(".eight-percent-choice-1701 > small")).toHaveText("(available if gross sales/receipts and other non-operating income do not exceed Three million pesos (P3M))");
});

test("1701 2018 keeps the official Schedule 3.A row-group bands", async ({ page }) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/1701-normal.json"));
  const secondPage = page.locator(".form-page").nth(1);

  await expectCriticalRegionGeometry(secondPage, [
    { name: "Schedule 3.A deductions band", selector: ".deductions-subtitle-1701", x: 28, y: 1366, width: 1165, height: 19 },
    { name: "Schedule 3.A OR band", selector: ".or-subtitle-1701", x: 28, y: 1513, width: 1165, height: 19 },
    { name: "Schedule 3.A other-income band", selector: ".other-income-subtitle-1701", x: 28, y: 1596, width: 1165, height: 19 }
  ]);
  await expect(secondPage.locator(".schedule-three-a-1701 > .paired-head-1701")).toHaveCount(0);
});

test("1701 2018 keeps the official page-two spouse row partitions", async ({ page }) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/1701-normal.json"));
  const secondPage = page.locator(".form-page").nth(1);

  await expectCriticalRegionGeometry(secondPage, [
    { name: "spouse TIN and RDO row", selector: ".spouse-background-1701 .tin-rdo-1701", x: 29.5, y: 213.5, width: 1165, height: 36 },
    { name: "spouse type row", selector: '.spouse-background-1701 > [data-item-number="3"]', x: 29.5, y: 249.5, width: 1165, height: 29 },
    { name: "spouse ATC row", selector: ".spouse-background-1701 .atc-choices-1701", x: 29.5, y: 278.5, width: 1165, height: 61 },
    { name: "spouse name row", selector: '.spouse-background-1701 > [data-item-number="5"]', x: 29.5, y: 339.5, width: 1165, height: 51 },
    { name: "spouse contact and citizenship row", selector: ".spouse-contact-1701", x: 29.5, y: 390.5, width: 1165, height: 29 },
    { name: "spouse foreign-credit row", selector: ".spouse-credit-1701", x: 29.5, y: 419.5, width: 1165, height: 37 },
    { name: "spouse exempt and special-rate row", selector: ".spouse-background-1701 > .two-choice-row-1701", x: 29.5, y: 456.5, width: 1165, height: 37 },
    { name: "spouse tax-election row", selector: '.spouse-background-1701 > [data-item-number="12"]', x: 29.5, y: 493.5, width: 1165, height: 109 }
  ]);
});

test("1701 2018 matches the complete official pages", async ({ page }, testInfo) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/1701-normal.json"));
  const pages = page.locator(".form-page");
  await expect(pages).toHaveCount(4);

  await page.addStyleTag({ content: `
    .form-page[data-visual-blank-values="true"] :is(
      .comb-value > span,
      .adaptive-plain-value,
      .check-box,
      .amount-1701
    ) { color: transparent !important; text-shadow: none !important; }
  ` });

  const pageResults: Array<{
    page: number;
    structuralChangedPercent: number;
    fullPageChangedPercent: number;
    expectedInkMissingPercent: number;
    unexpectedActualInkPercent: number;
  }> = [];
  console.log("1701 spouse regions", await pages.nth(1).locator(".spouse-background-1701 > *").evaluateAll((children) => children.map((child) => ({ class_name: child.className, height: child.getBoundingClientRect().height }))));
  for (let pageIndex = 0; pageIndex < 4; pageIndex += 1) {
    const renderedPage = pages.nth(pageIndex);
    console.log(`1701 page ${pageIndex + 1} regions`, await renderedPage.locator(":scope > *").evaluateAll((children) => children.map((child) => {
      const rect = child.getBoundingClientRect();
      const parent = child.parentElement?.getBoundingClientRect();
      return { class_name: child.className, top: rect.top - (parent?.top ?? 0), height: rect.height };
    })));
    await renderedPage.evaluate((element) => element.setAttribute("data-visual-blank-values", "true"));
    const referencePath = path.join(REPO_ROOT, `packages/form-renderer/references/1701-2018-page-${pageIndex + 1}.png`);
    const expectedBuffer = fs.readFileSync(referencePath);
    const actualBuffer = await renderedPage.screenshot({ animations: "disabled", caret: "hide" });
    const expected = PNG.sync.read(expectedBuffer);
    const actual = PNG.sync.read(actualBuffer);
    expect(actual.width).toBe(expected.width);
    expect(actual.height).toBe(expected.height);
    const { changedPixels, diff } = compareOfficialStructure(expected, actual);
    const completePage = compareCompleteOfficialPage(expected, actual);
    fs.writeFileSync(testInfo.outputPath(`1701-page-${pageIndex + 1}-actual.png`), actualBuffer);
    fs.writeFileSync(testInfo.outputPath(`1701-page-${pageIndex + 1}-structure-diff.png`), PNG.sync.write(diff));
    fs.writeFileSync(
      testInfo.outputPath(`1701-page-${pageIndex + 1}-full-page-diff.png`),
      PNG.sync.write(completePage.diff)
    );
    pageResults.push({
      page: pageIndex + 1,
      structuralChangedPercent: changedPixels * 100 / (expected.width * expected.height),
      fullPageChangedPercent: completePage.fullPageChangedPercent,
      expectedInkMissingPercent: completePage.expectedInkMissingPercent,
      unexpectedActualInkPercent: completePage.unexpectedActualInkPercent
    });
  }
  console.log(`1701 complete-page parity: ${JSON.stringify(pageResults)}`);
  for (const result of pageResults) {
    expect(
      result.fullPageChangedPercent,
      `1701 page ${result.page} complete pixels, including all static labels, instructions, fields, signatures, and artwork`
    ).toBeLessThanOrEqual(MAX_CHANGED_PERCENT);
  }
});

async function pageHasNoOverflow(locator: Locator) {
  const report = await locator.evaluate((element) => {
    const measurement = (window as Window & { measureEbirFormGeometry?: () => { pages: Array<{ client_height: number; client_width: number; descendant_clipped_x: number; descendant_clipped_y: number; descendant_overflow_x: number; descendant_overflow_y: number; scroll_height: number; scroll_width: number; }> } | null }).measureEbirFormGeometry?.();
    if (!measurement) throw new Error("renderer measurement unavailable");
    return measurement.pages[[...document.querySelectorAll(".form-page")].indexOf(element)];
  });
  const valid = report.scroll_height <= report.client_height + 1 &&
    report.scroll_width <= report.client_width + 1 &&
    report.descendant_overflow_x === 0 && report.descendant_overflow_y === 0 &&
    report.descendant_clipped_x === 0 && report.descendant_clipped_y === 0;
  if (!valid) {
    const details = await locator.evaluate((element) => ({
      offenders: [...element.querySelectorAll<HTMLElement>("*")]
        .filter((child) => child.scrollWidth > child.clientWidth + 1.25 || child.scrollHeight > child.clientHeight + 1.25)
        .map((child) => ({ class_name: child.className, client_width: child.clientWidth, scroll_width: child.scrollWidth, client_height: child.clientHeight, scroll_height: child.scrollHeight, text: child.textContent?.trim().slice(0, 100) })),
      direct_children: [...element.children].map((child) => ({ class_name: child.className, height: child.getBoundingClientRect().height })),
      background_children: [...(element.querySelector(".background-1701")?.children ?? [])]
        .map((child) => ({ class_name: child.className, height: child.getBoundingClientRect().height }))
    }));
    console.warn(`1701 overflow report: ${JSON.stringify({ report, ...details })}`);
  }
  return valid;
}

interface MutableEnvelope {
  taxpayer: {
    email: string;
    name: string;
    registered_address: string;
  };
  fields: Record<string, {
    type: string;
    value: string | number | boolean;
  }>;
}

async function reviewedStaticText(locator: Locator) {
  await expect(locator).toHaveCount(1);
  return locator.evaluate((element) => {
    const dynamicValues = [...element.querySelectorAll<HTMLElement>([
      ".comb-value",
      ".adaptive-plain-value",
      ".check-box",
      ".amount-1701",
      ".guided-field-1701",
      ".row-description-1701",
      ".inline-description-1701",
      "[data-field-key=\"machine_validation_or_receipt_details\"]"
    ].join(", "))];
    const priorDisplays = dynamicValues.map((dynamicValue) => dynamicValue.style.display);
    dynamicValues.forEach((dynamicValue) => { dynamicValue.style.display = "none"; });
    const visibleText = (element as HTMLElement).innerText.replace(/\s+/g, " ").trim();
    dynamicValues.forEach((dynamicValue, index) => { dynamicValue.style.display = priorDisplays[index] ?? ""; });
    return visibleText;
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

async function expectCriticalRegionGeometry(page: Locator, regions: CriticalRegion[]) {
  const pageBox = await page.boundingBox();
  expect(pageBox).not.toBeNull();
  if (!pageBox) return;
  const failures: Array<{
    region: string;
    dimension: string;
    actual: number;
    expected: number;
    difference: number;
  }> = [];
  for (const region of regions) {
    const box = await page.locator(region.selector).boundingBox();
    expect(box, region.name).not.toBeNull();
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
          actual: actual[key],
          expected: expected[key],
          difference
        });
      }
    }
  }
  expect(failures).toEqual([]);
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
    if (changed[index] === 1) { changedPixels += 1; diff.data[offset] = 255; diff.data[offset + 3] = 255; }
  }
  return { changedPixels, diff };
}

function structuralLineMask(image: PNG) {
  const dark = new Uint8Array(image.width * image.height);
  const lines = new Uint8Array(dark.length);
  for (let index = 0; index < dark.length; index += 1) {
    const offset = index * 4;
    dark[index] = image.data[offset] < STRUCTURAL_INK_THRESHOLD && image.data[offset + 1] < STRUCTURAL_INK_THRESHOLD && image.data[offset + 2] < STRUCTURAL_INK_THRESHOLD ? 1 : 0;
  }
  for (let y = 0; y < image.height; y += 1) {
    let start = -1;
    for (let x = 0; x <= image.width; x += 1) {
      const ink = x < image.width && dark[y * image.width + x] === 1;
      if (ink && start < 0) start = x;
      if (!ink && start >= 0) { if (x - start >= STRUCTURAL_LINE_MIN_RUN) for (let fill = start; fill < x; fill += 1) lines[y * image.width + fill] = 1; start = -1; }
    }
  }
  for (let x = 0; x < image.width; x += 1) {
    let start = -1;
    for (let y = 0; y <= image.height; y += 1) {
      const ink = y < image.height && dark[y * image.width + x] === 1;
      if (ink && start < 0) start = y;
      if (!ink && start >= 0) { if (y - start >= STRUCTURAL_LINE_MIN_RUN) for (let fill = start; fill < y; fill += 1) lines[fill * image.width + x] = 1; start = -1; }
    }
  }
  return lines;
}

function markUnmatchedStructure(source: Uint8Array, target: Uint8Array, changed: Uint8Array, width: number, height: number) {
  for (let y = 0; y < height; y += 1) for (let x = 0; x < width; x += 1) {
    const index = y * width + x;
    if (source[index] !== 1) continue;
    let matched = false;
    for (let ty = Math.max(0, y - STRUCTURAL_TOLERANCE_RADIUS); ty <= Math.min(height - 1, y + STRUCTURAL_TOLERANCE_RADIUS) && !matched; ty += 1) {
      for (let tx = Math.max(0, x - STRUCTURAL_TOLERANCE_RADIUS); tx <= Math.min(width - 1, x + STRUCTURAL_TOLERANCE_RADIUS); tx += 1) if (target[ty * width + tx] === 1) { matched = true; break; }
    }
    if (!matched) changed[index] = 1;
  }
}
