// Font-attribution sweep (non-promotional diagnostic).
//
// Sweeps candidate font stacks and micro-typography variants, diffing each
// against BOTH pinned rasters (the chromium gate reference and the Poppler
// diagnostic raster), and writes a ranked summary plus per-candidate region
// reports so calibration knows which stack moves which regions.
//
// NOTE ON WHAT THIS TOOL CAN AND CANNOT FIND. A 2026-07-20 investigation
// measured the typeface axis to be near-irrelevant for 2551Q: swapping in the
// real platform Arial (the face the official PDF specifies) scored 7.3631%
// against bundled Arimo's 7.3355% — marginally WORSE — and Helvetica and
// Liberation Sans landed within 0.11pp. The measured rasterization/anti-alias
// floor is only 0.6875% (page 1) / 0.5540% (page 2). The dominant term is
// therefore per-run GEOMETRY (size, advance width, and baseline position), not
// glyph outlines and not anti-aliasing. Use this sweep to rule the typeface
// axis in or out; use the region report and per-run measurement for the
// geometry error that actually drives the gate.
//
// Form-generic: FONT_SWEEP_FORM / FONT_SWEEP_REVISION select the target
// (default 2551Q:2018); fixtures and references resolve from the pinned
// reference manifest. Output is never promotion evidence.

import { expect, test } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { PNG } from "pngjs";
import { comparePixelMask } from "../official-page-diff";
import { writeRegionReport } from "../region-report";
import { criticalRegionsFor } from "../regions/2551q";
import {
  blankComparisonEnvelope,
  prepareOfficialBlankComparison,
  renderEnvelope
} from "../support/render-utils";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(HERE, "../../../..");
const PIXELMATCH_THRESHOLD = 0.1;
const FORM_CODE = (process.env.FONT_SWEEP_FORM ?? "2551Q").toUpperCase();
const FORM_REVISION = process.env.FONT_SWEEP_REVISION ?? "2018";

interface Candidate {
  slug: string;
  family: string | null;
  extraCss?: string;
}

const CANDIDATES: readonly Candidate[] = [
  { slug: "bundled-default", family: null },
  { slug: "bundled-arimo", family: '"eBIRForms Arimo", sans-serif' },
  {
    slug: "bundled-arimo-tight",
    family: '"eBIRForms Arimo", sans-serif',
    extraCss: "letter-spacing: -0.1px;"
  },
  {
    slug: "bundled-arimo-loose",
    family: '"eBIRForms Arimo", sans-serif',
    extraCss: "letter-spacing: 0.1px;"
  },
  {
    slug: "bundled-arimo-no-synthesis",
    family: '"eBIRForms Arimo", sans-serif',
    extraCss: "font-synthesis: none;"
  },
  { slug: "browser-arial", family: "Arial, sans-serif" },
  { slug: "browser-helvetica", family: "Helvetica, sans-serif" },
  { slug: "browser-nimbus", family: '"Nimbus Sans", sans-serif' },
  { slug: "browser-sans", family: "sans-serif" }
];

interface ManifestForm {
  code: string;
  revision: string;
  fixture: string;
  pages: Array<{
    page: number;
    reference_png: string;
    chromium_raster?: { reference_png: string };
  }>;
}

test(`font sweep attributes ${FORM_CODE}:${FORM_REVISION} glyph differences`, async ({
  page
}, testInfo) => {
  const manifest = JSON.parse(
    fs.readFileSync(
      path.join(REPO_ROOT, "packages/form-renderer/references/manifest.json"),
      "utf8"
    )
  ) as { forms: ManifestForm[] };
  const form = manifest.forms.find(
    (candidate) =>
      candidate.code === FORM_CODE && candidate.revision === FORM_REVISION
  );
  expect(form, `${FORM_CODE}:${FORM_REVISION} is not in the reference manifest`).toBeTruthy();
  if (!form) return;

  const fixture = JSON.parse(
    fs.readFileSync(path.join(REPO_ROOT, form.fixture), "utf8")
  ) as unknown;
  const blankFixture = blankComparisonEnvelope(fixture, FORM_CODE);
  const summaryDir = path.join(
    REPO_ROOT,
    "test-results/form-renderer/font-sweep",
    `${FORM_CODE.toLowerCase()}-${FORM_REVISION.toLowerCase()}`
  );
  fs.mkdirSync(summaryDir, { recursive: true });

  const rows: Array<{
    candidate: string;
    family: string | null;
    extra_css: string | null;
    page: number;
    gate_changed_pixels: number | null;
    gate_changed_percent: number | null;
    poppler_changed_pixels: number;
    poppler_changed_percent: number;
    region_report: string | null;
  }> = [];

  for (const candidate of CANDIDATES) {
    await renderEnvelope(page, blankFixture);
    if (candidate.family !== null || candidate.extraCss) {
      const familyRule =
        candidate.family === null
          ? ""
          : `font-family: ${candidate.family} !important;`;
      await page.addStyleTag({
        content: `
          .form-document .form-page {
            ${familyRule}
            ${candidate.extraCss ?? ""}
          }
        `
      });
    }
    await prepareOfficialBlankComparison(page);
    await page.evaluate(async () => {
      await document.fonts.ready;
      await new Promise((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(resolve))
      );
    });

    const renderedPages = page.locator(".form-page");
    await expect(renderedPages).toHaveCount(form.pages.length);
    for (const [pageIndex, manifestPage] of form.pages.entries()) {
      const actualBuffer = await renderedPages.nth(pageIndex).screenshot({
        animations: "disabled",
        caret: "hide"
      });
      const actual = PNG.sync.read(actualBuffer);
      const poppler = PNG.sync.read(
        fs.readFileSync(path.join(REPO_ROOT, manifestPage.reference_png))
      );
      const popplerComparison = comparePixelMask(
        poppler,
        actual,
        PIXELMATCH_THRESHOLD
      );
      const pixelCount = poppler.width * poppler.height;

      let gateChangedPixels: number | null = null;
      let regionReportPath: string | null = null;
      if (manifestPage.chromium_raster) {
        const chromium = PNG.sync.read(
          fs.readFileSync(
            path.join(REPO_ROOT, manifestPage.chromium_raster.reference_png)
          )
        );
        const gateComparison = comparePixelMask(
          chromium,
          actual,
          PIXELMATCH_THRESHOLD
        );
        gateChangedPixels = gateComparison.changedPixels;
        const report = writeRegionReport({
          outputDir: testInfo.outputDir,
          artifactStem: `${candidate.slug}-page-${manifestPage.page}`,
          formCode: FORM_CODE,
          formRevision: FORM_REVISION,
          pageNumber: manifestPage.page,
          comparison: "official-complete-page-v2",
          expected: chromium,
          actual,
          diff: gateComparison.diff,
          regions: criticalRegionsFor(FORM_CODE, FORM_REVISION, manifestPage.page),
          topN: 5
        });
        regionReportPath = path.relative(REPO_ROOT, report.markdownPath);
      }

      rows.push({
        candidate: candidate.slug,
        family: candidate.family,
        extra_css: candidate.extraCss ?? null,
        page: manifestPage.page,
        gate_changed_pixels: gateChangedPixels,
        gate_changed_percent:
          gateChangedPixels === null
            ? null
            : (gateChangedPixels / pixelCount) * 100,
        poppler_changed_pixels: popplerComparison.changedPixels,
        poppler_changed_percent:
          (popplerComparison.changedPixels / pixelCount) * 100,
        region_report: regionReportPath
      });
    }
  }

  const ranking = new Map<string, number>();
  for (const row of rows) {
    const score = row.gate_changed_percent ?? row.poppler_changed_percent;
    ranking.set(row.candidate, (ranking.get(row.candidate) ?? 0) + score);
  }
  const ranked = [...ranking.entries()]
    .map(([candidate, total]) => ({
      candidate,
      mean_gate_changed_percent: total / form.pages.length
    }))
    .sort(
      (left, right) =>
        left.mean_gate_changed_percent - right.mean_gate_changed_percent
    );

  const summary = {
    purpose: "non_promotional_diagnostic",
    promotion_eligible: false,
    form_code: FORM_CODE,
    form_revision: FORM_REVISION,
    pixelmatch_threshold: PIXELMATCH_THRESHOLD,
    ranked_by_mean_gate_changed_percent: ranked,
    rows
  };
  fs.writeFileSync(
    path.join(summaryDir, "summary.json"),
    `${JSON.stringify(summary, null, 2)}\n`
  );
  const lines = [
    `# ${FORM_CODE}:${FORM_REVISION} font sweep (non-promotional diagnostic)`,
    "",
    "| rank | candidate | mean gate changed % |",
    "| ---: | --- | ---: |",
    ...ranked.map(
      (entry, index) =>
        `| ${index + 1} | ${entry.candidate} | ${entry.mean_gate_changed_percent.toFixed(4)}% |`
    ),
    ""
  ];
  fs.writeFileSync(path.join(summaryDir, "summary.md"), lines.join("\n"));
  for (const entry of ranked) {
    console.log(
      `${entry.candidate}: mean gate ${entry.mean_gate_changed_percent.toFixed(4)}%`
    );
  }
  console.log(`font sweep summary: ${path.join(summaryDir, "summary.md")}`);
});
