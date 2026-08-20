import { describe, expect, it } from "vitest";
import { PNG } from "pngjs";
import {
  cropSha256,
  sha256Hex,
  verifyEncodedArtwork,
  type ArtworkObservation,
  type EncodedArtworkPin
} from "../visual/encoded-artwork";

function page(width = 32, height = 32): PNG {
  const image = new PNG({ width, height });
  for (let index = 0; index < width * height; index += 1) {
    const offset = index * 4;
    image.data[offset] = 255;
    image.data[offset + 1] = 255;
    image.data[offset + 2] = 255;
    image.data[offset + 3] = 255;
  }
  return image;
}

const CROP = { x: 4, y: 4, width: 8, height: 8 };

function pin(overrides: Partial<EncodedArtworkPin> = {}): EncodedArtworkPin {
  return {
    id: "symbol",
    page: 1,
    selector: ".symbol",
    payload: "FORM P1",
    symbolDataSha256: sha256Hex("M0 0h1v1h-1z"),
    crop: CROP,
    cropSha256: cropSha256(page(), CROP),
    ...overrides
  };
}

function observation(overrides: Partial<ArtworkObservation> = {}): ArtworkObservation {
  return {
    id: "symbol",
    page: 1,
    pathData: "M0 0h1v1h-1z",
    title: "PDF417 payload FORM P1",
    transform: "none",
    naturalWidth: null,
    naturalHeight: null,
    ...overrides
  };
}

describe("encoded artwork crop hashing", () => {
  it("is stable for identical regions and changes when a pixel changes", () => {
    const clean = page();
    expect(cropSha256(clean, CROP)).toBe(cropSha256(page(), CROP));

    const tampered = page();
    const offset = (5 * 32 + 5) * 4;
    tampered.data[offset] = 0;
    tampered.data[offset + 1] = 0;
    tampered.data[offset + 2] = 0;
    expect(cropSha256(tampered, CROP)).not.toBe(cropSha256(clean, CROP));
  });

  it("ignores pixels outside the pinned rectangle", () => {
    const clean = page();
    const outside = page();
    const offset = (20 * 32 + 20) * 4;
    outside.data[offset] = 0;
    expect(cropSha256(outside, CROP)).toBe(cropSha256(clean, CROP));
  });

  it("refuses a crop that leaves the page", () => {
    expect(() => cropSha256(page(), { x: 28, y: 28, width: 8, height: 8 })).toThrow(
      /lies outside/
    );
  });
});

describe("encoded artwork integrity", () => {
  const rasters = new Map([[1, page()]]);

  it("accepts artwork that matches every reviewed pin", () => {
    expect(verifyEncodedArtwork([pin()], [observation()], rasters)).toEqual([]);
  });

  it("catches a mirrored symbol whose bounding box is unchanged", () => {
    // The red-team attack this component exists for: scaleX(-1) leaves the
    // bbox, the path data and every geometry assertion intact, and renders an
    // unscannable symbol.
    const violations = verifyEncodedArtwork(
      [pin()],
      [observation({ transform: "matrix(-1, 0, 0, 1, 0, 0)" })],
      rasters
    );
    expect(violations.map((entry) => entry.kind)).toContain("transform");
  });

  it("catches substituted symbol data even when the payload title is right", () => {
    const violations = verifyEncodedArtwork(
      [pin()],
      [observation({ pathData: "M0 0h2v1h-2z" })],
      rasters
    );
    expect(violations.map((entry) => entry.kind)).toContain("symbol-data");
  });

  it("catches a wrong payload", () => {
    const violations = verifyEncodedArtwork(
      [pin()],
      [observation({ title: "PDF417 payload FORM P2" })],
      rasters
    );
    expect(violations.map((entry) => entry.kind)).toContain("payload");
  });

  it("catches artwork that did not render at all", () => {
    const violations = verifyEncodedArtwork([pin()], [], rasters);
    expect(violations).toHaveLength(1);
    expect(violations[0].kind).toBe("missing");
  });

  it("catches a raster whose crop no longer matches the review", () => {
    const tampered = page();
    const offset = (6 * 32 + 6) * 4;
    tampered.data[offset] = 0;
    tampered.data[offset + 1] = 0;
    tampered.data[offset + 2] = 0;
    const violations = verifyEncodedArtwork(
      [pin()],
      [observation()],
      new Map([[1, tampered]])
    );
    expect(violations.map((entry) => entry.kind)).toContain("crop");
  });

  it("reports an unpinned crop instead of passing it silently", () => {
    // An un-pinned crop is an unmade decision, not a satisfied one. Passing it
    // quietly is how an unreviewed symbol reaches a release.
    const violations = verifyEncodedArtwork(
      [pin({ cropSha256: undefined })],
      [observation()],
      rasters
    );
    expect(violations).toHaveLength(1);
    expect(violations[0].kind).toBe("unpinned");
    expect(violations[0].detail).toMatch(/observed [0-9a-f]{64}/);
  });

  it("reports every violation rather than stopping at the first", () => {
    const violations = verifyEncodedArtwork(
      [pin()],
      [
        observation({
          title: "wrong",
          pathData: "M0 0h9v9h-9z",
          transform: "matrix(-1, 0, 0, 1, 0, 0)"
        })
      ],
      rasters
    );
    expect(new Set(violations.map((entry) => entry.kind))).toEqual(
      new Set(["payload", "symbol-data", "transform"])
    );
  });
});
