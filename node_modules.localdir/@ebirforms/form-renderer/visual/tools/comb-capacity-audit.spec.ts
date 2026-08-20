// Audit every comb field's declared capacity against the official form.
//
// WHY THIS EXISTS. A comb field's cell count is a promise to the taxpayer
// about how many characters the form accepts. Item 12A was found declaring 26
// where the official has 16 - a 62% overstatement - and it was found by
// zooming one region by hand after the cell component flagged it. That does
// not scale to 35 forms, and a capacity error is invisible to every pixel
// metric we have: the guides are thin, so a wrong count moves the complete-page
// number by a fraction of a percent while being plainly wrong on paper.
//
// This measures instead. For each rendered comb field it counts our spans (the
// declared capacity) and counts the official form's interior dividers inside
// the same rectangle, then reports every mismatch.
//
// Diagnostic only, never promotion evidence. Its output is a review list: a
// mismatch is a question for a human against the pinned PDF, not an
// instruction to change a number until it matches.

import { expect, test } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { PNG } from "pngjs";
import {
  blankComparisonEnvelope,
  prepareOfficialBlankComparison,
  renderEnvelope
} from "../support/render-utils";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(HERE, "../../../..");
const DEVICE_SCALE_FACTOR = 1.5;

const FORM_CODE = process.env.COMB_AUDIT_FORM ?? "2551Q";
const FORM_REVISION = process.env.COMB_AUDIT_REVISION ?? "2018";
const FIXTURE =
  process.env.COMB_AUDIT_FIXTURE ?? "packages/form-contracts/fixtures/2551q-6-rows.json";
const REFERENCES = (
  process.env.COMB_AUDIT_REFERENCES ??
  "packages/form-renderer/references/2551q-2018-page-1-chromium.png," +
    "packages/form-renderer/references/2551q-2018-page-2-chromium.png"
).split(",");

/** A divider column must carry at least this fraction of the field's height. */
const MIN_DIVIDER_COVERAGE = 0.25;
/** Guides are often mid-grey; the ink threshold must not assume near-black. */
const DIVIDER_TONE_MAX = 200;

interface CombObservation {
  page: number;
  selector: string;
  spanCount: number;
  x: number;
  y: number;
  width: number;
  height: number;
}

/**
 * Count interior dividers of the official comb inside a rectangle. The field's
 * own left and right borders are excluded, so N dividers means N+1 cells.
 */
function countOfficialDividers(
  image: PNG,
  rect: { x: number; y: number; width: number; height: number }
): { dividers: number; pitch: number; tones: number[] } {
  const x0 = Math.max(0, Math.round(rect.x));
  const x1 = Math.min(image.width - 1, Math.round(rect.x + rect.width));
  const y0 = Math.max(0, Math.round(rect.y) + 3);
  const y1 = Math.min(image.height - 1, Math.round(rect.y + rect.height) - 3);
  if (x1 - x0 < 8 || y1 - y0 < 4) return { dividers: 0, pitch: 0, tones: [] };

  const needed = Math.max(3, Math.floor((y1 - y0) * MIN_DIVIDER_COVERAGE));
  const inked: number[] = [];
  const tones: number[] = [];
  for (let x = x0; x <= x1; x += 1) {
    let count = 0;
    let darkest = 255;
    for (let y = y0; y <= y1; y += 1) {
      const offset = (y * image.width + x) * 4;
      const value = image.data[offset];
      if (value < DIVIDER_TONE_MAX) {
        count += 1;
        if (value < darkest) darkest = value;
      }
    }
    if (count >= needed) {
      inked.push(x);
      tones.push(darkest);
    }
  }

  // Group adjacent columns into single dividers.
  const centres: number[] = [];
  let group: number[] = [];
  for (const x of inked) {
    if (group.length === 0 || x - group[group.length - 1] <= 2) group.push(x);
    else {
      centres.push(Math.round(group.reduce((a, b) => a + b, 0) / group.length));
      group = [x];
    }
  }
  if (group.length > 0) {
    centres.push(Math.round(group.reduce((a, b) => a + b, 0) / group.length));
  }

  // Drop the field's own edges: a divider within 3px of either border is the
  // border itself, not an interior cell boundary.
  const interior = centres.filter((c) => c - x0 > 3 && x1 - c > 3);
  const gaps = interior.slice(1).map((c, index) => c - interior[index]);
  const pitch = gaps.length
    ? gaps.slice().sort((a, b) => a - b)[Math.floor(gaps.length / 2)]
    : 0;
  return { dividers: interior.length, pitch, tones };
}

test(`audit ${FORM_CODE}:${FORM_REVISION} comb capacities against the official form`, async ({
  page
}, testInfo) => {
  const fixture = JSON.parse(
    fs.readFileSync(path.join(REPO_ROOT, FIXTURE), "utf8")
  ) as unknown;
  await renderEnvelope(page, blankComparisonEnvelope(fixture, FORM_CODE));
  await prepareOfficialBlankComparison(page);

  const pages = page.locator(".form-page");
  await expect(pages).toHaveCount(REFERENCES.length);

  const observations: CombObservation[] = [];
  for (let index = 0; index < REFERENCES.length; index += 1) {
    const formPage = pages.nth(index);
    const pageBox = await formPage.boundingBox();
    if (!pageBox) continue;
    const combs = await formPage.locator(".comb-value").all();
    for (const comb of combs) {
      const box = await comb.boundingBox();
      if (!box) continue;
      const spanCount = await comb.locator("> span").count();
      if (spanCount === 0) continue;
      const selector = await comb.evaluate((element) => {
        const parent = element.parentElement;
        const parentClass = parent?.className?.toString().split(" ")[0] ?? "?";
        const siblings = parent ? [...parent.children].indexOf(element) + 1 : 0;
        return `.${parentClass} > .comb-value:nth-child(${siblings})`;
      });
      observations.push({
        page: index + 1,
        selector,
        spanCount,
        x: (box.x - pageBox.x) * DEVICE_SCALE_FACTOR,
        y: (box.y - pageBox.y) * DEVICE_SCALE_FACTOR,
        width: box.width * DEVICE_SCALE_FACTOR,
        height: box.height * DEVICE_SCALE_FACTOR
      });
    }
  }

  const rows: Array<Record<string, unknown>> = [];
  let mismatches = 0;
  for (const [index, reference] of REFERENCES.entries()) {
    const image = PNG.sync.read(fs.readFileSync(path.join(REPO_ROOT, reference)));
    const pageNumber = index + 1;
    console.log(`\npage ${pageNumber}:`);
    for (const observation of observations.filter((o) => o.page === pageNumber)) {
      const measured = countOfficialDividers(image, observation);
      const officialCells = measured.dividers + 1;
      const matches = officialCells === observation.spanCount;
      const delta = officialCells - observation.spanCount;
      // Classify, because an undifferentiated list wastes review time. A
      // measured pitch far below a character cell means the scan locked onto
      // glyph strokes or borders rather than guides, so the count is not
      // evidence of anything. An off-by-one at a plausible pitch is usually
      // ambiguity about whether the field's own border was inside the
      // rectangle - real, but a question rather than a defect. Only a larger
      // difference at a plausible pitch resembles the one confirmed capacity
      // defect found so far (2551Q Item 12A: 26 declared against 16 official).
      const severity =
        measured.pitch === 0 || (measured.pitch > 0 && measured.pitch < 20)
          ? "low-confidence"
          : Math.abs(delta) === 1
            ? "boundary"
            : "capacity";
      if (!matches) mismatches += 1;
      const tone = measured.tones.length
        ? Math.round(measured.tones.reduce((a, b) => a + b, 0) / measured.tones.length)
        : null;
      console.log(
        `  ${matches ? "ok  " : severity === "capacity" ? "CAP " : severity === "boundary" ? "b1  " : "lowc"} ` +
          `declared=${String(observation.spanCount).padStart(3)} ` +
          `official=${String(officialCells).padStart(3)} ` +
          `pitch=${String(measured.pitch).padStart(3)}px tone=${String(tone ?? "-").padStart(3)} ` +
          `${observation.selector}`
      );
      rows.push({
        page: pageNumber,
        selector: observation.selector,
        declared_cells: observation.spanCount,
        official_cells: officialCells,
        official_dividers: measured.dividers,
        official_pitch_px: measured.pitch,
        official_mean_tone: tone,
        delta_official_minus_declared: delta,
        severity: matches ? "ok" : severity,
        rect: {
          x: Math.round(observation.x),
          y: Math.round(observation.y),
          width: Math.round(observation.width),
          height: Math.round(observation.height)
        },
        matches
      });
    }
  }

  const bySeverity = (name: string) =>
    rows.filter((row) => row.severity === name).length;
  console.log(
    `\n${rows.length} comb fields measured, ${mismatches} disagree: ` +
      `${bySeverity("capacity")} capacity, ${bySeverity("boundary")} off-by-one, ` +
      `${bySeverity("low-confidence")} low-confidence`
  );
  const reportPath = path.join(testInfo.outputDir, "comb-capacity-audit.json");
  fs.mkdirSync(testInfo.outputDir, { recursive: true });
  fs.writeFileSync(
    reportPath,
    `${JSON.stringify(
      {
        purpose: "non_promotional_comb_capacity_review_list",
        promotion_eligible: false,
        note:
          "A mismatch is a question for a reviewer against the pinned PDF, not " +
          "an instruction to change a declared capacity until it matches.",
        form_code: FORM_CODE,
        form_revision: FORM_REVISION,
        measured_fields: rows.length,
        mismatched_fields: mismatches,
        fields: rows
      },
      null,
      2
    )}\n`
  );
  console.log(`wrote ${path.relative(REPO_ROOT, reportPath)}`);
});
