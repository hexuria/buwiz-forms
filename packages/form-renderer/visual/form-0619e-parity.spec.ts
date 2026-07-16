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

test("0619E 2018 renders every Rust fixture as one stable unclipped Letter page", async ({ page }) => {
  for (const fixtureName of [
    "0619e-minimum.json",
    "0619e-normal.json",
    "0619e-long-values.json",
    "0619e-validation-edge.json",
    "0619e-payment.json"
  ]) {
    const fixture = readFixture(`packages/form-contracts/fixtures/${fixtureName}`);
    await renderEnvelope(page, fixture);
    const pages = page.locator(".form-page");
    await expect(pages, fixtureName).toHaveCount(1);
    await expect(pages.nth(0)).toHaveAttribute("data-paper", "letter");
    expect(await pageHasNoOverflow(pages.nth(0)), fixtureName).toBe(true);
  }
});

test("0619E 2018 keeps verified PDF417, caption, and seal geometry", async ({ page }) => {
  const fixture = readFixture("packages/form-contracts/fixtures/0619e-normal.json");
  await renderEnvelope(page, fixture);
  const formPage = page.locator(".form-page").first();
  await expect(formPage).toHaveCount(1);

  await expectCriticalRegionGeometry(formPage, [
    {
      name: "official seal XObject",
      selector: ".government-wordmark-0619e img",
      x: 464,
      y: 50,
      width: 62,
      height: 56
    },
    {
      name: "official PDF417 symbol",
      selector: ".official-pdf417-symbol-0619e",
      x: 910,
      y: 130,
      width: 270,
      height: 71
    },
    {
      name: "official PDF417 live caption",
      selector: ".barcode-0619e > small",
      x: 1062,
      y: 200,
      width: 124,
      height: 16
    }
  ]);

  const symbol = page.locator(".official-pdf417-symbol-0619e");
  await expect(symbol).toHaveAttribute("viewBox", "0 0 120 7");
  await expect(symbol).toHaveAttribute("preserveAspectRatio", "none");
  await expect(symbol).toHaveCSS("shape-rendering", "crispedges");

  const caption = page.locator(".barcode-0619e > small");
  await expect(caption).toHaveText("0619-E 01/18 P1");
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

  expect(await page.locator(".government-wordmark-0619e img").evaluate(
    (image) => ({
      naturalHeight: (image as HTMLImageElement).naturalHeight,
      naturalWidth: (image as HTMLImageElement).naturalWidth
    })
  )).toEqual({ naturalHeight: 78, naturalWidth: 86 });
});

test("0619E 2018 preserves official period, declaration, and signature bands", async ({ page }) => {
  const fixture = readFixture("packages/form-contracts/fixtures/0619e-normal.json");
  await renderEnvelope(page, fixture);
  const formPage = page.locator(".form-page").first();

  await expectCriticalRegionGeometry(formPage, [
    { name: "Item 1 value band", selector: ".month-value-0619e", x: 36, y: 260, width: 254, height: 39 },
    { name: "Item 2 value band", selector: ".due-date-value-0619e", x: 292, y: 260, width: 228, height: 39 },
    { name: "signature writing area", selector: ".signature-body-0619e", x: 36, y: 1002, width: 1151, height: 87 },
    { name: "signature labels", selector: ".signature-labels-0619e", x: 36, y: 1089, width: 1151, height: 53 },
    { name: "tax agent footer", selector: ".signature-footer-0619e", x: 36, y: 1142, width: 1151, height: 45 }
  ]);

  for (const selector of [".month-value-0619e", ".due-date-value-0619e"]) {
    await expect.poll(async () => page.locator(selector).evaluate((element) => ({
      background: getComputedStyle(element, "::before").backgroundColor,
      overflow: element.scrollHeight - element.clientHeight
    }))).toEqual({ background: "rgb(217, 217, 217)", overflow: 0 });
  }

  await expect(page.locator(".signature-labels-0619e")).toHaveCSS(
    "background-color",
    "rgb(217, 217, 217)"
  );
  await expect(page.locator(".declaration-0619e > p")).toHaveCSS(
    "background-color",
    "rgb(217, 217, 217)"
  );
  expect(await page.locator(".signature-footer-0619e").evaluate(
    (element) => element.scrollHeight - element.clientHeight
  )).toBe(0);
});

test("0619E 2018 preserves official payment row partitions", async ({ page }) => {
  const fixture = readFixture("packages/form-contracts/fixtures/0619e-normal.json");
  await renderEnvelope(page, fixture);
  const formPage = page.locator(".form-page").first();

  await expectCriticalRegionGeometry(formPage, [
    { name: "payment heading", selector: ".payment-0619e > h2", x: 36, y: 1192, width: 1151, height: 24 },
    { name: "payment column heading", selector: ".payment-head-0619e", x: 36, y: 1216, width: 1151, height: 22 },
    { name: "payment Item 19", selector: "[data-payment-row='payment_19']", x: 36, y: 1238, width: 1151, height: 36 },
    { name: "payment Item 20", selector: "[data-payment-row='payment_20']", x: 36, y: 1274, width: 1151, height: 37 },
    { name: "payment Item 21", selector: "[data-payment-row='payment_21']", x: 36, y: 1311, width: 1151, height: 36 },
    { name: "payment Item 22 label", selector: ".payment-other-label-0619e", x: 36, y: 1347, width: 1151, height: 22 },
    { name: "payment Item 22 value row", selector: "[data-payment-row='payment_22']", x: 36, y: 1369, width: 1151, height: 38 }
  ]);

  await expect(
    page.locator("[data-payment-row='payment_21'] > :nth-child(2)")
  ).toHaveCSS("background-color", "rgb(217, 217, 217)");

  const datePartitions = await page
    .locator("[data-payment-row='payment_19'] > :nth-child(4)")
    .evaluate((element) => ({
      first: {
        border: getComputedStyle(element, "::before").borderLeftWidth,
        left: getComputedStyle(element, "::before").left
      },
      second: {
        border: getComputedStyle(element, "::after").borderLeftWidth,
        left: getComputedStyle(element, "::after").left
      }
    }));
  expect(datePartitions).toEqual({
    first: { border: "1px", left: "38px" },
    second: { border: "1px", left: "77.3333px" }
  });

  const decimalPartition = page.locator(
    "[data-payment-row='payment_19'] .decimal-separator-0619e"
  );
  await expect(decimalPartition).toHaveCSS("border-left-width", "1px");
  await expect(decimalPartition).toHaveCSS("border-right-width", "1px");
});

test("0619E 2018 matches the complete pinned official page", async ({ page }, testInfo) => {
  const fixture = readFixture("packages/form-contracts/fixtures/0619e-normal.json");
  await renderEnvelope(page, fixture);
  const formPage = page.locator(".form-page").first();
  await expect(formPage).toHaveCount(1);

  await expectCriticalRegionGeometry(formPage, [
    { name: "government header", selector: ".government-header-0619e", x: 35, y: 54, width: 1154, height: 59 },
    { name: "masthead", selector: ".masthead-0619e", x: 35, y: 113, width: 1154, height: 126 },
    { name: "Items 1-6", selector: ".header-options-0619e", x: 35, y: 239, width: 1154, height: 62 },
    { name: "Part I", selector: ".background-0619e", x: 35, y: 304, width: 1154, height: 310 },
    { name: "Part II", selector: ".remittance-0619e", x: 35, y: 616, width: 1154, height: 340 },
    { name: "declaration", selector: ".declaration-0619e", x: 35, y: 956, width: 1154, height: 231 },
    { name: "Part III", selector: ".payment-0619e", x: 35, y: 1190, width: 1154, height: 347 }
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
    "packages/form-renderer/references/0619e-2018-page-1.png"
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
  console.log(`0619E complete-page parity: ${JSON.stringify({
    structuralChangedPercent,
    fullPageChangedPercent: completePage.fullPageChangedPercent,
    expectedInkMissingPercent: completePage.expectedInkMissingPercent,
    unexpectedActualInkPercent: completePage.unexpectedActualInkPercent
  })}`);
  fs.writeFileSync(testInfo.outputPath("0619e-page-1-actual.png"), actualBuffer);
  fs.writeFileSync(
    testInfo.outputPath("0619e-page-1-structure-diff.png"),
    PNG.sync.write(diff)
  );
  fs.writeFileSync(
    testInfo.outputPath("0619e-page-1-full-page-diff.png"),
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
    console.warn(`0619E overflow report: ${JSON.stringify({ report, offenders })}`);
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
