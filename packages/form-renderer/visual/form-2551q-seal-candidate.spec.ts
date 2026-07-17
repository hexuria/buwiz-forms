import { expect, test, type Page, type TestInfo } from "@playwright/test";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { PNG } from "pngjs";
import { compareCompleteOfficialPage } from "./official-page-diff";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(HERE, "../../..");
const DEVICE_SCALE_FACTOR = 1.5;
const PIXELMATCH_THRESHOLD = 0.1;
const FIXTURE = "packages/form-contracts/fixtures/2551q-6-rows.json";
const REFERENCE = "packages/form-renderer/references/2551q-2018-page-1.png";
const CANDIDATE =
  "packages/form-renderer/references/artwork/bir-seal-2551q-2018-candidate.svg";
const CANDIDATES = [
  {
    id: "binary_exact_white",
    file: "packages/form-renderer/references/artwork/bir-seal-commons-binary-exact-white.svg",
    mapping: "only_exact_white_stays_white",
    threshold: 255,
  },
] as const;

test("measure the retained 2551Q Commons-vector seal candidate without promoting it", async ({
  page,
}, testInfo) => {
  const envelope = JSON.parse(
    fs.readFileSync(path.join(REPO_ROOT, FIXTURE), "utf8")
  ) as unknown;
  await renderEnvelope(page, envelope);
  await blankFixtureValues(page);

  const pageOne = page.locator(".form-2551q-page-one");
  const seal = pageOne.locator(".government-seal");
  await expect(pageOne).toHaveCount(1);
  await expect(seal).toHaveCount(1);
  expect(await naturalSize(seal)).toEqual({ width: 95, height: 83 });

  const pageBox = await pageOne.boundingBox();
  const sealBox = await seal.boundingBox();
  expect(pageBox).not.toBeNull();
  expect(sealBox).not.toBeNull();
  if (!pageBox || !sealBox) throw new Error("2551Q seal geometry is unavailable");
  const sealRegion = {
    x: Math.round((sealBox.x - pageBox.x) * DEVICE_SCALE_FACTOR),
    y: Math.round((sealBox.y - pageBox.y) * DEVICE_SCALE_FACTOR),
    width: Math.round(sealBox.width * DEVICE_SCALE_FACTOR),
    height: Math.round(sealBox.height * DEVICE_SCALE_FACTOR),
  };
  expect(sealRegion).toEqual({ x: 490, y: 44, width: 58, height: 50 });

  const currentBuffer = await pageOne.screenshot({
    animations: "disabled",
    caret: "hide",
  });
  const referenceBuffer = fs.readFileSync(path.join(REPO_ROOT, REFERENCE));
  const reference = PNG.sync.read(referenceBuffer);
  const current = PNG.sync.read(currentBuffer);
  const currentPage = compareCompleteOfficialPage(reference, current, {
    pixelThreshold: PIXELMATCH_THRESHOLD,
  });
  const referenceSeal = crop(reference, sealRegion);
  const currentSeal = crop(current, sealRegion);
  const currentSealMetric = compareCompleteOfficialPage(referenceSeal, currentSeal, {
    pixelThreshold: PIXELMATCH_THRESHOLD,
  });
  const artifactImages: Record<string, PNG> = {
    current,
    currentPageDiff: currentPage.diff,
    currentSeal,
    currentSealDiff: currentSealMetric.diff,
    referenceSeal,
  };
  const candidateMeasurements = [];
  for (const candidateDefinition of CANDIDATES) {
    const candidateUrl = nestedCandidateDataUrl(candidateDefinition.file);
    await seal.evaluate((element, source) => {
      const image = element as HTMLImageElement;
      image.src = source;
    }, candidateUrl);
    await page.waitForFunction(() => {
      const image = document.querySelector<HTMLImageElement>(".government-seal");
      return image?.complete
        && image.naturalWidth === 95
        && image.naturalHeight === 83;
    });
    await twoFrames(page);
    expect(await naturalSize(seal)).toEqual({ width: 95, height: 83 });

    const candidateBuffer = await pageOne.screenshot({
      animations: "disabled",
      caret: "hide",
    });
    const candidate = PNG.sync.read(candidateBuffer);
    const candidatePage = compareCompleteOfficialPage(reference, candidate, {
      pixelThreshold: PIXELMATCH_THRESHOLD,
    });
    const pageMutation = compareCompleteOfficialPage(current, candidate, {
      pixelThreshold: PIXELMATCH_THRESHOLD,
    });
    const candidateSeal = crop(candidate, sealRegion);
    const candidateSealMetric = compareCompleteOfficialPage(
      referenceSeal,
      candidateSeal,
      { pixelThreshold: PIXELMATCH_THRESHOLD }
    );
    const sealMutation = compareCompleteOfficialPage(currentSeal, candidateSeal, {
      pixelThreshold: PIXELMATCH_THRESHOLD,
    });
    const artifactPrefix = camel(candidateDefinition.id);
    artifactImages[artifactPrefix] = candidate;
    artifactImages[`${artifactPrefix}PageDiff`] = candidatePage.diff;
    artifactImages[`${artifactPrefix}Seal`] = candidateSeal;
    artifactImages[`${artifactPrefix}SealDiff`] = candidateSealMetric.diff;
    candidateMeasurements.push({
      id: candidateDefinition.id,
      file: candidateDefinition.file,
      sha256: sha256(fs.readFileSync(path.join(REPO_ROOT, candidateDefinition.file))),
      mapping: candidateDefinition.mapping,
      threshold: candidateDefinition.threshold,
      page: {
        metric: metric(candidatePage),
        minus_current_changed_pixels:
          candidatePage.fullPageChangedPixels - currentPage.fullPageChangedPixels,
        current_to_candidate_changed_pixels: pageMutation.fullPageChangedPixels,
      },
      seal: {
        metric: metric(candidateSealMetric),
        minus_current_changed_pixels:
          candidateSealMetric.fullPageChangedPixels
          - currentSealMetric.fullPageChangedPixels,
        current_to_candidate_changed_pixels: sealMutation.fullPageChangedPixels,
      },
    });
  }
  for (const measurement of candidateMeasurements) {
    expect(measurement.page.minus_current_changed_pixels).toBeGreaterThan(0);
    expect(measurement.seal.minus_current_changed_pixels).toBeGreaterThan(0);
  }
  const artifactPaths = writeArtifacts(testInfo, artifactImages);
  const report = {
    schema_version: 2,
    evidence_scope: "development_non_promotional_calibration",
    form: "2551Q:2018",
    fixture: FIXTURE,
    official_reference: REFERENCE,
    geometry_wrapper: CANDIDATE,
    geometry_wrapper_sha256: sha256(fs.readFileSync(path.join(REPO_ROOT, CANDIDATE))),
    binary_threshold_policy: {
      selection: "retained_best_of_three_predeclared_source_based_thresholds",
      threshold: 255,
      rationale: "preserve_only_exact_source_white",
    },
    seal_region: sealRegion,
    official_canvas_pixels: [95, 83],
    official_active_bounds_pixels: [9, 7, 75, 73],
    current_official_pdf_xobject: {
      page: metric(currentPage),
      seal: metric(currentSealMetric),
    },
    candidates: candidateMeasurements,
    artifacts: artifactPaths,
  };
  const reportPath = path.join(testInfo.outputDir, "2551q-seal-calibration.json");
  fs.writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`);
  console.log(`2551Q_SEAL_CALIBRATION ${JSON.stringify(report)}`);
});

async function renderEnvelope(page: Page, envelope: unknown) {
  await page.goto("/");
  await page.waitForFunction(
    () => typeof (window as Window & {
      renderEbirForm?: (value: unknown) => void;
    }).renderEbirForm === "function"
  );
  await page.evaluate((value) => {
    const render = (window as Window & {
      renderEbirForm?: (input: unknown) => void;
    }).renderEbirForm;
    if (!render) throw new Error("renderEbirForm is not installed");
    render(value);
  }, envelope);
  await page.locator(".form-document").waitFor();
  await twoFrames(page);
  await page.evaluate(() => document.fonts.ready);
}

async function blankFixtureValues(page: Page) {
  await page.addStyleTag({
    content: `
      .form-page[data-visual-blank-values="true"] .comb-value > span,
      .form-page[data-visual-blank-values="true"] .adaptive-plain-value,
      .form-page[data-visual-blank-values="true"] .check-box,
      .form-page[data-visual-blank-values="true"] .tax-credit-description {
        color: transparent !important;
        text-shadow: none !important;
      }
    `,
  });
  await page.locator(".form-page").evaluateAll((pages) => {
    for (const formPage of pages) {
      formPage.setAttribute("data-visual-blank-values", "true");
    }
  });
  await twoFrames(page);
}

function nestedCandidateDataUrl(candidateFile: string): string {
  const wrapper = fs.readFileSync(path.join(REPO_ROOT, CANDIDATE), "utf8");
  const candidate = fs.readFileSync(path.join(REPO_ROOT, candidateFile));
  const nested = `data:image/svg+xml;base64,${candidate.toString("base64")}`;
  const selfContained = wrapper.replaceAll(
    "./bir-seal-commons-binary-exact-white.svg",
    nested
  );
  return `data:image/svg+xml;base64,${Buffer.from(selfContained).toString("base64")}`;
}

function crop(
  source: PNG,
  region: { x: number; y: number; width: number; height: number }
): PNG {
  const target = new PNG({ width: region.width, height: region.height });
  PNG.bitblt(
    source,
    target,
    region.x,
    region.y,
    region.width,
    region.height,
    0,
    0
  );
  return target;
}

function metric(comparison: ReturnType<typeof compareCompleteOfficialPage>) {
  return {
    changed_pixels: comparison.fullPageChangedPixels,
    changed_percent: comparison.fullPageChangedPercent,
    expected_ink_missing_percent: comparison.expectedInkMissingPercent,
    unexpected_actual_ink_percent: comparison.unexpectedActualInkPercent,
  };
}

function writeArtifacts(
  testInfo: TestInfo,
  artifacts: Record<string, PNG>
): Record<string, string> {
  fs.mkdirSync(testInfo.outputDir, { recursive: true });
  return Object.fromEntries(Object.entries(artifacts).map(([name, image]) => {
    const file = path.join(testInfo.outputDir, `${kebab(name)}.png`);
    fs.writeFileSync(file, PNG.sync.write(image));
    return [name, file];
  }));
}

async function naturalSize(locator: ReturnType<Page["locator"]>) {
  return locator.evaluate((element) => ({
    width: (element as HTMLImageElement).naturalWidth,
    height: (element as HTMLImageElement).naturalHeight,
  }));
}

async function twoFrames(page: Page) {
  await page.evaluate(() => new Promise<void>((resolve) => {
    requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
  }));
}

function sha256(payload: Buffer): string {
  return crypto.createHash("sha256").update(payload).digest("hex");
}

function kebab(value: string): string {
  return value.replaceAll(/([a-z0-9])([A-Z])/g, "$1-$2").toLowerCase();
}

function camel(value: string): string {
  return value.replaceAll(/_([a-z0-9])/g, (_match, character: string) =>
    character.toUpperCase()
  );
}
