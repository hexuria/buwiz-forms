import { expect, test, type Locator, type Page } from "@playwright/test";
import { spawnSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { PNG } from "pngjs";
import {
  compareSymmetricGrayscaleEdges,
  LAYERED_EDGE_EVIDENCE_POLICY
} from "./grayscale-edge-match";
import {
  compareCompleteOfficialPage,
  comparePixelMask
} from "./official-page-diff";
import {
  OFFICIAL_2551Q_ALLOWED_RESIDUAL,
  OFFICIAL_2551Q_COMPLETENESS_SELECTORS,
  OFFICIAL_2551Q_SOURCE_SHA256,
  OFFICIAL_2551Q_STATIC_TEXT,
  staticTextEntriesForPage,
  verifyPageIndexedStaticText,
  verifyStaticTextExhaustive,
  verifyStaticTextManifestCompleteness,
  verifyFixtureOwnedText,
  collectEnvelopeStrings,
  SINGLE_GLYPH_FIXTURE_SELECTORS
} from "./official-2551q-static-text";
import { writeRegionReport } from "./region-report";
import {
  computeCellEdgeF1,
  computePageInkBudget,
  computeStructuralInk,
  EDGE_THRESHOLD,
  TOLERANCE_RADIUS_PX,
  type PageInkBudgetResult,
  type StructuralInkResult
} from "./official-fidelity";
import {
  MIN_CELL_EDGE_PIXELS,
  MIN_EDGE_COVERAGE
} from "./fidelity-cells";
import {
  blankComparisonEnvelope,
  FIXTURE_OWNED_SELECTORS,
  prepareOfficialBlankComparison,
  renderEnvelope
} from "./support/render-utils";
import {
  criticalRegionsFor,
  PAGE_ONE_CRITICAL_REGIONS,
  PAGE_TWO_CRITICAL_REGIONS,
  type CriticalRegion
} from "./regions/2551q";
import { parsePromotionVisualThreshold } from "./release-visual-threshold";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(HERE, "../../..");
const MAX_CHANGED_PERCENT = parsePromotionVisualThreshold(
  process.env.FORM_VISUAL_MAX_CHANGED_PERCENT
);
const PIXELMATCH_THRESHOLD = 0.1;
const DEVICE_SCALE_FACTOR = 1.5;
const STRUCTURAL_INK_THRESHOLD = 100;
const STRUCTURAL_LINE_MIN_RUN = 20;
const STRUCTURAL_TOLERANCE_RADIUS = 4;
const EDGE_TOLERANCE_RADIUS_PX = 2;
const MIN_EDGE_PRECISION = 0.95;
const MIN_EDGE_RECALL = 0.94;
const MIN_EDGE_F1 = 0.95;
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
  references: Array<{ page: number; poppler: string; chromium: string }>;
  expectedRealRowKeys: number;
}

interface VisualPageMetric {
  form_code: string;
  form_revision: string;
  fixture: string;
  fixture_sha256: string;
  reference: string;
  reference_sha256: string;
  reference_kind: "official_chromium_raster";
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
  comparison: "official-complete-page-v2";
  expected_ink_missing_percent: number | null;
  unexpected_actual_ink_percent: number | null;
  poppler_reference: string;
  poppler_reference_sha256: string;
  poppler_diff: string | null;
  poppler_diff_sha256: string | null;
  poppler_raster_changed_pixels: number | null;
  poppler_raster_changed_percent: number | null;
  reference_noise_floor_changed_pixels: number;
  reference_noise_floor_changed_percent: number;
  region_report: string | null;
  region_report_sha256: string | null;
  structural_changed_pixels: number | null;
  structural_changed_percent: number | null;
  structural_diff: string | null;
  structural_diff_sha256: string | null;
  structural_ink_threshold: number;
  structural_line_min_run: number;
  structural_tolerance_radius: number;
  /**
   * The legacy line-only structural probe above and
   * `structural-ink-coverage-v1` below are DIFFERENT POPULATIONS (ink 100 /
   * run 20 / radius 4 versus ink 160 / run 24 / radius 1). They are kept side
   * by side for continuity and must never be averaged, substituted, or
   * presented as one another.
   */
  legacy_structural_superseded_by: "structural-ink-coverage-v1";
  /** official-fidelity-v1 pixel components, reporting mode until baselines pin. */
  cell_edge_f1: CellEdgeF1Summary | null;
  structural_ink_coverage: StructuralInkResult | null;
  page_ink_budget: PageInkBudgetResult | null;
  passed: boolean;
}

/**
 * Per-cell scores are recorded in full in the region report rather than inline
 * here: 1227 cells per page would bury the page-level numbers a reviewer reads
 * first, and the audit reconstructs the table itself from the pinned regions
 * and grid constants anyway.
 */
interface CellEdgeF1Summary {
  comparison: "cell-edge-f1-v1";
  tolerance_radius_px: number;
  edge_threshold: number;
  min_cell_edge_pixels: number;
  cell_table_sha256: string;
  cell_count: number;
  scored_cell_count: number;
  worst_scored_f1: number;
  edge_coverage: number;
  min_edge_coverage: number;
  cells: string | null;
  cells_sha256: string | null;
}

interface LayeredFidelityMetric {
  page: number;
  comparison: "symmetric-grayscale-sobel-edge-v1";
  tolerance_radius_px: number;
  edge_threshold: number;
  expected_edge_pixels: number;
  actual_edge_pixels: number;
  precision: number;
  recall: number;
  f1: number;
  minimum_precision: number;
  minimum_recall: number;
  minimum_f1: number;
  passed: boolean;
}

const visualPageMetrics: VisualPageMetric[] = [];
const layeredFidelityMetrics: LayeredFidelityMetric[] = [];

const cases: ParityCase[] = [
  {
    code: "2551Q",
    name: "2551Q 2018",
    revision: "2018",
    fixture: "packages/form-contracts/fixtures/2551q-6-rows.json",
    references: [
      {
        page: 1,
        poppler: "packages/form-renderer/references/2551q-2018-page-1.png",
        chromium: "packages/form-renderer/references/2551q-2018-page-1-chromium.png"
      },
      {
        page: 2,
        poppler: "packages/form-renderer/references/2551q-2018-page-2.png",
        chromium: "packages/form-renderer/references/2551q-2018-page-2-chromium.png"
      }
    ],
    expectedRealRowKeys: 6
  }
];

interface ChromiumManifestRaster {
  reference_png: string;
  reference_png_sha256: string;
  noise_floor_changed_pixels: number;
  noise_floor_changed_percent: number;
}

let cachedReferenceManifest: {
  forms: Array<{
    code: string;
    revision: string;
    pages: Array<{ page: number; chromium_raster?: ChromiumManifestRaster }>;
  }>;
} | null = null;

/**
 * The pinned same-rasterizer reference record for one official page. The gate
 * compares against this chromium raster; its pinned Poppler-vs-Chromium noise
 * floor is copied into the evidence so every report carries the floor next to
 * the gate number. Fails hard when a parity case lacks a pinned chromium
 * raster: this spec never silently falls back to a cross-rasterizer gate.
 */
function chromiumRasterFor(
  code: string,
  revision: string,
  pageNumber: number
): ChromiumManifestRaster {
  cachedReferenceManifest ??= JSON.parse(
    fs.readFileSync(
      path.join(REPO_ROOT, "packages/form-renderer/references/manifest.json"),
      "utf8"
    )
  ) as NonNullable<typeof cachedReferenceManifest>;
  const form = cachedReferenceManifest.forms.find(
    (candidate) => candidate.code === code && candidate.revision === revision
  );
  const raster = form?.pages.find(
    (candidate) => candidate.page === pageNumber
  )?.chromium_raster;
  if (!raster) {
    throw new Error(
      `${code}:${revision} page ${pageNumber} has no pinned chromium raster in the reference manifest`
    );
  }
  return raster;
}

for (const parityCase of cases) {
  test(`${parityCase.name} matches the official first two pages`, async ({
    page
  }, testInfo) => {
    visualPageMetrics.length = 0;
    layeredFidelityMetrics.length = 0;
    const fixtureBuffer = fs.readFileSync(
      path.join(REPO_ROOT, parityCase.fixture)
    );
    const fixtureSha256 = sha256(fixtureBuffer);
    const envelope = JSON.parse(fixtureBuffer.toString("utf8")) as unknown;

    await renderEnvelope(page, envelope);

    const pages = page.locator(".form-page");
    await expect(pages).toHaveCount(parityCase.references.length);

    const pageTwoName = pages
      .nth(1)
      .locator(".page-two-identity > .comb-value:last-child");
    await expect(pageTwoName).toHaveCount(1);
    await expect(pageTwoName.locator("> span")).toHaveCount(26);
    expect((await pageTwoName.locator("> span").allTextContents()).join(""))
      .toBe("ANDREA MAE RENDERER GALANG");
    await expect(
      pages.nth(1).locator(".page-two-identity > .adaptive-plain-value")
    ).toHaveCount(0);

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

    await expectCriticalRegionGeometry(pages.nth(0), PAGE_ONE_CRITICAL_REGIONS);
    await expectHeaderOptionsTopAlignment(pages.nth(0));
    await expectBackgroundInformationParity(pages.nth(0));
    await expectCriticalRegionGeometry(pages.nth(1), PAGE_TWO_CRITICAL_REGIONS);
    await expectCriticalRegionContent(pages.nth(0), pages.nth(1));
    const staticTextSnapshots = await collectStaticTextSnapshots(pages);
    const staticTextViolations = verifyPageIndexedStaticText(staticTextSnapshots);
    expect(
      staticTextViolations,
      "Every reviewed 2551Q static label must appear on its official page"
    ).toEqual([]);

    // static-text-exhaustive-v1. The pixel components cannot see content: a
    // wrong statutory rate scores 0.19e-4 and violates nothing above. These
    // three assertions are what close that hole, so none of them may be
    // relaxed to make a run pass.
    const exhaustiveStaticTextViolations = staticTextSnapshots.flatMap(
      (snapshot, index) =>
        verifyStaticTextExhaustive(
          snapshot.staticText,
          staticTextEntriesForPage(index + 1),
          OFFICIAL_2551Q_ALLOWED_RESIDUAL
        )
    );
    expect(
      exhaustiveStaticTextViolations,
      "Every reviewed 2551Q static string must appear in manifest order, and no unreviewed string may reach the page"
    ).toEqual([]);

    const manifestCompletenessViolations = verifyStaticTextManifestCompleteness(
      await collectCompletenessObservations(pages)
    );
    expect(
      manifestCompletenessViolations,
      "Every ATC row and payment heading the renderer emits must have a reviewed manifest entry"
    ).toEqual([]);

    // Both halves of the criterion deliberately look away from fixture-owned
    // elements: the pixel gate blanks them and the exhaustive walk suppresses
    // them. Adversarial testing showed that shared blind spot is a bypass - a
    // fabricated advisory line wrapped in `.comb-value > span` printed on the
    // page and scored zero violations everywhere, across a 725-element
    // surface. This constrains that set so suppression is not a free pass.
    const fixtureOwnedViolations = verifyFixtureOwnedText(
      await collectFixtureOwnedObservations(pages),
      collectEnvelopeStrings(envelope)
    );
    expect(
      fixtureOwnedViolations,
      "Fixture-owned elements are hidden from both the pixel gate and the static-text walk, so each must be a single glyph or match a value the envelope supplied"
    ).toEqual([]);

    // The pinned official PDF is an unfilled form while our authoritative
    // fixture exercises all six Schedule 1 rows. Item 17 is allowed to claim a
    // reviewed second line only when its specification does not fit the
    // official inline box. That adaptive state is covered by the dedicated
    // long-value tests below, but it is not the geometry of the blank official
    // reference. Re-render the same immutable fixture with only that adaptive
    // value blank before suppressing fixture glyphs. Borders, comb cells,
    // check boxes, labels, artwork, fills, and pagination remain inside the
    // strict complete-page pixel gate; no threshold or comparison mask changes.
    await renderEnvelope(page, blankComparisonEnvelope(envelope, parityCase.code));
    await expect(pages).toHaveCount(parityCase.references.length);
    await prepareOfficialBlankComparison(page);

    for (const [pageIndex, reference] of parityCase.references.entries()) {
      const renderedPage = pages.nth(pageIndex);
      const expectedBuffer = fs.readFileSync(
        path.join(REPO_ROOT, reference.chromium)
      );
      const popplerBuffer = fs.readFileSync(
        path.join(REPO_ROOT, reference.poppler)
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
        pageNumber: reference.page,
        fixtureSha256,
        referencePath: reference.chromium,
        popplerReferencePath: reference.poppler,
        actualBuffer,
        expectedBuffer,
        popplerBuffer,
        artifactStem: `${slug(parityCase.name)}-page-${reference.page}`,
        outputDir: testInfo.outputDir
      });
    }

    writeLayeredFidelityDiagnostic(
      staticTextViolations,
      exhaustiveStaticTextViolations,
      manifestCompletenessViolations
    );
    writeVisualEvidence();
    expect(
      layeredFidelityMetrics,
      "The layered diagnostic must measure every official 2551Q page"
    ).toHaveLength(parityCase.references.length);
    const failedLayeredEvidence = layeredFidelityMetrics
      .filter((metric) => !metric.passed)
      .map(({ page, precision, recall, f1 }) => ({
        page,
        precision,
        recall,
        f1
      }));
    expect(
      failedLayeredEvidence,
      "The non-promotional 2px grayscale edge diagnostic must retain reviewed precision, recall, and F1"
    ).toEqual([]);
    const failedPages = visualPageMetrics
      .filter((metric) => !metric.passed)
      .map((metric) => ({
        page: metric.page,
        changed_percent: metric.changed_percent,
        poppler_raster_changed_percent: metric.poppler_raster_changed_percent,
        reference_noise_floor_changed_percent:
          metric.reference_noise_floor_changed_percent,
        region_report: metric.region_report,
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

async function collectStaticTextSnapshots(pages: Locator) {
  const scopedSelectors = [
    ...new Set(
      OFFICIAL_2551Q_STATIC_TEXT.flatMap((entry) =>
        entry.selector ? [entry.selector] : []
      )
    )
  ];
  const pageCount = await pages.count();
  const staticTexts = await collectFixtureFreeStaticText(pages.page());
  return Promise.all(Array.from({ length: pageCount }, async (_, pageIndex) => {
    const formPage = pages.nth(pageIndex);
    const selectorText = Object.fromEntries(
      await Promise.all(scopedSelectors.map(async (selector) => [
        selector,
        (await formPage.locator(selector).allInnerTexts()).join(" ")
      ] as const))
    );
    const fullText = await formPage.innerText();
    return {
      fullText,
      staticText: staticTexts[pageIndex] ?? fullText,
      selectorText
    };
  }));
}

/**
 * Page text with only the fixture-supplied glyphs suppressed — the same
 * fixture-owned set the pixel gate blanks. The exhaustive walk asserts static
 * text, so a fixture's comb characters must not become residual; scoping them
 * out is what lets `allowedResidual` stay down to three punctuation glyphs
 * instead of an allowlist wide enough to hide a fabricated advisory line.
 *
 * The suppression is applied and reverted inside one evaluate call, so the DOM
 * the later screenshots capture is untouched.
 */
async function collectFixtureFreeStaticText(page: Page) {
  return page.evaluate((selectors) => {
    const owned = Array.from(
      document.querySelectorAll<HTMLElement>(selectors.join(","))
    );
    const previous = owned.map((element) => element.style.display);
    for (const element of owned) element.style.display = "none";
    try {
      return Array.from(document.querySelectorAll<HTMLElement>(".form-page")).map(
        (formPage) => formPage.innerText
      );
    } finally {
      owned.forEach((element, index) => {
        element.style.display = previous[index] ?? "";
      });
    }
  }, FIXTURE_OWNED_SELECTORS);
}

/**
 * Enumerates the DOM elements the manifest must account for. If the renderer
 * grows an ATC row or a payment heading, this makes the manifest fail closed
 * rather than silently fall behind.
 */
async function collectCompletenessObservations(pages: Locator) {
  const pageCount = await pages.count();
  const observations = await Promise.all(
    Array.from({ length: pageCount }, async (_, pageIndex) => {
      const formPage = pages.nth(pageIndex);
      const perSelector = await Promise.all(
        OFFICIAL_2551Q_COMPLETENESS_SELECTORS.map(async (selector) =>
          (await formPage.locator(selector).allInnerTexts()).map((text) => ({
            page: pageIndex + 1,
            selector,
            text
          }))
        )
      );
      return perSelector.flat();
    })
  );
  return observations.flat();
}

/**
 * Enumerate every fixture-owned element so the set both halves of the
 * criterion suppress can still be held to account. Single-glyph selectors are
 * flagged as such: comb cells and check boxes carry one character by
 * construction, so anything longer is a structural impossibility rather than a
 * value judgement.
 */
async function collectFixtureOwnedObservations(pages: Locator) {
  const pageCount = await pages.count();
  const observations = await Promise.all(
    Array.from({ length: pageCount }, async (_, pageIndex) => {
      const formPage = pages.nth(pageIndex);
      const perSelector = await Promise.all(
        FIXTURE_OWNED_SELECTORS.map(async (selector) =>
          (await formPage.locator(selector).allInnerTexts()).map((text) => ({
            page: pageIndex + 1,
            selector,
            text,
            singleGlyph: SINGLE_GLYPH_FIXTURE_SELECTORS.includes(selector)
          }))
        )
      );
      return perSelector.flat();
    })
  );
  return observations.flat();
}

test("2551Q PDF417 artwork keeps the reviewed source geometry", async ({ page }) => {
  await renderEnvelope(
    page,
    readFixture("packages/form-contracts/fixtures/2551q-6-rows.json")
  );
  const pages = page.locator(".form-page");
  await expect(pages).toHaveCount(2);

  await expectCriticalRegionGeometry(pages.nth(0), [
    { name: "Page 1 official seal", selector: ".government-seal", x: 490, y: 44, width: 58, height: 50 },
    { name: "Page 1 masthead", selector: ".official-masthead", x: 45, y: 100, width: 1136, height: 124 },
    { name: "Page 1 PDF417 symbol", selector: ".official-barcode > .official-pdf417-symbol", x: 852, y: 119, width: 323, height: 74 },
    { name: "Page 1 PDF417 caption", selector: ".official-barcode > small", x: 1009, y: 196, width: 161, height: 16 }
  ]);
  await expectCriticalRegionGeometry(pages.nth(1), [
    { name: "Page 2 masthead", selector: ".page-two-masthead", x: 45, y: 78, width: 1136, height: 117 },
    { name: "Page 2 PDF417 symbol", selector: ".page-two-barcode > .official-pdf417-symbol", x: 849, y: 97, width: 326, height: 75 },
    { name: "Page 2 PDF417 caption", selector: ".page-two-barcode > small", x: 1009, y: 177, width: 161, height: 16 }
  ]);

  expect(
    await page.locator(".official-pdf417-symbol").evaluateAll((symbols) =>
      symbols.slice(0, 2).map((symbol) => ({
        preserveAspectRatio: symbol.getAttribute("preserveAspectRatio"),
        shapeRendering: getComputedStyle(symbol).shapeRendering,
        viewBox: symbol.getAttribute("viewBox")
      }))
    )
  ).toEqual([
    { preserveAspectRatio: "none", shapeRendering: "crispedges", viewBox: "0 0 120 7" },
    { preserveAspectRatio: "none", shapeRendering: "crispedges", viewBox: "0 0 120 7" }
  ]);

  const captionStyles = await page
    .locator(".official-barcode > small, .page-two-barcode > small")
    .evaluateAll((captions) => captions.slice(0, 2).map((caption) => {
      const style = getComputedStyle(caption);
      return {
        fontFamily: style.fontFamily,
        fontSize: style.fontSize,
        lineHeight: style.lineHeight,
        textAlign: style.textAlign,
        whiteSpace: style.whiteSpace
      };
    }));
  expect(captionStyles).toEqual(Array.from({ length: 2 }, () => ({
    fontFamily: '"eBIRForms Arimo", sans-serif',
    fontSize: "10.6667px",
    lineHeight: "10.6667px",
    textAlign: "right",
    whiteSpace: "nowrap"
  })));

  const gaps = await Promise.all([
    measuredVerticalGap(pages.nth(0), ".official-barcode"),
    measuredVerticalGap(pages.nth(1), ".page-two-barcode")
  ]);
  expect(gaps[0]).toBeCloseTo(3.04, 1);
  expect(gaps[1]).toBeCloseTo(5.02, 1);

  expect(await page.locator(".government-seal").evaluate((image) => ({
    naturalHeight: (image as HTMLImageElement).naturalHeight,
    naturalWidth: (image as HTMLImageElement).naturalWidth
  }))).toEqual({ naturalHeight: 83, naturalWidth: 95 });
});

test("2551Q page-one typography keeps the reviewed bundled-font calibration", async ({
  page
}) => {
  await renderEnvelope(
    page,
    readFixture("packages/form-contracts/fixtures/2551q-6-rows.json")
  );

  const pageOne = page.locator(".form-2551q-page-one");
  const typography = await pageOne.evaluate((formPage) => {
    const read = (selector: string) => {
      const element = formPage.querySelector(selector);
      if (!element) throw new Error(`Missing typography target: ${selector}`);
      const style = getComputedStyle(element);
      return {
        fontFamily: style.fontFamily,
        fontSize: style.fontSize,
        fontWeight: style.fontWeight,
        transform: style.transform
      };
    };
    return {
      formNumber: read(".official-form-number > strong"),
      formRevision: read(".official-form-number > small"),
      formTitle: read(".official-form-title > strong"),
      instructions: read(".official-form-title > em"),
      optionLabel: read(".filing-basis > .option-label:first-child"),
      taxLine: read(".official-tax-line[data-item='14'] .tax-line-label"),
      taxLineNumber: read(".official-tax-line[data-item='14'] .tax-line-label b"),
      taxLineNote: read(".official-tax-line[data-item='14'] .tax-line-label em"),
      paymentLabel: read(".payment-row-25 > span:first-child")
    };
  });

  const bundledFont = '"eBIRForms Arimo", sans-serif';
  expect(typography).toEqual({
    formNumber: {
      fontFamily: bundledFont,
      fontSize: "32px",
      fontWeight: "400",
      transform: "matrix(1, 0, 0, 1, -1.33333, -1.33333)"
    },
    formRevision: {
      fontFamily: bundledFont,
      fontSize: "10.6667px",
      fontWeight: "400",
      transform: "none"
    },
    formTitle: {
      fontFamily: bundledFont,
      fontSize: "23.6667px",
      fontWeight: "400",
      transform: "matrix(1, 0, 0, 1, -0.666667, -2)"
    },
    instructions: {
      fontFamily: bundledFont,
      fontSize: "10.3333px",
      fontWeight: "400",
      transform: "matrix(1, 0, 0, 1, -0.666667, -6.66667)"
    },
    optionLabel: {
      fontFamily: bundledFont,
      fontSize: "12.4444px",
      fontWeight: "400",
      transform: "none"
    },
    taxLine: {
      fontFamily: bundledFont,
      fontSize: "12.4px",
      fontWeight: "400",
      transform: "none"
    },
    taxLineNumber: {
      fontFamily: bundledFont,
      fontSize: "12.4px",
      fontWeight: "500",
      transform: "none"
    },
    taxLineNote: {
      fontFamily: bundledFont,
      fontSize: "9.66667px",
      fontWeight: "400",
      transform: "none"
    },
    paymentLabel: {
      fontFamily: bundledFont,
      fontSize: "12.3333px",
      fontWeight: "400",
      transform: "none"
    }
  });

  expect(await pageOne.evaluate((formPage) => {
    const style = (selector: string) => {
      const element = formPage.querySelector(selector);
      if (!element) throw new Error(`Missing parity typography target: ${selector}`);
      return getComputedStyle(element);
    };
    return {
      declarationLineHeight: style(".official-declaration > p").lineHeight,
      formNumberLetterSpacing: style(".official-form-number > strong").letterSpacing,
      formTitleLetterSpacing: style(".official-form-title > strong").letterSpacing,
      instructionsLetterSpacing: style(".official-form-title > em").letterSpacing,
      item13EmFontSize: style(".income-rate-question em").fontSize,
      item13FontSize: style(".income-rate-question").fontSize,
      item13Transform: style(".income-rate-question").transform,
      machineValidationFontSize: style(".machine-validation").fontSize,
      nameLabelBorderBottomWidth: style(".name-field > .field-label").borderBottomWidth,
      nameLabelBoxShadow: style(".name-field > .field-label").boxShadow,
      paymentHeadingsFontSize: style(".payment-headings").fontSize,
      signatureTransforms: Array.from(
        formPage.querySelectorAll(".signature-caption > *"),
        (element) => getComputedStyle(element).transform
      ),
      taxLineLetterSpacing: style(
        ".official-tax-line[data-item='14'] .tax-line-label"
      ).letterSpacing
    };
  })).toEqual({
    declarationLineHeight: "10.6px",
    formNumberLetterSpacing: "normal",
    formTitleLetterSpacing: "0.933333px",
    instructionsLetterSpacing: "-0.0933333px",
    item13EmFontSize: "10.3333px",
    item13FontSize: "12px",
    item13Transform: "matrix(1, 0, 0, 1, 0, -0.333333)",
    machineValidationFontSize: "9.6px",
    nameLabelBorderBottomWidth: "1px",
    nameLabelBoxShadow: "none",
    paymentHeadingsFontSize: "9.6px",
    signatureTransforms: Array.from(
      { length: 4 },
      () => "matrix(1, 0, 0, 1, 0, -0.666667)"
    ),
    taxLineLetterSpacing: "0.133333px"
  });

  expect(
    await page.locator(".page-two-form-title > strong").evaluate((element) => {
      const style = getComputedStyle(element);
      return {
        fontSize: style.fontSize,
        fontWeight: style.fontWeight,
        letterSpacing: style.letterSpacing
      };
    })
  ).toEqual({ fontSize: "24px", fontWeight: "400", letterSpacing: "0.8px" });

  expect(await page.locator(".form-2551q-page-two").evaluate((pageTwo) => {
    const style = (selector: string) => {
      const element = pageTwo.querySelector(selector);
      if (!element) throw new Error(`Missing page-two calibration target: ${selector}`);
      return getComputedStyle(element);
    };
    const identity = style(".page-two-identity-label");
    return {
      decimalFontWeight: style(".schedule-decimal-separator").fontWeight,
      identityPaddingLeft: identity.paddingLeft,
      identityPosition: identity.position,
      identityTop: identity.top,
      rowNumberFontWeight: style(".official-schedule-row-number").fontWeight
    };
  })).toEqual({
    decimalFontWeight: "400",
    identityPaddingLeft: "7.33333px",
    identityPosition: "relative",
    identityTop: "0.5px",
    rowNumberFontWeight: "400"
  });

  const item25Label = pageOne.locator(".payment-row-25 > span:first-child");
  await expect(item25Label).toHaveText("25 Cash/Bank Debit Memo");
  const item25Geometry = await item25Label.evaluate((element) => ({
    clientHeight: element.clientHeight,
    clientWidth: element.clientWidth,
    letterSpacing: getComputedStyle(element).letterSpacing,
    scrollHeight: element.scrollHeight,
    scrollWidth: element.scrollWidth
  }));
  expect(item25Geometry.letterSpacing).toBe("-1px");
  expect(item25Geometry.scrollWidth).toBeLessThanOrEqual(
    item25Geometry.clientWidth
  );
  expect(item25Geometry.scrollHeight).toBeLessThanOrEqual(
    item25Geometry.clientHeight
  );
});

test("2551Q Schedule 1 keeps black rules and bottom-anchored comb guides", async ({
  page
}) => {
  await renderEnvelope(
    page,
    readFixture("packages/form-contracts/fixtures/2551q-6-rows.json")
  );
  const pageTwo = page.locator(".form-2551q-page-two");
  await expect(pageTwo).toHaveCount(1);

  const styles = await pageTwo.evaluate((element) => {
    const normalCell = element.querySelector(
      ".page-two-identity > .comb-value > span"
    );
    const majorCell = element.querySelector(
      ".visual-integer-comb-leading-1 > .comb-value > span:nth-child(2)"
    );
    const tinGroupCells = [3, 6, 9].map((index) => element.querySelector(
      `.page-two-identity > .comb-value:first-of-type > span:nth-child(${index})`
    ));
    const masthead = element.querySelector(".page-two-masthead");
    const atcCell = element.querySelector(".official-atc-table td");
    const indentedAtcCells = ["PT105", "PT101", "PT113", "PT114"].map((code) =>
      element.querySelector(`.atc-entry[data-atc-code="${code}"] > td:nth-child(2)`)
    );
    const normalAtcCell = element.querySelector(
      '.atc-entry[data-atc-code="PT010"] > td:nth-child(2)'
    );
    const categoryAtcCell = element.querySelector(
      ".official-atc-table .atc-category > th"
    );
    const noteAtcCell = element.querySelector(
      ".official-atc-table .atc-note > td:nth-child(2)"
    );
    if (
      !normalCell ||
      !majorCell ||
      !masthead ||
      !atcCell ||
      !normalAtcCell ||
      !categoryAtcCell ||
      !noteAtcCell ||
      tinGroupCells.some((cell) => !cell) ||
      indentedAtcCells.some((cell) => !cell)
    ) {
      throw new Error("2551Q Schedule 1 calibration targets are missing");
    }
    const normalStyle = getComputedStyle(normalCell);
    const normalGuide = getComputedStyle(normalCell, "::after");
    const majorGuide = getComputedStyle(majorCell, "::after");
    return {
      atcBorder: getComputedStyle(atcCell).borderTopColor,
      majorGuideColor: majorGuide.borderRightColor,
      majorGuideHeight: Number.parseFloat(majorGuide.height),
      mastheadBorder: getComputedStyle(masthead).borderTopColor,
      normalCellBorderWidth: normalStyle.borderRightWidth,
      normalGuideColor: normalGuide.borderRightColor,
      normalGuideBorderWidth: Number.parseFloat(normalGuide.borderRightWidth),
      normalGuideHeight: Number.parseFloat(normalGuide.height),
      normalGuidePosition: normalGuide.position,
      normalAtcFontSize: Number.parseFloat(getComputedStyle(normalAtcCell).fontSize),
      normalAtcLetterSpacing: Number.parseFloat(
        getComputedStyle(normalAtcCell).letterSpacing
      ),
      normalAtcPaddingLeft: Number.parseFloat(
        getComputedStyle(normalAtcCell).paddingLeft
      ),
      categoryAtcPaddingLeft: Number.parseFloat(
        getComputedStyle(categoryAtcCell).paddingLeft
      ),
      noteAtcFontSize: Number.parseFloat(getComputedStyle(noteAtcCell).fontSize),
      noteAtcLetterSpacing: Number.parseFloat(
        getComputedStyle(noteAtcCell).letterSpacing
      ),
      indentedAtcPaddingLeft: indentedAtcCells.map((cell) =>
        Number.parseFloat(getComputedStyle(cell as Element).paddingLeft)
      ),
      tinGroupGuides: tinGroupCells.map((cell) => {
        const guide = getComputedStyle(cell as Element, "::after");
        return {
          borderWidth: Number.parseFloat(guide.borderRightWidth),
          height: Number.parseFloat(guide.height)
        };
      })
    };
  });

  expect(styles.mastheadBorder).toBe("rgb(0, 0, 0)");
  expect(styles.atcBorder).toBe("rgb(0, 0, 0)");
  expect(styles.normalCellBorderWidth).toBe("0px");
  expect(styles.normalGuidePosition).toBe("absolute");
  expect(styles.normalGuideColor).toBe("rgb(0, 0, 0)");
  expect(styles.normalGuideHeight).toBeCloseTo(9.33, 1);
  expect(styles.majorGuideColor).toBe("rgb(0, 0, 0)");
  expect(styles.majorGuideHeight).toBeGreaterThan(styles.normalGuideHeight);
  expect(styles.normalAtcFontSize).toBeCloseTo(11, 1);
  expect(styles.normalAtcLetterSpacing).toBeCloseTo(.133, 2);
  expect(styles.normalAtcPaddingLeft).toBeCloseTo(4, 1);
  expect(styles.categoryAtcPaddingLeft).toBeCloseTo(6.933, 2);
  expect(styles.noteAtcFontSize).toBeCloseTo(10.533, 2);
  expect(styles.noteAtcLetterSpacing).toBeCloseTo(.333, 2);
  expect(styles.indentedAtcPaddingLeft).toHaveLength(4);
  for (const padding of styles.indentedAtcPaddingLeft) {
    expect(padding).toBeCloseTo(22, 1);
  }
  expect(styles.tinGroupGuides).toHaveLength(3);
  for (const guide of styles.tinGroupGuides) {
    expect(guide.height).toBeGreaterThan(styles.normalGuideHeight);
    expect(guide.borderWidth).toBeGreaterThanOrEqual(styles.normalGuideBorderWidth);
  }
  expect(await pageHasNoOverflow(pageTwo)).toBe(true);
});

test("2551Q Part III amount cells preserve the official partition", async ({ page }) => {
  await renderEnvelope(
    page,
    readFixture("packages/form-contracts/fixtures/2551q-6-rows.json")
  );
  const pageOne = page.locator(".form-page").first();
  await expectCriticalRegionGeometry(pageOne, [
    { name: "Item 25 Amount", selector: ".payment-row-25 .blank-money-value", x: 757, y: 1502, width: 422, height: 35 },
    { name: "Item 25 decimal cell", selector: ".payment-row-25 .decimal-separator", x: 1097, y: 1502, width: 30, height: 35 },
    { name: "Item 25 cents cells", selector: ".payment-row-25 .blank-money-value > .comb-value:last-child", x: 1127, y: 1502, width: 53, height: 35 },
    { name: "Item 28 Amount", selector: ".payment-other-row .blank-money-value", x: 757, y: 1636, width: 422, height: 35 },
    { name: "Item 28 decimal cell", selector: ".payment-other-row .decimal-separator", x: 1097, y: 1636, width: 30, height: 35 },
    { name: "Item 28 cents cells", selector: ".payment-other-row .blank-money-value > .comb-value:last-child", x: 1127, y: 1636, width: 53, height: 35 }
  ]);

  const amountRows = pageOne.locator(
    ".payment-row-25 .blank-money-value, .payment-row-26 .blank-money-value, .payment-row-27 .blank-money-value, .payment-other-row .blank-money-value"
  );
  await expect(amountRows).toHaveCount(4);
  for (let index = 0; index < 4; index += 1) {
    const amount = amountRows.nth(index);
    const integerCells = amount.locator(":scope > .comb-value:first-child > span");
    await expect(integerCells).toHaveCount(12);
    await expect(amount.locator(":scope > .comb-value:last-child > span")).toHaveCount(2);

    const widths = await amount.evaluate((element) => {
      const integer = [...element.querySelectorAll(":scope > .comb-value:first-child > span")]
        .map((cell) => cell.getBoundingClientRect().width);
      const decimal = element.querySelector(":scope > .decimal-separator")?.getBoundingClientRect().width ?? 0;
      const cents = [...element.querySelectorAll(":scope > .comb-value:last-child > span")]
        .map((cell) => cell.getBoundingClientRect().width);
      return { integer, decimal, cents };
    });
    const integerAverage = widths.integer.reduce((sum, width) => sum + width, 0) /
      widths.integer.length;
    expect(Math.max(...widths.integer) - Math.min(...widths.integer)).toBeLessThan(.05);
    expect(widths.decimal / integerAverage).toBeGreaterThan(.95);
    expect(widths.decimal / integerAverage).toBeLessThan(1.1);
    expect(widths.cents.every((width) => width / integerAverage > .9)).toBe(true);
  }

  const exactPaymentCombs = [
    ["item-25-drawee", 5],
    ["item-25-number", 7],
    ["item-25-date", 8],
    ["item-26-drawee", 5],
    ["item-26-number", 7],
    ["item-26-date", 8],
    ["item-27-number", 7],
    ["item-27-date", 8],
    ["item-28-description", 7],
    ["item-28-drawee", 5],
    ["item-28-number", 7],
    ["item-28-date", 8]
  ] as const;
  for (const [field, cells] of exactPaymentCombs) {
    const comb = pageOne.locator(`[data-payment-field="${field}"]`);
    await expect(comb).toHaveAttribute("data-cell-capacity", String(cells));
    await expect(comb.locator(":scope > .comb-value > span")).toHaveCount(cells);
    await expect(comb.locator(":scope > .adaptive-plain-value")).toHaveCount(0);
  }
  await expect(
    pageOne.locator(
      '[data-payment-field="item-28-description"] > .payment-other-description-leading'
    )
  ).toHaveCount(1);
  const item28DescriptionGeometry = await pageOne
    .locator('[data-payment-field="item-28-description"]')
    .evaluate((element) => {
      const leading = element.querySelector(".payment-other-description-leading");
      const cells = [...element.querySelectorAll(":scope > .comb-value > span")];
      if (!leading || cells.length !== 7) {
        throw new Error("Item 28 description geometry is incomplete");
      }
      return {
        cellWidths: cells.map((cell) => cell.getBoundingClientRect().width),
        leadingWidth: leading.getBoundingClientRect().width
      };
    });
  expect(item28DescriptionGeometry.leadingWidth).toBeCloseTo(18.56, 1);
  expect(
    Math.max(...item28DescriptionGeometry.cellWidths) -
    Math.min(...item28DescriptionGeometry.cellWidths)
  ).toBeLessThan(.05);

  await expect(pageOne.locator('[data-payment-field="item-27-drawee"]')).toHaveCount(0);
  expect(
    await pageOne.locator(".payment-row-27 > span:first-child").evaluate((element) => {
      const style = getComputedStyle(element);
      return { end: style.gridColumnEnd, start: style.gridColumnStart };
    })
  ).toEqual({ start: "1", end: "span 2" });
  expect(await pageHasNoOverflow(pageOne)).toBe(true);
});

test("2551Q Part I labels and writable cells preserve the official rendering", async ({
  page
}) => {
  await renderEnvelope(
    page,
    readFixture("packages/form-contracts/fixtures/2551q-6-rows.json")
  );
  const pageOne = page.locator(".form-page").first();
  await expectCriticalRegionGeometry(pageOne, [
    { name: "Item 6 label", selector: ".tin-rdo-row > .field-label", x: 47, y: 320, width: 394, height: 35 },
    { name: "TIN first group", selector: ".tin-rdo-row > .comb-value:nth-child(2)", x: 441, y: 320, width: 85, height: 35 },
    { name: "TIN first separator", selector: ".tin-rdo-row > .tin-separator:nth-child(3)", x: 526, y: 320, width: 29, height: 35 },
    { name: "TIN branch group", selector: ".tin-rdo-row > .comb-value:nth-child(8)", x: 782, y: 320, width: 142, height: 35 },
    { name: "Item 7 label", selector: ".tin-rdo-row > .rdo-label", x: 924, y: 320, width: 170, height: 35 },
    { name: "Item 8 label", selector: ".name-field > .field-label", x: 47, y: 357, width: 1133, height: 23 },
    { name: "Item 8 value", selector: ".name-field > .comb-value", x: 47, y: 380, width: 1133, height: 35 },
    { name: "Item 9 label", selector: ".address-field > .field-label", x: 47, y: 415, width: 1133, height: 23 },
    { name: "Item 9 continuation", selector: ".address-continuation > .comb-value:first-child", x: 47, y: 473, width: 881, height: 38 },
    { name: "Item 9A label", selector: ".address-continuation > .zip-label", x: 928, y: 473, width: 141, height: 38 },
    { name: "Item 10 label", selector: ".contact-email-field > .field-label:first-child", x: 47, y: 510, width: 337, height: 23 },
    { name: "Item 11 value", selector: ".contact-email-field > .comb-value:nth-child(4)", x: 383, y: 533, width: 796, height: 35 },
    { name: "Item 12 Yes box", selector: ".relief-choices .check-choice:first-child .check-box", x: 383, y: 574, width: 28, height: 28 },
    { name: "Item 12 No box", selector: ".relief-choices .check-choice:last-child .check-box", x: 469, y: 574, width: 28, height: 28 },
    { name: "Item 12A label", selector: ".tax-relief-field > .tax-relief-spec", x: 554, y: 568, width: 170, height: 38 },
    { name: "Item 12A value", selector: ".tax-relief-field > .comb-value", x: 724, y: 568, width: 455, height: 38 }
  ]);
  await expectBackgroundInformationParity(pageOne);
  expect(await pageHasNoOverflow(pageOne)).toBe(true);
});

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

test("canonical fixtures render safely or fail closed at the readable floor", async ({ page }) => {
  const fixtures = [
    "2551q-10-rows.json",
    "2551q-6-rows.json",
    "2551q-fiscal-period.json",
    "2551q-item13-eight-percent.json",
    "2551q-long-values.json",
    "2551q-minimum.json",
    "2551q-overpayment-refund.json",
    "2551q-overpayment-tcc.json",
    "2551q-tax-relief.json",
    "2551q-validation-edge.json"
  ];

  for (const fixture of fixtures) {
    const envelope = readFixture(`packages/form-contracts/fixtures/${fixture}`) as {
      fields: Record<string, { type: string; value: unknown }>;
      schedules: Array<{ rows: unknown[] }>;
    };
    await renderEnvelope(page, envelope);
    const pages = page.locator(".form-page");
    const expectedPages = envelope.schedules[0].rows.length > 6 ? 3 : 2;
    await expect(pages, fixture).toHaveCount(expectedPages);
    if (fixture === "2551q-long-values.json") {
      const description = envelope.fields.other_tax_credit_description?.value;
      if (typeof description !== "string") {
        throw new Error("2551Q long-value fixture Item 17 description must be text");
      }
      const renderedDescription = page.locator(".tax-credit-description");
      await expect(renderedDescription).toHaveText(description);
      await expect(renderedDescription).toHaveAttribute(
        "aria-label",
        `Item 17 specification: ${description}`
      );
      await expect(renderedDescription).toHaveAttribute("data-overflow-mode", "plain");
      await expect(renderedDescription).toHaveAttribute(
        "data-adaptive-font-size-px",
        "12.0"
      );
      await expect(renderedDescription).toHaveAttribute(
        "data-adaptive-fit-state",
        "fit"
      );
      const descriptionGeometry = await renderedDescription.evaluate((element) => ({
        clientHeight: element.clientHeight,
        clientWidth: element.clientWidth,
        scrollHeight: element.scrollHeight,
        scrollWidth: element.scrollWidth
      }));
      expect(descriptionGeometry.scrollWidth).toBeLessThanOrEqual(
        descriptionGeometry.clientWidth + 1
      );
      expect(descriptionGeometry.scrollHeight).toBeLessThanOrEqual(
        descriptionGeometry.clientHeight + 1
      );
    }
    if (fixture === "2551q-validation-edge.json") {
      await expect(page.locator(".tax-credit-description")).toHaveText("");
      await expect(page.locator(".tax-credit-description")).toHaveAttribute(
        "aria-label",
        "Item 17 specification, blank"
      );
    }
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

  const adaptiveStateReports = await firstPage.evaluate((element) => {
    const probe = document.createElement("span");
    probe.dataset.adaptiveFitState = "pending";
    element.append(probe);
    const measure = (window as Window & {
      measureEbirFormGeometry?: () => {
        pages: Array<{ descendant_overflow_x: number }>;
      } | null;
    }).measureEbirFormGeometry;
    if (!measure) throw new Error("renderer geometry measurement is unavailable");
    const pending = measure();
    probe.dataset.adaptiveFitState = "unresolved";
    const unresolved = measure();
    probe.remove();
    return { pending, unresolved };
  });
  expect(adaptiveStateReports.pending).toBeNull();
  expect(adaptiveStateReports.unresolved?.pages[0].descendant_overflow_x)
    .toBeGreaterThan(0);

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

test("2551Q just-over-comb values use the largest reviewed field-height candidate", async ({
  page
}) => {
  const fixture = readFixture(
    "packages/form-contracts/fixtures/2551q-6-rows.json"
  ) as {
    fields: Record<string, { type: string; value: unknown }>;
    taxpayer: {
      email: string;
      name: string;
      registered_address: string;
    };
  };
  fixture.taxpayer.name = "N".repeat(41);
  fixture.taxpayer.registered_address = "A".repeat(72);
  fixture.taxpayer.email = `${"E".repeat(16)}@EXAMPLE.TEST`;
  fixture.fields.tax_relief_specification.value = "R".repeat(27);

  await renderEnvelope(page, fixture);
  await page.waitForFunction(
    () => !document.querySelector('[data-adaptive-fit-state="pending"]')
  );

  const expectMaximizedValue = async (
    selector: string,
    expectedText: string,
    expectedFontSize: number,
    expectedMaxFontSize: number
  ) => {
    const rendered = page.locator(selector);
    await expect(rendered).toHaveCount(1);
    await expect(rendered).toHaveText(expectedText);
    const measurement = await rendered.evaluate((element) => {
      const tolerance = 1;
      const fitsField = () => {
        if (
          element.scrollWidth > element.clientWidth + tolerance ||
          element.scrollHeight > element.clientHeight + tolerance
        ) {
          return false;
        }
        const valueBounds = element.getBoundingClientRect();
        const textRange = document.createRange();
        textRange.selectNodeContents(element);
        for (const textBounds of textRange.getClientRects()) {
          if (
            textBounds.left < valueBounds.left - tolerance ||
            textBounds.top < valueBounds.top - tolerance ||
            textBounds.right > valueBounds.right + tolerance ||
            textBounds.bottom > valueBounds.bottom + tolerance
          ) {
            return false;
          }
        }
        const ownerBounds = element.parentElement?.getBoundingClientRect();
        return !ownerBounds || (
          valueBounds.left >= ownerBounds.left - tolerance &&
          valueBounds.top >= ownerBounds.top - tolerance &&
          valueBounds.right <= ownerBounds.right + tolerance &&
          valueBounds.bottom <= ownerBounds.bottom + tolerance
        );
      };
      const fontSize = Number.parseFloat(getComputedStyle(element).fontSize);
      const fontStep = Number.parseFloat(
        element.getAttribute("data-adaptive-step-px") ?? "NaN"
      );
      const originalSize = element.style.fontSize;
      element.style.fontSize = `${fontSize + fontStep}px`;
      const nextStepOverflows = !fitsField();
      element.style.fontSize = originalSize;
      return {
        fontSize,
        fontStep,
        maxFontSize: Number.parseFloat(
          element.getAttribute("data-adaptive-max-font-px") ?? "NaN"
        ),
        nextStepOverflows,
        state: element.getAttribute("data-adaptive-fit-state")
      };
    });
    expect(measurement.state, selector).toBe("fit");
    expect(measurement.fontStep, selector).toBe(.5);
    expect(measurement.fontSize, selector).toBe(expectedFontSize);
    expect(measurement.maxFontSize, selector).toBe(expectedMaxFontSize);
    expect(measurement.nextStepOverflows, selector).toBe(true);
  };

  await expectMaximizedValue(
    ".name-field > .adaptive-plain-value",
    fixture.taxpayer.name,
    21,
    21
  );
  await expectMaximizedValue(
    ".address-field > .adaptive-plain-value",
    fixture.taxpayer.registered_address,
    15.5,
    21
  );
  await expectMaximizedValue(
    ".contact-email-field > .adaptive-plain-value",
    fixture.taxpayer.email,
    21,
    21
  );
  await expectMaximizedValue(
    ".tax-relief-field > .adaptive-plain-value",
    fixture.fields.tax_relief_specification.value as string,
    15,
    21.5
  );
  await expectMaximizedValue(
    ".page-two-identity > .adaptive-plain-value",
    fixture.taxpayer.name,
    16,
    21.5
  );

  const pageTwoOnlyFixture = structuredClone(fixture);
  pageTwoOnlyFixture.taxpayer.name = "N".repeat(27);
  await renderEnvelope(page, pageTwoOnlyFixture);
  await page.waitForFunction(
    () => !document.querySelector('[data-adaptive-fit-state="pending"]')
  );
  await expectMaximizedValue(
    ".page-two-identity > .adaptive-plain-value",
    pageTwoOnlyFixture.taxpayer.name,
    21.5,
    21.5
  );
});

test("2551Q plain-box overflow uses the largest readable font that fits the real field", async ({
  page
}) => {
  const fixture = readFixture(
    "packages/form-contracts/fixtures/2551q-long-values.json"
  );
  await renderEnvelope(page, fixture);
  await page.waitForFunction(
    () => !document.querySelector('[data-adaptive-fit-state="pending"]')
  );

  const values = page.locator(".adaptive-plain-value:not(.tax-credit-description)");
  await expect(values).toHaveCount(5);
  const measurements = await values.evaluateAll((elements) => elements.map((element) => {
    const fontSize = Number.parseFloat(getComputedStyle(element).fontSize);
    const maxFontSize = Number.parseFloat(
      element.getAttribute("data-adaptive-max-font-px") ?? "NaN"
    );
    const minFontSize = Number.parseFloat(
      element.getAttribute("data-adaptive-min-font-px") ?? "NaN"
    );
    const fontStep = Number.parseFloat(
      element.getAttribute("data-adaptive-step-px") ?? "NaN"
    );
    const tolerance = 1;
    const owner = element.parentElement;
    const fitsOwner = () => {
      if (
        element.scrollWidth > element.clientWidth + tolerance ||
        element.scrollHeight > element.clientHeight + tolerance
      ) {
        return false;
      }
      if (!owner) return true;
      const valueBounds = element.getBoundingClientRect();
      const textRange = document.createRange();
      textRange.selectNodeContents(element);
      for (const textBounds of textRange.getClientRects()) {
        if (
          textBounds.left < valueBounds.left - tolerance ||
          textBounds.top < valueBounds.top - tolerance ||
          textBounds.right > valueBounds.right + tolerance ||
          textBounds.bottom > valueBounds.bottom + tolerance
        ) {
          return false;
        }
      }
      const ownerBounds = owner.getBoundingClientRect();
      return valueBounds.left >= ownerBounds.left - tolerance &&
        valueBounds.top >= ownerBounds.top - tolerance &&
        valueBounds.right <= ownerBounds.right + tolerance &&
        valueBounds.bottom <= ownerBounds.bottom + tolerance;
    };
    const originalSize = element.style.fontSize;
    const candidates: Array<{ fits: boolean; size: number }> = [];
    for (
      let candidate = maxFontSize;
      candidate >= minFontSize - .001;
      candidate = Math.round((candidate - fontStep) * 10) / 10
    ) {
      element.style.fontSize = `${candidate}px`;
      candidates.push({ fits: fitsOwner(), size: candidate });
    }
    const largestFittingSize = candidates.find((candidate) => candidate.fits)?.size;
    const nextSize = Math.round((fontSize + fontStep) * 10) / 10;
    element.style.fontSize = `${nextSize}px`;
    const nextStepOverflows = !fitsOwner();
    element.style.fontSize = originalSize;
    const ownerBounds = owner?.getBoundingClientRect();
    const textRange = document.createRange();
    textRange.selectNodeContents(element);
    const textRects = Array.from(textRange.getClientRects()).map((rect) => ({
      bottom: rect.bottom,
      left: rect.left,
      right: rect.right,
      top: rect.top
    }));
    return {
      clientHeight: element.clientHeight,
      clientWidth: element.clientWidth,
      fontSize,
      fontStep,
      largestFittingSize,
      maxFontSize,
      minFontSize,
      nextSize,
      nextStepOverflows,
      ownerBounds: ownerBounds && {
        bottom: ownerBounds.bottom,
        left: ownerBounds.left,
        right: ownerBounds.right,
        top: ownerBounds.top
      },
      scrollHeight: element.scrollHeight,
      scrollWidth: element.scrollWidth,
      state: element.getAttribute("data-adaptive-fit-state"),
      text: element.textContent?.trim(),
      textRects
    };
  }));
  for (const measurement of measurements) {
    expect(measurement.state, measurement.text).toBe("fit");
    expect(measurement.fontStep, measurement.text).toBe(.5);
    expect(measurement.fontSize, measurement.text)
      .toBeGreaterThanOrEqual(measurement.minFontSize);
    expect(measurement.fontSize, measurement.text)
      .toBeLessThanOrEqual(measurement.maxFontSize);
    expect(measurement.fontSize, measurement.text)
      .toBe(measurement.largestFittingSize);
    expect(measurement.scrollWidth, measurement.text)
      .toBeLessThanOrEqual(measurement.clientWidth + 1);
    expect(measurement.scrollHeight, measurement.text)
      .toBeLessThanOrEqual(measurement.clientHeight + 1);
    if (measurement.fontSize < measurement.maxFontSize) {
      expect(measurement.nextStepOverflows, measurement.text).toBe(true);
    }
    expect(measurement.ownerBounds, measurement.text).toBeDefined();
    if (measurement.ownerBounds) {
      for (const textRect of measurement.textRects) {
        expect(textRect.left, measurement.text)
          .toBeGreaterThanOrEqual(measurement.ownerBounds.left - 1);
        expect(textRect.top, measurement.text)
          .toBeGreaterThanOrEqual(measurement.ownerBounds.top - 1);
        expect(textRect.right, measurement.text)
          .toBeLessThanOrEqual(measurement.ownerBounds.right + 1);
        expect(textRect.bottom, measurement.text)
          .toBeLessThanOrEqual(measurement.ownerBounds.bottom + 1);
      }
    }
  }

  const expectedFontSizes = [
    [".name-field > .adaptive-plain-value", "20.5"],
    [".address-field > .adaptive-plain-value", "12.5"],
    [".contact-email-field > .adaptive-plain-value", "17.5"],
    [".tax-relief-field > .adaptive-plain-value", "12.0"],
    [".page-two-identity > .adaptive-plain-value", "13.0"]
  ] as const;
  for (const [selector, fontSize] of expectedFontSizes) {
    await expect(page.locator(selector)).toHaveAttribute(
      "data-adaptive-font-size-px",
      fontSize
    );
  }

  expect(await page.locator(
    ".tax-relief-field > .adaptive-plain-value"
  ).evaluate((element) => {
    const range = document.createRange();
    range.selectNodeContents(element);
    return range.getClientRects().length;
  })).toBe(2);

  const pageTwoName = page.locator(
    ".page-two-identity > .adaptive-plain-value"
  );
  await expect(pageTwoName).toHaveAttribute("data-adaptive-font-size-px", "13.0");
  const pageTwoLongNameCandidates = await pageTwoName.evaluate((element) => {
    const originalSize = element.style.fontSize;
    const ownerBounds = element.parentElement?.getBoundingClientRect();
    const candidates = [13, 13.5].map((fontSize) => {
      element.style.fontSize = `${fontSize}px`;
      const valueBounds = element.getBoundingClientRect();
      const range = document.createRange();
      range.selectNodeContents(element);
      const textBounds = range.getBoundingClientRect();
      return {
        clientHeight: element.clientHeight,
        clientWidth: element.clientWidth,
        fontSize,
        scrollHeight: element.scrollHeight,
        scrollWidth: element.scrollWidth,
        textBounds: {
          bottom: textBounds.bottom,
          height: textBounds.height,
          left: textBounds.left,
          right: textBounds.right,
          top: textBounds.top,
          width: textBounds.width
        },
        valueBounds: {
          bottom: valueBounds.bottom,
          height: valueBounds.height,
          left: valueBounds.left,
          right: valueBounds.right,
          top: valueBounds.top,
          width: valueBounds.width
        }
      };
    });
    element.style.fontSize = originalSize;
    return {
      candidates,
      ownerBounds: ownerBounds && {
        bottom: ownerBounds.bottom,
        height: ownerBounds.height,
        left: ownerBounds.left,
        right: ownerBounds.right,
        top: ownerBounds.top,
        width: ownerBounds.width
      }
    };
  });
  const [pageTwoAt13, pageTwoAt13Point5] = pageTwoLongNameCandidates.candidates;
  expect(pageTwoAt13.clientWidth).toBe(490);
  expect(pageTwoAt13.valueBounds.width).toBeCloseTo(490.6875, 3);
  expect(pageTwoAt13.textBounds.width).toBeCloseTo(471.46875, 3);
  expect(pageTwoAt13.scrollWidth).toBeLessThanOrEqual(pageTwoAt13.clientWidth + 1);
  expect(pageTwoAt13.textBounds.right)
    .toBeLessThanOrEqual(pageTwoAt13.valueBounds.right + 1);
  expect(pageTwoAt13Point5.textBounds.width).toBeCloseTo(489.59375, 3);
  expect(pageTwoAt13Point5.scrollWidth)
    .toBeGreaterThan(pageTwoAt13Point5.clientWidth + 1);
  expect(pageTwoAt13Point5.textBounds.right)
    .toBeGreaterThan(pageTwoAt13Point5.valueBounds.right + 1);
  expect(pageTwoLongNameCandidates.ownerBounds).toBeDefined();
  if (pageTwoLongNameCandidates.ownerBounds) {
    expect(pageTwoAt13Point5.textBounds.right)
      .toBeGreaterThan(pageTwoLongNameCandidates.ownerBounds.right + 1);
  }
  await pageTwoName.evaluate((element) => {
    const owner = element.parentElement;
    if (!owner) throw new Error("page-two adaptive value is missing its field owner");
    owner.dataset.originalInlineWidth = owner.style.width;
    owner.style.width = `${owner.clientWidth - (element.clientWidth - 400)}px`;
  });
  await expect.poll(async () => Number.parseFloat(
    await pageTwoName.getAttribute("data-adaptive-font-size-px") ?? "NaN"
  )).toBeLessThan(13);
  await expect(pageTwoName).toHaveAttribute("data-adaptive-fit-state", "fit");
  expect(Number.parseFloat(
    await pageTwoName.getAttribute("data-adaptive-font-size-px") ?? "NaN"
  )).toBeGreaterThanOrEqual(10.5);
  await pageTwoName.evaluate((element) => {
    const owner = element.parentElement;
    if (!owner) throw new Error("page-two adaptive value is missing its field owner");
    owner.style.width = owner.dataset.originalInlineWidth ?? "";
    delete owner.dataset.originalInlineWidth;
  });
  await expect.poll(async () => (
    await pageTwoName.getAttribute("data-adaptive-font-size-px")
  )).toBe("13.0");

  const unfitFixture = structuredClone(fixture) as {
    taxpayer: { name: string };
  };
  unfitFixture.taxpayer.name = "W".repeat(160);
  await renderEnvelope(page, unfitFixture);
  await page.waitForFunction(
    () => !document.querySelector('[data-adaptive-fit-state="pending"]')
  );
  const unfitNames = page.locator(
    ".name-field > .adaptive-plain-value, .page-two-identity > .adaptive-plain-value"
  );
  await expect(unfitNames).toHaveCount(2);
  for (const unfitName of await unfitNames.all()) {
    await expect(unfitName).toHaveAttribute("data-adaptive-font-size-px", "10.5");
    await expect(unfitName).toHaveAttribute("data-adaptive-fit-state", "unresolved");
  }
  const unfitGeometry = await page.evaluate(() => (
    window as Window & {
      measureEbirFormGeometry?: () => {
        pages: Array<{ descendant_overflow_x: number }>;
      } | null;
    }
  ).measureEbirFormGeometry?.());
  expect(unfitGeometry?.pages.some((renderedPage) =>
    renderedPage.descendant_overflow_x > 0
  )).toBe(true);
});

test("2551Q Item 17 measures its fixed field and blocks unreadable values", async ({
  page
}) => {
  const baseFixture = readFixture(
    "packages/form-contracts/fixtures/2551q-6-rows.json"
  ) as {
    fields: Record<string, { type: string; value: unknown }>;
  };
  await renderEnvelope(page, baseFixture);
  await page.waitForFunction(
    () => !document.querySelector('[data-adaptive-fit-state="pending"]')
  );

  const normalDescription = page.locator(".tax-credit-description");
  await expect(normalDescription).toHaveText("Validated prior payment");
  await expect(normalDescription).toHaveAttribute("data-adaptive-fit-state", "fit");
  await expect(normalDescription).toHaveAttribute("data-adaptive-font-size-px", "12.0");
  await expect(normalDescription).toHaveAttribute("data-adaptive-max-font-px", "12");
  await expect(normalDescription).toHaveAttribute("data-adaptive-min-font-px", "10.5");
  await expect(normalDescription).toHaveAttribute("data-adaptive-step-px", "0.5");
  await expect(page.locator('.official-tax-line[data-item="17"]')).not.toHaveClass(
    /specify-line-extended/
  );
  const officialField = await normalDescription.boundingBox();
  expect(officialField).not.toBeNull();

  const item15 = await page.locator('.official-tax-line[data-item="15"]').boundingBox();
  const item17 = await page.locator('.official-tax-line[data-item="17"]').boundingBox();
  const item17Label = await page.locator('.official-tax-line[data-item="17"] .tax-line-label').boundingBox();
  const item17Primary = await page.locator('.official-tax-line[data-item="17"] .tax-line-primary').boundingBox();
  const item17Money = await page.locator('.official-tax-line[data-item="17"] > .money-value').boundingBox();
  expect(item15).not.toBeNull();
  expect(item17).not.toBeNull();
  expect(item17Label).not.toBeNull();
  expect(item17Primary).not.toBeNull();
  expect(item17Money).not.toBeNull();
  if (item15 && item17 && item17Label && item17Primary && item17Money && officialField) {
    expect(item17.height).toBeCloseTo(item15.height, 0);
    expect(Math.abs(item17Label.height - item17.height)).toBeLessThanOrEqual(1);
    expect(Math.abs(item17Money.height - item17.height)).toBeLessThanOrEqual(1);
    expect(item17Primary.height).toBeLessThanOrEqual(item17.height);
    expect(officialField.height).toBeLessThan(item17.height);
    expect(officialField.y).toBeGreaterThan(item17.y);
    expect(officialField.y + officialField.height).toBeLessThan(item17.y + item17.height);
    expect(officialField.x).toBeGreaterThanOrEqual(item17Primary.x + item17Primary.width);
    expect(Math.abs(
      officialField.x + officialField.width -
      (item17Label.x + item17Label.width)
    )).toBeLessThanOrEqual(1);
    expect(item17Money.x).toBeCloseTo(item17Label.x + item17Label.width, 1);
  }

  const longFixture = readFixture(
    "packages/form-contracts/fixtures/2551q-long-values.json"
  ) as {
    fields: Record<string, { type: string; value: unknown }>;
  };
  const longDescription = longFixture.fields.other_tax_credit_description?.value;
  if (typeof longDescription !== "string" || Array.from(longDescription).length !== 98) {
    throw new Error("2551Q long-value fixture must retain its reviewed 98-character Item 17 value");
  }

  const reviewedLongFixture = structuredClone(baseFixture);
  reviewedLongFixture.fields.other_tax_credit_description.value = longDescription;
  await renderEnvelope(page, reviewedLongFixture);
  await expect(page.locator('.official-tax-line[data-item="17"]')).toHaveClass(
    /specify-line-extended/
  );
  await page.waitForFunction(
    () => !document.querySelector('[data-adaptive-fit-state="pending"]')
  );

  const reviewedLong = page.locator(".tax-credit-description");
  await expect(reviewedLong).toHaveText(longDescription);
  await expect(reviewedLong).toHaveAttribute(
    "aria-label",
    `Item 17 specification: ${longDescription}`
  );
  await expect(reviewedLong).toHaveAttribute("data-adaptive-font-size-px", "12.0");
  await expect(reviewedLong).toHaveAttribute("data-adaptive-fit-state", "fit");
  const reviewedMeasurement = await reviewedLong.evaluate((element) => {
    const range = document.createRange();
    range.selectNodeContents(element);
    return {
      clientHeight: element.clientHeight,
      clientWidth: element.clientWidth,
      lineCount: range.getClientRects().length,
      scrollHeight: element.scrollHeight,
      scrollWidth: element.scrollWidth,
      whiteSpace: getComputedStyle(element).whiteSpace
    };
  });
  expect(reviewedMeasurement.scrollWidth).toBeLessThanOrEqual(
    reviewedMeasurement.clientWidth + 1
  );
  expect(reviewedMeasurement.scrollHeight).toBeLessThanOrEqual(
    reviewedMeasurement.clientHeight + 1
  );
  expect(reviewedMeasurement.whiteSpace).toBe("normal");
  expect(reviewedMeasurement.lineCount).toBe(2);
  const reviewedField = await reviewedLong.boundingBox();
  expect(reviewedField).not.toBeNull();
  const reviewedLine = await page.locator('.official-tax-line[data-item="17"]').boundingBox();
  const reviewedLabel = await page.locator('.official-tax-line[data-item="17"] .tax-line-label').boundingBox();
  if (item15 && reviewedField && reviewedLine && reviewedLabel) {
    expect(reviewedLine.height).toBeCloseTo(item15.height * 2, 0);
    expect(Math.abs(reviewedField.width - reviewedLabel.width)).toBeLessThanOrEqual(1);
    expect(Math.abs(reviewedField.height - reviewedLine.height / 2)).toBeLessThanOrEqual(.5);
  }
  expect(await pageHasNoOverflow(page.locator(".form-page").first())).toBe(true);

  for (const value of ["W".repeat(100), "W".repeat(320)]) {
    const fixture = structuredClone(baseFixture);
    fixture.fields.other_tax_credit_description.value = value;
    await renderEnvelope(page, fixture);
    await expect(page.locator('.official-tax-line[data-item="17"]')).toHaveClass(
      /specify-line-extended/
    );
    await page.waitForFunction(
      () => !document.querySelector('[data-adaptive-fit-state="pending"]')
    );

    const rendered = page.locator(".tax-credit-description");
    await expect(rendered).toHaveText(value);
    await expect(rendered).toHaveAttribute(
      "aria-label",
      `Item 17 specification: ${value}`
    );
    await expect(rendered).toHaveAttribute("data-adaptive-font-size-px", "10.5");
    await expect(rendered).toHaveAttribute("data-adaptive-fit-state", "unresolved");

    const measurement = await rendered.evaluate((element) => {
      const range = document.createRange();
      range.selectNodeContents(element);
      return {
        clientHeight: element.clientHeight,
        clientWidth: element.clientWidth,
        lineCount: range.getClientRects().length,
        scrollHeight: element.scrollHeight,
        scrollWidth: element.scrollWidth,
        whiteSpace: getComputedStyle(element).whiteSpace
      };
    });
    expect(
      measurement.scrollWidth > measurement.clientWidth + 1 ||
      measurement.scrollHeight > measurement.clientHeight + 1
    ).toBe(true);
    expect(measurement.whiteSpace).toBe("normal");
    expect(measurement.lineCount).toBeGreaterThanOrEqual(2);

    const field = await rendered.boundingBox();
    expect(field).not.toBeNull();
    const unsafeLine = await page.locator('.official-tax-line[data-item="17"]').boundingBox();
    const unsafeLabel = await page.locator('.official-tax-line[data-item="17"] .tax-line-label').boundingBox();
    if (field && unsafeLine && unsafeLabel) {
      expect(Math.abs(field.width - unsafeLabel.width)).toBeLessThanOrEqual(1);
      expect(Math.abs(field.height - unsafeLine.height / 2)).toBeLessThanOrEqual(.5);
    }

    const geometry = await measuredPage(page.locator(".form-page").first());
    expect(
      geometry.descendant_overflow_x + geometry.descendant_overflow_y
    ).toBeGreaterThan(0);
  }
});

test("2551Q adaptive fields retain domain-boundary text and report unsafe geometry", async ({
  page
}) => {
  const fixture = readFixture(
    "packages/form-contracts/fixtures/2551q-long-values.json"
  ) as {
    fields: Record<string, { type: string; value: unknown }>;
    taxpayer: {
      email: string;
      name: string;
      registered_address: string;
    };
  };
  const name = "N".repeat(160);
  const address = "A".repeat(320);
  const email = `${"E".repeat(242)}@EXAMPLE.COM`;
  const relief = "R".repeat(160);
  const item17 = "C".repeat(100);
  fixture.taxpayer.name = name;
  fixture.taxpayer.registered_address = address;
  fixture.taxpayer.email = email;
  fixture.fields.tax_relief_specification.value = relief;
  fixture.fields.other_tax_credit_description.value = item17;

  await renderEnvelope(page, fixture);
  await page.waitForFunction(
    () => !document.querySelector('[data-adaptive-fit-state="pending"]')
  );

  const boundaries = [
    [".name-field > .adaptive-plain-value", name, name],
    [".address-field > .adaptive-plain-value", address, address],
    [".contact-email-field > .adaptive-plain-value", email, email],
    [".tax-relief-field > .adaptive-plain-value", relief, relief],
    [".page-two-identity > .adaptive-plain-value", name, name]
  ] as const;

  for (const [selector, expectedText, expectedLabel] of boundaries) {
    const rendered = page.locator(selector);
    await expect(rendered).toHaveCount(1);
    await expect(rendered).toHaveText(expectedText);
    await expect(rendered).toHaveAttribute("aria-label", expectedLabel);
    const measurement = await rendered.evaluate((element) => ({
      clientHeight: element.clientHeight,
      clientWidth: element.clientWidth,
      fontSize: Number.parseFloat(getComputedStyle(element).fontSize),
      maxFontSize: Number.parseFloat(element.getAttribute("data-adaptive-max-font-px") ?? "NaN"),
      minFontSize: Number.parseFloat(element.getAttribute("data-adaptive-min-font-px") ?? "NaN"),
      scrollHeight: element.scrollHeight,
      scrollWidth: element.scrollWidth,
      state: element.getAttribute("data-adaptive-fit-state")
    }));
    expect(measurement.state, selector).toBe("unresolved");
    expect(measurement.fontSize, selector).toBe(measurement.minFontSize);
    expect(measurement.fontSize, selector).toBeLessThanOrEqual(measurement.maxFontSize);
  }

  const item17Value = page.locator(".tax-credit-description");
  await expect(item17Value).toHaveText(item17);
  await expect(item17Value).toHaveAttribute(
    "aria-label",
    `Item 17 specification: ${item17}`
  );
  await expect(item17Value).toHaveAttribute("data-adaptive-fit-state", "fit");
  await expect(page.locator('.official-tax-line[data-item="17"]')).toHaveClass(
    /specify-line-extended/
  );

  const geometry = await measuredPage(page.locator(".form-page").first());
  expect(geometry.descendant_overflow_x + geometry.descendant_overflow_y)
    .toBeGreaterThan(0);
});

function writeLayeredFidelityDiagnostic(
  staticTextViolations: ReturnType<typeof verifyPageIndexedStaticText>,
  exhaustiveStaticTextViolations: ReturnType<typeof verifyStaticTextExhaustive>,
  manifestCompletenessViolations: ReturnType<
    typeof verifyStaticTextManifestCompleteness
  >
) {
  const reportPath = path.join(
    REPO_ROOT,
    "test-results/form-renderer/2551q-layered-fidelity-diagnostic.json"
  );
  const report = {
    schema_version: 1,
    form_code: "2551Q",
    revision: "2018",
    purpose: "non_promotional_cross_rasterizer_diagnostic",
    promotion_eligible: LAYERED_EDGE_EVIDENCE_POLICY.promotionEligible,
    authoritative_visual_gate:
      LAYERED_EDGE_EVIDENCE_POLICY.authoritativeVisualGate,
    authoritative_max_changed_percent: MAX_CHANGED_PERCENT,
    replaces_authoritative_gate:
      LAYERED_EDGE_EVIDENCE_POLICY.replacesAuthoritativeGate,
    static_text: {
      comparison: "page-indexed-exact-normalized-static-text-v1",
      reviewed_entry_count: OFFICIAL_2551Q_STATIC_TEXT.length,
      manifest_sha256: sha256(Buffer.from(JSON.stringify(OFFICIAL_2551Q_STATIC_TEXT))),
      violations: staticTextViolations,
      passed: staticTextViolations.length === 0
    },
    static_text_exhaustive: {
      comparison: "static-text-exhaustive-v1",
      is_content_assertion: true,
      proves_parity: false,
      reviewed_entry_count: OFFICIAL_2551Q_STATIC_TEXT.length,
      manifest_sha256: sha256(Buffer.from(JSON.stringify(OFFICIAL_2551Q_STATIC_TEXT))),
      official_source_sha256: OFFICIAL_2551Q_SOURCE_SHA256,
      allowed_residual: OFFICIAL_2551Q_ALLOWED_RESIDUAL,
      completeness_selectors: OFFICIAL_2551Q_COMPLETENESS_SELECTORS,
      ordered_violations: exhaustiveStaticTextViolations,
      completeness_violations: manifestCompletenessViolations,
      passed:
        exhaustiveStaticTextViolations.length === 0 &&
        manifestCompletenessViolations.length === 0
    },
    edges: layeredFidelityMetrics,
    passed:
      staticTextViolations.length === 0 &&
      exhaustiveStaticTextViolations.length === 0 &&
      manifestCompletenessViolations.length === 0 &&
      layeredFidelityMetrics.length === 2 &&
      layeredFidelityMetrics.every((metric) => metric.passed)
  };
  fs.mkdirSync(path.dirname(reportPath), { recursive: true });
  fs.writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`);
  console.log(`2551Q_LAYERED_FIDELITY ${JSON.stringify(report)}`);
}

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
    schema_version: 2,
    gate: "official-fidelity-v1",
    // THE TWO FLAGS THAT MAKE A PARITY CLAIM STRUCTURALLY IMPOSSIBLE.
    // official-fidelity-v1 certifies "no worse than a reviewed baseline". It
    // cannot certify "matches the official form" - the source PDFs do not
    // embed their fonts, so the reference encodes Poppler's substituted glyph
    // outlines and text pixels are unwinnable by proof. The audit rejects any
    // report whose flags disagree with these values.
    proves_parity: false,
    is_non_regression_gate: true,
    // Baselines are not pinned yet, so no component gates. The criterion may
    // run in reporting mode now and may not gate a promotion until the
    // structural defects it localizes are fixed and reviewed baselines exist.
    baselines_pinned: false,
    components_reporting_only: true,
    superseded_gate: "official_complete_page_v2_retained_as_diagnostic",
    legacy_gate: "visual_parity",
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
    // `passed` stays bound to the unchanged 1% complete-page gate. Until
    // baselines are pinned, the new components cannot make a run pass that the
    // old gate failed - reporting mode must never be a promotion shortcut.
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
  await expect(pages.nth(1).locator(".page-two-barcode")).toHaveAttribute(
    "data-machine-readable-code",
    "official-pdf417"
  );
  await expect(pages.nth(1).locator(".official-pdf417-symbol")).toHaveCount(1);
  await expect(pages.nth(1).locator(".page-two-barcode > small")).toHaveText(
    "2551Q 01/18ENCS P2"
  );
  await expect(pages.nth(2).locator(".page-two-barcode")).toHaveAttribute(
    "data-machine-readable-code",
    "absent"
  );
  await expect(pages.nth(2).locator(".official-pdf417-symbol")).toHaveCount(0);
  await expect(pages.nth(2).locator(".page-two-barcode")).toContainText(
    "CONTINUATION ATTACHMENT"
  );
  await expect(pages.first().locator(".sheets-value .comb-value")).toHaveText("01");
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

async function expectCriticalRegionGeometry(
  formPage: Locator,
  regions: readonly CriticalRegion[]
) {
  const pageBox = await formPage.boundingBox();
  expect(pageBox, "critical-region page must have geometry").not.toBeNull();
  if (!pageBox) return;

  const failures: Array<{ region: string; dimension: string; difference_css_px: number }> = [];
  for (const region of regions) {
    const box = await formPage.locator(region.selector).boundingBox();
    expect(box, `${region.name} must render`).not.toBeNull();
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
          difference_css_px: difference
        });
      }
    }
  }
  expect(
    failures,
    "critical regions must match the pinned 144 DPI reference within two pixels"
  ).toEqual([]);
}

async function expectHeaderOptionsTopAlignment(pageOne: Locator) {
  const labels = pageOne.locator(".official-header-options .option-label");
  await expect(labels).toHaveCount(5);
  expect(
    await labels.evaluateAll((elements) =>
      elements.map((element) => getComputedStyle(element).alignItems)
    )
  ).toEqual(["flex-start", "flex-start", "flex-start", "flex-start", "flex-start"]);

  for (const selector of [
    ".filing-basis > .option-label:first-child",
    ".filing-basis > .option-choices"
  ]) {
    const style = await pageOne.locator(selector).evaluate((element) => {
      const computed = getComputedStyle(element);
      return {
        borderBottomStyle: computed.borderBottomStyle,
        borderBottomWidth: Number.parseFloat(computed.borderBottomWidth)
      };
    });
    expect(style.borderBottomStyle).toBe("solid");
    expect(style.borderBottomWidth).toBeGreaterThan(0);
  }

  expect(
    await pageOne.locator(".official-header-options .check-box").evaluateAll((elements) =>
      elements.map((element) => getComputedStyle(element).backgroundColor)
    )
  ).toEqual(Array.from({ length: 8 }, () => "rgb(255, 255, 255)"));
}

async function expectBackgroundInformationParity(pageOne: Locator) {
  const labels = [
    [".tin-rdo-row > .field-label", "6 Taxpayer Identification Number (TIN)"],
    [".name-field > .field-label", "8 Taxpayer’s Name (Last Name, First Name, Middle Name for Individual OR Registered Name for Non-Individual)"],
    [".address-field > .field-label", "9 Registered Address (Indicate complete address. If branch, indicate the branch address. If the registered address is different from the current address, go to the RDO to update registered address by using BIR Form No. 1905)"],
    [".contact-email-field > .field-label:first-child", "10 Contact Number (Landline/Cellphone No.)"],
    [".contact-email-field > .field-label:nth-child(2)", "11 Email Address"],
    [".tax-relief-field > .field-label", "12 Are you availing of tax relief under Special Law or International Tax Treaty?"],
    [".tax-relief-spec", "12A If yes, specify"]
  ] as const;
  for (const [selector, wording] of labels) {
    await expect(pageOne.locator(selector)).toHaveText(wording);
  }

  const styles = await pageOne.locator(
    ".tin-rdo-row > .field-label, .name-field > .field-label, .address-field > .field-label, .contact-email-field > .field-label, .tax-relief-spec"
  ).evaluateAll((elements) => elements.map((element) => {
    const style = getComputedStyle(element);
    return {
      backgroundColor: style.backgroundColor,
      color: style.color,
      fontSize: style.fontSize
    };
  }));
  expect(styles).toEqual(Array.from({ length: 6 }, () => ({
    backgroundColor: "rgb(217, 217, 217)",
    color: "rgb(0, 0, 0)",
    fontSize: "12.8px"
  })));

  const noteStyles = await pageOne.locator(
    ".name-field > .field-label em, .address-field > .field-label em, .contact-email-field > .field-label em"
  ).evaluateAll((elements) => elements.map((element) => {
    const style = getComputedStyle(element);
    return {
      fontSize: style.fontSize,
      fontStyle: style.fontStyle,
      letterSpacing: style.letterSpacing,
      transform: style.transform
    };
  }));
  expect(noteStyles).toEqual([
    { fontSize: "10.6667px", fontStyle: "italic", letterSpacing: "normal", transform: "none" },
    { fontSize: "7.06667px", fontStyle: "italic", letterSpacing: "-0.106667px", transform: "none" },
    { fontSize: "9.93333px", fontStyle: "italic", letterSpacing: "normal", transform: "none" }
  ]);

  const separatorStyles = await pageOne.locator(".tin-separator").evaluateAll(
    (elements) => elements.map((element) => ({
      backgroundColor: getComputedStyle(element).backgroundColor,
      leftRule: getComputedStyle(element, "::before").borderLeftColor,
      rightRule: getComputedStyle(element, "::after").borderRightColor
    }))
  );
  expect(separatorStyles).toEqual(Array.from({ length: 3 }, () => ({
    backgroundColor: "rgb(166, 166, 166)",
    leftRule: "rgb(0, 0, 0)",
    rightRule: "rgb(0, 0, 0)"
  })));

  const guide = await pageOne.locator(".name-field .comb-value > span").first().evaluate(
    (element) => ({
      color: getComputedStyle(element, "::after").borderRightColor,
      height: Number.parseFloat(getComputedStyle(element, "::after").height)
    })
  );
  expect(guide.color).toBe("rgb(17, 17, 17)");
  expect(guide.height).toBeCloseTo(9.33, 1);

  expect(await pageOne.locator(".tin-rdo-row .comb-value > span").count()).toBe(17);
  expect(await pageOne.locator(".name-field .comb-value > span").count()).toBe(40);
  expect(await pageOne.locator(".address-field .comb-value > span").count()).toBe(75);
  expect(await pageOne.locator(".contact-email-field .comb-value > span").count()).toBe(40);
  expect(await pageOne.locator(".tax-relief-field .comb-value > span").count()).toBe(26);

  expect(
    await pageOne.locator(".relief-choices .check-box").evaluateAll((elements) =>
      elements.map((element) => getComputedStyle(element).backgroundColor)
    )
  ).toEqual(["rgb(255, 255, 255)", "rgb(255, 255, 255)"]);
}

async function measuredVerticalGap(formPage: Locator, barcodeSelector: string) {
  const barcode = formPage.locator(barcodeSelector);
  const symbol = await barcode.locator(".official-pdf417-symbol").boundingBox();
  const caption = await barcode.locator("small").boundingBox();
  expect(symbol, "PDF417 symbol must have geometry").not.toBeNull();
  expect(caption, "PDF417 caption must have geometry").not.toBeNull();
  if (!symbol || !caption) return Number.NaN;
  return (caption.y - symbol.y - symbol.height) * DEVICE_SCALE_FACTOR;
}

async function expectCriticalRegionContent(pageOne: Locator, pageTwo: Locator) {
  for (const label of [
    "For the",
    "Year Ended",
    "Quarter",
    "Amended Return?",
    "Number of Sheet/s"
  ]) {
    await expect(pageOne.locator(".official-header-options")).toContainText(label);
  }
  for (const label of [
    "Taxpayer Identification Number (TIN)",
    "RDO Code",
    "Taxpayer’s Name",
    "Registered Address",
    "Contact Number",
    "Email Address",
    "Special Law or International Tax Treaty?",
    "What income tax rates are you availing?"
  ]) {
    await expect(pageOne.locator(".background-information")).toContainText(label);
  }
  const taxLines = pageOne.locator(".official-tax-line");
  await expect(taxLines).toHaveCount(11);
  expect(
    await taxLines.evaluateAll((lines) =>
      lines.map((line) => line.getAttribute("data-item"))
    )
  ).toEqual(
    ["14", "15", "16", "17", "18", "19", "20", "21", "22", "23", "24"]
  );
  await expect(pageOne.locator(".tax-credit-description")).toHaveText(
    "Validated prior payment"
  );
  await expect(pageOne.locator(".tax-credit-description")).toHaveAttribute(
    "aria-label",
    "Item 17 specification: Validated prior payment"
  );
  await expect(pageOne.locator(".official-declaration")).toContainText(
    "Signature over Printed Name of Taxpayer/Authorized Representative/Tax Agent"
  );
  await expect(pageOne.locator(".official-declaration > p")).toContainText(
    "I/We declare under the penalties of perjury that this return, and all its attachments, have been made in good faith"
  );
  await expect(
    pageOne.locator(".official-signature-grid > div:first-child .signature-caption b")
  ).toHaveText(
    "Signature over Printed Name of Taxpayer/Authorized Representative/Tax Agent"
  );
  await expect(
    pageOne.locator(".official-signature-grid > div:last-child .signature-caption b")
  ).toHaveText(
    "Signature over Printed Name of President/Vice President/Authorized Officer or Representative/Tax Agent"
  );
  await expect(pageOne.locator(".tax-agent-strip")).toContainText(
    "Tax Agent Accreditation No./Attorney’s Roll No."
  );
  await expect(pageOne.locator(".official-barcode > small")).toHaveText(
    "2551Q 01/18ENCS P1"
  );
  await expect(pageOne.locator(".payment-row .decimal-separator")).toHaveCount(3);
  await expect(pageOne.locator(".payment-other-row .decimal-separator")).toHaveCount(1);
  await expect(pageOne.locator(".blank-money-value")).toHaveCount(4);
  await expect(
    pageOne.locator(".blank-money-value > .comb-value:first-child > span")
  ).toHaveCount(48);
  await expect(
    pageOne.locator(".blank-money-value > .comb-value:last-child > span")
  ).toHaveCount(8);
  await expect(pageTwo.locator(".page-two-barcode > small")).toHaveText(
    "2551Q 01/18ENCS P2"
  );
  await expect(pageTwo.locator(".official-schedule > h2")).toContainText(
    "Schedule 1 – Computation of Tax"
  );
  await expect(pageTwo.locator(".official-schedule-total")).toContainText(
    "Total Tax Due"
  );
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
  popplerReferencePath,
  actualBuffer,
  expectedBuffer,
  popplerBuffer,
  artifactStem,
  outputDir
}: {
  parityCase: ParityCase;
  pageNumber: number;
  fixtureSha256: string;
  referencePath: string;
  popplerReferencePath: string;
  actualBuffer: Buffer;
  expectedBuffer: Buffer;
  popplerBuffer: Buffer;
  artifactStem: string;
  outputDir: string;
}) {
  const actual = PNG.sync.read(actualBuffer);
  const expected = PNG.sync.read(expectedBuffer);
  const chromiumRaster = chromiumRasterFor(
    parityCase.code,
    parityCase.revision,
    pageNumber
  );
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
      reference_kind: "official_chromium_raster",
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
      comparison: "official-complete-page-v2",
      expected_ink_missing_percent: null,
      unexpected_actual_ink_percent: null,
      poppler_reference: popplerReferencePath,
      poppler_reference_sha256: sha256(popplerBuffer),
      poppler_diff: null,
      poppler_diff_sha256: null,
      poppler_raster_changed_pixels: null,
      poppler_raster_changed_percent: null,
      reference_noise_floor_changed_pixels:
        chromiumRaster.noise_floor_changed_pixels,
      reference_noise_floor_changed_percent:
        chromiumRaster.noise_floor_changed_percent,
      region_report: null,
      region_report_sha256: null,
      structural_changed_pixels: null,
      structural_changed_percent: null,
      structural_diff: null,
      structural_diff_sha256: null,
      structural_ink_threshold: STRUCTURAL_INK_THRESHOLD,
      structural_line_min_run: STRUCTURAL_LINE_MIN_RUN,
      structural_tolerance_radius: STRUCTURAL_TOLERANCE_RADIUS,
      legacy_structural_superseded_by: "structural-ink-coverage-v1",
      cell_edge_f1: null,
      structural_ink_coverage: null,
      page_ink_budget: null,
      passed: false
    });
    return;
  }

  const structural = compareOfficialStructure(expected, actual);
  const structuralChangedPercent =
    (structural.changedPixels / (expected.width * expected.height)) * 100;
  const completePage = compareCompleteOfficialPage(expected, actual, {
    pixelThreshold: PIXELMATCH_THRESHOLD
  });

  // official-fidelity-v1 pixel components. REPORTING MODE: these are computed
  // and recorded, but nothing here gates yet. The criterion is a
  // non-regression gate against reviewed baselines, and no baseline may be
  // pinned until the structural defects it localizes are fixed - pinning now
  // would bake a 1130px rule misplacement into the accepted floor.
  const cellEdgeF1 = computeCellEdgeF1(
    expected,
    actual,
    parityCase.code,
    parityCase.revision,
    pageNumber
  );
  const structuralInk = computeStructuralInk(expected, actual);
  const pageInkBudget = computePageInkBudget(expected, actual);
  const cellsPath = path.join(outputDir, `${artifactStem}-fidelity-cells.json`);
  const cellsPayload = `${JSON.stringify(cellEdgeF1.cells, null, 2)}\n`;
  fs.mkdirSync(outputDir, { recursive: true });
  fs.writeFileSync(cellsPath, cellsPayload);
  const edgeMatch = compareSymmetricGrayscaleEdges(expected, actual, {
    toleranceRadiusPx: EDGE_TOLERANCE_RADIUS_PX
  });
  const layeredPassed =
    edgeMatch.precision >= MIN_EDGE_PRECISION &&
    edgeMatch.recall >= MIN_EDGE_RECALL &&
    edgeMatch.f1 >= MIN_EDGE_F1;
  layeredFidelityMetrics.push({
    page: pageNumber,
    comparison: "symmetric-grayscale-sobel-edge-v1",
    tolerance_radius_px: edgeMatch.toleranceRadiusPx,
    edge_threshold: edgeMatch.edgeThreshold,
    expected_edge_pixels: edgeMatch.expectedEdgePixels,
    actual_edge_pixels: edgeMatch.actualEdgePixels,
    precision: edgeMatch.precision,
    recall: edgeMatch.recall,
    f1: edgeMatch.f1,
    minimum_precision: MIN_EDGE_PRECISION,
    minimum_recall: MIN_EDGE_RECALL,
    minimum_f1: MIN_EDGE_F1,
    passed: layeredPassed
  });
  const passed = completePage.fullPageChangedPercent <= MAX_CHANGED_PERCENT;

  // Non-gating raw Poppler diagnostic: the same comparison against the
  // Poppler raster of the same official page. Reported next to the gate so
  // the cross-rasterizer difference stays visible, never hidden.
  const poppler = PNG.sync.read(popplerBuffer);
  const popplerComparison = comparePixelMask(poppler, actual, PIXELMATCH_THRESHOLD);
  const popplerDiffBuffer = PNG.sync.write(popplerComparison.diff);
  const popplerDiffPath = path.join(
    outputDir,
    `${artifactStem}-poppler-diff.png`
  );
  fs.mkdirSync(outputDir, { recursive: true });
  fs.writeFileSync(popplerDiffPath, popplerDiffBuffer);

  // Region-ranked localization diagnostic (always written, pass or fail).
  const regionReport = writeRegionReport({
    outputDir,
    artifactStem,
    formCode: parityCase.code,
    formRevision: parityCase.revision,
    pageNumber,
    comparison: "official-complete-page-v2",
    expected,
    actual,
    diff: completePage.diff,
    regions: criticalRegionsFor(parityCase.code, parityCase.revision, pageNumber)
  });

  const diffBuffer = PNG.sync.write(completePage.diff);
  const artifacts = writeVisualArtifacts({
    actualBuffer,
    expectedBuffer,
    diffBuffer,
    artifactStem,
    outputDir
  });
  const structuralDiffBuffer = PNG.sync.write(structural.diff);
  const structuralDiffPath = path.join(
    outputDir,
    `${artifactStem}-structure-diff.png`
  );
  fs.writeFileSync(structuralDiffPath, structuralDiffBuffer);

  visualPageMetrics.push({
    form_code: parityCase.code,
    form_revision: parityCase.revision,
    fixture: parityCase.fixture,
    fixture_sha256: fixtureSha256,
    reference: referencePath,
    reference_sha256: sha256(expectedBuffer),
    reference_kind: "official_chromium_raster",
    actual: artifacts.actual.path,
    actual_sha256: artifacts.actual.sha256,
    diff: artifacts.diff?.path ?? null,
    diff_sha256: artifacts.diff?.sha256 ?? null,
    page: pageNumber,
    expected_width: expected.width,
    expected_height: expected.height,
    actual_width: actual.width,
    actual_height: actual.height,
    changed_pixels: completePage.fullPageChangedPixels,
    changed_percent: completePage.fullPageChangedPercent,
    max_changed_percent: MAX_CHANGED_PERCENT,
    pixelmatch_threshold: PIXELMATCH_THRESHOLD,
    comparison: "official-complete-page-v2",
    expected_ink_missing_percent: completePage.expectedInkMissingPercent,
    unexpected_actual_ink_percent: completePage.unexpectedActualInkPercent,
    poppler_reference: popplerReferencePath,
    poppler_reference_sha256: sha256(popplerBuffer),
    poppler_diff: repositoryRelativePath(popplerDiffPath),
    poppler_diff_sha256: sha256(popplerDiffBuffer),
    poppler_raster_changed_pixels: popplerComparison.changedPixels,
    poppler_raster_changed_percent:
      (popplerComparison.changedPixels / (expected.width * expected.height)) * 100,
    reference_noise_floor_changed_pixels:
      chromiumRaster.noise_floor_changed_pixels,
    reference_noise_floor_changed_percent:
      chromiumRaster.noise_floor_changed_percent,
    region_report: repositoryRelativePath(regionReport.jsonPath),
    region_report_sha256: regionReport.jsonSha256,
    structural_changed_pixels: structural.changedPixels,
    structural_changed_percent: structuralChangedPercent,
    structural_diff: repositoryRelativePath(structuralDiffPath),
    structural_diff_sha256: sha256(structuralDiffBuffer),
    structural_ink_threshold: STRUCTURAL_INK_THRESHOLD,
    structural_line_min_run: STRUCTURAL_LINE_MIN_RUN,
    structural_tolerance_radius: STRUCTURAL_TOLERANCE_RADIUS,
    legacy_structural_superseded_by: "structural-ink-coverage-v1",
    cell_edge_f1: {
      comparison: "cell-edge-f1-v1",
      tolerance_radius_px: TOLERANCE_RADIUS_PX,
      edge_threshold: EDGE_THRESHOLD,
      min_cell_edge_pixels: MIN_CELL_EDGE_PIXELS,
      cell_table_sha256: cellEdgeF1.cellTableSha256,
      cell_count: cellEdgeF1.cells.length,
      scored_cell_count: cellEdgeF1.scoredCellCount,
      worst_scored_f1: cellEdgeF1.worstScoredF1,
      edge_coverage: cellEdgeF1.edgeCoverage,
      min_edge_coverage: MIN_EDGE_COVERAGE,
      cells: repositoryRelativePath(cellsPath),
      cells_sha256: sha256(Buffer.from(cellsPayload, "utf8"))
    },
    structural_ink_coverage: structuralInk,
    page_ink_budget: pageInkBudget,
    passed
  });
}

/**
 * Compare the semantic form's ruled structure while tolerating the rasterizer
 * differences between Poppler's pinned-PDF output and Chromium's HTML output.
 * Only continuous rule segments participate; text and discrete artwork have
 * independent content/hash assertions and fixture-provided glyphs are blanked
 * before capture.  A four-pixel radius is two typographic points at 144 DPI.
 */
function compareOfficialStructure(expected: PNG, actual: PNG) {
  const expectedLines = structuralLineMask(expected);
  const actualLines = structuralLineMask(actual);
  const width = expected.width;
  const height = expected.height;
  const changed = new Uint8Array(width * height);

  markUnmatchedStructure(
    expectedLines,
    actualLines,
    changed,
    width,
    height,
    STRUCTURAL_TOLERANCE_RADIUS
  );
  markUnmatchedStructure(
    actualLines,
    expectedLines,
    changed,
    width,
    height,
    STRUCTURAL_TOLERANCE_RADIUS
  );

  const diff = new PNG({ width, height });
  let changedPixels = 0;
  for (let index = 0; index < changed.length; index += 1) {
    const offset = index * 4;
    if (changed[index] === 1) {
      changedPixels += 1;
      diff.data[offset] = 255;
      diff.data[offset + 1] = 0;
      diff.data[offset + 2] = 0;
      diff.data[offset + 3] = 255;
    } else {
      diff.data[offset] = 0;
      diff.data[offset + 1] = 0;
      diff.data[offset + 2] = 0;
      diff.data[offset + 3] = 0;
    }
  }
  return { changedPixels, diff };
}

function structuralLineMask(image: PNG) {
  const { width, height } = image;
  const dark = new Uint8Array(width * height);
  const lines = new Uint8Array(width * height);
  for (let index = 0; index < dark.length; index += 1) {
    const offset = index * 4;
    dark[index] =
      image.data[offset] < STRUCTURAL_INK_THRESHOLD &&
      image.data[offset + 1] < STRUCTURAL_INK_THRESHOLD &&
      image.data[offset + 2] < STRUCTURAL_INK_THRESHOLD
        ? 1
        : 0;
  }

  for (let y = 0; y < height; y += 1) {
    let runStart = -1;
    for (let x = 0; x <= width; x += 1) {
      const isDark = x < width && dark[y * width + x] === 1;
      if (isDark && runStart < 0) runStart = x;
      if (!isDark && runStart >= 0) {
        if (x - runStart >= STRUCTURAL_LINE_MIN_RUN) {
          for (let fillX = runStart; fillX < x; fillX += 1) {
            lines[y * width + fillX] = 1;
          }
        }
        runStart = -1;
      }
    }
  }

  for (let x = 0; x < width; x += 1) {
    let runStart = -1;
    for (let y = 0; y <= height; y += 1) {
      const isDark = y < height && dark[y * width + x] === 1;
      if (isDark && runStart < 0) runStart = y;
      if (!isDark && runStart >= 0) {
        if (y - runStart >= STRUCTURAL_LINE_MIN_RUN) {
          for (let fillY = runStart; fillY < y; fillY += 1) {
            lines[fillY * width + x] = 1;
          }
        }
        runStart = -1;
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
  height: number,
  radius: number
) {
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const index = y * width + x;
      if (source[index] !== 1) continue;
      let matched = false;
      for (let targetY = Math.max(0, y - radius); targetY <= Math.min(height - 1, y + radius) && !matched; targetY += 1) {
        for (let targetX = Math.max(0, x - radius); targetX <= Math.min(width - 1, x + radius); targetX += 1) {
          if (target[targetY * width + targetX] === 1) {
            matched = true;
            break;
          }
        }
      }
      if (!matched) changed[index] = 1;
    }
  }
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
    const diffPath = path.join(outputDir, `${artifactStem}-full-page-diff.png`);
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
