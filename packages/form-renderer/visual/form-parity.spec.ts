import { expect, test, type Locator, type Page } from "@playwright/test";
import { spawnSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import pixelmatch from "pixelmatch";
import { PNG } from "pngjs";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(HERE, "../../..");
const MAX_CHANGED_PERCENT = Number(
  process.env.FORM_VISUAL_MAX_CHANGED_PERCENT ?? "1"
);
const PIXELMATCH_THRESHOLD = 0.1;
const DEVICE_SCALE_FACTOR = 1.5;
const VISUAL_EVIDENCE_PRODUCER = "playwright-form-parity-v1";
const VISUAL_EVIDENCE_PRODUCER_PATH =
  "packages/form-renderer/visual/form-parity.spec.ts";
const NON_PROMOTING_ALLOW_DIRTY_SOURCE =
  process.env.FORM_VISUAL_NON_PROMOTING_ALLOW_DIRTY_SOURCE === "1";
// Resolve and, for promotion-capable runs, validate the curated source before
// Playwright loads a page. An explicit dirty-source opt-out is diagnostic only
// and writes a report that the migration audit will refuse as evidence.
const VISUAL_SOURCE_REVISION = curatedSourceRevision();

interface ParityCase {
  code: string;
  name: string;
  revision: string;
  fixture: string;
  references: string[];
  expectedRealRowKeys: number;
}

interface VisualPageMetric {
  form_code: string;
  form_revision: string;
  fixture: string;
  fixture_sha256: string;
  reference: string;
  reference_sha256: string;
  actual: string;
  actual_sha256: string;
  diff: string | null;
  diff_sha256: string | null;
  page: number;
  expected_width: number;
  expected_height: number;
  actual_width: number;
  actual_height: number;
  changed_pixels: number | null;
  changed_percent: number | null;
  max_changed_percent: number;
  pixelmatch_threshold: number;
  passed: boolean;
}

const visualPageMetrics: VisualPageMetric[] = [];

const cases: ParityCase[] = [
  {
    code: "2551Q",
    name: "2551Q 2018",
    revision: "2018",
    fixture: "packages/form-contracts/fixtures/2551q-6-rows.json",
    references: [
      "packages/form-renderer/references/2551q-2018-page-1.png",
      "packages/form-renderer/references/2551q-2018-page-2.png"
    ],
    expectedRealRowKeys: 6
  }
];

for (const parityCase of cases) {
  test(`${parityCase.name} matches the official first two pages`, async ({
    page
  }, testInfo) => {
    visualPageMetrics.length = 0;
    const fixtureBuffer = fs.readFileSync(
      path.join(REPO_ROOT, parityCase.fixture)
    );
    const fixtureSha256 = sha256(fixtureBuffer);
    const envelope = JSON.parse(fixtureBuffer.toString("utf8")) as unknown;

    await renderEnvelope(page, envelope);

    const pages = page.locator(".form-page");
    await expect(pages).toHaveCount(parityCase.references.length);

    const compactName = pages.nth(1).locator('[data-overflow-mode="compact"]');
    await expect(compactName).toHaveCount(1);
    await expect(compactName.locator(".compact-comb-text")).toHaveAttribute(
      "aria-label",
      "RENDERER FIXTURE CORPORATION"
    );
    const compactGeometry = await compactName.evaluate((element) => {
        const boundary = element.getBoundingClientRect();
        const characters = element.querySelectorAll(".compact-comb-text > span");
        return {
          clientWidth: element.clientWidth,
          clientHeight: element.clientHeight,
          scrollWidth: element.scrollWidth,
          scrollHeight: element.scrollHeight,
          outsideCharacters: [...characters].filter((character) => {
            const rect = character.getBoundingClientRect();
            return !(
              rect.left >= boundary.left - 0.5 &&
              rect.right <= boundary.right + 0.5 &&
              rect.top >= boundary.top - 0.5 &&
              rect.bottom <= boundary.bottom + 0.5
            );
          }).length
        };
      });
    expect(compactGeometry.scrollWidth).toBeLessThanOrEqual(
      compactGeometry.clientWidth + 1
    );
    expect(compactGeometry.outsideCharacters).toBe(0);

    const realRowKeys = await page
      .locator("[data-row-key]")
      .evaluateAll((rows) =>
        rows
          .map((row) => row.getAttribute("data-row-key"))
          .filter(
            (key): key is string =>
              key !== null &&
              !key.includes("-empty-") &&
              !key.startsWith("payment-empty-")
          )
      );
    expect(realRowKeys).toHaveLength(parityCase.expectedRealRowKeys);
    expect(new Set(realRowKeys).size).toBe(realRowKeys.length);

    const officialSchedulePage = pages.nth(1);
    await expect(officialSchedulePage.locator(".official-schedule-row")).toHaveCount(6);
    await expect(officialSchedulePage.locator(".official-atc-table .atc-entry")).toHaveCount(22);
    await expect(officialSchedulePage.locator(".official-atc-table .atc-category")).toHaveCount(3);
    await expect(officialSchedulePage.locator(".official-atc-table")).toContainText(
      "Tax on Banks and Non-Bank Financial Intermediaries Performing Quasi-Banking Functions"
    );
    await expect(officialSchedulePage.locator(".official-atc-table")).toContainText(
      "Agents of Foreign Insurance Companies"
    );
    expect(await pageHasNoOverflow(officialSchedulePage)).toBe(true);

    for (const [pageIndex, referencePath] of parityCase.references.entries()) {
      const renderedPage = pages.nth(pageIndex);
      const expectedBuffer = fs.readFileSync(
        path.join(REPO_ROOT, referencePath)
      );
      const expectedImage = PNG.sync.read(expectedBuffer);
      const box = await renderedPage.boundingBox();
      expect(box).not.toBeNull();
      expect(box?.width).toBeCloseTo(
        expectedImage.width / DEVICE_SCALE_FACTOR,
        0
      );
      expect(box?.height).toBeCloseTo(
        expectedImage.height / DEVICE_SCALE_FACTOR,
        0
      );

      const actualBuffer = await renderedPage.screenshot({
        animations: "disabled",
        caret: "hide"
      });
      assertVisualParity({
        parityCase,
        pageNumber: pageIndex + 1,
        fixtureSha256,
        referencePath,
        actualBuffer,
        expectedBuffer,
        artifactStem: `${slug(parityCase.name)}-page-${pageIndex + 1}`,
        outputDir: testInfo.outputDir
      });
    }

    writeVisualEvidence();
    const failedPages = visualPageMetrics
      .filter((metric) => !metric.passed)
      .map((metric) => ({
        page: metric.page,
        changed_percent: metric.changed_percent,
        expected_width: metric.expected_width,
        expected_height: metric.expected_height,
        actual_width: metric.actual_width,
        actual_height: metric.actual_height
      }));
    expect(
      failedPages,
      `Visual parity must be at or below ${MAX_CHANGED_PERCENT}% changed pixels per page`
    ).toEqual([]);
  });
}

test("development preview fallback renders without a contract error", async ({ page }) => {
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));

  await page.goto("/");
  await page.locator(".form-document").waitFor();
  await expect(page.locator(".form-page")).toHaveCount(3);
  for (const renderedPage of await page.locator(".form-page").all()) {
    expect(await pageHasNoOverflow(renderedPage)).toBe(true);
  }
  expect(pageErrors).toEqual([]);
});

test("every canonical fixture produces stable, unclipped pages", async ({ page }) => {
  const fixtures = [
    "2551q-10-rows.json",
    "2551q-6-rows.json",
    "2551q-fiscal-period.json",
    "2551q-item13-eight-percent.json",
    "2551q-minimum.json",
    "2551q-overpayment-refund.json",
    "2551q-overpayment-tcc.json",
    "2551q-tax-relief.json"
  ];

  for (const fixture of fixtures) {
    const envelope = readFixture(`packages/form-contracts/fixtures/${fixture}`) as {
      schedules: Array<{ rows: unknown[] }>;
    };
    await renderEnvelope(page, envelope);
    const pages = page.locator(".form-page");
    const expectedPages = envelope.schedules[0].rows.length > 6 ? 3 : 2;
    await expect(pages, fixture).toHaveCount(expectedPages);
    for (let index = 0; index < expectedPages; index += 1) {
      expect(await pageHasNoOverflow(pages.nth(index)), `${fixture} page ${index + 1}`).toBe(true);
    }
  }

  const capacityEnvelope = readFixture(
    "packages/form-contracts/fixtures/2551q-tax-relief.json"
  ) as {
    fields: Record<string, { type: string; value: unknown }>;
    taxpayer: Record<string, string>;
  };
  capacityEnvelope.taxpayer.name = "N".repeat(40);
  capacityEnvelope.taxpayer.registered_address = "A".repeat(71);
  capacityEnvelope.taxpayer.contact_number = "+63 912 345 6789";
  capacityEnvelope.taxpayer.email = "renderer.2551q@example.com";
  capacityEnvelope.fields.tax_relief_specification.value =
    "Special law reference 1234";
  await renderEnvelope(page, capacityEnvelope);
  await expect(page.locator(".name-field .comb-value")).toContainText("N".repeat(40));
  const contactAndEmail = page.locator(".contact-email-field > .comb-value");
  expect((await contactAndEmail.nth(0).locator("> span").allTextContents()).join("").trimEnd()).toBe("639123456789");
  expect((await contactAndEmail.nth(1).locator("> span").allTextContents()).join("").trimEnd()).toBe("RENDERER.2551Q@EXAMPLE.COM");
  expect((await page.locator(".tax-relief-field > .comb-value > span").allTextContents()).join("").trimEnd()).toBe("SPECIAL LAW REFERENCE 1234");
  const addressCharacters = await page
    .locator(".address-field > .comb-value > span, .address-continuation > .comb-value:first-child > span")
    .allTextContents();
  expect(addressCharacters.join("").trimEnd()).toBe("A".repeat(71));
  for (const renderedPage of await page.locator(".form-page").all()) {
    expect(await pageHasNoOverflow(renderedPage)).toBe(true);
  }
});

test("geometry readiness sees overflow hidden descendants and clipping", async ({ page }) => {
  await renderEnvelope(
    page,
    readFixture("packages/form-contracts/fixtures/2551q-6-rows.json")
  );
  const firstPage = page.locator(".form-page").first();
  expect(await pageHasNoOverflow(firstPage)).toBe(true);

  await firstPage.evaluate((element) => {
    const clipped = document.createElement("div");
    clipped.dataset.geometryProbe = "clipped";
    Object.assign(clipped.style, {
      height: "10px",
      left: "30px",
      overflow: "hidden",
      position: "absolute",
      top: "30px",
      width: "20px"
    });
    const oversized = document.createElement("div");
    Object.assign(oversized.style, { height: "40px", width: "80px" });
    clipped.append(oversized);
    element.append(clipped);
  });

  const geometry = await measuredPage(firstPage);
  expect(geometry.descendant_overflow_x).toBeGreaterThan(0);
  expect(geometry.descendant_overflow_y).toBeGreaterThan(0);
  expect(geometry.descendant_clipped_x).toBeGreaterThan(0);
  expect(geometry.descendant_clipped_y).toBeGreaterThan(0);
});

function writeVisualEvidence() {
  const expectedPageCount = cases.reduce(
    (count, parityCase) => count + parityCase.references.length,
    0
  );
  const evidencePath = process.env.FORM_VISUAL_EVIDENCE_PATH
    ? path.resolve(process.env.FORM_VISUAL_EVIDENCE_PATH)
    : path.join(
        REPO_ROOT,
        "test-results/form-renderer/visual-evidence.json"
      );
  const referenceManifest = path.join(
    REPO_ROOT,
    "packages/form-renderer/references/manifest.json"
  );
  const report = {
    schema_version: 1,
    gate: "visual_parity",
    producer: VISUAL_EVIDENCE_PRODUCER,
    producer_path: VISUAL_EVIDENCE_PRODUCER_PATH,
    producer_sha256: sha256(
      fs.readFileSync(path.join(REPO_ROOT, VISUAL_EVIDENCE_PRODUCER_PATH))
    ),
    promotion_eligible: !NON_PROMOTING_ALLOW_DIRTY_SOURCE,
    source_worktree_clean: !NON_PROMOTING_ALLOW_DIRTY_SOURCE,
    generated_at: new Date().toISOString(),
    source_revision: VISUAL_SOURCE_REVISION,
    ci_run_id: process.env.GITHUB_RUN_ID ?? null,
    platform: process.platform,
    architecture: process.arch,
    browser: "chromium",
    device_scale_factor: DEVICE_SCALE_FACTOR,
    expected_page_count: expectedPageCount,
    measured_page_count: visualPageMetrics.length,
    references_manifest:
      "packages/form-renderer/references/manifest.json",
    references_manifest_sha256: sha256(
      fs.readFileSync(referenceManifest)
    ),
    passed:
      !NON_PROMOTING_ALLOW_DIRTY_SOURCE &&
      visualPageMetrics.length === expectedPageCount &&
      visualPageMetrics.every((metric) => metric.passed),
    pages: visualPageMetrics
  };

  fs.mkdirSync(path.dirname(evidencePath), { recursive: true });
  fs.writeFileSync(evidencePath, `${JSON.stringify(report, null, 2)}\n`);
}

function curatedSourceRevision(): string {
  const arguments_ = [
    path.join(REPO_ROOT, "scripts/run_python.mjs"),
    path.join(REPO_ROOT, "scripts/audit_html_form_migration.py"),
    "--root",
    REPO_ROOT,
    "--print-source-revision"
  ];
  if (!NON_PROMOTING_ALLOW_DIRTY_SOURCE) {
    arguments_.push("--require-clean-source");
  }
  const result = spawnSync(
    process.execPath,
    arguments_,
    { encoding: "utf8" }
  );
  if (result.status !== 0) {
    throw new Error(
      `Unable to resolve curated renderer source revision: ${result.stderr.trim()}`
    );
  }
  const revision = result.stdout.trim();
  if (!/^[0-9a-f]{40}(?:[0-9a-f]{24})?$/.test(revision)) {
    throw new Error(`Invalid curated renderer source revision: ${revision}`);
  }
  return revision;
}

test("2551Q continuation pages preserve every row and move totals to the final page", async ({
  page
}) => {
  const envelope = readFixture(
    "packages/form-contracts/fixtures/2551q-10-rows.json"
  );

  await renderEnvelope(page, envelope);

  const pages = page.locator(".form-page");
  await expect(pages).toHaveCount(3);
  await expect(pages.nth(1).locator("[data-row-key]")).toHaveCount(6);
  await expect(pages.nth(2).locator("[data-row-key]")).toHaveCount(4);
  await expect(pages.nth(1).locator(".official-schedule-row")).toHaveCount(6);
  await expect(pages.nth(2).locator(".official-schedule-row")).toHaveCount(12);
  await expect(pages.nth(2).locator("[data-visual-placeholder='true']")).toHaveCount(8);
  await expect(pages.nth(1).locator("[data-final-total='true']")).toHaveCount(0);
  await expect(pages.nth(1).locator("[data-final-total='false']")).toHaveCount(1);
  await expect(pages.nth(1).locator("[data-summary-kind='page_2_subtotal']")).toHaveCount(1);
  await expect(pages.nth(1).locator(".official-schedule-total")).toContainText(
    "Subtotal carried to Schedule 1 continuation"
  );
  await expect(pages.nth(1).locator(".official-schedule-total")).toHaveAttribute(
    "data-summary-value",
    "6300"
  );
  await expect(pages.nth(1).locator(".official-schedule-total")).not.toContainText(
    "To Part II Item 14"
  );
  await expect(pages.nth(2).locator("[data-final-total='true']")).toHaveCount(1);
  await expect(pages.nth(2).locator("[data-summary-kind='final_total']")).toHaveCount(1);
  await expect(pages.nth(2).locator(".official-schedule-total")).toHaveAttribute(
    "data-summary-value",
    "16500"
  );
  await expect(pages.nth(2).locator(".official-schedule-total")).toContainText(
    "To Part II Item 14"
  );
  await expect(pages.nth(2).locator(".official-schedule-head > span").first()).toContainText(
    /Alphanumeric Tax\s*Code/
  );
  await expect(pages.nth(2).locator(".page-two-identity")).toContainText(
    "Taxpayer’s Last Name"
  );
  await expect(pages.nth(2).locator(".official-atc-table")).toHaveCount(0);
  await expect(pages.nth(2).locator(".declaration, .official-declaration")).toHaveCount(0);
  expect(await pageHasNoOverflow(pages.nth(1))).toBe(true);
  expect(await pageHasNoOverflow(pages.nth(2))).toBe(true);
  await expectRealRowKeys(
    page,
    ["PT010", "PT040", "PT041", "PT060", "PT070", "PT090", "PT140", "PT150", "PT160", "PT170"]
      .map((atc, index) => `schedule-1-${index + 1}-${atc}`)
  );
});

async function pageHasNoOverflow(locator: Locator) {
  const geometry = await measuredPage(locator);
  const fits =
    geometry.scroll_height <= geometry.client_height + 1 &&
    geometry.scroll_width <= geometry.client_width + 1 &&
    geometry.descendant_overflow_x === 0 &&
    geometry.descendant_overflow_y === 0 &&
    geometry.descendant_clipped_x === 0 &&
    geometry.descendant_clipped_y === 0;
  if (!fits) {
    const offenders = await locator.evaluate((element) =>
      [...element.querySelectorAll<HTMLElement>("*")]
        .filter((child) =>
          child.scrollWidth > child.clientWidth + 1.25 ||
          child.scrollHeight > child.clientHeight + 1.25
        )
        .slice(0, 20)
        .map((child) => ({
          className: child.className,
          clientHeight: child.clientHeight,
          clientWidth: child.clientWidth,
          overflowX: getComputedStyle(child).overflowX,
          overflowY: getComputedStyle(child).overflowY,
          scrollHeight: child.scrollHeight,
          scrollWidth: child.scrollWidth,
          tagName: child.tagName,
          text: child.textContent?.trim().slice(0, 80)
        }))
    );
    console.error(`Form page overflow: ${JSON.stringify({ geometry, offenders })}`);
  }
  return fits;
}

async function measuredPage(locator: Locator) {
  return locator.evaluate((element) => {
    const hostWindow = window as Window & {
      measureEbirFormGeometry?: () => {
        pages: Array<{
          client_height: number;
          client_width: number;
          descendant_clipped_x: number;
          descendant_clipped_y: number;
          descendant_overflow_x: number;
          descendant_overflow_y: number;
          scroll_height: number;
          scroll_width: number;
        }>;
      } | null;
    };
    const report = hostWindow.measureEbirFormGeometry?.();
    if (!report) throw new Error("renderer geometry measurement is unavailable");
    const pageIndex = [...document.querySelectorAll(".form-page")].indexOf(element);
    const page = report.pages[pageIndex];
    if (!page) throw new Error(`renderer geometry omitted page ${pageIndex + 1}`);
    return page;
  });
}

function readFixture(relativePath: string): unknown {
  return JSON.parse(
    fs.readFileSync(path.join(REPO_ROOT, relativePath), "utf8")
  ) as unknown;
}

async function renderEnvelope(page: Page, envelope: unknown) {
  await page.goto("/");
  await page.waitForFunction(
    () =>
      typeof (
        window as Window & {
          renderEbirForm?: (value: unknown) => void;
        }
      ).renderEbirForm === "function"
  );
  await page.evaluate((value) => {
    const render = (
      window as Window & {
        renderEbirForm?: (input: unknown) => void;
      }
    ).renderEbirForm;
    if (!render) throw new Error("renderEbirForm is not installed");
    render(value);
  }, envelope);
  await page.locator(".form-document").waitFor();
  await page.evaluate(() =>
    new Promise<void>((resolve) => {
      requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
    })
  );
  await page.evaluate(() => document.fonts.ready);
}

async function expectRealRowKeys(page: Page, expected: string[]) {
  const keys = await page
    .locator("[data-row-key]")
    .evaluateAll((rows) =>
      rows
        .map((row) => row.getAttribute("data-row-key"))
        .filter(
          (key): key is string =>
            key !== null &&
            !key.includes("-empty-") &&
            !key.startsWith("payment-empty-")
        )
    );
  expect(keys).toEqual(expected);
  expect(new Set(keys).size).toBe(keys.length);
}

function assertVisualParity({
  parityCase,
  pageNumber,
  fixtureSha256,
  referencePath,
  actualBuffer,
  expectedBuffer,
  artifactStem,
  outputDir
}: {
  parityCase: ParityCase;
  pageNumber: number;
  fixtureSha256: string;
  referencePath: string;
  actualBuffer: Buffer;
  expectedBuffer: Buffer;
  artifactStem: string;
  outputDir: string;
}) {
  const actual = PNG.sync.read(actualBuffer);
  const expected = PNG.sync.read(expectedBuffer);
  const dimensionsMatch =
    actual.width === expected.width && actual.height === expected.height;

  if (!dimensionsMatch) {
    const artifacts = writeVisualArtifacts({
      actualBuffer,
      expectedBuffer,
      artifactStem,
      outputDir
    });
    visualPageMetrics.push({
      form_code: parityCase.code,
      form_revision: parityCase.revision,
      fixture: parityCase.fixture,
      fixture_sha256: fixtureSha256,
      reference: referencePath,
      reference_sha256: sha256(expectedBuffer),
      actual: artifacts.actual.path,
      actual_sha256: artifacts.actual.sha256,
      diff: null,
      diff_sha256: null,
      page: pageNumber,
      expected_width: expected.width,
      expected_height: expected.height,
      actual_width: actual.width,
      actual_height: actual.height,
      changed_pixels: null,
      changed_percent: null,
      max_changed_percent: MAX_CHANGED_PERCENT,
      pixelmatch_threshold: PIXELMATCH_THRESHOLD,
      passed: false
    });
    return;
  }

  const diff = new PNG({ width: expected.width, height: expected.height });
  const changedPixels = pixelmatch(
    expected.data,
    actual.data,
    diff.data,
    expected.width,
    expected.height,
    { threshold: PIXELMATCH_THRESHOLD, diffMask: true }
  );
  const changedPercent =
    (changedPixels / (expected.width * expected.height)) * 100;
  const passed = changedPercent <= MAX_CHANGED_PERCENT;

  const diffBuffer = PNG.sync.write(diff);
  const artifacts = writeVisualArtifacts({
    actualBuffer,
    expectedBuffer,
    diffBuffer,
    artifactStem,
    outputDir
  });

  visualPageMetrics.push({
    form_code: parityCase.code,
    form_revision: parityCase.revision,
    fixture: parityCase.fixture,
    fixture_sha256: fixtureSha256,
    reference: referencePath,
    reference_sha256: sha256(expectedBuffer),
    actual: artifacts.actual.path,
    actual_sha256: artifacts.actual.sha256,
    diff: artifacts.diff?.path ?? null,
    diff_sha256: artifacts.diff?.sha256 ?? null,
    page: pageNumber,
    expected_width: expected.width,
    expected_height: expected.height,
    actual_width: actual.width,
    actual_height: actual.height,
    changed_pixels: changedPixels,
    changed_percent: changedPercent,
    max_changed_percent: MAX_CHANGED_PERCENT,
    pixelmatch_threshold: PIXELMATCH_THRESHOLD,
    passed
  });
}

function writeVisualArtifacts({
  actualBuffer,
  expectedBuffer,
  diffBuffer,
  artifactStem,
  outputDir
}: {
  actualBuffer: Buffer;
  expectedBuffer: Buffer;
  diffBuffer?: Buffer;
  artifactStem: string;
  outputDir: string;
}) {
  fs.mkdirSync(outputDir, { recursive: true });
  const actualPath = path.join(outputDir, `${artifactStem}-actual.png`);
  fs.writeFileSync(actualPath, actualBuffer);
  fs.writeFileSync(
    path.join(outputDir, `${artifactStem}-expected.png`),
    expectedBuffer
  );
  let diffArtifact: { path: string; sha256: string } | null = null;
  if (diffBuffer) {
    const diffPath = path.join(outputDir, `${artifactStem}-diff.png`);
    fs.writeFileSync(diffPath, diffBuffer);
    diffArtifact = {
      path: repositoryRelativePath(diffPath),
      sha256: sha256(diffBuffer)
    };
  }
  return {
    actual: {
      path: repositoryRelativePath(actualPath),
      sha256: sha256(actualBuffer)
    },
    diff: diffArtifact
  };
}

function repositoryRelativePath(artifactPath: string) {
  const relative = path.relative(REPO_ROOT, artifactPath);
  if (!relative || relative.startsWith("..") || path.isAbsolute(relative)) {
    throw new Error(`Visual evidence artifact is outside the repository: ${artifactPath}`);
  }
  return relative.split(path.sep).join("/");
}

function sha256(value: Buffer) {
  return crypto.createHash("sha256").update(value).digest("hex");
}

function slug(value: string) {
  return value.toLowerCase().replaceAll(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
}
