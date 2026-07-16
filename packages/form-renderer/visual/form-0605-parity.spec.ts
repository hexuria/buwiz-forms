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
