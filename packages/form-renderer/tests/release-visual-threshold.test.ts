import { describe, expect, it } from "vitest";
import {
  parsePromotionVisualThreshold,
  RELEASE_VISUAL_MAX_CHANGED_PERCENT
} from "../visual/release-visual-threshold";

describe("promotion visual threshold", () => {
  it("defaults to the strict release threshold", () => {
    expect(parsePromotionVisualThreshold(undefined)).toBe(
      RELEASE_VISUAL_MAX_CHANGED_PERCENT
    );
  });

  it("accepts stricter thresholds", () => {
    expect(parsePromotionVisualThreshold("0")).toBe(0);
    expect(parsePromotionVisualThreshold("0.25")).toBe(0.25);
    expect(parsePromotionVisualThreshold("1")).toBe(1);
  });

  it.each(["1.0001", "100", "-0.1", "NaN", "Infinity"])(
    "rejects non-promoting threshold %s",
    (value) => {
      expect(() => parsePromotionVisualThreshold(value)).toThrow(
        "must be between 0 and 1 for promotion evidence"
      );
    }
  );
});
