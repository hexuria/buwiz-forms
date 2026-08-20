// Sweep the three provisional official-fidelity-v1 constants and record their
// response curves — criterion section 8.3, one of the four preconditions that
// block the criterion from ever gating.
//
// The constants were originally inherited or chosen a priori, never swept.
// A constant with a cliff next to it is a liability: a small rendering change
// can flip a component's verdict for reasons that have nothing to do with the
// form. This measures each constant's neighbourhood so the pinned value is
// justified by a recorded curve rather than by inheritance. The 2026-07-20 run
// re-pinned INK_THRESHOLD 160 -> 150 (plateau [136, 166], cliffs localized to
// one tone) and confirmed EDGE_THRESHOLD=48 (flat) and STRUCTURAL_MIN_RUN=24
// (past the glyph-run knee); curves live in criterion section 2.6 and
// docs/form-print-readiness/data/fidelity-constant-sweeps.json.
//
// Browserless: reads the captured page rasters from the last visual run.
// Sweeps run on TWO forms (2551Q and 1601C) so a value justified on one form's
// idiosyncrasies is visible as such. Diagnostic only, never promotion evidence.

import { expect, test } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { PNG } from "pngjs";
import { dilate, f1Score, ratio, sobelEdgeMask } from "../grayscale-edge-match";
import {
  darkInkMaskP4,
  EDGE_THRESHOLD,
  INK_THRESHOLD,
  largestComponent,
  STRUCTURAL_MIN_RUN,
  structuralStratum,
  TOLERANCE_RADIUS_PX
} from "../official-fidelity";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(HERE, "../../../..");

// Expected side comes from the pinned chromium references (stable, tracked);
// actual side comes from the last visual run's captures.
const PAGE_SETS = [
  {
    form: "2551Q:2018",
    dir: "form-parity-2551Q-2018-matches-the-official-first-two-pages",
    pages: [
      {
        expected: "packages/form-renderer/references/2551q-2018-page-1-chromium.png",
        actual: "2551q-2018-page-1-actual.png"
      },
      {
        expected: "packages/form-renderer/references/2551q-2018-page-2-chromium.png",
        actual: "2551q-2018-page-2-actual.png"
      }
    ]
  },
  {
    form: "1601C:2018",
    dir: "form-1601c-parity-1601C-20-e3a00-plete-pinned-official-pages",
    pages: [
      {
        expected: "packages/form-renderer/references/1601c-2018-page-1-chromium.png",
        actual: "1601c-page-1-actual.png"
      },
      {
        expected: "packages/form-renderer/references/1601c-2018-page-2-chromium.png",
        actual: "1601c-page-2-actual.png"
      }
    ]
  }
];

const INK_COARSE = [100, 110, 120, 130, 140, 150, 160, 170, 180, 190, 200, 210, 220];
// Fine steps bracket the two cliffs the coarse sweep exposed: recall collapses
// somewhere in (130, 140) and again in (165, 170). Locate both to one tone so
// the pinned value's margin is a measured number, not an interval guess.
const INK_FINE = [
  132, 134, 135, 136, 137, 138, 139, 141, 143, 145, 147, 149,
  155, 156, 157, 158, 159, 161, 162, 163, 164, 165, 166, 167, 168, 169
];
const RUN_SWEEP = [8, 12, 16, 20, 24, 28, 32, 40, 48];
const EDGE_SWEEP = [16, 24, 32, 40, 48, 56, 64, 80, 96];

function structuralAt(
  expected: PNG,
  actual: PNG,
  inkThreshold: number,
  minRun: number
) {
  const width = expected.width;
  const height = expected.height;
  const expectedStratum = structuralStratum(
    darkInkMaskP4(expected, inkThreshold), width, height, minRun
  );
  const actualStratum = structuralStratum(
    darkInkMaskP4(actual, inkThreshold), width, height, minRun
  );
  const expectedDilated = dilate(expectedStratum, width, height, TOLERANCE_RADIUS_PX);
  const actualDilated = dilate(actualStratum, width, height, TOLERANCE_RADIUS_PX);
  const unmatched = new Uint8Array(expectedStratum.length);
  let expectedPixels = 0;
  let actualPixels = 0;
  let matchedExpected = 0;
  let matchedActual = 0;
  for (let index = 0; index < expectedStratum.length; index += 1) {
    if (expectedStratum[index] === 1) {
      expectedPixels += 1;
      if (actualDilated[index] === 1) matchedExpected += 1;
      else unmatched[index] = 1;
    }
    if (actualStratum[index] === 1) {
      actualPixels += 1;
      if (expectedDilated[index] === 1) matchedActual += 1;
    }
  }
  return {
    expected_pixels: expectedPixels,
    actual_pixels: actualPixels,
    recall: ratio(matchedExpected, expectedPixels),
    precision: ratio(matchedActual, actualPixels),
    largest_unmatched: largestComponent(unmatched, width, height)
  };
}

function edgeAt(expected: PNG, actual: PNG, threshold: number) {
  const width = expected.width;
  const height = expected.height;
  const expectedEdges = sobelEdgeMask(expected, threshold);
  const actualEdges = sobelEdgeMask(actual, threshold);
  const expectedDilated = dilate(expectedEdges, width, height, TOLERANCE_RADIUS_PX);
  const actualDilated = dilate(actualEdges, width, height, TOLERANCE_RADIUS_PX);
  let expectedCount = 0;
  let actualCount = 0;
  let matchedExpected = 0;
  let matchedActual = 0;
  for (let index = 0; index < expectedEdges.length; index += 1) {
    if (expectedEdges[index] === 1) {
      expectedCount += 1;
      if (actualDilated[index] === 1) matchedExpected += 1;
    }
    if (actualEdges[index] === 1) {
      actualCount += 1;
      if (expectedDilated[index] === 1) matchedActual += 1;
    }
  }
  const precision = ratio(matchedActual, actualCount);
  const recall = ratio(matchedExpected, expectedCount);
  return {
    expected_edges: expectedCount,
    actual_edges: actualCount,
    precision,
    recall,
    f1: f1Score(precision, recall)
  };
}

test("sweep the provisional fidelity constants", () => {
  const report: Record<string, unknown> = {
    purpose: "criterion_section_8_3_constant_sweeps",
    promotion_eligible: false,
    tolerance_radius_px: TOLERANCE_RADIUS_PX,
    // The pinned values each curve pivots around; a reviewer needs these to
    // read the curves as neighbourhoods of the pin rather than free axes.
    pinned: {
      edge_threshold: EDGE_THRESHOLD,
      ink_threshold: INK_THRESHOLD,
      structural_min_run: STRUCTURAL_MIN_RUN
    },
    forms: {} as Record<string, unknown>
  };

  for (const set of PAGE_SETS) {
    const pages = set.pages.map((entry) => {
      const dir = path.join(REPO_ROOT, "test-results/form-renderer", set.dir);
      return {
        expected: PNG.sync.read(fs.readFileSync(path.join(REPO_ROOT, entry.expected))),
        actual: PNG.sync.read(fs.readFileSync(path.join(dir, entry.actual)))
      };
    });

    const inkCurve = [...INK_COARSE, ...INK_FINE]
      .sort((a, b) => a - b)
      .map((threshold) => ({
        ink_threshold: threshold,
        pages: pages.map((page) =>
          structuralAt(page.expected, page.actual, threshold, STRUCTURAL_MIN_RUN)
        )
      }));

    const runCurve = RUN_SWEEP.map((minRun) => ({
      min_run: minRun,
      pages: pages.map((page) =>
        structuralAt(page.expected, page.actual, INK_THRESHOLD, minRun)
      )
    }));

    const edgeCurve = EDGE_SWEEP.map((threshold) => ({
      edge_threshold: threshold,
      pages: pages.map((page) => edgeAt(page.expected, page.actual, threshold))
    }));

    (report.forms as Record<string, unknown>)[set.form] = {
      ink_threshold_curve: inkCurve,
      structural_min_run_curve: runCurve,
      edge_threshold_curve: edgeCurve
    };

    console.log(`\n=== ${set.form} ===`);
    console.log("INK_THRESHOLD -> p1 recall / precision / largest-unmatched");
    for (const point of inkCurve) {
      const p = point.pages[0];
      console.log(
        `  ${String(point.ink_threshold).padStart(3)}: ${p.recall.toFixed(6)} / ` +
          `${p.precision.toFixed(6)} / ${p.largest_unmatched}`
      );
    }
    console.log("STRUCTURAL_MIN_RUN -> p1 stratum px / recall");
    for (const point of runCurve) {
      const p = point.pages[0];
      console.log(
        `  ${String(point.min_run).padStart(3)}: ${p.expected_pixels} / ${p.recall.toFixed(6)}`
      );
    }
    console.log("EDGE_THRESHOLD -> p1 edges / f1");
    for (const point of edgeCurve) {
      const p = point.pages[0];
      console.log(
        `  ${String(point.edge_threshold).padStart(3)}: ${p.expected_edges} / ${p.f1.toFixed(6)}`
      );
    }
  }

  const outPath = path.join(
    REPO_ROOT,
    "test-results/form-renderer/fidelity-constant-sweeps.json"
  );
  fs.writeFileSync(outPath, `${JSON.stringify(report, null, 2)}\n`);
  console.log(`\nwrote ${path.relative(REPO_ROOT, outPath)}`);
  expect(Object.keys(report.forms as object).length).toBe(PAGE_SETS.length);
});
