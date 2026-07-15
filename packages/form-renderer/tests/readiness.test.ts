import { describe, expect, it } from "vitest";
import { rendererGeometryIsSafe, type RendererGeometryMeasurement } from "../src/geometry";
import { geometryStabilityDecision } from "../src/readiness";

describe("renderer geometry stability", () => {
  it("requires two consecutive identical signatures", () => {
    expect(geometryStabilityDecision(undefined, "first", false)).toBe("retry");
    expect(geometryStabilityDecision("first", "first", false)).toBe("ready");
  });

  it("fails closed when geometry is still changing at the deadline", () => {
    expect(geometryStabilityDecision("first", "second", true)).toBe("timed_out");
  });
});

describe("renderer geometry safety", () => {
  function geometry(): RendererGeometryMeasurement {
    return {
      type: "page_count",
      page_count: 1,
      page_width_pt: 612,
      page_height_pt: 936,
      pages: [{
        x: 0,
        y: 0,
        width: 816,
        height: 1248,
        client_width: 816,
        client_height: 1248,
        scroll_width: 816,
        scroll_height: 1248,
        descendant_overflow_x: 0,
        descendant_overflow_y: 0,
        descendant_clipped_x: 0,
        descendant_clipped_y: 0
      }]
    };
  }

  it("rejects descendant overflow and clipping before renderer_ready", () => {
    expect(rendererGeometryIsSafe(geometry())).toBe(true);

    for (const key of [
      "descendant_overflow_x",
      "descendant_overflow_y",
      "descendant_clipped_x",
      "descendant_clipped_y"
    ] as const) {
      const unsafe = geometry();
      unsafe.pages[0][key] = 1;
      expect(rendererGeometryIsSafe(unsafe), key).toBe(false);
    }
  });
});
