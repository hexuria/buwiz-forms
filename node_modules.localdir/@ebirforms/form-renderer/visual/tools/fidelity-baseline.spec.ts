// Emit official-fidelity-v1 component values from already-captured page
// rasters, without a browser.
//
// Two jobs, both required by the criterion:
//  1. Produce the exact integers the pure-Python audit reimplementation must
//     reproduce, so TypeScript/Python divergence is caught by a test rather
//     than discovered during a promotion attempt.
//  2. Produce the numbers a reviewer pins as the baseline in the Rust
//     providers (criterion section 9).
//
// Reads the captured rasters from the last visual run. Output is a diagnostic
// artifact, never promotion evidence.

import { expect, test } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { PNG } from "pngjs";
import {
  computeCellEdgeF1,
  computePageInkBudget,
  computeStructuralInk
} from "../official-fidelity";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(HERE, "../../../..");

const FORM_CODE = process.env.FIDELITY_FORM ?? "2551Q";
const FORM_REVISION = process.env.FIDELITY_REVISION ?? "2018";
const CAPTURE_DIR =
  process.env.FIDELITY_CAPTURE_DIR ??
  path.join(
    REPO_ROOT,
    "test-results/form-renderer/form-parity-2551Q-2018-matches-the-official-first-two-pages"
  );
const OUTPUT_PATH =
  process.env.FIDELITY_OUTPUT ??
  path.join(REPO_ROOT, "test-results/form-renderer/fidelity-baseline.json");

test(`emit official-fidelity-v1 values for ${FORM_CODE}:${FORM_REVISION}`, () => {
  const slug = `${FORM_CODE.toLowerCase()}-${FORM_REVISION.toLowerCase()}`;
  const pages: Record<string, unknown> = {};

  for (let page = 1; ; page += 1) {
    const expectedPath = path.join(CAPTURE_DIR, `${slug}-page-${page}-expected.png`);
    const actualPath = path.join(CAPTURE_DIR, `${slug}-page-${page}-actual.png`);
    if (!fs.existsSync(expectedPath) || !fs.existsSync(actualPath)) break;

    const expected = PNG.sync.read(fs.readFileSync(expectedPath));
    const actual = PNG.sync.read(fs.readFileSync(actualPath));
    const cell = computeCellEdgeF1(expected, actual, FORM_CODE, FORM_REVISION, page);
    const structural = computeStructuralInk(expected, actual);
    const ink = computePageInkBudget(expected, actual);

    pages[String(page)] = {
      cell_edge_f1: {
        cell_table_sha256: cell.cellTableSha256,
        cell_count: cell.cells.length,
        scored_cell_count: cell.scoredCellCount,
        worst_scored_f1: cell.worstScoredF1,
        edge_coverage: cell.edgeCoverage,
        cells: cell.cells.map((entry) => ({
          id: entry.id,
          kind: entry.kind,
          x: entry.x,
          y: entry.y,
          width: entry.width,
          height: entry.height,
          expected_edge_pixels: entry.expectedEdgePixels,
          actual_edge_pixels: entry.actualEdgePixels,
          matched_expected_pixels: entry.matchedExpectedPixels,
          matched_actual_pixels: entry.matchedActualPixels,
          precision: entry.precision,
          recall: entry.recall,
          f1: entry.f1,
          scored: entry.scored
        }))
      },
      structural_ink_coverage: structural,
      page_ink_budget: ink
    };

    console.log(
      `page ${page}: cells=${cell.cells.length} scored=${cell.scoredCellCount} ` +
        `worstF1=${cell.worstScoredF1.toFixed(6)} coverage=${cell.edgeCoverage.toFixed(6)} ` +
        `cellTable=${cell.cellTableSha256.slice(0, 12)}`
    );
    console.log(
      `  structural recall=${structural.structuralRecall.toFixed(6)} ` +
        `precision=${structural.structuralPrecision.toFixed(6)} ` +
        `largestUnmatched=${structural.largestUnmatchedClusterPx}px`
    );
    console.log(
      `  ink expected=${ink.expectedInk} actual=${ink.actualInk} ` +
        `missing=${ink.inkMissing} unexpected=${ink.inkUnexpected} paper=${ink.paperPixels}`
    );
  }

  expect(
    Object.keys(pages).length,
    `no captured rasters found in ${CAPTURE_DIR}; run the visual suite first`
  ).toBeGreaterThan(0);

  const report = {
    purpose: "non_promotional_criterion_cross_check_and_baseline_source",
    promotion_eligible: false,
    criterion: "official-fidelity-v1",
    form_code: FORM_CODE,
    form_revision: FORM_REVISION,
    capture_dir: path.relative(REPO_ROOT, CAPTURE_DIR),
    pages
  };
  fs.mkdirSync(path.dirname(OUTPUT_PATH), { recursive: true });
  fs.writeFileSync(OUTPUT_PATH, `${JSON.stringify(report, null, 2)}\n`);
  console.log(`wrote ${path.relative(REPO_ROOT, OUTPUT_PATH)}`);
});
