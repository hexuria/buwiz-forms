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

/** This diagnostic can corroborate structure but can never promote a form. */
export const LAYERED_EDGE_EVIDENCE_POLICY = Object.freeze({
  promotionEligible: false,
  authoritativeVisualGate: "official-complete-page-v1",
  replacesAuthoritativeGate: false
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

function grayscaleSobelEdges(image: PNG, threshold: number) {
  const width = image.width;
  const height = image.height;
  const grayscale = new Float32Array(width * height);
  const edges = new Uint8Array(width * height);

  for (let index = 0; index < grayscale.length; index += 1) {
    const offset = index * 4;
    const alpha = image.data[offset + 3] / 255;
    const red = image.data[offset] * alpha + 255 * (1 - alpha);
    const green = image.data[offset + 1] * alpha + 255 * (1 - alpha);
    const blue = image.data[offset + 2] * alpha + 255 * (1 - alpha);
    grayscale[index] = 0.2126 * red + 0.7152 * green + 0.0722 * blue;
  }

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
      if (Math.hypot(gradientX, gradientY) >= threshold) {
        edges[y * width + x] = 1;
      }
    }
  }
  return edges;
}

function dilate(mask: Uint8Array, width: number, height: number, radius: number) {
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

function ratio(numerator: number, denominator: number) {
  if (denominator === 0) return numerator === 0 ? 1 : 0;
  return numerator / denominator;
}
