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
    { name: "guidelines", selector: ".guidelines-1601c", x: 35, y: 609, width: 1153, height: 1050 }
  ]);

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
      `packages/form-renderer/references/1601c-2018-page-${index + 1}.png`
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
