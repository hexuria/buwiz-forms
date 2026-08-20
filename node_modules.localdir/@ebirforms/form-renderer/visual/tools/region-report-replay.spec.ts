// Browserless replay: re-slice the region-ranked diff report from the LAST
// captured visual run without relaunching the dev server or browser.
//
// Reads test-results/form-renderer/visual-evidence.json, sha-verifies the
// recorded actual screenshots and pinned references (a stale or moved capture
// fails loudly instead of silently re-ranking old pixels), recomputes the
// changed-pixel mask, and rewrites the region reports in seconds. This is a
// calibration convenience only; it produces no promotion evidence.

import { expect, test } from "@playwright/test";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { PNG } from "pngjs";
import { comparePixelMask } from "../official-page-diff";
import { writeRegionReport } from "../region-report";
import { criticalRegionsFor } from "../regions/2551q";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(HERE, "../../../..");

interface RecordedPage {
  form_code: string;
  form_revision: string;
  page: number;
  reference: string;
  reference_sha256: string;
  actual: string;
  actual_sha256: string;
  changed_pixels: number | null;
  changed_percent: number | null;
  poppler_raster_changed_percent: number | null;
  reference_noise_floor_changed_percent: number;
  pixelmatch_threshold: number;
}

test("replay region-ranked diff reports from the last visual run", async ({}, testInfo) => {
  const evidencePath = process.env.FORM_VISUAL_EVIDENCE_PATH
    ? path.resolve(process.env.FORM_VISUAL_EVIDENCE_PATH)
    : path.join(REPO_ROOT, "test-results/form-renderer/visual-evidence.json");
  expect(
    fs.existsSync(evidencePath),
    `No captured visual evidence at ${evidencePath}; run npm run test:forms:visual first`
  ).toBe(true);
  const evidence = JSON.parse(fs.readFileSync(evidencePath, "utf8")) as {
    pages: RecordedPage[];
  };
  expect(evidence.pages.length, "captured run has no pages").toBeGreaterThan(0);

  for (const recorded of evidence.pages) {
    const label = `${recorded.form_code}:${recorded.form_revision} page ${recorded.page}`;
    const actualPath = path.join(REPO_ROOT, recorded.actual);
    const referencePath = path.join(REPO_ROOT, recorded.reference);
    expect(
      fs.existsSync(actualPath),
      `${label}: recorded actual screenshot is gone (${recorded.actual}); rerun the visual suite`
    ).toBe(true);
    const actualBuffer = fs.readFileSync(actualPath);
    expect(
      sha256(actualBuffer),
      `${label}: recorded actual screenshot changed on disk; rerun the visual suite`
    ).toBe(recorded.actual_sha256);
    const referenceBuffer = fs.readFileSync(referencePath);
    expect(
      sha256(referenceBuffer),
      `${label}: pinned reference changed since the capture; rerun the visual suite`
    ).toBe(recorded.reference_sha256);

    const actual = PNG.sync.read(actualBuffer);
    const expected = PNG.sync.read(referenceBuffer);
    const comparison = comparePixelMask(
      expected,
      actual,
      recorded.pixelmatch_threshold
    );
    if (recorded.changed_pixels !== null) {
      expect(
        comparison.changedPixels,
        `${label}: replay disagrees with the recorded gate count`
      ).toBe(recorded.changed_pixels);
    }
    const report = writeRegionReport({
      outputDir: testInfo.outputDir,
      artifactStem: `${recorded.form_code.toLowerCase()}-${recorded.form_revision.toLowerCase()}-page-${recorded.page}`,
      formCode: recorded.form_code,
      formRevision: recorded.form_revision,
      pageNumber: recorded.page,
      comparison: "official-complete-page-v2",
      expected,
      actual,
      diff: comparison.diff,
      regions: criticalRegionsFor(
        recorded.form_code,
        recorded.form_revision,
        recorded.page
      )
    });
    const worst = report.stats
      .slice(0, 5)
      .map(
        (stat) =>
          `#${stat.rank} ${stat.id} (${stat.changed_pixels}px, ${stat.changed_percent_of_page.toFixed(1)}% of page)`
      )
      .join("; ");
    console.log(
      `${label}: gate ${recorded.changed_percent?.toFixed(4)}% | poppler ${recorded.poppler_raster_changed_percent?.toFixed(4)}% | floor ${recorded.reference_noise_floor_changed_percent.toFixed(4)}%`
    );
    console.log(`${label}: worst regions -> ${worst}`);
    console.log(`${label}: report ${report.markdownPath}`);
  }
});

function sha256(value: Buffer) {
  return crypto.createHash("sha256").update(value).digest("hex");
}
