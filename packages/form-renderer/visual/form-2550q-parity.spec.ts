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

test("2550Q April 2024 renders every Rust fixture as two stable unclipped 8.5x14 pages", async ({ page }) => {
  for (const fixtureName of [
    "2550q-minimum.json",
    "2550q-normal.json",
    "2550q-long-values.json",
    "2550q-validation-edge.json",
    "2550q-two-row-capacity.json"
  ]) {
    await renderEnvelope(page, readFixture(`packages/form-contracts/fixtures/${fixtureName}`));
    const pages = page.locator(".form-page");
    await expect(pages, fixtureName).toHaveCount(2);
    for (let pageIndex = 0; pageIndex < 2; pageIndex += 1) {
      await expect(pages.nth(pageIndex)).toHaveAttribute("data-paper", "legal");
      expect(await pageHasNoOverflow(pages.nth(pageIndex)), `${fixtureName} page ${pageIndex + 1}`).toBe(true);
    }
  }
});

test("2550Q April 2024 keeps verified page-specific PDF417, caption, and seal geometry", async ({ page }) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/2550q-normal.json"));
  const pages = page.locator(".form-page");
  await expect(pages).toHaveCount(2);

  await expectCriticalRegionGeometry(pages.nth(0), [
    {
      name: "official seal XObject",
      selector: ".government-wordmark-2550q img",
      x: 461.6,
      y: 34.4,
      width: 62.2,
      height: 59.4
    },
    {
      name: "page 1 official PDF417 XObject",
      selector: '.barcode-2550q[data-barcode-page="1"] .official-pdf417-object-2550q',
      x: 868.6,
      y: 102.6,
      width: 312.4,
      height: 93.4
    },
    {
      name: "page 1 official PDF417 live caption",
      selector: '.barcode-2550q[data-barcode-page="1"] > small',
      x: 1018.8,
      y: 199.312,
      width: 160.816,
      height: 14.8
    }
  ]);
  await expectCriticalRegionGeometry(pages.nth(1), [
    {
      name: "page 2 official PDF417 XObject",
      selector: '.barcode-2550q[data-barcode-page="2"] .official-pdf417-object-2550q',
      x: 867.8,
      y: 48.8,
      width: 307.8,
      height: 90.2
    },
    {
      name: "page 2 official PDF417 live caption",
      selector: '.barcode-2550q[data-barcode-page="2"] > small',
      x: 1011,
      y: 141.512,
      width: 160.816,
      height: 14.8
    }
  ]);

  expect(await page.locator(".official-pdf417-symbol-2550q").evaluateAll(
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

  const captions = page.locator(".barcode-2550q > small");
  await expect(captions.nth(0)).toHaveText("2550Q 04/24ENCS P1");
  await expect(captions.nth(1)).toHaveText("2550Q 04/24ENCS P2");
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
    fontSize: "10.6667px",
    fontWeight: "400",
    whiteSpace: "nowrap"
  })));

  expect(await page.locator(".government-wordmark-2550q img").evaluate(
    (image) => ({
      naturalHeight: (image as HTMLImageElement).naturalHeight,
      naturalWidth: (image as HTMLImageElement).naturalWidth
    })
  )).toEqual({ naturalHeight: 82, naturalWidth: 86 });
});

test("2550Q April 2024 preserves official disabled panels and page-one payment canvas", async ({ page }) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/2550q-normal.json"));
  const pages = page.locator(".form-page");

  await expectCriticalRegionGeometry(pages.nth(0), [
    {
      name: "Item 10A ZIP panel",
      selector: ".zip-2550q",
      x: 923,
      y: 486,
      width: 262,
      height: 38
    }
  ]);

  for (const item of [32, 33, 48, 49]) {
    await expect(
      pages.nth(1).locator(`.item-${item}-2550q > .disabled-cell-2550q`)
    ).toHaveCSS("background-color", "rgb(217, 217, 217)");
  }
  await expect(pages.nth(0).locator(".machine-validation-2550q"))
    .toHaveCSS("background-color", "rgb(255, 255, 255)");
  await expect(pages.nth(0).locator(".privacy-note-2550q"))
    .toHaveText("*NOTE: The BIR Data Privacy Policy is in the BIR website (www.bir.gov.ph)");

  const blankPayment = pages.nth(0).locator(".payment-row-2550q").first();
  await expect(blankPayment.locator(".comb-value")).toHaveCount(5);
  await expect(blankPayment.locator(".decimal-cell-2550q")).toHaveCount(1);
});

test("2550Q April 2024 preserves the official Schedule 2 fraction and result bands", async ({ page }) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/2550q-normal.json"));
  const schedule = page.locator(".schedule-two-2550q");

  await expect(schedule.locator(".schedule-two-fraction-2550q")).toHaveCount(1);
  await expect(schedule.locator(".schedule-two-fraction-2550q > span")).toHaveCount(2);
  await expect(schedule.locator(".schedule-two-formula-2550q"))
    .toHaveCSS("background-color", "rgb(217, 217, 217)");
  await expect(schedule.locator(".schedule-two-results-2550q > .schedule-money-2550q")).toHaveCount(3);
  await expect(schedule.locator(".schedule-two-results-2550q > .schedule-money-2550q").nth(0))
    .toHaveCSS("border-bottom-style", "solid");
  await expect(schedule.locator(".schedule-two-results-2550q > .schedule-money-2550q").nth(1))
    .toHaveCSS("border-bottom-style", "solid");
  await expect(schedule.locator(".schedule-two-results-2550q > .schedule-money-2550q").nth(2))
    .toHaveCSS("border-bottom-style", "none");
  const resultHeights = await schedule.locator(".schedule-two-results-2550q > .schedule-money-2550q")
    .evaluateAll((elements) => elements.map((element) => element.getBoundingClientRect().height));
  expect(resultHeights[1]).toBeGreaterThan(resultHeights[0] * 2);
  expect(Math.abs(resultHeights[2] - resultHeights[0])).toBeLessThan(1);
});

test("2550Q April 2024 matches the complete official pages", async ({ page }, testInfo) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/2550q-normal.json"));
  const pages = page.locator(".form-page");
  await expect(pages).toHaveCount(2);

  await expectCriticalRegionGeometry(pages.nth(0), [
    { name: "masthead", selector: ".masthead-2550q", x: 43, y: 98, width: 1143, height: 118 },
    { name: "Items 1-6", selector: ".header-options-2550q", x: 43, y: 216, width: 1143, height: 94 },
    { name: "Part I", selector: ".part-one-2550q", x: 43, y: 313, width: 1143, height: 348 },
    { name: "Part II", selector: ".part-two-2550q", x: 43, y: 664, width: 1143, height: 487 },
    { name: "declaration", selector: ".declaration-2550q", x: 43, y: 1155, width: 1143, height: 249 },
    { name: "Part III", selector: ".part-three-2550q", x: 43, y: 1410, width: 1143, height: 365 }
  ]);
  await expectCriticalRegionGeometry(pages.nth(1), [
    { name: "masthead", selector: ".masthead-2550q", x: 43, y: 43, width: 1137, height: 116 },
    { name: "identity", selector: ".page-two-identity-2550q", x: 43, y: 159, width: 1137, height: 57 },
    { name: "Part IV", selector: ".part-four-2550q", x: 43, y: 221, width: 1137, height: 1117 },
    { name: "Part V", selector: ".part-five-2550q", x: 43, y: 1339, width: 1139, height: 500 }
  ]);

  expect(await pages.nth(0).locator("img").count()).toBe(1);
  expect(await pages.nth(1).locator("img").count()).toBe(0);
  expect(await pages.nth(0).locator(".official-pdf417-symbol-2550q").count()).toBe(1);
  expect(await pages.nth(1).locator(".official-pdf417-symbol-2550q").count()).toBe(1);

  await page.addStyleTag({
    content: `
      .form-page[data-visual-blank-values="true"] .comb-value > span,
      .form-page[data-visual-blank-values="true"] .adaptive-plain-value,
      .form-page[data-visual-blank-values="true"] .plain-value-2550q,
      .form-page[data-visual-blank-values="true"] .value-line-2550q,
      .form-page[data-visual-blank-values="true"] .check-box,
      .form-page[data-visual-blank-values="true"] .schedule-money-2550q {
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
  for (let pageIndex = 0; pageIndex < 2; pageIndex += 1) {
    const expectedBuffer = fs.readFileSync(path.join(
      REPO_ROOT,
      `packages/form-renderer/references/2550q-2024-page-${pageIndex + 1}.png`
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
    fs.writeFileSync(testInfo.outputPath(`2550q-page-${pageIndex + 1}-actual.png`), actualBuffer);
    fs.writeFileSync(testInfo.outputPath(`2550q-page-${pageIndex + 1}-structure-diff.png`), PNG.sync.write(diff));
    fs.writeFileSync(
      testInfo.outputPath(`2550q-page-${pageIndex + 1}-full-page-diff.png`),
      PNG.sync.write(completePage.diff)
    );
  }
  console.log(`2550Q complete-page parity: ${JSON.stringify(results)}`);
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
    const actual = { x: box.x - pageBox.x, y: box.y - pageBox.y, width: box.width, height: box.height };
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
    console.warn(`2550Q overflow report: ${JSON.stringify({ report, offenders })}`);
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
    if (changed[index] !== 1) continue;
    changedPixels += 1;
    diff.data[index * 4] = 255;
    diff.data[index * 4 + 3] = 255;
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
