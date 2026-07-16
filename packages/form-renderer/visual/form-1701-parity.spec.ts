import { expect, test, type Locator, type Page } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { PNG } from "pngjs";
import { compareCompleteOfficialPage } from "./official-page-diff";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(HERE, "../../..");
const MAX_CHANGED_PERCENT = 1;
const STRUCTURAL_INK_THRESHOLD = 100;
const STRUCTURAL_LINE_MIN_RUN = 20;
const STRUCTURAL_TOLERANCE_RADIUS = 4;

test("1701 2018 renders every Rust fixture as four stable unclipped 612x936 pages", async ({ page }) => {
  for (const fixtureName of [
    "1701-minimum.json",
    "1701-normal.json",
    "1701-long-values.json",
    "1701-validation-edge.json",
    "1701-fixed-capacity.json"
  ]) {
    await renderEnvelope(page, readFixture(`packages/form-contracts/fixtures/${fixtureName}`));
    const pages = page.locator(".form-page");
    await expect(pages, fixtureName).toHaveCount(4);
    for (let pageIndex = 0; pageIndex < 4; pageIndex += 1) {
      await expect(pages.nth(pageIndex)).toHaveAttribute("data-paper", "folio");
      expect(await pageHasNoOverflow(pages.nth(pageIndex)), `${fixtureName} page ${pageIndex + 1}`).toBe(true);
    }
  }
});

test("1701 2018 matches the complete official pages", async ({ page }, testInfo) => {
  await renderEnvelope(page, readFixture("packages/form-contracts/fixtures/1701-normal.json"));
  const pages = page.locator(".form-page");
  await expect(pages).toHaveCount(4);

  await page.addStyleTag({ content: `
    .form-page[data-visual-blank-values="true"] :is(
      .comb-value > span,
      .adaptive-plain-value,
      .check-box,
      .amount-1701
    ) { color: transparent !important; text-shadow: none !important; }
  ` });

  const pageResults: Array<{
    page: number;
    structuralChangedPercent: number;
    fullPageChangedPercent: number;
    expectedInkMissingPercent: number;
    unexpectedActualInkPercent: number;
  }> = [];
  console.log("1701 spouse regions", await pages.nth(1).locator(".spouse-background-1701 > *").evaluateAll((children) => children.map((child) => ({ class_name: child.className, height: child.getBoundingClientRect().height }))));
  for (let pageIndex = 0; pageIndex < 4; pageIndex += 1) {
    const renderedPage = pages.nth(pageIndex);
    console.log(`1701 page ${pageIndex + 1} regions`, await renderedPage.locator(":scope > *").evaluateAll((children) => children.map((child) => {
      const rect = child.getBoundingClientRect();
      const parent = child.parentElement?.getBoundingClientRect();
      return { class_name: child.className, top: rect.top - (parent?.top ?? 0), height: rect.height };
    })));
    await renderedPage.evaluate((element) => element.setAttribute("data-visual-blank-values", "true"));
    const referencePath = path.join(REPO_ROOT, `packages/form-renderer/references/1701-2018-page-${pageIndex + 1}.png`);
    const expectedBuffer = fs.readFileSync(referencePath);
    const actualBuffer = await renderedPage.screenshot({ animations: "disabled", caret: "hide" });
    const expected = PNG.sync.read(expectedBuffer);
    const actual = PNG.sync.read(actualBuffer);
    expect(actual.width).toBe(expected.width);
    expect(actual.height).toBe(expected.height);
    const { changedPixels, diff } = compareOfficialStructure(expected, actual);
    const completePage = compareCompleteOfficialPage(expected, actual);
    fs.writeFileSync(testInfo.outputPath(`1701-page-${pageIndex + 1}-actual.png`), actualBuffer);
    fs.writeFileSync(testInfo.outputPath(`1701-page-${pageIndex + 1}-structure-diff.png`), PNG.sync.write(diff));
    fs.writeFileSync(
      testInfo.outputPath(`1701-page-${pageIndex + 1}-full-page-diff.png`),
      PNG.sync.write(completePage.diff)
    );
    pageResults.push({
      page: pageIndex + 1,
      structuralChangedPercent: changedPixels * 100 / (expected.width * expected.height),
      fullPageChangedPercent: completePage.fullPageChangedPercent,
      expectedInkMissingPercent: completePage.expectedInkMissingPercent,
      unexpectedActualInkPercent: completePage.unexpectedActualInkPercent
    });
  }
  console.log(`1701 complete-page parity: ${JSON.stringify(pageResults)}`);
  for (const result of pageResults) {
    expect(
      result.fullPageChangedPercent,
      `1701 page ${result.page} complete pixels, including all static labels, instructions, fields, signatures, and artwork`
    ).toBeLessThanOrEqual(MAX_CHANGED_PERCENT);
  }
});

async function pageHasNoOverflow(locator: Locator) {
  const report = await locator.evaluate((element) => {
    const measurement = (window as Window & { measureEbirFormGeometry?: () => { pages: Array<{ client_height: number; client_width: number; descendant_clipped_x: number; descendant_clipped_y: number; descendant_overflow_x: number; descendant_overflow_y: number; scroll_height: number; scroll_width: number; }> } | null }).measureEbirFormGeometry?.();
    if (!measurement) throw new Error("renderer measurement unavailable");
    return measurement.pages[[...document.querySelectorAll(".form-page")].indexOf(element)];
  });
  const valid = report.scroll_height <= report.client_height + 1 &&
    report.scroll_width <= report.client_width + 1 &&
    report.descendant_overflow_x === 0 && report.descendant_overflow_y === 0 &&
    report.descendant_clipped_x === 0 && report.descendant_clipped_y === 0;
  if (!valid) {
    const details = await locator.evaluate((element) => ({
      offenders: [...element.querySelectorAll<HTMLElement>("*")]
        .filter((child) => child.scrollWidth > child.clientWidth + 1.25 || child.scrollHeight > child.clientHeight + 1.25)
        .map((child) => ({ class_name: child.className, client_width: child.clientWidth, scroll_width: child.scrollWidth, client_height: child.clientHeight, scroll_height: child.scrollHeight, text: child.textContent?.trim().slice(0, 100) })),
      direct_children: [...element.children].map((child) => ({ class_name: child.className, height: child.getBoundingClientRect().height })),
      background_children: [...(element.querySelector(".background-1701")?.children ?? [])]
        .map((child) => ({ class_name: child.className, height: child.getBoundingClientRect().height }))
    }));
    console.warn(`1701 overflow report: ${JSON.stringify({ report, ...details })}`);
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
    if (changed[index] === 1) { changedPixels += 1; diff.data[offset] = 255; diff.data[offset + 3] = 255; }
  }
  return { changedPixels, diff };
}

function structuralLineMask(image: PNG) {
  const dark = new Uint8Array(image.width * image.height);
  const lines = new Uint8Array(dark.length);
  for (let index = 0; index < dark.length; index += 1) {
    const offset = index * 4;
    dark[index] = image.data[offset] < STRUCTURAL_INK_THRESHOLD && image.data[offset + 1] < STRUCTURAL_INK_THRESHOLD && image.data[offset + 2] < STRUCTURAL_INK_THRESHOLD ? 1 : 0;
  }
  for (let y = 0; y < image.height; y += 1) {
    let start = -1;
    for (let x = 0; x <= image.width; x += 1) {
      const ink = x < image.width && dark[y * image.width + x] === 1;
      if (ink && start < 0) start = x;
      if (!ink && start >= 0) { if (x - start >= STRUCTURAL_LINE_MIN_RUN) for (let fill = start; fill < x; fill += 1) lines[y * image.width + fill] = 1; start = -1; }
    }
  }
  for (let x = 0; x < image.width; x += 1) {
    let start = -1;
    for (let y = 0; y <= image.height; y += 1) {
      const ink = y < image.height && dark[y * image.width + x] === 1;
      if (ink && start < 0) start = y;
      if (!ink && start >= 0) { if (y - start >= STRUCTURAL_LINE_MIN_RUN) for (let fill = start; fill < y; fill += 1) lines[fill * image.width + x] = 1; start = -1; }
    }
  }
  return lines;
}

function markUnmatchedStructure(source: Uint8Array, target: Uint8Array, changed: Uint8Array, width: number, height: number) {
  for (let y = 0; y < height; y += 1) for (let x = 0; x < width; x += 1) {
    const index = y * width + x;
    if (source[index] !== 1) continue;
    let matched = false;
    for (let ty = Math.max(0, y - STRUCTURAL_TOLERANCE_RADIUS); ty <= Math.min(height - 1, y + STRUCTURAL_TOLERANCE_RADIUS) && !matched; ty += 1) {
      for (let tx = Math.max(0, x - STRUCTURAL_TOLERANCE_RADIUS); tx <= Math.min(width - 1, x + STRUCTURAL_TOLERANCE_RADIUS); tx += 1) if (target[ty * width + tx] === 1) { matched = true; break; }
    }
    if (!matched) changed[index] = 1;
  }
}
