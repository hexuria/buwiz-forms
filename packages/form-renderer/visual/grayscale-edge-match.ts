import { PNG } from "pngjs";

export type GrayscaleEdgeMatchOptions = Readonly<{
  edgeThreshold?: number;
  toleranceRadiusPx?: number;
}>;

export type GrayscaleEdgeMatch = Readonly<{
  expectedEdgePixels: number;
  actualEdgePixels: number;
  matchedExpectedEdgePixels: number;
  matchedActualEdgePixels: number;
  precision: number;
  recall: number;
  f1: number;
  edgeThreshold: number;
  toleranceRadiusPx: number;
}>;

const DEFAULT_EDGE_THRESHOLD = 48;
const DEFAULT_TOLERANCE_RADIUS_PX = 2;

/**
 * This page-global diagnostic can corroborate structure but can never promote
 * a form, and it is NOT the cell-scoped component of `official-fidelity-v1`.
 *
 * Two measured reasons it cannot stand in for that component:
 *  - Page-global scope is blind to localized defects: a removed comb field
 *    scores 0.999837 here, better than the 0.998313 cross-rasterizer floor.
 *  - Its default tolerance radius of 2 scores a whole-page 1px misregistration
 *    as exactly 1.000000. The criterion pins radius 1 for that reason.
 * See docs/form-print-readiness/official-fidelity-criterion-v1.md sections 1.2
 * and 2.1. Use `cell-edge-f1-v1` in ./official-fidelity.ts for gating.
 */
export const LAYERED_EDGE_EVIDENCE_POLICY = Object.freeze({
  promotionEligible: false,
  authoritativeVisualGate: "official-fidelity-v1",
  replacesAuthoritativeGate: false,
  supersededBy: "cell-edge-f1-v1",
  scope: "page-global"
} as const);

/**
 * Compare page structure as grayscale Sobel edges. Precision measures how
 * much rendered-page edge ink has an official neighbour; recall measures how
 * much official edge ink has a rendered neighbour. The symmetric F1 score is
 * diagnostic evidence only and must never replace the complete-page pixel
 * gate.
 */
export function compareSymmetricGrayscaleEdges(
  expected: PNG,
  actual: PNG,
  options: GrayscaleEdgeMatchOptions = {}
): GrayscaleEdgeMatch {
  if (expected.width !== actual.width || expected.height !== actual.height) {
    throw new Error(
      `edge-match dimensions differ: expected ${expected.width}x${expected.height}, actual ${actual.width}x${actual.height}`
    );
  }

  const edgeThreshold = options.edgeThreshold ?? DEFAULT_EDGE_THRESHOLD;
  const toleranceRadiusPx =
    options.toleranceRadiusPx ?? DEFAULT_TOLERANCE_RADIUS_PX;
  if (!Number.isFinite(edgeThreshold) || edgeThreshold <= 0) {
    throw new Error("edgeThreshold must be a positive finite number");
  }
  if (!Number.isInteger(toleranceRadiusPx) || toleranceRadiusPx < 0) {
    throw new Error("toleranceRadiusPx must be a non-negative integer");
  }

  const expectedEdges = grayscaleSobelEdges(expected, edgeThreshold);
  const actualEdges = grayscaleSobelEdges(actual, edgeThreshold);
  const expectedNeighbourhood = dilate(
    expectedEdges,
    expected.width,
    expected.height,
    toleranceRadiusPx
  );
  const actualNeighbourhood = dilate(
    actualEdges,
    actual.width,
    actual.height,
    toleranceRadiusPx
  );

  let expectedEdgePixels = 0;
  let actualEdgePixels = 0;
  let matchedExpectedEdgePixels = 0;
  let matchedActualEdgePixels = 0;
  for (let index = 0; index < expectedEdges.length; index += 1) {
    if (expectedEdges[index] === 1) {
      expectedEdgePixels += 1;
      if (actualNeighbourhood[index] === 1) matchedExpectedEdgePixels += 1;
    }
    if (actualEdges[index] === 1) {
      actualEdgePixels += 1;
      if (expectedNeighbourhood[index] === 1) matchedActualEdgePixels += 1;
    }
  }

  const precision = ratio(matchedActualEdgePixels, actualEdgePixels);
  const recall = ratio(matchedExpectedEdgePixels, expectedEdgePixels);
  const f1 = precision + recall === 0
    ? 0
    : (2 * precision * recall) / (precision + recall);

  return {
    expectedEdgePixels,
    actualEdgePixels,
    matchedExpectedEdgePixels,
    matchedActualEdgePixels,
    precision,
    recall,
    f1,
    edgeThreshold,
    toleranceRadiusPx
  };
}

/**
 * Composite-over-white luminance (criterion primitive P2).
 *
 * Storage is IEEE-754 binary64 on BOTH sides. This is a correctness
 * requirement, not an optimization: the Python audit must reproduce these
 * integers exactly, and emulating a JS `Float32Array` precision detail in
 * Python would pin an implementation artifact into the audit forever, where a
 * future refactor of one line desynchronizes the two implementations with no
 * test able to see it. binary64 is native in both languages and the expression
 * is evaluated left-to-right under IEEE rules in both.
 */
export function compositeLuminance(image: PNG): Float64Array {
  const luminance = new Float64Array(image.width * image.height);
  for (let index = 0; index < luminance.length; index += 1) {
    const offset = index * 4;
    const alpha = image.data[offset + 3] / 255;
    const red = image.data[offset] * alpha + 255 * (1 - alpha);
    const green = image.data[offset + 1] * alpha + 255 * (1 - alpha);
    const blue = image.data[offset + 2] * alpha + 255 * (1 - alpha);
    luminance[index] = 0.2126 * red + 0.7152 * green + 0.0722 * blue;
  }
  return luminance;
}

/**
 * Sobel edge mask (criterion primitive P3). Border pixels are never edges,
 * matching the loop bounds pinned by the criterion.
 *
 * `Math.hypot` is deliberately NOT used: it is a scaled libm-class routine and
 * is not bit-reproducible against Python's `math.hypot`. Two prior
 * investigations found the two agreed on this data, which is luck, not a
 * guarantee. The squared comparison is provably deterministic.
 */
export function sobelEdgeMask(image: PNG, threshold: number): Uint8Array {
  const width = image.width;
  const height = image.height;
  const grayscale = compositeLuminance(image);
  const edges = new Uint8Array(width * height);
  const thresholdSquared = threshold * threshold;

  for (let y = 1; y < height - 1; y += 1) {
    for (let x = 1; x < width - 1; x += 1) {
      const topLeft = grayscale[(y - 1) * width + x - 1];
      const top = grayscale[(y - 1) * width + x];
      const topRight = grayscale[(y - 1) * width + x + 1];
      const left = grayscale[y * width + x - 1];
      const right = grayscale[y * width + x + 1];
      const bottomLeft = grayscale[(y + 1) * width + x - 1];
      const bottom = grayscale[(y + 1) * width + x];
      const bottomRight = grayscale[(y + 1) * width + x + 1];
      const gradientX =
        -topLeft + topRight - 2 * left + 2 * right - bottomLeft + bottomRight;
      const gradientY =
        -topLeft - 2 * top - topRight + bottomLeft + 2 * bottom + bottomRight;
      if (gradientX * gradientX + gradientY * gradientY >= thresholdSquared) {
        edges[y * width + x] = 1;
      }
    }
  }
  return edges;
}

function grayscaleSobelEdges(image: PNG, threshold: number) {
  return sobelEdgeMask(image, threshold);
}

/**
 * Euclidean-disc dilation (criterion primitive P5). The criterion uses the
 * disc form EVERYWHERE, with no exceptions; see `dilateMask` in
 * ./official-page-diff.ts for the legacy square (Chebyshev) form it replaces.
 */
export function dilate(mask: Uint8Array, width: number, height: number, radius: number) {
  if (radius === 0) return mask.slice();
  const result = new Uint8Array(mask.length);
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      if (mask[y * width + x] !== 1) continue;
      for (
        let targetY = Math.max(0, y - radius);
        targetY <= Math.min(height - 1, y + radius);
        targetY += 1
      ) {
        for (
          let targetX = Math.max(0, x - radius);
          targetX <= Math.min(width - 1, x + radius);
          targetX += 1
        ) {
          const deltaX = targetX - x;
          const deltaY = targetY - y;
          if (deltaX * deltaX + deltaY * deltaY <= radius * radius) {
            result[targetY * width + targetX] = 1;
          }
        }
      }
    }
  }
  return result;
}

/** Criterion primitive P8. Pins today's semantics exactly. */
export function ratio(numerator: number, denominator: number) {
  if (denominator === 0) return numerator === 0 ? 1 : 0;
  return numerator / denominator;
}

/** Criterion primitive P8. */
export function f1Score(precision: number, recall: number) {
  return precision + recall === 0
    ? 0
    : (2 * precision * recall) / (precision + recall);
}
