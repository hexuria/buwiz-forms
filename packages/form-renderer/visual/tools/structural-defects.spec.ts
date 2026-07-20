// Localize structural defects precisely enough to fix them.
//
// `structural-ink-coverage-v1` reports that page 1 has a 1130px unmatched
// cluster. That says a defect exists; it does not say which CSS rule to
// change. This tool closes that gap: it isolates each unmatched structural
// cluster, reports its bounding box, and searches for the offset at which the
// rendered structure would have matched - so a defect becomes "this rule is 2
// device px low" rather than "something is wrong somewhere".
//
// Only the gate's own fixture-value blanking is applied. The structural
// stratum's 24px minimum run already excludes body text, and display strokes
// appear on both sides and match, so no extra text handling is needed - and
// adding it on one side only manufactures phantom findings. Output is a
// diagnostic, never promotion evidence.

import { expect, test } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { PNG } from "pngjs";
import { dilate } from "../grayscale-edge-match";
import {
  darkInkMaskP4,
  structuralStratum,
  TOLERANCE_RADIUS_PX
} from "../official-fidelity";
import {
  blankComparisonEnvelope,
  prepareOfficialBlankComparison,
  renderEnvelope
} from "../support/render-utils";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(HERE, "../../../..");

const FIXTURE = "packages/form-contracts/fixtures/2551q-6-rows.json";
const REFERENCES = [
  { page: 1, chromium: "packages/form-renderer/references/2551q-2018-page-1-chromium.png" },
  { page: 2, chromium: "packages/form-renderer/references/2551q-2018-page-2-chromium.png" }
];

/** Offsets searched when asking "where would this have matched?". */
const SEARCH_RADIUS = 6;
/** Clusters smaller than this are anti-alias noise, not defects worth naming. */
const MIN_REPORTABLE_CLUSTER = 40;
const TOP_CLUSTERS = 12;

interface Cluster {
  pixels: number[];
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

function components(mask: Uint8Array, width: number, height: number): Cluster[] {
  const visited = new Uint8Array(mask.length);
  const clusters: Cluster[] = [];
  for (let seed = 0; seed < mask.length; seed += 1) {
    if (mask[seed] !== 1 || visited[seed] === 1) continue;
    visited[seed] = 1;
    const stack = [seed];
    const pixels: number[] = [];
    let x0 = width;
    let y0 = height;
    let x1 = 0;
    let y1 = 0;
    while (stack.length > 0) {
      const index = stack.pop() as number;
      pixels.push(index);
      const x = index % width;
      const y = (index - x) / width;
      if (x < x0) x0 = x;
      if (y < y0) y0 = y;
      if (x > x1) x1 = x;
      if (y > y1) y1 = y;
      for (let dy = -1; dy <= 1; dy += 1) {
        for (let dx = -1; dx <= 1; dx += 1) {
          if (dx === 0 && dy === 0) continue;
          const nx = x + dx;
          const ny = y + dy;
          if (nx < 0 || ny < 0 || nx >= width || ny >= height) continue;
          const neighbour = ny * width + nx;
          if (mask[neighbour] !== 1 || visited[neighbour] === 1) continue;
          visited[neighbour] = 1;
          stack.push(neighbour);
        }
      }
    }
    clusters.push({ pixels, x0, y0, x1, y1 });
  }
  return clusters;
}

/**
 * Stroke thickness at a cluster, measured perpendicular to its run.
 *
 * WHY THIS EXISTS. Offset search alone misdescribes weight defects. Measured
 * on 2551Q: three full-width rules reported "dy=+2, recovers 100%", which is
 * true and misleading - the official stroke is 3 device px and ours was 2, so
 * shifting ours down DOES land it on official ink while the real fault is that
 * it never covers the official's top row. Acting on the offset would have
 * moved correct rules; the actual fix was 1pt to 1.5pt. Reporting both
 * thicknesses makes the two cases distinguishable at a glance.
 */
function strokeThickness(
  mask: Uint8Array,
  width: number,
  height: number,
  cluster: Cluster,
  horizontal: boolean
): number {
  const midX = Math.floor((cluster.x0 + cluster.x1) / 2);
  const midY = Math.floor((cluster.y0 + cluster.y1) / 2);
  let thickness = 0;
  if (horizontal) {
    for (let y = midY; y >= 0 && mask[y * width + midX] === 1; y -= 1) thickness += 1;
    for (let y = midY + 1; y < height && mask[y * width + midX] === 1; y += 1) thickness += 1;
  } else {
    for (let x = midX; x >= 0 && mask[midY * width + x] === 1; x -= 1) thickness += 1;
    for (let x = midX + 1; x < width && mask[midY * width + x] === 1; x += 1) thickness += 1;
  }
  return thickness;
}

/**
 * The offset at which this cluster's expected structure would have been
 * matched by rendered structure. This is what turns a defect into an
 * actionable instruction.
 */
function bestOffset(
  cluster: Cluster,
  actualStructural: Uint8Array,
  width: number,
  height: number
): { dx: number; dy: number; matched: number } {
  let best = { dx: 0, dy: 0, matched: 0 };
  for (let dy = -SEARCH_RADIUS; dy <= SEARCH_RADIUS; dy += 1) {
    for (let dx = -SEARCH_RADIUS; dx <= SEARCH_RADIUS; dx += 1) {
      let matched = 0;
      for (const index of cluster.pixels) {
        const x = (index % width) + dx;
        const y = ((index - (index % width)) / width) + dy;
        if (x < 0 || y < 0 || x >= width || y >= height) continue;
        if (actualStructural[y * width + x] === 1) matched += 1;
      }
      const better =
        matched > best.matched ||
        (matched === best.matched &&
          Math.abs(dx) + Math.abs(dy) < Math.abs(best.dx) + Math.abs(best.dy));
      if (better) best = { dx, dy, matched };
    }
  }
  return best;
}

test("localize 2551Q structural defects", async ({ page }, testInfo) => {
  const fixture = JSON.parse(
    fs.readFileSync(path.join(REPO_ROOT, FIXTURE), "utf8")
  ) as unknown;
  const blanked = blankComparisonEnvelope(fixture, "2551Q");

  await renderEnvelope(page, blanked);
  // Only the gate's own fixture blanking is applied. Neutralizing ALL text on
  // our side would be an ASYMMETRY, not a simplification: the reference keeps
  // its glyphs (as filled paths), so large display strokes - the "2551Q"
  // wordmark, page numbers - would be reported as missing structure. An
  // earlier version of this tool did exactly that and produced six
  // 0%-recovery phantom clusters at identical coordinates on both pages.
  // The structural stratum's 24px minimum run already excludes body text, and
  // display strokes appear on both sides and match.
  await prepareOfficialBlankComparison(page);
  await page.evaluate(async () => {
    await document.fonts.ready;
    await new Promise((resolve) =>
      requestAnimationFrame(() => requestAnimationFrame(resolve))
    );
  });

  const pages = page.locator(".form-page");
  await expect(pages).toHaveCount(REFERENCES.length);

  const report: Array<Record<string, unknown>> = [];

  for (const [index, entry] of REFERENCES.entries()) {
    const reference = PNG.sync.read(
      fs.readFileSync(path.join(REPO_ROOT, entry.chromium))
    );
    const shot = await pages.nth(index).screenshot({
      animations: "disabled",
      caret: "hide"
    });
    const actual = PNG.sync.read(shot);
    const width = reference.width;
    const height = reference.height;

    const expectedStructural = structuralStratum(darkInkMaskP4(reference), width, height);
    const actualStructural = structuralStratum(darkInkMaskP4(actual), width, height);
    const actualDilated = dilate(actualStructural, width, height, TOLERANCE_RADIUS_PX);

    const unmatched = new Uint8Array(expectedStructural.length);
    let expectedTotal = 0;
    for (let i = 0; i < expectedStructural.length; i += 1) {
      if (expectedStructural[i] !== 1) continue;
      expectedTotal += 1;
      if (actualDilated[i] !== 1) unmatched[i] = 1;
    }

    const clusters = components(unmatched, width, height)
      .filter((cluster) => cluster.pixels.length >= MIN_REPORTABLE_CLUSTER)
      .sort((a, b) => b.pixels.length - a.pixels.length)
      .slice(0, TOP_CLUSTERS);

    console.log(
      `\npage ${entry.page}: ${expectedTotal} expected structural px, ` +
        `${clusters.length} reportable unmatched clusters`
    );
    const pageClusters = clusters.map((cluster) => {
      const offset = bestOffset(cluster, actualStructural, width, height);
      const horizontal = cluster.y1 - cluster.y0 <= 3;
      const vertical = cluster.x1 - cluster.x0 <= 3;
      const shape = horizontal
        ? "horizontal rule"
        : vertical
          ? "vertical rule"
          : "box or fill";
      const recovered = offset.matched / cluster.pixels.length;

      // Distinguish a weight deficit from a displacement. A thinner stroke of
      // ours that still overlaps the official one reports a confident offset,
      // because shifting it does land on ink - but moving it would be wrong.
      const expectedThickness = horizontal || vertical
        ? strokeThickness(expectedStructural, width, height, cluster, horizontal)
        : 0;
      const actualThickness = horizontal || vertical
        ? strokeThickness(actualStructural, width, height, cluster, horizontal)
        : 0;
      const weightDeficit =
        (horizontal || vertical) &&
        actualThickness > 0 &&
        expectedThickness > actualThickness;
      const diagnosis = weightDeficit
        ? `WEIGHT: official ${expectedThickness}px vs ours ${actualThickness}px`
        : recovered >= 0.9
          ? `DISPLACED: dx=${offset.dx} dy=${offset.dy}`
          : `PARTIAL (${(recovered * 100).toFixed(0)}%): likely missing or extra structure`;

      console.log(
        `  ${String(cluster.pixels.length).padStart(5)}px ${shape.padEnd(15)} ` +
          `x=${cluster.x0}..${cluster.x1} y=${cluster.y0}..${cluster.y1}  ${diagnosis}`
      );
      return {
        pixels: cluster.pixels.length,
        shape,
        x0: cluster.x0,
        y0: cluster.y0,
        x1: cluster.x1,
        y1: cluster.y1,
        best_dx: offset.dx,
        best_dy: offset.dy,
        recovered_fraction: recovered,
        expected_thickness_px: expectedThickness,
        actual_thickness_px: actualThickness,
        diagnosis: weightDeficit
          ? "weight-deficit"
          : recovered >= 0.9
            ? "displaced"
            : "partial-missing-or-extra"
      };
    });
    report.push({ page: entry.page, expected_structural_px: expectedTotal, clusters: pageClusters });
  }

  const reportPath = path.join(testInfo.outputDir, "structural-defects.json");
  fs.mkdirSync(testInfo.outputDir, { recursive: true });
  fs.writeFileSync(
    reportPath,
    `${JSON.stringify(
      {
        purpose: "non_promotional_structural_defect_localization",
        promotion_eligible: false,
        fixture_values_blanked: true,
        tolerance_radius_px: TOLERANCE_RADIUS_PX,
        pages: report
      },
      null,
      2
    )}\n`
  );
  console.log(`\nwrote ${path.relative(REPO_ROOT, reportPath)}`);
});
