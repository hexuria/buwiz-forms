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
  "packages/form-renderer/references/1701q-2018-static-text-inventory.json"
), "utf8")) as {
  official_source_sha256: string;
  regions: Array<{ selector: string; text: string }>;
};
const MAX_CHANGED_PERCENT = 1;
const DEVICE_SCALE_FACTOR = 1.5;
const STRUCTURAL_INK_THRESHOLD = 100;
const STRUCTURAL_LINE_MIN_RUN = 20;
const STRUCTURAL_TOLERANCE_RADIUS = 4;

test("1701Q 2018 renders every Rust fixture as two stable unclipped Folio pages", async ({ page }) => {
  for (const fixtureName of [
    "1701q-minimum.json",
    "1701q-normal.json",
    "1701q-long-values.json",
    "1701q-validation-edge.json",
    "1701q-all-lines.json"
  ]) {
    await renderEnvelope(page, readFixture(`packages/form-contracts/fixtures/${fixtureName}`));
    const pages = page.locator(".form-page");
    await expect(pages, fixtureName).toHaveCount(2);
    for (let pageIndex = 0; pageIndex < 2; pageIndex += 1) {
      await expect(pages.nth(pageIndex)).toHaveAttribute("data-paper", "folio");
      const box = await pages.nth(pageIndex).boundingBox();
      expect(box, `${fixtureName} page ${pageIndex + 1}`).not.toBeNull();
      expect(box?.width, `${fixtureName} page ${pageIndex + 1} width`).toBeCloseTo(816, 1);
      expect(box?.height, `${fixtureName} page ${pageIndex + 1} height`).toBeCloseTo(1248, 1);
      expect(await pageHasNoOverflow(pages.nth(pageIndex)), `${fixtureName} page ${pageIndex + 1}`).toBe(true);
    }
  }
});

test("1701Q 2018 preserves every reviewed official static label", async ({ page }) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/1701q-minimum.json"));

  expect(staticTextInventory.official_source_sha256).toBe(
    "c731d3f12556e6f19ab81f6113ca7c4a23f7ed099675c03451ac0074d96b85ed"
  );
  for (const region of staticTextInventory.regions) {
    const visibleText = await reviewedStaticText(page.locator(region.selector));
    expect(visibleText, region.selector).toBe(region.text);
  }
});

test("1701Q 2018 uses only the pinned official comb capacities and plain cells", async ({ page }) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/1701q-minimum.json"));

  for (const [item, capacity] of [
    ["9", 40], ["11", 8], ["12", 32], ["13", 16], ["14", 16],
    ["21", 40], ["22", 16], ["23", 16]
  ] as const) {
    const locator = page.locator(`.label-comb-1701q[data-item-number="${item}"]`);
    await expect(locator, `Item ${item}`).toHaveAttribute("data-field-mode", "guided");
    await expect(locator, `Item ${item}`).toHaveAttribute("data-cell-capacity", String(capacity));
    await expect(locator.locator(":scope > .comb-value > span"), `Item ${item}`).toHaveCount(capacity);
  }

  for (const tin of await page.locator(".tin-1701q").all()) {
    expect(await tin.locator(":scope > .comb-value").evaluateAll((groups) =>
      groups.map((group) => group.children.length)
    )).toEqual([3, 3, 3, 5]);
  }
  await expect(page.locator(".address-1701q > .comb-value > span")).toHaveCount(40);
  // 32 (was 31): official item 10 continuation comb, box edges x=15.60/475.54pt with 31 interior
  // dividers at uniform 14.39pt pitch; the right edge is a real 0.48pt rule and the last cell
  // (461.26->475.54pt) is on-pitch, so the 32nd cell is genuine.
  await expect(page.locator(".address-second-1701q > .comb-value:first-child > span")).toHaveCount(32);
  await expect(page.locator(".address-second-1701q > .comb-value:last-child > span")).toHaveCount(4);
  await expect(page.locator(".page-two-identity-1701q > .comb-value:nth-child(3) > span")).toHaveCount(14);
  await expect(page.locator(".page-two-identity-1701q > .comb-value:nth-child(4) > span")).toHaveCount(26);

  for (const [fieldName, capacity] of [
    ["payment_32_bank", 6], ["payment_32_number", 11], ["payment_32_date", 8],
    ["payment_33_bank", 6], ["payment_33_number", 11], ["payment_33_date", 8],
    ["payment_34_number", 11], ["payment_34_date", 8],
    ["payment_35_particular", 7], ["payment_35_bank", 6], ["payment_35_number", 11], ["payment_35_date", 8]
  ] as const) {
    const locator = page.locator(`[data-field-name="${fieldName}"]`);
    await expect(locator, fieldName).toHaveAttribute("data-field-mode", "guided");
    await expect(locator, fieldName).toHaveAttribute("data-cell-capacity", String(capacity));
    await expect(locator.locator(".comb-value > span"), fieldName).toHaveCount(capacity);
  }

  const item34Bank = page.locator('[data-field-name="payment_34_bank"]');
  await expect(item34Bank).toHaveAttribute("data-field-mode", "not-applicable");
  await expect(item34Bank.locator(".comb-value")).toHaveCount(0);
  expect(await item34Bank.evaluate((element) => getComputedStyle(element).backgroundColor)).toBe("rgb(217, 217, 217)");

  for (const fieldName of ["item_43_description", "item_48_description", "item_61_description", "machine_validation_or_receipt_details"]) {
    const locator = page.locator(`[data-field-name="${fieldName}"]`);
    await expect(locator, fieldName).toHaveAttribute("data-field-mode", "plain");
    await expect(locator.locator(".comb-value"), `${fieldName} invented comb`).toHaveCount(0);
  }

  // Every money comb on the official form is 8 cells (XX,XXX,XXX), item 31 included: its comb was
  // measured from the pinned PDF contract as 0.48pt box edges x=420.91/536.86pt with 7 interior
  // dividers at uniform 14.49pt pitch and 1.44pt digit-group separators at 450.07/493.66 (after
  // cells 2 and 5). item_31 previously carried a 12-cell override.
  for (const amount of await page.locator(".amount-1701q").all()) {
    const fieldName = await amount.getAttribute("data-field-name");
    await expect(amount, fieldName ?? "amount").toHaveAttribute("data-cell-capacity", "8");
  }
});

test("1701Q 2018 switches exact-capacity overflow to plain text without dropping characters", async ({ page }) => {
  const fixture = readFixture("packages/form-contracts/fixtures/1701q-minimum.json") as MutableEnvelope;

  fixture.taxpayer.email = `${"A".repeat(30)}@B`;
  fixture.fields.taxpayer_last_name = { type: "text", value: `D${"E".repeat(24)}.` };
  fixture.fields.payment_32_bank = { type: "text", value: "A B.C!" };
  await renderEnvelope(page, fixture);
  await expect(page.locator('.label-comb-1701q[data-item-number="12"]')).toHaveAttribute("data-field-mode", "guided");
  await expect(page.locator('.label-comb-1701q[data-item-number="12"] > .comb-value > span')).toHaveCount(32);
  await expect(page.locator(".page-two-identity-1701q > .comb-value:nth-child(4) > span")).toHaveCount(26);
  await expect(page.locator('[data-field-name="payment_32_bank"] .comb-value > span')).toHaveCount(6);

  fixture.taxpayer.email += "C";
  fixture.fields.taxpayer_last_name = { type: "text", value: `D${"E".repeat(25)}.` };
  fixture.fields.payment_32_bank = { type: "text", value: "A B.C!D" };
  await renderEnvelope(page, fixture);
  const email = page.locator('.label-comb-1701q[data-item-number="12"]');
  await expect(email).toHaveAttribute("data-field-mode", "plain");
  await expect(email.locator(":scope > .comb-value")).toHaveCount(0);
  await expect(email.locator(":scope > .adaptive-plain-value")).toHaveText(fixture.taxpayer.email);
  await expect(page.locator(".page-two-identity-1701q > .adaptive-plain-value:nth-child(4)")).toHaveText(
    String(fixture.fields.taxpayer_last_name.value)
  );
  const paymentBank = page.locator('[data-field-name="payment_32_bank"]');
  await expect(paymentBank).toHaveAttribute("data-field-mode", "plain");
  await expect(paymentBank.locator(".comb-value")).toHaveCount(0);
  await expect(paymentBank.locator(".adaptive-plain-value")).toHaveText("A B.C!D");

  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/1701q-long-values.json"));
  for (const locator of [
    page.locator('.label-comb-1701q[data-item-number="9"]'),
    page.locator('.label-comb-1701q[data-item-number="12"]'),
    page.locator('[data-field-name="payment_32_bank"]')
  ]) {
    await expect(locator).toHaveAttribute("data-field-mode", "plain");
    await expect(locator.locator(".comb-value")).toHaveCount(0);
  }
  await expect(page.locator(".address-1701q > .adaptive-plain-value")).toHaveText(
    "UNIT 1201, A DELIBERATELY LONG REGISTERED ADDRESS USED TO PROVE THAT THE HTML FORM PRESERVES EVERY VALID CHARACTER"
  );
  await expect(page.locator(".page-two-identity-1701q > .adaptive-plain-value:nth-child(4)")).toHaveText(
    "DELA CRUZ-SANTOS WITH A VALID LAST NAME LONGER THAN THE OFFICIAL COMB"
  );
});

test("1701Q 2018 all-lines fixture renders every official computation row once", async ({ page }) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/1701q-all-lines.json"));

  const expectedRows = [
    ...Array.from({ length: 11 }, (_, index) => index + 36),
    ...Array.from({ length: 8 }, (_, index) => index + 47),
    ...Array.from({ length: 8 }, (_, index) => index + 55),
    ...Array.from({ length: 4 }, (_, index) => index + 64)
  ];
  for (const number of expectedRows) {
    const row = page.locator(`.item-${number}-1701q`);
    await expect(row, `Item ${number}`).toHaveCount(1);
    await expect(row.locator(":scope > .amount-1701q"), `Item ${number} amounts`).toHaveCount(2);
  }
  await expect(page.locator(".single-total-1701q:not(.final-total-1701q)")).toHaveCount(1);
  await expect(page.locator(".final-total-1701q")).toHaveCount(1);
  await expect(page.locator(".part-five-1701q [data-schedule-start]")).toHaveCount(4);
});

test("1701Q 2018 keeps verified page-specific PDF417, caption, and seal geometry", async ({ page }) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/1701q-normal.json"));
  const pages = page.locator(".form-page");
  await expect(pages).toHaveCount(2);

  await expectCriticalRegionGeometry(pages.nth(0), [
    {
      name: "official seal XObject",
      selector: ".government-wordmark-1701q img",
      x: 472.14,
      y: 15.228,
      width: 64.746,
      height: 58.212
    },
    {
      name: "page 1 official PDF417 active matrix",
      selector: '.barcode-1701q[data-barcode-page="1"] .official-pdf417-object-1701q',
      x: 882.48,
      y: 93.36,
      width: 298.192592,
      height: 68.04
    },
    {
      name: "page 1 official PDF417 live caption",
      selector: '.barcode-1701q[data-barcode-page="1"] > small',
      x: 1024.04,
      y: 165.61456,
      width: 161.3016,
      height: 14.874
    }
  ]);
  await expectCriticalRegionGeometry(pages.nth(1), [
    {
      name: "page 2 official PDF417 active matrix",
      selector: '.barcode-1701q[data-barcode-page="2"] .official-pdf417-object-1701q',
      x: 888.48,
      y: 75.6,
      width: 298.666666,
      height: 78.435
    },
    {
      name: "page 2 official PDF417 live caption",
      selector: '.barcode-1701q[data-barcode-page="2"] > small',
      x: 1029.32,
      y: 160.33456,
      width: 161.3016,
      height: 14.874
    }
  ]);

  expect(await page.locator(".official-pdf417-symbol-1701q").evaluateAll(
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

  const captions = page.locator(".barcode-1701q > small");
  await expect(captions.nth(0)).toHaveText("1701Q 01/18ENCS P1");
  await expect(captions.nth(1)).toHaveText("1701Q 01/18ENCS P2");
  expect(await captions.evaluateAll((elements) => elements.map((element) => {
    const style = getComputedStyle(element);
    return {
      fontFamily: style.fontFamily,
      fontSize: style.fontSize,
      fontWeight: style.fontWeight,
      whiteSpace: style.whiteSpace
    };
  }))).toEqual(Array.from({ length: 2 }, () => ({
    fontFamily: '"eBIRForms Arimo", Arial, sans-serif',
    fontSize: "10.72px",
    fontWeight: "400",
    whiteSpace: "nowrap"
  })));

  expect(await page.locator(".government-wordmark-1701q img").evaluate(
    (image) => ({
      naturalHeight: (image as HTMLImageElement).naturalHeight,
      naturalWidth: (image as HTMLImageElement).naturalWidth
    })
  )).toEqual({ naturalHeight: 102, naturalWidth: 119 });
});

test("1701Q 2018 uses the official 15 percent neutral form fill", async ({ page }) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/1701q-normal.json"));
  const fills = await page.locator([
    ".form-1701q-page-one .part-1701q > h2",
    ".form-1701q-page-two .paired-section-1701q > h2"
  ].join(", ")).evaluateAll((elements) =>
    elements.map((element) => getComputedStyle(element).backgroundColor)
  );
  expect(fills.length).toBeGreaterThan(1);
  expect(fills.every((fill) => fill === "rgb(217, 217, 217)")).toBe(true);
});

test("1701Q 2018 preserves the reviewed readable text hierarchy and fixed choice geometry", async ({ page }) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/1701q-minimum.json"));

  const fontSizes = await page.locator([
    ".header-options-1701q",
    ".part-five-1701q .item-36-1701q > span:first-child",
    ".part-five-1701q .item-44-1701q > span:first-child",
    ".part-five-1701q .item-52-1701q > span:first-child"
  ].join(", ")).evaluateAll((elements) =>
    elements.map((element) => Number.parseFloat(getComputedStyle(element).fontSize))
  );
  expect(fontSizes).toHaveLength(4);
  expect(fontSizes[0]).toBeCloseTo(8.15 * 4 / 3, 2);
  expect(fontSizes[1]).toBeCloseTo(9 * 4 / 3, 2);
  expect(fontSizes[2]).toBeCloseTo(7 * 4 / 3, 2);
  expect(fontSizes[3]).toBeCloseTo(7 * 4 / 3, 2);

  const scheduleHeading = page.locator(".part-five-1701q .schedule-36-1701q > h2");
  expect(await scheduleHeading.evaluate((element) => getComputedStyle(element).fontWeight)).toBe("400");
  expect(await scheduleHeading.locator("b").evaluate((element) => getComputedStyle(element).fontWeight)).toBe("700");

  const fixedChoiceBox = await page.locator(".tax-election-1701q .check-box").first().evaluate((element) => {
    const style = getComputedStyle(element);
    return { flexShrink: style.flexShrink, width: Number.parseFloat(style.width) };
  });
  expect(fixedChoiceBox.flexShrink).toBe("0");
  expect(fixedChoiceBox.width).toBeCloseTo(14 * 4 / 3, 1);
});

test("1701Q 2018 keeps the official Item 16 and Item 25 choice partitions", async ({ page }) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/1701q-normal.json"));
  const firstPage = page.locator(".form-page").nth(0);
  await expectCriticalRegionGeometry(firstPage, [
    { name: "Item 16 graduated box", selector: ".taxpayer-background-1701q .graduated-choice-1701q .check-box", x: 119, y: 661, width: 28, height: 24 },
    { name: "Item 16 itemized box", selector: ".taxpayer-background-1701q .deduction-choices-1701q > span:first-child .check-box", x: 454, y: 660, width: 28, height: 24 },
    { name: "Item 16 OSD box", selector: ".taxpayer-background-1701q .deduction-choices-1701q > span:last-child .check-box", x: 715, y: 660, width: 28, height: 24 },
    { name: "Item 16 eight-percent box", selector: ".taxpayer-background-1701q .eight-percent-choice-1701q .check-box", x: 117, y: 704, width: 28, height: 24 },
    { name: "Item 25 graduated box", selector: ".spouse-background-1701q .graduated-choice-1701q .check-box", x: 119, y: 1034, width: 28, height: 24 },
    { name: "Item 25 itemized box", selector: ".spouse-background-1701q .deduction-choices-1701q > span:first-child .check-box", x: 454, y: 1033, width: 28, height: 24 },
    { name: "Item 25 OSD box", selector: ".spouse-background-1701q .deduction-choices-1701q > span:last-child .check-box", x: 715, y: 1033, width: 28, height: 24 },
    { name: "Item 25 eight-percent box", selector: ".spouse-background-1701q .eight-percent-choice-1701q .check-box", x: 117, y: 1077, width: 28, height: 24 }
  ]);
});

test("1701Q 2018 preserves the official gray declaration copy band", async ({ page }) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/1701q-normal.json"));

  await expect(page.locator(".declaration-1701q > p")).toHaveCSS(
    "background-color",
    "rgb(217, 217, 217)"
  );
  await expect(page.locator(".declaration-1701q > div")).toHaveCSS(
    "background-color",
    "rgba(0, 0, 0, 0)"
  );
});

test("1701Q 2018 matches the complete official pages", async ({ page }, testInfo) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/1701q-normal.json"));
  const pages = page.locator(".form-page");
  await expect(pages).toHaveCount(2);

  await page.addStyleTag({
    content: `
      .form-page[data-visual-blank-values="true"] .comb-value > span,
      .form-page[data-visual-blank-values="true"] .adaptive-plain-value,
      .form-page[data-visual-blank-values="true"] .check-box,
      .form-page[data-visual-blank-values="true"] .amount-1701q {
        color: transparent !important;
        text-shadow: none !important;
      }
    `
  });

  const results: Array<{
    page: number;
    structuralChangedPercent: number;
    fullPageChangedPercent: number;
    expectedInkMissingPercent: number;
    unexpectedActualInkPercent: number;
  }> = [];
  for (let pageIndex = 0; pageIndex < 2; pageIndex += 1) {
    const renderedPage = pages.nth(pageIndex);
    await renderedPage.evaluate((element) => element.setAttribute("data-visual-blank-values", "true"));
    const referencePath = path.join(
      REPO_ROOT,
      // Compares against the SAME-RASTERIZER chromium reference, not the Poppler
      // raster. The Poppler comparison carries a cross-rasterizer noise floor - 1.55%
      // to 4.54% depending on the form - which flatters every number by an amount
      // that has nothing to do with this renderer. The chromium reference is built
      // from the same pinned PDF through pdftocairo and rasterized by this exact
      // Chromium build, so the difference it reports is ours. Expect the number to
      // RISE on this change: that is the floor being removed, not a regression.
      `packages/form-renderer/references/1701q-2018-page-${pageIndex + 1}-chromium.png`
    );
    const expectedBuffer = fs.readFileSync(referencePath);
    const actualBuffer = await renderedPage.screenshot({ animations: "disabled", caret: "hide" });
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
    fs.writeFileSync(testInfo.outputPath(`1701q-page-${pageIndex + 1}-actual.png`), actualBuffer);
    fs.writeFileSync(testInfo.outputPath(`1701q-page-${pageIndex + 1}-structure-diff.png`), PNG.sync.write(diff));
    fs.writeFileSync(
      testInfo.outputPath(`1701q-page-${pageIndex + 1}-full-page-diff.png`),
      PNG.sync.write(completePage.diff)
    );
  }
  console.log(`1701Q complete-page parity: ${JSON.stringify(results)}`);
  expect(
    results.filter((result) => result.fullPageChangedPercent > MAX_CHANGED_PERCENT),
    "complete page pixels, including all static labels, instructions, fields, signatures, and artwork"
  ).toEqual([]);
});

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
          text: child.textContent?.trim().slice(0, 100)
        }))
    );
    console.warn(`1701Q overflow report: ${JSON.stringify({ report, offenders })}`);
  }
  return valid;
}

interface MutableEnvelope {
  taxpayer: {
    email: string;
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
      ".amount-1701q",
      ".guided-payment-field-1701q",
      ".not-applicable-payment-field-1701q",
      ".line-description-1701q",
      ".receipt-value-1701q"
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
          if (target[ty * width + tx] === 1) {
            matched = true;
            break;
          }
        }
      }
      if (!matched) changed[index] = 1;
    }
  }
}
