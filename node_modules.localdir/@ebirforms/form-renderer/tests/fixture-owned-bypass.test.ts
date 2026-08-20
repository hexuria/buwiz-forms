import { describe, expect, it } from "vitest";
import {
  collectEnvelopeStrings,
  verifyFixtureOwnedText,
  verifyStaticTextManifestCompleteness,
  type FixtureOwnedObservation,
  type OfficialStaticTextEntry
} from "../visual/official-2551q-static-text";

const ENVELOPE = {
  taxpayer: { name: "ANDREA MAE RENDERER GALANG", tin: "274476433" },
  fields: { other_tax_credit_description: { value: "Reviewed prior-payment credit" } }
};

function owned(overrides: Partial<FixtureOwnedObservation> = {}): FixtureOwnedObservation {
  return {
    page: 1,
    selector: ".comb-value > span",
    text: "A",
    singleGlyph: true,
    ...overrides
  };
}

describe("fixture-owned bypass", () => {
  const values = collectEnvelopeStrings(ENVELOPE);

  it("accepts genuine per-character comb cells", () => {
    expect(verifyFixtureOwnedText([owned({ text: "A" }), owned({ text: "7" })], values))
      .toEqual([]);
  });

  it("catches a fabricated advisory line hidden in a comb cell", () => {
    // The exact attack that previously scored zero violations everywhere:
    // it prints on the real form, but both the pixel gate and the static-text
    // walk suppress this selector.
    const violations = verifyFixtureOwnedText(
      [owned({ text: "NOT VALID FOR FILING" })],
      values
    );
    expect(violations).toHaveLength(1);
    expect(violations[0].kind).toBe("multi-glyph-cell");
  });

  it("catches invented prose in a free-text fixture field", () => {
    const violations = verifyFixtureOwnedText(
      [owned({ selector: ".adaptive-plain-value", text: "NOT VALID FOR FILING", singleGlyph: false })],
      values
    );
    expect(violations).toHaveLength(1);
    expect(violations[0].kind).toBe("unexplained-fixture-text");
  });

  it("accepts free text the envelope actually supplied", () => {
    expect(verifyFixtureOwnedText(
      [owned({ selector: ".adaptive-plain-value", text: "Reviewed prior-payment credit", singleGlyph: false })],
      values
    )).toEqual([]);
  });

  it("accepts an empty cell", () => {
    expect(verifyFixtureOwnedText([owned({ text: "" })], values)).toEqual([]);
  });
});

describe("manifest completeness containment direction", () => {
  const manifest: readonly OfficialStaticTextEntry[] = [
    { id: "rate-3", page: 2, order: 1, kind: "item", selector: ".r", text: "3%" },
    { id: "atc-pt010", page: 2, order: 2, kind: "item", selector: ".a", text: "PT010 Persons exempt from VAT (Sec. 116) 3%" }
  ];

  it("accepts a reviewed row", () => {
    expect(verifyStaticTextManifestCompleteness(
      [{ page: 2, selector: ".a", text: "PT010 Persons exempt from VAT (Sec. 116) 3%" }],
      manifest
    )).toEqual([]);
  });

  it("catches a fabricated row that merely ends in a reviewed string", () => {
    // Previously MISSED: containment ran observed-contains-manifest, so any
    // fabricated row ending "3%" was considered covered because "3%" is itself
    // a manifest entry.
    const violations = verifyStaticTextManifestCompleteness(
      [{ page: 2, selector: ".a", text: "PT999 Fabricated row (Sec. 999) 3%" }],
      manifest
    );
    expect(violations).toHaveLength(1);
    expect(violations[0].kind).toBe("unmanifested-element");
  });
});
