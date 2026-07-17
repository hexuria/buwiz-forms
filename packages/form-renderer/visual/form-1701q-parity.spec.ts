import { expect, test, type Locator, type Page } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { PNG } from "pngjs";
import { compareCompleteOfficialPage } from "./official-page-diff";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(HERE, "../../..");
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
      expect(await pageHasNoOverflow(pages.nth(pageIndex)), `${fixtureName} page ${pageIndex + 1}`).toBe(true);
    }
  }
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
      `packages/form-renderer/references/1701q-2018-page-${pageIndex + 1}.png`
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
