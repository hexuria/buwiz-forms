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
  "packages/form-renderer/references/0605-1999-static-text-inventory.json"
), "utf8")) as {
  official_source_sha256: string;
  regions: Array<{ selector: string; text: string }>;
  atc_rows: string[][];
  tax_type_rows: string[][];
};
const DEVICE_SCALE_FACTOR = 1.5;
const MAX_CHANGED_PERCENT = 1;
const STRUCTURAL_INK_THRESHOLD = 100;
const STRUCTURAL_LINE_MIN_RUN = 20;
const STRUCTURAL_TOLERANCE_RADIUS = 4;

test("0605 1999 renders every Rust fixture as two stable unclipped BIR Folio pages", async ({ page }) => {
  for (const fixtureName of [
    "0605-minimum.json",
    "0605-normal.json",
    "0605-long-values.json",
    "0605-validation-edge.json",
    "0605-variant.json"
  ]) {
    const fixture = readFixture(`packages/form-contracts/fixtures/${fixtureName}`);
    await renderEnvelope(page, fixture);
    const pages = page.locator(".form-page");
    await expect(pages, fixtureName).toHaveCount(2);
    await expect(pages.nth(0)).toHaveAttribute("data-paper", "folio");
    await expect(pages.nth(1)).toHaveAttribute("data-paper", "folio");
    expect(await pageHasNoOverflow(pages.nth(0)), `${fixtureName} page 1`).toBe(true);
    expect(await pageHasNoOverflow(pages.nth(1)), `${fixtureName} page 2`).toBe(true);
  }
});

test("0605 1999 keeps the native seal and live official masthead typography without machine-readable symbols", async ({ page }) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/0605-normal.json"));
  const pages = page.locator(".form-page");
  await expect(pages).toHaveCount(2);

  await expectCriticalRegionGeometry(pages.nth(0), [{
    name: "official DeviceGray seal XObject",
    selector: ".government-seal-0605",
    x: 29.0398076,
    y: 101.0399974,
    width: 82.0758034,
    height: 67.5182712
  }]);

  expect(await page.locator(".government-seal-0605").evaluate((image) => ({
    naturalHeight: (image as HTMLImageElement).naturalHeight,
    naturalWidth: (image as HTMLImageElement).naturalWidth,
    objectFit: getComputedStyle(image).objectFit,
    officialObject: image.getAttribute("data-official-pdf-object")
  }))).toEqual({
    naturalHeight: 93,
    naturalWidth: 113,
    objectFit: "fill",
    officialObject: "13 0"
  });

  await expectOfficialMastheadTypography(pages.nth(0), [
    {
      selector: ".government-line-one-0605",
      text: "Republika ng Pilipinas",
      x: 62.6848831,
      y: 56.0695953,
      width: 81.5290909,
      fontWeight: "400"
    },
    {
      selector: ".government-line-two-0605",
      text: "Kagawaran ng Pananalapi",
      x: 62.6848831,
      y: 64.5474854,
      width: 97.6294785,
      fontWeight: "400"
    },
    {
      selector: ".government-line-three-0605",
      text: "Kawanihan ng Rentas Internas",
      x: 62.6848831,
      y: 72.9308548,
      width: 141.6231766,
      fontWeight: "400"
    },
    {
      selector: ".masthead-0605 h1",
      text: "Payment Form",
      x: 252.8769226,
      y: 55.8901863,
      width: 158.0611877,
      fontWeight: "700"
    },
    {
      selector: ".form-number-0605 > span",
      text: "BIR Form No.",
      x: 490.4837646,
      y: 47.7416229,
      width: 50.0783081,
      fontWeight: "400"
    },
    {
      selector: ".form-number-0605 strong",
      text: "0605",
      x: 485.0532532,
      y: 52.2101860,
      width: 75.4885559,
      fontWeight: "700"
    },
    {
      selector: ".form-number-0605 small",
      text: "July 1999 (ENCS)",
      x: 487.5012512,
      y: 85.7287750,
      width: 66.5780334,
      fontWeight: "400"
    }
  ]);

  const machineReadableSelector = [
    "[data-barcode-page]",
    "[data-symbology]",
    "[class*='barcode']",
    "[class*='pdf417']",
    "[class*='qr-code']",
    "[aria-label*='PDF417']",
    "[aria-label*='QR code']"
  ].join(",");
  await expect(pages.nth(0).locator(machineReadableSelector)).toHaveCount(0);
  await expect(pages.nth(1).locator(machineReadableSelector)).toHaveCount(0);
  await expect(pages.nth(0).locator("img")).toHaveCount(1);
  await expect(pages.nth(1).locator("img")).toHaveCount(0);
});

test("0605 1999 preserves the official page-two ATC category bands and merged petroleum row", async ({ page }) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/0605-normal.json"));
  const table = page.locator(".atc-table-0605");
  await expect(table.locator("tbody tr")).toHaveCount(25);

  const mergedStart = table.locator(".atc-merged-start-0605");
  const mergedEnd = table.locator(".atc-merged-end-0605");
  await expect(mergedStart).toHaveCount(2);
  await expect(mergedEnd).toHaveCount(2);
  await expect(mergedStart.nth(0)).toHaveText("XP010, XP020 &");
  await expect(mergedStart.nth(1)).toHaveText("Basetocks, Lubes and");
  await expect(mergedEnd.nth(0)).toHaveText("XP190");
  await expect(mergedEnd.nth(1)).toHaveText("Greases");
  expect(await mergedStart.evaluateAll((cells) =>
    cells.map((cell) => getComputedStyle(cell).borderBottomColor)
  )).toEqual(Array(2).fill("rgba(0, 0, 0, 0)"));
  expect(await mergedEnd.evaluateAll((cells) =>
    cells.map((cell) => getComputedStyle(cell).borderTopColor)
  )).toEqual(Array(2).fill("rgba(0, 0, 0, 0)"));
  await expect(table.locator("tbody tr").nth(12).locator("td").nth(4)).toHaveText("XP040");
  await expect(table.locator("tbody tr").nth(23).locator("td").nth(4)).toHaveText("XM051");

  const categoryCells = table.locator(".atc-category-0605");
  await expect(categoryCells).toHaveCount(9);
  expect(await categoryCells.allTextContents()).toEqual([
    "Sweetened Products",
    "Invasive Cosmetic Products",
    "Tobacco Products",
    "Miscellaneous Products/Articles",
    "Mineral Products",
    "Tobacco Inspection Fees",
    "Excise Tax on Goods",
    "Alcohol Products",
    "Petroleum Products"
  ]);
  expect(await categoryCells.evaluateAll((cells) =>
    cells.map((cell) => getComputedStyle(cell).backgroundColor)
  )).toEqual(Array(9).fill("rgb(191, 191, 191)"));

  const officialNarrowCells = table.locator('[data-official-font-role="Arial Narrow"]');
  await expect(officialNarrowCells).toHaveCount(4);
  expect(await officialNarrowCells.allTextContents()).toEqual([
    "Powdered Drinks not Classified as Milk, Juice, Tea and Coffee",
    "Other Non-Alcoholic Beverages that Contain Added Sugar",
    "Using Purely Coconut Sap Sugar & Purely Steviol Glycosides",
    "Performance of Services on Invasive Cosmetic Procedures"
  ]);
  expect(await officialNarrowCells.locator(":scope > span").evaluateAll((spans) =>
    spans.map((span) => ({
      fontFamily: getComputedStyle(span).fontFamily,
      scaleX: new DOMMatrixReadOnly(getComputedStyle(span).transform).a
    }))
  )).toEqual([
    { fontFamily: '"eBIRForms Arimo", sans-serif', scaleX: 0.716 },
    { fontFamily: '"eBIRForms Arimo", sans-serif', scaleX: 0.772 },
    { fontFamily: '"eBIRForms Arimo", sans-serif', scaleX: 0.712 },
    { fontFamily: '"eBIRForms Arimo", sans-serif', scaleX: 0.772 }
  ]);
});

test("0605 1999 preserves exact official static wording and page-two code tables", async ({ page }) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/0605-minimum.json"));

  expect(staticTextInventory.official_source_sha256).toBe(
    "de04419766c59bf27fdeb854c0f7c3f98601900caa20630442e671e2313e536f"
  );
  for (const region of staticTextInventory.regions) {
    const visibleText = await page.locator(region.selector).evaluate((element) =>
      (element as HTMLElement).innerText
        .replace(/\s+/g, " ")
        .replace(/\s*►\s*/g, " ► ")
        .trim()
    );
    expect(visibleText, region.selector).toBe(region.text);
  }

  const atcRows = await page.locator(".atc-table-0605 tbody tr").evaluateAll((rows) =>
    rows.map((row) => [...row.querySelectorAll("td")].map((cell) =>
      (cell as HTMLElement).innerText.replace(/\s+/g, " ").trim()
    ))
  );
  expect(atcRows).toEqual(staticTextInventory.atc_rows);

  const taxTypeRows = await page.locator(".tax-type-table-0605 tbody tr").evaluateAll((rows) =>
    rows.map((row) => [...row.querySelectorAll("td")].map((cell) =>
      (cell as HTMLElement).innerText.replace(/\s+/g, " ").trim()
    ))
  );
  expect(taxTypeRows).toEqual(staticTextInventory.tax_type_rows);
});

test("0605 1999 preserves the official Part II amount-box partitions", async ({ page }) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/0605-normal.json"));
  const firstPage = page.locator(".form-page").nth(0);

  await expectCriticalRegionGeometry(firstPage, [
    { name: "Item 19 amount", selector: ".item-19-row-0605 > .money-0605", x: 808, y: 906, width: 345, height: 43 },
    { name: "Item 20A amount", selector: ".penalties-row-0605 label:nth-of-type(1) > .money-0605", x: 96, y: 968, width: 216, height: 42 },
    { name: "Item 20B amount", selector: ".penalties-row-0605 label:nth-of-type(2) > .money-0605", x: 340, y: 968, width: 214, height: 42 },
    { name: "Item 20C amount", selector: ".penalties-row-0605 label:nth-of-type(3) > .money-0605", x: 586, y: 968, width: 190, height: 42 },
    { name: "Item 20D amount", selector: ".penalties-row-0605 label:nth-of-type(4) > .money-0605", x: 808, y: 968, width: 345, height: 42 }
  ]);
});

test("0605 1999 uses plain rectangles and only the pinned short-guide counts", async ({ page }) => {
  const normal = readFixture("packages/form-contracts/fixtures/0605-normal.json");
  await renderEnvelope(page, normal);

  for (const field of [
    "6-atc",
    "12-line-of-business",
    "13-taxpayer-name",
    "15-registered-address",
    "17-other-manner",
    "18-installment-count",
    "19-basic-tax",
    "20a-surcharge",
    "21-total-payable",
    "23-amount",
    "24a-bank",
    "24b-number",
    "24d-amount",
    "25a-number",
    "25c-amount",
    "26a-bank",
    "26b-number",
    "26d-amount",
    "22a-taxpayer-signature",
    "22a-title-position",
    "22b-head-of-office-signature",
    "machine-validation"
  ]) {
    const locator = page.locator(`[data-official-field="${field}"]`);
    await expect(locator, field).toHaveAttribute("data-field-mode", "plain");
    await expect(locator, field).toHaveAttribute("data-guide-count", "0");
    await expect(locator.locator(".comb-value"), `${field} invented comb`).toHaveCount(0);
  }

  const installmentCount = page.locator('[data-official-field="18-installment-count"]');
  await expect(installmentCount).toHaveCSS("border-bottom-style", "none");
  await expect(installmentCount).toHaveCSS("background-color", "rgba(0, 0, 0, 0)");
  await expect(installmentCount.locator("xpath=preceding-sibling::*[2]"))
    .not.toHaveClass(/checked/);

  for (const [field, segments, guideCount] of [
    ["2-year-ended", "2-4", 4],
    ["4-due-date", "2-2-4", 5],
    ["5-sheets", "2", 1],
    ["7-return-period", "2-2-4", 5],
    ["8-tax-type", "2", 1],
    ["10-rdo", "3", 2],
    ["16-zip", "4", 3],
    ["24c-date", "2-2-4", 5],
    ["25b-date", "2-2-4", 5],
    ["26c-date", "2-2-4", 5]
  ] as const) {
    const locator = page.locator(`[data-official-field="${field}"]`);
    await expect(locator, field).toHaveAttribute("data-field-mode", "guided");
    await expect(locator, field).toHaveAttribute("data-guide-segments", segments);
    await expect(locator, field).toHaveAttribute("data-guide-count", String(guideCount));
    await expect(
      locator.locator(".guided-segment-0605 .comb-value > span:not(:last-child)"),
      `${field} rendered short guides`
    ).toHaveCount(guideCount);
  }

  await expect(page.locator('[data-official-field="9-tin"]')).toHaveAttribute(
    "data-field-mode",
    "plain"
  );
  await expect(page.locator('[data-official-field="14-telephone"]')).toHaveAttribute(
    "data-field-mode",
    "plain"
  );

  const legacyCapacity = structuredClone(normal) as Record<string, any>;
  legacyCapacity.taxpayer.tin = "123456789000";
  legacyCapacity.taxpayer.contact_number = "1234567";
  await renderEnvelope(page, legacyCapacity);
  await expect(page.locator('[data-official-field="9-tin"]')).toHaveAttribute(
    "data-guide-count",
    "8"
  );
  await expect(
    page.locator('[data-official-field="9-tin"] .comb-value > span:not(:last-child)')
  ).toHaveCount(8);
  await expect(page.locator('[data-official-field="14-telephone"]')).toHaveAttribute(
    "data-guide-count",
    "6"
  );
  await expect(
    page.locator('[data-official-field="14-telephone"] .comb-value > span:not(:last-child)')
  ).toHaveCount(6);

  await renderEnvelope(
    page,
    readFixture("packages/form-contracts/fixtures/0605-variant.json")
  );
  const enteredInstallmentCount = page.locator(
    '[data-official-field="18-installment-count"]'
  );
  await expect(enteredInstallmentCount).toContainText("3");
  await expect(enteredInstallmentCount).toHaveCSS("border-bottom-style", "none");
  await expect(enteredInstallmentCount).toHaveCSS(
    "background-color",
    "rgba(0, 0, 0, 0)"
  );
  await expect(enteredInstallmentCount.locator("xpath=preceding-sibling::*[2]"))
    .toHaveClass(/checked/);
});

test("0605 1999 keeps reviewed plain fields plain when empty or populated", async ({ page }) => {
  for (const populated of [false, true]) {
    const fixture = structuredClone(
      readFixture("packages/form-contracts/fixtures/0605-minimum.json")
    ) as Record<string, any>;
    const value = populated ? "REVIEWED PLAIN FIELD VALUE" : "";
    fixture.fields.other_manner_description.value = value;
    fixture.fields.payment_24_drawee_bank_or_agency.value = value;
    fixture.fields.payment_24_number.value = value;
    fixture.fields.signature_taxpayer_or_representative.value = value;
    fixture.fields.signature_title_or_position.value = value;
    fixture.fields.signature_head_of_office.value = value;
    fixture.fields.machine_validation_or_receipt_details.value = value;
    await renderEnvelope(page, fixture);

    for (const field of [
      "17-other-manner",
      "24a-bank",
      "24b-number",
      "22a-taxpayer-signature",
      "22a-title-position",
      "22b-head-of-office-signature",
      "machine-validation"
    ]) {
      const locator = page.locator(`[data-official-field="${field}"]`);
      await expect(locator, `${field} populated=${populated}`).toHaveAttribute(
        "data-field-mode",
        "plain"
      );
      await expect(locator).toHaveAttribute("data-guide-count", "0");
      await expect(locator.locator(".comb-value"), `${field} invented comb`).toHaveCount(0);
      await expect(locator).toHaveCSS("background-image", "none");
    }
  }
});

test("0605 1999 preserves the official Part III row field types without invented combs", async ({ page }) => {
  for (const fixtureName of ["0605-minimum.json", "0605-variant.json"]) {
    await renderEnvelope(
      page,
      readFixture(`packages/form-contracts/fixtures/${fixtureName}`)
    );

    const expectedRows = [
      {
        row: "payment_23",
        fields: [["23-amount", "plain", "0"]]
      },
      {
        row: "payment_24",
        fields: [
          ["24a-bank", "plain", "0"],
          ["24b-number", "plain", "0"],
          ["24c-date", "guided", "5"],
          ["24d-amount", "plain", "0"]
        ]
      },
      {
        row: "payment_25",
        fields: [
          ["25a-number", "plain", "0"],
          ["25b-date", "guided", "5"],
          ["25c-amount", "plain", "0"]
        ]
      },
      {
        row: "payment_26",
        fields: [
          ["26a-bank", "plain", "0"],
          ["26b-number", "plain", "0"],
          ["26c-date", "guided", "5"],
          ["26d-amount", "plain", "0"]
        ]
      }
    ] as const;

    for (const expectedRow of expectedRows) {
      const row = page.locator(`[data-payment-row="${expectedRow.row}"]`);
      await expect(row, `${fixtureName} ${expectedRow.row}`).toHaveCount(1);
      await expect(row.locator("[data-official-field]")).toHaveCount(
        expectedRow.fields.length
      );
      for (const [field, mode, guides] of expectedRow.fields) {
        const locator = row.locator(`[data-official-field="${field}"]`);
        await expect(locator, `${fixtureName} ${field}`).toHaveAttribute(
          "data-field-mode",
          mode
        );
        await expect(locator).toHaveAttribute("data-guide-count", guides);
        if (mode === "plain") {
          await expect(locator.locator(".comb-value"), `${field} invented comb`)
            .toHaveCount(0);
          await expect(locator).toHaveCSS("background-image", "none");
        }
      }
    }

    await expect(
      page.locator('[data-payment-row="payment_25"] [data-official-field="25a-bank"]'),
      "the official Tax Debit Memo row has no drawee-bank field"
    ).toHaveCount(0);
  }
});

test("0605 1999 reports an unreadable legal plain value as unsafe instead of clipping it", async ({ page }) => {
  const fixture = structuredClone(
    readFixture("packages/form-contracts/fixtures/0605-minimum.json")
  ) as Record<string, any>;
  const value = "N".repeat(160);
  fixture.fields.payment_24_number.value = value;
  await renderEnvelope(page, fixture);

  const field = page.locator('[data-official-field="24b-number"]');
  await expect(field).toHaveAttribute("data-field-mode", "plain");
  await expect(field).toHaveAttribute("data-guide-count", "0");
  await expect(field.locator(".comb-value")).toHaveCount(0);
  const renderedValue = field.locator(".adaptive-plain-value");
  await expect(renderedValue).toHaveAttribute("aria-label", value);
  await expect(renderedValue).toHaveAttribute("data-adaptive-fit-state", "unresolved");
  expect(await pageHasNoOverflow(page.locator(".form-page").nth(0))).toBe(false);
});

test("0605 1999 switches whole guided fields only for valid values beyond official capacity", async ({ page }) => {
  const cases: Array<{
    field: string;
    values: Array<{ value: string | number; mode: "guided" | "plain" }>;
    mutate: (fixture: Record<string, any>, value: string | number) => void;
  }> = [
    {
      field: "9-tin",
      values: [
        { value: "123456789000", mode: "guided" },
        { value: "1234567890001", mode: "plain" }
      ],
      mutate: (fixture, value) => { fixture.taxpayer.tin = value; }
    },
    {
      field: "14-telephone",
      values: [
        { value: "1234567", mode: "guided" },
        { value: "12345678", mode: "plain" }
      ],
      mutate: (fixture, value) => { fixture.taxpayer.contact_number = value; }
    }
  ];

  for (const boundary of cases) {
    for (const sample of boundary.values) {
      const fixture = structuredClone(
        readFixture("packages/form-contracts/fixtures/0605-normal.json")
      ) as Record<string, any>;
      boundary.mutate(fixture, sample.value);
      await renderEnvelope(page, fixture);
      const locator = page.locator(`[data-official-field="${boundary.field}"]`);
      await expect(
        locator,
        `${boundary.field} value=${JSON.stringify(sample.value)}`
      ).toHaveAttribute("data-field-mode", sample.mode);
      if (sample.mode === "plain") {
        await expect(locator).toHaveAttribute("data-guide-count", "0");
        await expect(locator.locator(".comb-value")).toHaveCount(0);
      }
    }
  }
});

test("0605 1999 matches the complete pinned official pages", async ({ page }, testInfo) => {
  const fixture = readFixture("packages/form-contracts/fixtures/0605-normal.json");
  await renderEnvelope(page, fixture);
  const pages = page.locator(".form-page");
  await expect(pages).toHaveCount(2);

  await expectCriticalRegionGeometry(pages.nth(0), [
    { name: "masthead", selector: ".masthead-0605", x: 25, y: 83, width: 1137, height: 137 },
    { name: "fill instruction", selector: ".fill-instruction-0605", x: 25, y: 223, width: 1137, height: 43 },
    { name: "Items 1-8", selector: ".header-fields-0605", x: 25, y: 269, width: 1137, height: 152 },
    { name: "Part I", selector: ".part-one-0605", x: 25, y: 425, width: 1137, height: 436 },
    { name: "Part II", selector: ".part-two-0605", x: 25, y: 864, width: 1137, height: 203 },
    { name: "declaration", selector: ".declaration-0605", x: 25, y: 1070, width: 1137, height: 258 },
    { name: "Part III", selector: ".part-three-0605", x: 25, y: 1324, width: 1137, height: 410 }
  ]);
  await expectCriticalRegionGeometry(pages.nth(1), [
    { name: "ATC table", selector: ".atc-table-0605", x: 42, y: 93, width: 1143, height: 438 },
    { name: "Tax Type band", selector: ".reference-tables-0605 > h2", x: 42, y: 527, width: 1143, height: 23 },
    { name: "Tax Type table", selector: ".tax-type-table-0605", x: 42, y: 548, width: 1143, height: 167 },
    { name: "guideline columns", selector: ".instructions-0605 > div", x: 75, y: 786, width: 1074, height: 925 }
  ]);

  expect(await page.locator(".form-0605-page-one img").count()).toBe(1);
  expect(await page.locator("[class*='barcode']").count()).toBe(0);
  expect(await page.locator(".atc-table-0605 tbody tr").count()).toBe(25);
  expect(await page.locator(".tax-type-table-0605 tbody tr").count()).toBe(10);

  await page.addStyleTag({
    content: `
      .form-page[data-visual-blank-values="true"] .comb-value > span,
      .form-page[data-visual-blank-values="true"] .adaptive-plain-value,
      .form-page[data-visual-blank-values="true"] .check-box {
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
      `packages/form-renderer/references/0605-1999-page-${index + 1}.png`
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
    fs.writeFileSync(testInfo.outputPath(`0605-page-${index + 1}-actual.png`), actualBuffer);
    fs.writeFileSync(
      testInfo.outputPath(`0605-page-${index + 1}-structure-diff.png`),
      PNG.sync.write(diff)
    );
    fs.writeFileSync(
      testInfo.outputPath(`0605-page-${index + 1}-full-page-diff.png`),
      PNG.sync.write(completePage.diff)
    );
  }
  console.log(`0605 complete-page parity: ${JSON.stringify(results)}`);
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

interface OfficialMastheadSpan {
  selector: string;
  text: string;
  x: number;
  y: number;
  width: number;
  fontWeight: string;
}

async function expectOfficialMastheadTypography(
  page: Locator,
  spans: OfficialMastheadSpan[]
) {
  const pageBox = await page.boundingBox();
  expect(pageBox).not.toBeNull();
  if (!pageBox) return;

  for (const expected of spans) {
    const locator = page.locator(expected.selector);
    await expect(locator).toHaveText(expected.text);
    const actual = await locator.evaluate((element) => {
      const ownerPage = element.closest(".form-page");
      if (!ownerPage) throw new Error("masthead span is outside a form page");
      const pageRect = ownerPage.getBoundingClientRect();
      const range = document.createRange();
      range.selectNodeContents(element);
      const rect = range.getBoundingClientRect();
      const style = getComputedStyle(element);
      return {
        fontFamily: style.fontFamily,
        fontWeight: style.fontWeight,
        width: rect.width * 0.75,
        x: (rect.x - pageRect.x) * 0.75,
        y: (rect.y - pageRect.y) * 0.75
      };
    });
    expect(actual.fontFamily).toBe('"eBIRForms Arimo", sans-serif');
    expect(actual.fontWeight).toBe(expected.fontWeight);
    expect(Math.abs(actual.x - expected.x), `${expected.text} x`).toBeLessThanOrEqual(0.02);
    expect(Math.abs(actual.y - expected.y), `${expected.text} y`).toBeLessThanOrEqual(0.02);
    expect(Math.abs(actual.width - expected.width), `${expected.text} width`).toBeLessThanOrEqual(0.02);
  }
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
        failures.push({ region: region.name, dimension: key, difference });
      }
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
        .map((child) => {
          const style = getComputedStyle(child);
          return {
            class_name: child.className,
            parent_class_name: child.parentElement?.className,
            grandparent_class_name: child.parentElement?.parentElement?.className,
            client_width: child.clientWidth,
            scroll_width: child.scrollWidth,
            client_height: child.clientHeight,
            scroll_height: child.scrollHeight,
            font_size: style.fontSize,
            display: style.display,
            white_space: style.whiteSpace,
            padding: style.padding,
            text: child.textContent?.trim().slice(0, 100)
          };
        })
    );
    console.warn(`0605 overflow report: ${JSON.stringify({ report, offenders })}`);
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
