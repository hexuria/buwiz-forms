import { expect, test, type Locator, type Page } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { PNG } from "pngjs";
import { compareCompleteOfficialPage } from "./official-page-diff";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(HERE, "../../..");
const MAX_FULL_PAGE_CHANGED_PERCENT = 1;
const STRUCTURAL_INK_THRESHOLD = 100;
const STRUCTURAL_LINE_MIN_RUN = 20;
const STRUCTURAL_TOLERANCE_RADIUS = 4;
const OFFICIAL_ARTWORK = [
  {
    payload: "1702-RT 01/18ENCS P1",
    symbol: { x: 432.6, y: 70.2, width: 157.384232, height: 38.347397 },
    caption: { x: 503.380005, y: 109.383774, width: 87.750183, height: 8.964599 }
  },
  {
    payload: "1702-RT 01/18ENCS P2",
    symbol: { x: 433.2, y: 91.8, width: 157.742739, height: 35.980274 },
    caption: { x: 504.100006, y: 128.343796, width: 87.750214, height: 8.964599 }
  },
  {
    payload: "1702-RT 01/18ENCS P3",
    symbol: { x: 432.84, y: 41.16, width: 157.623237, height: 37.045479 },
    caption: { x: 504.100006, y: 79.50383, width: 87.750214, height: 8.9646 }
  },
  {
    payload: "1702-RT 01/18ENCS P4",
    symbol: { x: 432.6, y: 43.2, width: 157.742739, height: 36.216986 },
    caption: { x: 503.380005, y: 79.50383, width: 87.750183, height: 8.9646 }
  }
] as const;

test("1702RT January 2018C renders every Rust fixture as four stable unclipped 612x936 pages", async ({ page }) => {
  for (const fixtureName of [
    "1702rt-minimum.json",
    "1702rt-normal.json",
    "1702rt-long-values.json",
    "1702rt-validation-edge.json",
    "1702rt-schedule-capacity.json"
  ]) {
    await renderEnvelope(page, readFixture(`packages/form-contracts/fixtures/${fixtureName}`));
    const pages = page.locator(".form-page");
    await expect(pages, fixtureName).toHaveCount(4);
    for (let pageIndex = 0; pageIndex < 4; pageIndex += 1) {
      await expect(pages.nth(pageIndex)).toHaveAttribute("data-paper", "folio");
      const box = await pages.nth(pageIndex).boundingBox();
      expect(box?.width, `${fixtureName} page ${pageIndex + 1} width`).toBeCloseTo(816, 0);
      expect(box?.height, `${fixtureName} page ${pageIndex + 1} height`).toBeCloseTo(1248, 0);
      expect(await pageHasNoOverflow(pages.nth(pageIndex)), `${fixtureName} page ${pageIndex + 1}`).toBe(true);
    }
  }
});

test("1702RT January 2018C uses exact native seal and page-specific PDF417 geometry", async ({ page }) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/1702rt-normal.json"));
  const pages = page.locator(".form-page");
  await expect(pages).toHaveCount(4);
  expect(await pages.nth(0).locator("img").count()).toBe(1);
  for (let pageIndex = 0; pageIndex < 4; pageIndex += 1) {
    const formPage = pages.nth(pageIndex);
    if (pageIndex > 0) expect(await formPage.locator("img").count()).toBe(0);
    await expect(formPage.locator(".official-pdf417-symbol-1702rt")).toHaveCount(1);
    await expect(formPage.locator(".barcode-1702rt")).toHaveAttribute(
      "aria-label",
      OFFICIAL_ARTWORK[pageIndex].payload
    );
    const measured = await measureArtworkInPoints(formPage);
    expectArtworkBox(measured.symbol, OFFICIAL_ARTWORK[pageIndex].symbol);
    expectArtworkBox(measured.caption, OFFICIAL_ARTWORK[pageIndex].caption);
    expect(measured.captionFontFamily).toContain("eBIRForms Arimo");
    expect(measured.captionFontSizePoints).toBeCloseTo(8.04, 2);
  }
  expectArtworkBox(
    await measureElementInPoints(pages.nth(0), ".government-wordmark-1702rt img"),
    { x: 228.52, y: 28.968, width: 33.682, height: 28.152 }
  );
});

test("1702RT January 2018C preserves the official Schedule IIIA header and dead-cell partitions", async ({ page }) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/1702rt-normal.json"));
  const pageFour = page.locator(".form-page").nth(3);
  const headerCells = pageFour.locator(".nolco-primary-heading-1702rt > span");
  const firstRowCells = pageFour.locator('[data-nolco-item="4"] > *');

  await expect(headerCells).toHaveCount(4);
  await expect(firstRowCells).toHaveCount(7);
  expect(await cellWidthsInPoints(headerCells)).toEqual([379, 151.5, 227.5, 196.5]);
  expect(await cellWidthsInPoints(firstRowCells)).toEqual([30, 30, 61, 61, 182, 30, 181.5]);
  expect(await firstRowCells.evaluateAll((cells) => cells.map((cell) => getComputedStyle(cell).backgroundColor))).toEqual([
    "rgb(217, 217, 217)",
    "rgb(166, 166, 166)",
    "rgb(255, 255, 255)",
    "rgb(166, 166, 166)",
    "rgb(255, 255, 255)",
    "rgb(166, 166, 166)",
    "rgb(255, 255, 255)"
  ]);
});

test("1702RT January 2018C preserves the official Item 10 through 12 label partitions", async ({ page }) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/1702rt-normal.json"));
  const pageOne = page.locator(".form-page").first();
  const labels = pageOne.locator([
    ".split-identity-1702rt > .labeled-comb-1702rt:first-child > span:first-child",
    ".split-identity-1702rt > .labeled-comb-1702rt:last-child > span:first-child",
    ".part-one-1702rt > .labeled-comb-1702rt > span:first-child"
  ].join(", "));

  await expect(labels).toHaveCount(3);
  expect(await cellWidthsInPoints(labels)).toEqual([166.5, 90.5, 90.5]);
  expect(await labels.evaluateAll((cells) => cells.map((cell) => getComputedStyle(cell).backgroundColor))).toEqual([
    "rgb(217, 217, 217)",
    "rgb(217, 217, 217)",
    "rgb(217, 217, 217)"
  ]);
});

test("1702RT January 2018C preserves the official Item 1 and Item 2 hierarchy", async ({ page }) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/1702rt-normal.json"));
  const pageOne = page.locator(".form-page").first();
  const basis = pageOne.locator(".basis-1702rt");

  await expect(basis.locator(":scope > div")).toHaveCount(2);
  await expect(basis.locator(".basis-item-two-1702rt > span:first-child")).toHaveText(
    "2 Year Ended (MM/20YY)"
  );
  expectArtworkBox(await measureElementInPoints(pageOne, ".year-ended-1702rt"), {
    x: 33.5,
    y: 152,
    width: 75,
    height: 12.5
  });
  expect(await cellWidthsInPoints(basis.locator(".year-ended-1702rt > *"))).toEqual([
    27,
    22,
    26
  ]);
});

test("1702RT January 2018C matches the complete official pages", async ({ page }, testInfo) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/1702rt-normal.json"));
  const pages = page.locator(".form-page");
  await expect(pages).toHaveCount(4);

  await page.addStyleTag({
    content: `
      .form-page[data-visual-blank-values="true"] .comb-value > span,
      .form-page[data-visual-blank-values="true"] .adaptive-plain-value,
      .form-page[data-visual-blank-values="true"] .plain-value-1702rt,
      .form-page[data-visual-blank-values="true"] .check-box,
      .form-page[data-visual-blank-values="true"] .money-1702rt > span,
      .form-page[data-visual-blank-values="true"] .signature-space-1702rt,
      .form-page[data-visual-blank-values="true"] .signature-details-1702rt b {
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
      `packages/form-renderer/references/1702rt-2018c-page-${pageIndex + 1}.png`
    ));
    const actualBuffer = await pages.nth(pageIndex).screenshot({ animations: "disabled", caret: "hide" });
    const expected = PNG.sync.read(expectedBuffer);
    const actual = PNG.sync.read(actualBuffer);
    expect(actual.width).toBe(expected.width);
    expect(actual.height).toBe(expected.height);
    const { changedPixels, diff } = compareOfficialStructure(expected, actual);
    const structuralChangedPercent = changedPixels * 100 / (expected.width * expected.height);
    const completePage = compareCompleteOfficialPage(expected, actual);
    results.push({
      page: pageIndex + 1,
      structuralChangedPercent,
      fullPageChangedPercent: completePage.fullPageChangedPercent,
      expectedInkMissingPercent: completePage.expectedInkMissingPercent,
      unexpectedActualInkPercent: completePage.unexpectedActualInkPercent
    });
    fs.writeFileSync(testInfo.outputPath(`1702rt-page-${pageIndex + 1}-actual.png`), actualBuffer);
    fs.writeFileSync(testInfo.outputPath(`1702rt-page-${pageIndex + 1}-structure-diff.png`), PNG.sync.write(diff));
    fs.writeFileSync(
      testInfo.outputPath(`1702rt-page-${pageIndex + 1}-full-page-diff.png`),
      PNG.sync.write(completePage.diff)
    );
  }
  console.log(`1702RT complete-page parity: ${JSON.stringify(results)}`);
  // Retain the ruled-line number only to help localize geometry drift. It is
  // intentionally not an acceptance assertion: sparse long rules cannot prove
  // that the labels, instructions, short marks, or field composition match.
  expect(
    results.filter((result) => result.fullPageChangedPercent > MAX_FULL_PAGE_CHANGED_PERCENT),
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
        text: child.textContent?.trim().slice(0, 120)
      })));
    console.warn(`1702RT overflow report: ${JSON.stringify({ report, offenders })}`);
  }
  return valid;
}

async function measureArtworkInPoints(locator: Locator) {
  return locator.evaluate((element) => {
    const pageBox = element.getBoundingClientRect();
    const symbol = element.querySelector<HTMLElement>(".official-pdf417-object-1702rt");
    const caption = element.querySelector<HTMLElement>(".barcode-1702rt > small");
    if (!symbol || !caption) throw new Error("1702RT official artwork is unavailable");
    const toPoints = (box: DOMRect) => ({
      x: (box.x - pageBox.x) * 0.75,
      y: (box.y - pageBox.y) * 0.75,
      width: box.width * 0.75,
      height: box.height * 0.75
    });
    const captionStyle = getComputedStyle(caption);
    return {
      symbol: toPoints(symbol.getBoundingClientRect()),
      caption: toPoints(caption.getBoundingClientRect()),
      captionFontFamily: captionStyle.fontFamily,
      captionFontSizePoints: Number.parseFloat(captionStyle.fontSize) * 0.75
    };
  });
}

async function measureElementInPoints(locator: Locator, selector: string) {
  return locator.evaluate((element, childSelector) => {
    const pageBox = element.getBoundingClientRect();
    const child = element.querySelector<HTMLElement>(childSelector);
    if (!child) throw new Error(`1702RT artwork ${childSelector} is unavailable`);
    const box = child.getBoundingClientRect();
    return {
      x: (box.x - pageBox.x) * 0.75,
      y: (box.y - pageBox.y) * 0.75,
      width: box.width * 0.75,
      height: box.height * 0.75
    };
  }, selector);
}

function expectArtworkBox(
  actual: { x: number; y: number; width: number; height: number },
  expected: { x: number; y: number; width: number; height: number }
) {
  expect(actual.x).toBeCloseTo(expected.x, 1);
  expect(actual.y).toBeCloseTo(expected.y, 1);
  expect(actual.width).toBeCloseTo(expected.width, 1);
  expect(actual.height).toBeCloseTo(expected.height, 1);
}

async function cellWidthsInPoints(locator: Locator) {
  return locator.evaluateAll((elements) => elements.map((element) =>
    Math.round(element.getBoundingClientRect().width * 0.75 * 10) / 10
  ));
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
