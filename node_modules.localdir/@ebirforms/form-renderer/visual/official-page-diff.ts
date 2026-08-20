import pixelmatch from "pixelmatch";
import { PNG } from "pngjs";

export type CompletePageComparison = Readonly<{
  fullPageChangedPixels: number;
  fullPageChangedPercent: number;
  expectedInkMissingPercent: number;
  unexpectedActualInkPercent: number;
  diff: PNG;
}>;

export type CompletePageComparisonOptions = Readonly<{
  pixelThreshold?: number;
  inkThreshold?: number;
  inkToleranceRadius?: number;
}>;

/**
 * Compare every printable page pixel after the caller has hidden only dynamic
 * fixture values. Unlike the old ruled-line diagnostic, this includes static
 * labels, instructions, checkboxes, artwork, signatures, and short rules.
 */
export function compareCompleteOfficialPage(
  expected: PNG,
  actual: PNG,
  options: CompletePageComparisonOptions = {}
): CompletePageComparison {
  if (expected.width !== actual.width || expected.height !== actual.height) {
    throw new Error(
      `official-page dimensions differ: expected ${expected.width}x${expected.height}, actual ${actual.width}x${actual.height}`
    );
  }

  const pixelThreshold = options.pixelThreshold ?? 0.1;
  const inkThreshold = options.inkThreshold ?? 160;
  const inkToleranceRadius = options.inkToleranceRadius ?? 2;
  // diffMask keeps the artifact a pure red-on-transparent changed-pixel mask,
  // byte-identical to the mask the migration audit independently recomputes.
  const { changedPixels: fullPageChangedPixels, diff } = comparePixelMask(
    expected,
    actual,
    pixelThreshold
  );
  const pixelCount = expected.width * expected.height;

  const expectedInk = darkInkMask(expected, inkThreshold);
  const actualInk = darkInkMask(actual, inkThreshold);
  const expectedDilated = dilateMask(
    expectedInk,
    expected.width,
    expected.height,
    inkToleranceRadius
  );
  const actualDilated = dilateMask(
    actualInk,
    actual.width,
    actual.height,
    inkToleranceRadius
  );
  let expectedInkCount = 0;
  let actualInkCount = 0;
  let expectedInkMissing = 0;
  let unexpectedActualInk = 0;
  for (let index = 0; index < expectedInk.length; index += 1) {
    if (expectedInk[index] === 1) {
      expectedInkCount += 1;
      if (actualDilated[index] !== 1) expectedInkMissing += 1;
    }
    if (actualInk[index] === 1) {
      actualInkCount += 1;
      if (expectedDilated[index] !== 1) unexpectedActualInk += 1;
    }
  }

  return {
    fullPageChangedPixels,
    fullPageChangedPercent: fullPageChangedPixels * 100 / pixelCount,
    expectedInkMissingPercent: percentage(expectedInkMissing, expectedInkCount),
    unexpectedActualInkPercent: percentage(unexpectedActualInk, actualInkCount),
    diff
  };
}

/**
 * Raw pixelmatch of one full page into a pure changed-pixel mask (red on
 * transparent, anti-aliased pixels excluded and undrawn), matching the exact
 * mask semantics the migration audit recomputes byte-for-byte.
 */
export function comparePixelMask(
  expected: PNG,
  actual: PNG,
  threshold: number
): { changedPixels: number; diff: PNG } {
  if (expected.width !== actual.width || expected.height !== actual.height) {
    throw new Error(
      `official-page dimensions differ: expected ${expected.width}x${expected.height}, actual ${actual.width}x${actual.height}`
    );
  }
  const diff = new PNG({ width: expected.width, height: expected.height });
  const changedPixels = pixelmatch(
    expected.data,
    actual.data,
    diff.data,
    expected.width,
    expected.height,
    { threshold, includeAA: false, diffMask: true }
  );
  return { changedPixels, diff };
}

function percentage(numerator: number, denominator: number) {
  if (denominator === 0) return numerator === 0 ? 0 : 100;
  return numerator * 100 / denominator;
}

export function darkInkMask(image: PNG, threshold: number) {
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

export function dilateMask(
  mask: Uint8Array,
  width: number,
  height: number,
  radius: number
) {
  if (!Number.isInteger(radius) || radius < 0) {
    throw new Error("inkToleranceRadius must be a non-negative integer");
  }
  const dilated = new Uint8Array(mask.length);
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
          dilated[targetY * width + targetX] = 1;
        }
      }
    }
  }
  return dilated;
}
