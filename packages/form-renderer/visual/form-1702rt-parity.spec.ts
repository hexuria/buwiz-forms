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

test("1702RT January 2018C matches the complete official pages", async ({ page }, testInfo) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/1702rt-normal.json"));
  const pages = page.locator(".form-page");
  await expect(pages).toHaveCount(4);
  expect(await pages.nth(0).locator("img").count()).toBe(2);
  for (let pageIndex = 1; pageIndex < 4; pageIndex += 1) {
    expect(await pages.nth(pageIndex).locator("img").count()).toBe(1);
  }

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
