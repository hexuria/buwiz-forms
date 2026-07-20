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
  "packages/form-renderer/references/0619f-2018-static-text-inventory.json"
), "utf8")) as {
  official_source_sha256: string;
  regions: Array<{ selector: string; text: string }>;
};
const DEVICE_SCALE_FACTOR = 1.5;
const MAX_CHANGED_PERCENT = 1;
const STRUCTURAL_INK_THRESHOLD = 100;
const STRUCTURAL_LINE_MIN_RUN = 20;
const STRUCTURAL_TOLERANCE_RADIUS = 4;

test("0619F 2018 renders every Rust fixture as one stable unclipped Letter page", async ({ page }) => {
  for (const fixtureName of [
    "0619f-minimum.json",
    "0619f-normal.json",
    "0619f-long-values.json",
    "0619f-validation-edge.json",
    "0619f-all-payments.json"
  ]) {
    const fixture = readFixture(`packages/form-contracts/fixtures/${fixtureName}`);
    await renderEnvelope(page, fixture);
    const pages = page.locator(".form-page");
    await expect(pages, fixtureName).toHaveCount(1);
    await expect(pages.nth(0)).toHaveAttribute("data-paper", "letter");
    expect(await pageHasNoOverflow(pages.nth(0)), fixtureName).toBe(true);
  }
});

test("0619F 2018 keeps verified PDF417 active geometry, padding, caption, and seal", async ({ page }) => {
  const fixture = readFixture("packages/form-contracts/fixtures/0619f-normal.json");
  await renderEnvelope(page, fixture);
  const formPage = page.locator(".form-page").first();
  await expect(formPage).toHaveCount(1);

  await expectCriticalRegionGeometry(formPage, [
    {
      name: "official seal XObject",
      selector: ".government-wordmark-0619f img",
      x: 463,
      y: 30,
      width: 62,
      height: 56
    },
    {
      name: "official PDF417 XObject frame",
      selector: ".official-pdf417-object-0619f",
      x: 909,
      y: 101,
      width: 276,
      height: 65
    },
    {
      name: "official PDF417 active matrix",
      selector: ".official-pdf417-symbol-0619f",
      x: 909,
      y: 101,
      width: 273,
      height: 64
    },
    {
      name: "official PDF417 live caption",
      selector: ".barcode-0619f > small",
      x: 1066,
      y: 166,
      width: 120,
      height: 16
    }
  ]);

  const symbol = page.locator(".official-pdf417-symbol-0619f");
  await expect(symbol).toHaveAttribute("viewBox", "0 0 120 7");
  await expect(symbol).toHaveAttribute("preserveAspectRatio", "none");
  await expect(symbol).toHaveCSS("shape-rendering", "crispedges");

  const padding = await page.locator(".official-pdf417-object-0619f").evaluate((frame) => {
    const symbolElement = frame.querySelector("svg");
    if (!symbolElement) throw new Error("0619F PDF417 active matrix is missing");
    const frameBox = frame.getBoundingClientRect();
    const symbolBox = symbolElement.getBoundingClientRect();
    return {
      bottom: frameBox.bottom - symbolBox.bottom,
      right: frameBox.right - symbolBox.right
    };
  });
  expect(padding.right).toBeCloseTo(1.7051852 * 4 / 3, 1);
  expect(padding.bottom).toBeCloseTo(0.504375 * 4 / 3, 1);

  const caption = page.locator(".barcode-0619f > small");
  await expect(caption).toHaveText("0619-F 01/18 P1");
  expect(await caption.evaluate((element) => {
    const style = getComputedStyle(element);
    return {
      fontFamily: style.fontFamily,
      fontSize: style.fontSize,
      lineHeight: style.lineHeight,
      textAlign: style.textAlign,
      whiteSpace: style.whiteSpace
    };
  })).toEqual({
    fontFamily: '"eBIRForms Arimo", sans-serif',
    fontSize: "10.72px",
    lineHeight: "10.72px",
    textAlign: "right",
    whiteSpace: "nowrap"
  });

  expect(await page.locator(".government-wordmark-0619f img").evaluate(
    (image) => ({
      naturalHeight: (image as HTMLImageElement).naturalHeight,
      naturalWidth: (image as HTMLImageElement).naturalWidth
    })
  )).toEqual({ naturalHeight: 77, naturalWidth: 86 });
});

test("0619F 2018 preserves the official declaration bands and grayscale fills", async ({ page }) => {
  const fixture = readFixture("packages/form-contracts/fixtures/0619f-normal.json");
  await renderEnvelope(page, fixture);
  const formPage = page.locator(".form-page").first();

  await expectCriticalRegionGeometry(formPage, [
    { name: "declaration copy", selector: ".declaration-0619f > p", x: 36, y: 1010, width: 1152, height: 44 },
    { name: "signature body", selector: ".signature-body-0619f", x: 36, y: 1054, width: 1152, height: 118 },
    { name: "individual signature band", selector: ".signature-body-0619f > div:first-child b", x: 36, y: 1118, width: 605, height: 54 },
    { name: "non-individual signature column", selector: ".signature-body-0619f > div:last-child", x: 641, y: 1054, width: 548, height: 118 },
    { name: "accreditation footer", selector: ".signature-footer-0619f", x: 36, y: 1172, width: 1152, height: 36 }
  ]);

  expect(await page.locator(".declaration-0619f > p").evaluate((element) =>
    getComputedStyle(element).backgroundColor
  )).toBe("rgb(217, 217, 217)");
  expect(await page.locator(".signature-body-0619f b").first().evaluate((element) =>
    getComputedStyle(element).backgroundColor
  )).toBe("rgb(217, 217, 217)");
  expect(await page.locator(".signature-footer-0619f > span").first().evaluate((element) =>
    getComputedStyle(element).backgroundColor
  )).toBe("rgb(217, 217, 217)");
  expect(await page.locator(".category-choices-0619f").evaluate((element) =>
    getComputedStyle(element).backgroundColor
  )).toBe("rgb(217, 217, 217)");
  expect(await page.locator(".decimal-separator-0619f").first().evaluate((element) =>
    getComputedStyle(element).backgroundColor
  )).toBe("rgb(166, 166, 166)");
});

test("0619F 2018 preserves the official static wording and reviewed type hierarchy", async ({ page }) => {
  const fixture = readFixture("packages/form-contracts/fixtures/0619f-minimum.json");
  await renderEnvelope(page, fixture);

  expect(staticTextInventory.official_source_sha256).toBe(
    "edd7357390b1f0d95f2a38c9bb76252341c15b54b82bffd338bd540452ff15e1"
  );
  for (const region of staticTextInventory.regions) {
    const visibleText = await page.locator(region.selector).evaluate((element) =>
      (element as HTMLElement).innerText.replace(/\s+/g, " ").trim()
    );
    expect(visibleText, region.selector).toBe(region.text);
  }

  expect(await page.locator(".government-wordmark-0619f").evaluate((element) =>
    getComputedStyle(element).fontWeight
  )).toBe("600");
  expect(await page.locator(".part-0619f > h2").first().evaluate((element) =>
    getComputedStyle(element).fontWeight
  )).toBe("600");
  expect(await page.locator(".final-tax-row-0619f strong").first().evaluate((element) =>
    getComputedStyle(element).fontWeight
  )).toBe("600");

  expect(await page.locator(".declaration-0619f > p").evaluate((element) => {
    const style = getComputedStyle(element);
    return { textAlign: style.textAlign, textIndent: style.textIndent };
  })).toEqual({ textAlign: "left", textIndent: "14.6667px" });

  expect(await page.locator('[data-payment-row="payment_20"] > span:first-child').evaluate(
    (element) => ({ clientHeight: element.clientHeight, scrollHeight: element.scrollHeight })
  )).toEqual({ clientHeight: 24, scrollHeight: 24 });
});

test("0619F 2018 preserves the official Items 1-5 value-band partitions", async ({ page }) => {
  const fixture = readFixture("packages/form-contracts/fixtures/0619f-normal.json");
  await renderEnvelope(page, fixture);
  const formPage = page.locator(".form-page").first();

  await expectCriticalRegionGeometry(formPage, [
    { name: "Item 1 month field", selector: '[data-official-field="1-month"]', x: 92, y: 219, width: 171, height: 39 },
    { name: "Item 2 due-date field", selector: '[data-official-field="2-due-date"]', x: 349, y: 219, width: 231, height: 39 },
    { name: "Item 5 tax-type field", selector: '[data-official-field="5-tax-type-code"]', x: 1071, y: 219, width: 59, height: 39 }
  ]);

  for (const selector of [
    ".header-option-0619f:nth-child(1) > span",
    ".header-option-0619f:nth-child(3) > span",
    ".header-option-0619f:nth-child(5) > span"
  ]) {
    expect(await page.locator(selector).evaluate((element) =>
      getComputedStyle(element).backgroundColor
    )).toBe("rgb(217, 217, 217)");
  }
  expect(await page.locator('[data-official-field="1-month"]').evaluate((element) =>
    getComputedStyle(element).backgroundColor
  )).toBe("rgb(255, 255, 255)");
});

test("0619F 2018 uses only exact pinned guides and measured plain overflow fields", async ({ page }) => {
  const normal = readFixture("packages/form-contracts/fixtures/0619f-normal.json");
  await renderEnvelope(page, normal);

  for (const field of [
    "13-atc",
    "14-atc",
    "tax-agent-accreditation",
    "tax-agent-date-of-issue",
    "tax-agent-date-of-expiry",
    "20-bank",
    "20-number",
    "22-bank",
    "machine-validation"
  ]) {
    const locator = page.locator(`[data-official-field="${field}"]`);
    await expect(locator, field).toHaveAttribute("data-field-mode", "plain");
    await expect(locator, field).toHaveAttribute("data-guide-count", "0");
    await expect(locator.locator(".comb-value"), `${field} invented comb`).toHaveCount(0);
  }

  for (const [field, segments, guideCount] of [
    ["1-month", "2-4", 4],
    ["2-due-date", "2-2-4", 5],
    ["6-tin", "3-3-3-5", 10],
    ["7-rdo", "3", 2],
    ["8-agent-name", "40", 39],
    ["9-address-line-1", "40", 39],
    ["9-address-line-2", "31", 30],
    ["9a-zip", "4", 3],
    ["10-contact", "12", 11],
    ["12-email", "40", 39],
    ["13-amount", "11-2", 11],
    ["19-amount", "11-2", 11],
    ["20-date", "2-2-4", 5],
    ["20-amount", "11-2", 11]
  ] as const) {
    const locator = page.locator(`[data-official-field="${field}"]`);
    await expect(locator, field).toHaveAttribute("data-field-mode", "guided");
    await expect(locator, field).toHaveAttribute("data-guide-segments", segments);
    await expect(locator, field).toHaveAttribute("data-guide-count", String(guideCount));
    await expect(
      locator.locator(".guided-segment-0619f .comb-value > span:not(:last-child)"),
      `${field} short rules`
    ).toHaveCount(guideCount);
  }

  // Item 5 is the one guided field whose official entry box carries no interior
  // divider at all: raw rules_v of the pinned contract in x 520-580 / y 95-140 holds
  // only the two 0.48pt full-height box edges at x=535.90 and x=564.70 (y 109.7-127.6).
  // Items 1 and 2 on the same row do carry detected 0.24pt ticks in the identical
  // y-band, so this is field-specific absence, not extractor blindness. The box is
  // 28.8pt = 2 x 14.4pt cell pitch, so capacity stays 2 and only the guide ink is
  // removed - which is why the cell-span count (2 cells, 1 non-last span) here is
  // deliberately asserted separately from the drawn-guide count (0).
  const taxTypeCode = page.locator('[data-official-field="5-tax-type-code"]');
  await expect(taxTypeCode).toHaveAttribute("data-field-mode", "guided");
  await expect(taxTypeCode).toHaveAttribute("data-guide-segments", "2");
  await expect(taxTypeCode).toHaveAttribute("data-guide-count", "0");
  await expect(
    taxTypeCode.locator(".guided-segment-0619f .comb-value > span"),
    "5-tax-type-code cells"
  ).toHaveCount(2);
  expect(
    await taxTypeCode
      .locator(".guided-segment-0619f .comb-value > span:not(:last-child)")
      .evaluate((element) => getComputedStyle(element, "::after").content)
  ).toBe("none");

  const minimum = readFixture("packages/form-contracts/fixtures/0619f-minimum.json");
  await renderEnvelope(page, minimum);
  for (const [field, segments, guideCount] of [
    ["20-bank", "5", 4],
    ["20-number", "6", 5],
    ["20-date", "2-2-4", 5],
    ["21-bank", "5", 4],
    ["21-number", "6", 5],
    ["21-date", "2-2-4", 5],
    ["22-number", "6", 5],
    ["22-date", "2-2-4", 5],
    ["23-particular", "7", 6],
    ["23-bank", "5", 4],
    ["23-number", "6", 5],
    ["23-date", "2-2-4", 5]
  ] as const) {
    const locator = page.locator(`[data-official-field="${field}"]`);
    await expect(locator, field).toHaveAttribute("data-field-mode", "guided");
    await expect(locator, field).toHaveAttribute("data-guide-segments", segments);
    await expect(
      locator.locator(".guided-segment-0619f .comb-value > span:not(:last-child)"),
      `${field} short rules`
    ).toHaveCount(guideCount);
  }

  const longValues = readFixture("packages/form-contracts/fixtures/0619f-long-values.json");
  await renderEnvelope(page, longValues);
  await expect(page.locator('[data-adaptive-fit-state="unresolved"]')).toHaveCount(0);
  for (const field of ["8-agent-name", "20-bank", "20-number"]) {
    const locator = page.locator(`[data-official-field="${field}"]`);
    await expect(locator, field).toHaveAttribute("data-field-mode", "plain");
    await expect(locator, field).toHaveAttribute("data-guide-count", "0");
    await expect(locator.locator(".comb-value"), `${field} overflow comb`).toHaveCount(0);
  }
});

test("0619F 2018 preserves the official payment-row partitions", async ({ page }) => {
  const fixture = readFixture("packages/form-contracts/fixtures/0619f-normal.json");
  await renderEnvelope(page, fixture);
  const formPage = page.locator(".form-page").first();

  await expectCriticalRegionGeometry(formPage, [
    {
      name: "Item 22 merged particulars and bank cell",
      selector: '.payment-tax-debit-row-0619f > span:first-child',
      x: 36,
      y: 1336,
      width: 343,
      height: 37
    },
    {
      name: "Item 23 particulars entry comb",
      selector: '.payment-row-0619f[data-payment-row="payment_23"] > .payment-particular-field-0619f',
      x: 36,
      y: 1395,
      width: 199,
      height: 37
    }
  ]);

  expect(await page.locator(".payment-particular-field-0619f").evaluate((element) =>
    getComputedStyle(element).backgroundColor
  )).toBe("rgb(255, 255, 255)");
});

test("0619F 2018 matches the complete pinned official page", async ({ page }, testInfo) => {
  const fixture = readFixture("packages/form-contracts/fixtures/0619f-minimum.json");
  await renderEnvelope(page, fixture);
  const formPage = page.locator(".form-page").first();
  await expect(formPage).toHaveCount(1);

  await expectCriticalRegionGeometry(formPage, [
    { name: "government header", selector: ".government-header-0619f", x: 35, y: 29, width: 1154, height: 59 },
    { name: "masthead", selector: ".masthead-0619f", x: 35, y: 88, width: 1154, height: 110 },
    { name: "Items 1-5", selector: ".header-options-0619f", x: 35, y: 198, width: 1154, height: 60 },
    { name: "Part I", selector: ".background-0619f", x: 35, y: 260, width: 1154, height: 310 },
    { name: "Part II", selector: ".remittance-0619f", x: 35, y: 573, width: 1154, height: 436 },
    { name: "declaration", selector: ".declaration-0619f", x: 35, y: 1009, width: 1154, height: 203 },
    { name: "Part III", selector: ".payment-0619f", x: 35, y: 1215, width: 1154, height: 321 }
  ]);

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
  await formPage.evaluate((element) => element.setAttribute("data-visual-blank-values", "true"));

  const referencePath = path.join(
    REPO_ROOT,
    // Compares against the SAME-RASTERIZER chromium reference, not the Poppler
    // raster. The Poppler comparison carries a cross-rasterizer noise floor - 1.55%
    // to 4.54% depending on the form - which flatters every number by an amount
    // that has nothing to do with this renderer. The chromium reference is built
    // from the same pinned PDF through pdftocairo and rasterized by this exact
    // Chromium build, so the difference it reports is ours. Expect the number to
    // RISE on this change: that is the floor being removed, not a regression.
    "packages/form-renderer/references/0619f-2018-page-1-chromium.png"
  );
  const expectedBuffer = fs.readFileSync(referencePath);
  const actualBuffer = await formPage.screenshot({ animations: "disabled", caret: "hide" });
  const expected = PNG.sync.read(expectedBuffer);
  const actual = PNG.sync.read(actualBuffer);
  expect(actual.width).toBe(expected.width);
  expect(actual.height).toBe(expected.height);
  const { changedPixels, diff } = compareOfficialStructure(expected, actual);
  const structuralChangedPercent = changedPixels * 100 / (expected.width * expected.height);
  const completePage = compareCompleteOfficialPage(expected, actual);
  console.log(`0619F complete-page parity: ${JSON.stringify({
    structuralChangedPercent,
    fullPageChangedPercent: completePage.fullPageChangedPercent,
    expectedInkMissingPercent: completePage.expectedInkMissingPercent,
    unexpectedActualInkPercent: completePage.unexpectedActualInkPercent
  })}`);
  fs.writeFileSync(testInfo.outputPath("0619f-page-1-actual.png"), actualBuffer);
  fs.writeFileSync(
    testInfo.outputPath("0619f-page-1-structure-diff.png"),
    PNG.sync.write(diff)
  );
  fs.writeFileSync(
    testInfo.outputPath("0619f-page-1-full-page-diff.png"),
    PNG.sync.write(completePage.diff)
  );
  expect(
    completePage.fullPageChangedPercent,
    "complete page pixels, including all static labels, instructions, fields, signatures, and artwork"
  ).toBeLessThanOrEqual(MAX_CHANGED_PERCENT);
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
        .map((child) => ({
          class_name: child.className,
          client_width: child.clientWidth,
          scroll_width: child.scrollWidth,
          client_height: child.clientHeight,
          scroll_height: child.scrollHeight,
          text: child.textContent?.trim().slice(0, 80)
        }))
    );
    console.warn(`0619F overflow report: ${JSON.stringify({ report, offenders })}`);
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
