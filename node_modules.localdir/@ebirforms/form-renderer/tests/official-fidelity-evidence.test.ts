import { describe, expect, it } from "vitest";
import { PNG } from "pngjs";
import {
  compareSymmetricGrayscaleEdges,
  LAYERED_EDGE_EVIDENCE_POLICY
} from "../visual/grayscale-edge-match";
import {
  normalizeStaticText,
  OFFICIAL_2551Q_ALLOWED_RESIDUAL,
  OFFICIAL_2551Q_STATIC_TEXT,
  staticTextEntriesForPage,
  verifyPageIndexedStaticText,
  verifyStaticTextExhaustive,
  verifyStaticTextManifestCompleteness,
  type OfficialStaticTextEntry
} from "../visual/official-2551q-static-text";
import { parsePromotionVisualThreshold } from "../visual/release-visual-threshold";

function whitePage(width = 80, height = 60) {
  const page = new PNG({ width, height });
  page.data.fill(255);
  return page;
}

function drawBlackLine(page: PNG, x1: number, y1: number, x2: number, y2: number) {
  if (x1 !== x2 && y1 !== y2) throw new Error("test lines must be axis-aligned");
  for (let y = Math.min(y1, y2); y <= Math.max(y1, y2); y += 1) {
    for (let x = Math.min(x1, x2); x <= Math.max(x1, x2); x += 1) {
      const offset = (y * page.width + x) * 4;
      page.data[offset] = 0;
      page.data[offset + 1] = 0;
      page.data[offset + 2] = 0;
      page.data[offset + 3] = 255;
    }
  }
}

describe("symmetric grayscale edge evidence", () => {
  it("reports identical edge structure as perfect precision, recall, and F1", () => {
    const expected = whitePage();
    drawBlackLine(expected, 8, 12, 70, 12);
    drawBlackLine(expected, 8, 38, 70, 38);
    const actual = PNG.sync.read(PNG.sync.write(expected));

    expect(compareSymmetricGrayscaleEdges(expected, actual)).toMatchObject({
      precision: 1,
      recall: 1,
      f1: 1,
      toleranceRadiusPx: 2
    });
  });

  it("accepts a two-pixel rasterizer registration shift symmetrically", () => {
    const expected = whitePage();
    const actual = whitePage();
    drawBlackLine(expected, 8, 20, 70, 20);
    drawBlackLine(actual, 8, 22, 70, 22);

    const metric = compareSymmetricGrayscaleEdges(expected, actual, {
      toleranceRadiusPx: 2
    });
    expect(metric.precision).toBe(1);
    expect(metric.recall).toBe(1);
    expect(metric.f1).toBe(1);
  });

  it("fails recall evidence when an official line is omitted", () => {
    const expected = whitePage();
    const actual = whitePage();
    drawBlackLine(expected, 8, 12, 70, 12);
    drawBlackLine(expected, 8, 42, 70, 42);
    drawBlackLine(actual, 8, 12, 70, 12);

    const metric = compareSymmetricGrayscaleEdges(expected, actual);
    expect(metric.recall).toBeLessThan(0.75);
    expect(metric.f1).toBeLessThan(0.9);
  });

  it("fails precision evidence when an unverified line is added", () => {
    const expected = whitePage();
    const actual = whitePage();
    drawBlackLine(expected, 8, 12, 70, 12);
    drawBlackLine(actual, 8, 12, 70, 12);
    drawBlackLine(actual, 8, 42, 70, 42);

    const metric = compareSymmetricGrayscaleEdges(expected, actual);
    expect(metric.precision).toBeLessThan(0.75);
    expect(metric.f1).toBeLessThan(0.9);
  });

  it("is additive evidence and cannot replace or relax the raw page gate", () => {
    // This page-global diagnostic is also NOT the cell-scoped component of
    // official-fidelity-v1: page-global scope scores a removed comb field
    // better than the cross-rasterizer floor, and its default radius of 2
    // scores a whole-page 1px misregistration as exactly 1.000000.
    expect(LAYERED_EDGE_EVIDENCE_POLICY).toEqual({
      promotionEligible: false,
      authoritativeVisualGate: "official-fidelity-v1",
      replacesAuthoritativeGate: false,
      supersededBy: "cell-edge-f1-v1",
      scope: "page-global"
    });
    expect(parsePromotionVisualThreshold(undefined)).toBe(1);
  });

  it("forgives at radius 2 a misregistration that radius 1 penalises", () => {
    // Guards the measured reason official-fidelity-v1 pins radius 1: a wider
    // tolerance disc can only ever match more, and on the real page radius 2
    // scored a whole-page 1px misregistration as exactly 1.000000. Sobel marks
    // both sides of a rule, so a 2px displacement is the smallest that
    // separates the two radii on a synthetic single-line page.
    const expected = whitePage();
    const shifted = whitePage();
    drawBlackLine(expected, 8, 12, 70, 12);
    drawBlackLine(shifted, 8, 14, 70, 14);

    const atRadiusTwo = compareSymmetricGrayscaleEdges(expected, shifted, {
      toleranceRadiusPx: 2
    });
    const atRadiusOne = compareSymmetricGrayscaleEdges(expected, shifted, {
      toleranceRadiusPx: 1
    });
    expect(atRadiusTwo.f1).toBe(1);
    expect(atRadiusOne.f1).toBeLessThan(1);
  });
});

describe("2551Q page-indexed static-text evidence", () => {
  const manifest: readonly OfficialStaticTextEntry[] = [
    { id: "page-one-label", page: 1, order: 1, kind: "item", selector: ".tin-rdo-row > .field-label", text: "6 Taxpayer Identification Number (TIN)" },
    { id: "page-two-heading", page: 2, order: 2, kind: "table-heading", selector: ".official-schedule > h2", text: "Schedule 1 – Computation of Tax" }
  ];

  it("reports no violations when exact text is on the reviewed page", () => {
    expect(verifyPageIndexedStaticText([
      "6 Taxpayer Identification Number (TIN)",
      "Schedule 1 – Computation of Tax"
    ], manifest)).toEqual([]);
  });

  it("fails with the omitted label identified", () => {
    expect(verifyPageIndexedStaticText([
      "Part I – Background Information",
      "Schedule 1 – Computation of Tax"
    ], manifest)).toEqual([
      {
        id: "page-one-label",
        expectedPage: 1,
        text: "6 Taxpayer Identification Number (TIN)",
        foundOnPages: []
      }
    ]);
  });

  it("fails when an exact label is present on the wrong page", () => {
    expect(verifyPageIndexedStaticText([
      "Schedule 1 – Computation of Tax",
      "6 Taxpayer Identification Number (TIN)"
    ], manifest)).toEqual([
      {
        id: "page-one-label",
        expectedPage: 1,
        text: "6 Taxpayer Identification Number (TIN)",
        foundOnPages: [2]
      },
      {
        id: "page-two-heading",
        expectedPage: 2,
        text: "Schedule 1 – Computation of Tax",
        foundOnPages: [1]
      }
    ]);
  });

  it("keeps the reviewed manifest page-scoped and uniquely addressable", () => {
    expect(OFFICIAL_2551Q_STATIC_TEXT.length).toBeGreaterThan(100);
    expect(new Set(OFFICIAL_2551Q_STATIC_TEXT.map(({ id }) => id)).size)
      .toBe(OFFICIAL_2551Q_STATIC_TEXT.length);
    expect(OFFICIAL_2551Q_STATIC_TEXT.every(({ page }) => page === 1 || page === 2))
      .toBe(true);
  });

  it("uses selector-scoped evidence for otherwise ambiguous Schedule row numbers", () => {
    const rowNumberEntry: readonly OfficialStaticTextEntry[] = [{
      id: "schedule-row-one",
      page: 2,
      order: 1,
      kind: "item",
      text: "1",
      selector: ".official-schedule-row[data-row-slot='1'] > .official-schedule-row-number"
    }];
    expect(verifyPageIndexedStaticText([
      { fullText: "1", selectorText: {} },
      { fullText: "1 elsewhere", selectorText: {} }
    ], rowNumberEntry)).toEqual([
      {
        id: "schedule-row-one",
        expectedPage: 2,
        text: "1",
        foundOnPages: []
      }
    ]);
    expect(verifyPageIndexedStaticText([
      { fullText: "1", selectorText: {} },
      {
        fullText: "1 elsewhere",
        selectorText: {
          ".official-schedule-row[data-row-slot='1'] > .official-schedule-row-number": "1"
        }
      }
    ], rowNumberEntry)).toEqual([]);
  });
});

describe("static-text-exhaustive-v1", () => {
  // Deliberately short. Each case is one of the attacks the criterion names as
  // invisible to every pixel component.
  const ordered: readonly OfficialStaticTextEntry[] = [
    { id: "atc", page: 2, order: 1, kind: "table-heading", selector: "th:nth-child(1)", text: "ATC" },
    { id: "on", page: 2, order: 2, kind: "table-heading", selector: "th:nth-child(2)", text: "Percentage Tax On" },
    { id: "rate", page: 2, order: 3, kind: "table-heading", selector: "th:nth-child(3)", text: "Tax Rate" },
    { id: "pt010", page: 2, order: 4, kind: "table-entry", selector: "tr[data-atc-code='PT010']", text: "PT 010 Persons exempt from VAT" },
    { id: "pt010-rate", page: 2, order: 5, kind: "tax-rate", selector: "tr[data-atc-code='PT010'] > td:nth-child(3)", text: "3%" }
  ];
  const clean = "ATC Percentage Tax On Tax Rate PT 010 Persons exempt from VAT 3%";

  it("passes the reviewed page", () => {
    expect(verifyStaticTextExhaustive(clean, ordered)).toEqual([]);
  });

  it("catches a wrong statutory tax rate that every pixel component forgives", () => {
    const wrong = clean.replace("3%", "5%");
    expect(verifyStaticTextExhaustive(wrong, ordered)).toEqual([
      { kind: "missing-or-reordered", id: "pt010-rate", page: 2, text: "3%", foundEarlierAt: null },
      { kind: "unexpected-residual", page: 2, tokens: ["5%"] }
    ]);
  });

  it("catches swapped column headings that containment cannot see", () => {
    const swapped = "ATC Tax Rate Percentage Tax On PT 010 Persons exempt from VAT 3%";
    // Containment passes: both strings are still on the page.
    expect(verifyPageIndexedStaticText(
      ["", swapped],
      ordered.filter((entry) => entry.id === "on" || entry.id === "rate")
        .map((entry) => ({ ...entry, selector: "" }))
    )).toEqual([]);
    // The ordered walk does not. The greedy cursor consumes the heading that
    // moved earlier, so the one that fails is whichever now sits behind it --
    // the identity of the loser is incidental, the detection is not.
    const violations = verifyStaticTextExhaustive(swapped, ordered);
    expect(violations).toContainEqual({
      kind: "missing-or-reordered",
      id: "rate",
      page: 2,
      text: "Tax Rate",
      foundEarlierAt: expect.any(Number)
    });
  });

  it("catches a fabricated advisory line inserted between reviewed strings", () => {
    const fabricated = clean.replace("Tax Rate PT 010", "Tax Rate NOT VALID FOR FILING PT 010");
    expect(verifyStaticTextExhaustive(fabricated, ordered)).toEqual([
      { kind: "unexpected-residual", page: 2, tokens: ["FILING", "FOR", "NOT", "VALID"] }
    ]);
  });

  it("permits only the pinned structural residual glyphs", () => {
    expect(OFFICIAL_2551Q_ALLOWED_RESIDUAL).toEqual([".", "%", "-"]);
    const withScaffolding = "ATC Percentage Tax On Tax Rate . - PT 010 Persons exempt from VAT 3%";
    expect(verifyStaticTextExhaustive(withScaffolding, ordered)).toEqual([]);
  });

  it("keeps every reviewed 2551Q page ordered, unique and selector-addressed", () => {
    for (const page of [1, 2] as const) {
      const entries = staticTextEntriesForPage(page);
      expect(entries.length).toBeGreaterThan(0);
      const orders = entries.map(({ order }) => order);
      expect([...orders].sort((left, right) => left - right)).toEqual(orders);
      expect(new Set(orders).size).toBe(orders.length);
    }
    expect(OFFICIAL_2551Q_STATIC_TEXT.every(({ selector }) => selector.length > 0)).toBe(true);
  });

  it("covers every ATC rate the renderer prints", () => {
    const rates = OFFICIAL_2551Q_STATIC_TEXT.filter(({ kind }) => kind === "tax-rate");
    expect(rates).toHaveLength(22);
    expect(rates.every(({ text }) => /^\d+%$/.test(text))).toBe(true);
  });

  it("lets officialText differ from text by whitespace only", () => {
    // The official 2551Q prints "18 %" for some rates and "18%" for others.
    // Recording the literal must never become a place a wrong value can hide.
    const divergent = OFFICIAL_2551Q_STATIC_TEXT.filter(({ officialText }) => officialText);
    expect(divergent.length).toBeGreaterThan(0);
    for (const entry of divergent) {
      const strip = (value: string) => normalizeStaticText(value).replace(/\s+/g, "");
      expect(strip(entry.officialText!)).toBe(strip(entry.text));
    }
  });
});

describe("static-text manifest completeness", () => {
  it("flags a renderer string that no reviewed entry accounts for", () => {
    const violations = verifyStaticTextManifestCompleteness([
      { page: 2, selector: "[data-atc-code]", text: "PT 999\tSomething nobody reviewed\t9%" }
    ]);
    expect(violations).toHaveLength(1);
    expect(violations[0]).toMatchObject({
      kind: "unmanifested-element",
      page: 2,
      selector: "[data-atc-code]",
      text: "PT 999 Something nobody reviewed 9%"
    });
    // The unaccounted tokens name the defect precisely rather than merely
    // flagging the row. Asserted by containment so the test does not become
    // brittle against unrelated manifest growth.
    expect(violations[0].unaccounted).toEqual(
      expect.arrayContaining(["999", "nobody", "reviewed", "9%"])
    );
  });

  it("accepts a renderer string the manifest already covers", () => {
    expect(verifyStaticTextManifestCompleteness([
      { page: 2, selector: "[data-atc-code]", text: "PT 010\tPersons exempt from VAT under Sec. 109(BB) (Sec. 116)\t3%" },
      { page: 1, selector: ".payment-headings > b", text: "Particulars" }
    ])).toEqual([]);
  });
});
