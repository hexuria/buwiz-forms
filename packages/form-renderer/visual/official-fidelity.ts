// The three PIXEL components of `official-fidelity-v1`.
//
// WHAT THIS IS. A NON-REGRESSION criterion pinned to a reviewed baseline. It
// certifies "no worse than a reviewed state". It CANNOT certify "matches the
// official form", and no report derived from it may claim parity.
//
// WHY. The official PDFs do not embed their fonts (pdffonts: emb=no on every
// one of the 35 source forms), so the pinned references encode Poppler's
// SUBSTITUTED glyph outlines rather than BIR's typography. Glyph outline shape
// is ~57% of the text residual and no rendering-side change fixes it. Text
// correctness is therefore proven by content assertions, never by these
// pixels; these components measure STRUCTURE, where the reference is
// trustworthy and the error is fixable displacement.
//
// WHAT THESE COMPONENTS CANNOT SEE. Measured, not assumed: replacing every
// statutory tax rate on 2551Q page 2 with a wrong value produced a maximum
// per-cell regression of 0.19e-4 and passed every existing assertion. These
// components have ZERO content-correctness signal. They must never be run
// without their companion assertions (static-text-exhaustive-v1,
// encoded-artwork-integrity-v1); the audit fails closed if any is absent.
//
// See docs/form-print-readiness/official-fidelity-criterion-v1.md.

import { PNG } from "pngjs";
import {
  dilate,
  f1Score,
  ratio,
  sobelEdgeMask
} from "./grayscale-edge-match";
import {
  buildFidelityCells,
  cellTableSha256,
  MIN_CELL_EDGE_PIXELS,
  MIN_EDGE_COVERAGE,
  type FidelityCell
} from "./fidelity-cells";

/** Pinned tolerance radius. Radius 2 scores a whole-page 1px misregistration
 *  as exactly 1.000000 and a fake-bold render as an improvement; see
 *  criterion section 2.1. Do not change without a fresh sweep. */
export const TOLERANCE_RADIUS_PX = 1;
export const EDGE_THRESHOLD = 48;
export const INK_THRESHOLD = 160;
export const STRUCTURAL_MIN_RUN = 24;
export const INK_TOLERANCE_RADIUS_PX = 2;

export interface CellScore {
  id: string;
  kind: "named" | "grid";
  x: number;
  y: number;
  width: number;
  height: number;
  expectedEdgePixels: number;
  actualEdgePixels: number;
  matchedExpectedPixels: number;
  matchedActualPixels: number;
  precision: number;
  recall: number;
  f1: number;
  scored: boolean;
}

export interface CellEdgeF1Result {
  comparison: "cell-edge-f1-v1";
  toleranceRadiusPx: number;
  edgeThreshold: number;
  minCellEdgePixels: number;
  cellTableSha256: string;
  cells: CellScore[];
  scoredCellCount: number;
  worstScoredF1: number;
  edgeCoverage: number;
  minEdgeCoverage: number;
}

export interface StructuralInkResult {
  comparison: "structural-ink-coverage-v1";
  inkThreshold: number;
  structuralMinRun: number;
  expectedStructuralPixels: number;
  actualStructuralPixels: number;
  structuralRecall: number;
  structuralPrecision: number;
  largestUnmatchedClusterPx: number;
  unmatchedExpectedPixels: number;
}

export interface PageInkBudgetResult {
  comparison: "page-ink-budget-v1";
  inkThreshold: number;
  inkToleranceRadiusPx: number;
  expectedInk: number;
  actualInk: number;
  inkMissing: number;
  inkUnexpected: number;
  inkMissingRatio: number;
  inkUnexpectedRatio: number;
  paperPixels: number;
}

/** Criterion primitive P4. Raw channel comparison; alpha deliberately ignored,
 *  matching the existing darkInkMask so the two cannot drift. */
export function darkInkMaskP4(image: PNG, threshold = INK_THRESHOLD): Uint8Array {
  const mask = new Uint8Array(image.width * image.height);
  for (let index = 0; index < mask.length; index += 1) {
    const offset = index * 4;
    if (
      image.data[offset] < threshold &&
      image.data[offset + 1] < threshold &&
      image.data[offset + 2] < threshold
    ) {
      mask[index] = 1;
    }
  }
  return mask;
}

/** Criterion primitive P6: maximal runs of ink >= minRun along a row or
 *  column contribute all their pixels to the structural stratum. */
export function structuralStratum(
  ink: Uint8Array,
  width: number,
  height: number,
  minRun = STRUCTURAL_MIN_RUN
): Uint8Array {
  const stratum = new Uint8Array(ink.length);

  for (let y = 0; y < height; y += 1) {
    let runStart = -1;
    for (let x = 0; x <= width; x += 1) {
      const isInk = x < width && ink[y * width + x] === 1;
      if (isInk && runStart < 0) runStart = x;
      if (!isInk && runStart >= 0) {
        if (x - runStart >= minRun) {
          for (let fill = runStart; fill < x; fill += 1) stratum[y * width + fill] = 1;
        }
        runStart = -1;
      }
    }
  }

  for (let x = 0; x < width; x += 1) {
    let runStart = -1;
    for (let y = 0; y <= height; y += 1) {
      const isInk = y < height && ink[y * width + x] === 1;
      if (isInk && runStart < 0) runStart = y;
      if (!isInk && runStart >= 0) {
        if (y - runStart >= minRun) {
          for (let fill = runStart; fill < y; fill += 1) stratum[fill * width + x] = 1;
        }
        runStart = -1;
      }
    }
  }

  return stratum;
}

/** Criterion primitive P7: largest 8-connected component, iterative
 *  explicit-stack flood fill, seeds visited in raster order. */
export function largestComponent(
  mask: Uint8Array,
  width: number,
  height: number
): number {
  const visited = new Uint8Array(mask.length);
  const stack: number[] = [];
  let largest = 0;

  for (let seed = 0; seed < mask.length; seed += 1) {
    if (mask[seed] !== 1 || visited[seed] === 1) continue;
    visited[seed] = 1;
    stack.length = 0;
    stack.push(seed);
    let size = 0;
    while (stack.length > 0) {
      const index = stack.pop() as number;
      size += 1;
      const x = index % width;
      const y = (index - x) / width;
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
    if (size > largest) largest = size;
  }
  return largest;
}

/**
 * `cell-edge-f1-v1`. Dilation is computed page-globally ONCE and then counted
 * within each cell. This is mandatory: per-cell dilation would fabricate false
 * mismatches at every cell border, and with two overlapping grids that would
 * swamp the signal.
 */
export function computeCellEdgeF1(
  expected: PNG,
  actual: PNG,
  formCode: string,
  formRevision: string,
  pageNumber: number
): CellEdgeF1Result {
  assertSameGeometry(expected, actual);
  const width = expected.width;
  const height = expected.height;

  const expectedEdges = sobelEdgeMask(expected, EDGE_THRESHOLD);
  const actualEdges = sobelEdgeMask(actual, EDGE_THRESHOLD);
  const expectedDilated = dilate(expectedEdges, width, height, TOLERANCE_RADIUS_PX);
  const actualDilated = dilate(actualEdges, width, height, TOLERANCE_RADIUS_PX);

  const cells = buildFidelityCells(formCode, formRevision, pageNumber, width, height);
  const covered = new Uint8Array(width * height);
  const scores: CellScore[] = [];

  for (const cell of cells) {
    let expectedEdgePixels = 0;
    let actualEdgePixels = 0;
    let matchedExpectedPixels = 0;
    let matchedActualPixels = 0;
    const x1 = cell.x + cell.width;
    const y1 = cell.y + cell.height;
    for (let y = cell.y; y < y1; y += 1) {
      for (let x = cell.x; x < x1; x += 1) {
        const index = y * width + x;
        if (expectedEdges[index] === 1) {
          expectedEdgePixels += 1;
          if (actualDilated[index] === 1) matchedExpectedPixels += 1;
        }
        if (actualEdges[index] === 1) {
          actualEdgePixels += 1;
          if (expectedDilated[index] === 1) matchedActualPixels += 1;
        }
      }
    }
    const precision = ratio(matchedActualPixels, actualEdgePixels);
    const recall = ratio(matchedExpectedPixels, expectedEdgePixels);
    const scored = expectedEdgePixels >= MIN_CELL_EDGE_PIXELS;
    if (scored) {
      for (let y = cell.y; y < y1; y += 1) {
        for (let x = cell.x; x < x1; x += 1) covered[y * width + x] = 1;
      }
    }
    scores.push({
      id: cell.id,
      kind: cell.kind,
      x: cell.x,
      y: cell.y,
      width: cell.width,
      height: cell.height,
      expectedEdgePixels,
      actualEdgePixels,
      matchedExpectedPixels,
      matchedActualPixels,
      precision,
      recall,
      f1: f1Score(precision, recall),
      scored
    });
  }

  let expectedEdgeTotal = 0;
  let expectedEdgeCovered = 0;
  for (let index = 0; index < expectedEdges.length; index += 1) {
    if (expectedEdges[index] !== 1) continue;
    expectedEdgeTotal += 1;
    if (covered[index] === 1) expectedEdgeCovered += 1;
  }

  const scoredCells = scores.filter((cell) => cell.scored);
  return {
    comparison: "cell-edge-f1-v1",
    toleranceRadiusPx: TOLERANCE_RADIUS_PX,
    edgeThreshold: EDGE_THRESHOLD,
    minCellEdgePixels: MIN_CELL_EDGE_PIXELS,
    cellTableSha256: cellTableSha256(cells),
    cells: scores,
    scoredCellCount: scoredCells.length,
    worstScoredF1: scoredCells.reduce(
      (worst, cell) => Math.min(worst, cell.f1),
      scoredCells.length > 0 ? 1 : 0
    ),
    edgeCoverage: ratio(expectedEdgeCovered, expectedEdgeTotal),
    minEdgeCoverage: MIN_EDGE_COVERAGE
  };
}

/**
 * `structural-ink-coverage-v1`. Font-independent BY CONSTRUCTION: every
 * text-only defect leaves this stratum bit-identical to baseline. That is
 * correct behaviour, not insensitivity, and must be stated rather than
 * mistaken for a weakness — text is bound by the content assertions.
 */
export function computeStructuralInk(expected: PNG, actual: PNG): StructuralInkResult {
  assertSameGeometry(expected, actual);
  const width = expected.width;
  const height = expected.height;

  const expectedStructural = structuralStratum(darkInkMaskP4(expected), width, height);
  const actualStructural = structuralStratum(darkInkMaskP4(actual), width, height);
  const expectedDilated = dilate(expectedStructural, width, height, TOLERANCE_RADIUS_PX);
  const actualDilated = dilate(actualStructural, width, height, TOLERANCE_RADIUS_PX);

  const unmatched = new Uint8Array(expectedStructural.length);
  let expectedPixels = 0;
  let actualPixels = 0;
  let matchedExpected = 0;
  let matchedActual = 0;
  for (let index = 0; index < expectedStructural.length; index += 1) {
    if (expectedStructural[index] === 1) {
      expectedPixels += 1;
      if (actualDilated[index] === 1) matchedExpected += 1;
      else unmatched[index] = 1;
    }
    if (actualStructural[index] === 1) {
      actualPixels += 1;
      if (expectedDilated[index] === 1) matchedActual += 1;
    }
  }

  return {
    comparison: "structural-ink-coverage-v1",
    inkThreshold: INK_THRESHOLD,
    structuralMinRun: STRUCTURAL_MIN_RUN,
    expectedStructuralPixels: expectedPixels,
    actualStructuralPixels: actualPixels,
    structuralRecall: ratio(matchedExpected, expectedPixels),
    structuralPrecision: ratio(matchedActual, actualPixels),
    largestUnmatchedClusterPx: largestComponent(unmatched, width, height),
    unmatchedExpectedPixels: expectedPixels - matchedExpected
  };
}

/**
 * `page-ink-budget-v1`. `paperPixels` counts exactly-white pixels and is the
 * direct answer to the tint attack: a page tinted to luminance 232 drives it
 * to ~0 while producing a max cell regression of only 10.9e-4.
 */
export function computePageInkBudget(expected: PNG, actual: PNG): PageInkBudgetResult {
  assertSameGeometry(expected, actual);
  const width = expected.width;
  const height = expected.height;

  const expectedInkMask = darkInkMaskP4(expected);
  const actualInkMask = darkInkMaskP4(actual);
  const expectedDilated = dilate(expectedInkMask, width, height, INK_TOLERANCE_RADIUS_PX);
  const actualDilated = dilate(actualInkMask, width, height, INK_TOLERANCE_RADIUS_PX);

  let expectedInk = 0;
  let actualInk = 0;
  let inkMissing = 0;
  let inkUnexpected = 0;
  for (let index = 0; index < expectedInkMask.length; index += 1) {
    if (expectedInkMask[index] === 1) {
      expectedInk += 1;
      if (actualDilated[index] !== 1) inkMissing += 1;
    }
    if (actualInkMask[index] === 1) {
      actualInk += 1;
      if (expectedDilated[index] !== 1) inkUnexpected += 1;
    }
  }

  let paperPixels = 0;
  for (let index = 0; index < actualInkMask.length; index += 1) {
    const offset = index * 4;
    if (
      actual.data[offset] === 255 &&
      actual.data[offset + 1] === 255 &&
      actual.data[offset + 2] === 255
    ) {
      paperPixels += 1;
    }
  }

  return {
    comparison: "page-ink-budget-v1",
    inkThreshold: INK_THRESHOLD,
    inkToleranceRadiusPx: INK_TOLERANCE_RADIUS_PX,
    expectedInk,
    actualInk,
    inkMissing,
    inkUnexpected,
    inkMissingRatio: ratio(inkMissing, expectedInk),
    inkUnexpectedRatio: ratio(inkUnexpected, actualInk),
    paperPixels
  };
}

function assertSameGeometry(expected: PNG, actual: PNG) {
  if (expected.width !== actual.width || expected.height !== actual.height) {
    throw new Error(
      `official-fidelity dimensions differ: expected ${expected.width}x${expected.height}, actual ${actual.width}x${actual.height}`
    );
  }
}

export type { FidelityCell };
