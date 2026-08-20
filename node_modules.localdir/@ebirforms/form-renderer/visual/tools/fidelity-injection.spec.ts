// THE CRITERION'S OWN REGRESSION TEST.
//
// official-fidelity-v1 is a non-regression gate: for every scored cell,
// f1 >= pinned_baseline_f1 - M_CELL, plus structural and ink-budget bounds. A
// gate like that is only worth having if it actually fires on real defects, so
// this suite seeds known defects and proves detection.
//
// Method mirrors the real gate exactly: components are computed for the CLEAN
// render against the pinned reference (the baseline), then for the DEFECTIVE
// render against the same reference, and the two are compared. Nothing is
// measured against a self-render, because that is not how the gate works.
//
// Two honest properties this suite pins, not just the happy path:
//   - The structural stratum is font-independent BY CONSTRUCTION, so text-only
//     defects leave it bit-identical. That is correct behaviour and is
//     asserted, not assumed.
//   - Content defects are NOT RELIED UPON to be caught by pixels. The cell
//     component may detect some of them incidentally - measured here at
//     ~1.07e-2 for an in-place digit change, well above M_CELL, and far more
//     than the 0.19e-4 the criterion's named-region-only analysis predicted,
//     because the 64px grid cells added for coverage also raised text
//     sensitivity. That is a happy accident, not a guarantee: detection scales
//     with how much of a cell the changed glyph occupies, so a same-width
//     substitution inside a large cell can still vanish. Content correctness is
//     therefore bound by static-text-exhaustive-v1, and this suite records the
//     incidental number without ever letting it stand in for that assertion.

import { expect, test } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { PNG } from "pngjs";
import {
  computeCellEdgeF1,
  computePageInkBudget,
  computeStructuralInk,
  type CellEdgeF1Result,
  type PageInkBudgetResult,
  type StructuralInkResult
} from "../official-fidelity";
import {
  blankComparisonEnvelope,
  prepareOfficialBlankComparison,
  renderEnvelope
} from "../support/render-utils";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(HERE, "../../../..");

/** Criterion section 2.2. Measured local floor is exactly zero. */
const M_CELL = 0.001;
const STRUCTURAL_DROP = 0.002;
const INK_RATIO_BAND = 0.03;

const FIXTURE = "packages/form-contracts/fixtures/2551q-6-rows.json";
const REFERENCES = [
  { page: 1, chromium: "packages/form-renderer/references/2551q-2018-page-1-chromium.png" },
  { page: 2, chromium: "packages/form-renderer/references/2551q-2018-page-2-chromium.png" }
];

/**
 * Which component must fire. `content-assertion` means the pixel components
 * are NOT relied upon for this defect class: the structural stratum must be
 * bit-identical (font-independence by construction) and any cell movement is
 * recorded as incidental only.
 */
type Detector = "cell" | "structural" | "ink" | "content-assertion";

interface DefectCase {
  id: string;
  description: string;
  /** CSS injected to seed the defect. */
  css?: string;
  /**
   * DOM mutation, evaluated in the page. Used where CSS cannot express the
   * defect faithfully - notably wrong text content, which must change glyphs
   * in place without moving or removing ink, or the test measures the wrong
   * thing and reports a stronger claim than the criterion earns.
   */
  mutate?: string;
  expect: Detector;
  /** Required when expect === "content-assertion": what actually binds it. */
  coveredBy?: string;
}

const DEFECTS: readonly DefectCase[] = [
  {
    id: "translate-region-1px",
    description: "one label block displaced by 1 device px",
    css: `.background-information { transform: translateY(0.667px); }`,
    expect: "cell"
  },
  {
    id: "translate-region-3px",
    description: "one label block displaced by 3 device px",
    css: `.background-information { transform: translateY(2px); }`,
    expect: "cell"
  },
  {
    id: "translate-page-1px",
    description: "whole page misregistered by 1 device px",
    css: `.form-page > * { transform: translateY(0.667px); }`,
    expect: "cell"
  },
  {
    id: "font-size-declaration-plus-4pct",
    description: "declaration copy rendered 4% too large",
    // Applied to the text-bearing element itself: a percentage on a container
    // does not cascade past children carrying explicit pt sizes, which made an
    // earlier version of this case a pixel-identical no-op.
    css: `.official-declaration > p { font-size: 5.824pt !important; }`,
    expect: "cell"
  },
  {
    id: "missing-checkbox",
    description: "a checkbox outline removed entirely",
    css: `.filing-basis .check-choice:nth-child(1) .check-box { visibility: hidden !important; }`,
    expect: "cell"
  },
  {
    id: "missing-rule",
    description: "a ruled line deleted",
    css: `.official-declaration { border-top-color: transparent !important; }`,
    expect: "structural"
  },
  {
    id: "border-doubled",
    description: "box borders doubled in thickness",
    css: `.official-header-options { border-width: 2px !important; }`,
    expect: "cell"
  },
  {
    id: "extra-element",
    description: "a stray advisory line that is not on the official form",
    css: `.privacy-note::after {
      content: "NOT VALID FOR FILING";
      display: block;
      font-size: 8pt;
      font-weight: 700;
    }`,
    expect: "cell"
  },
  {
    id: "faded-print",
    description: "ink faded toward grey, simulating a weak print path",
    css: `.form-page { color: #777777 !important; }`,
    expect: "ink"
  },
  {
    id: "tinted-paper",
    description: "paper tinted off-white, which percentage gates handle badly",
    css: `.form-page { background-color: #fbfbfb !important; }`,
    expect: "ink"
  },
  {
    id: "wrong-static-text",
    description: "statutory ATC rates changed in place to wrong values",
    // Mutates glyphs in place - same element, same position, same styling, and
    // a same-width digit - so this measures the real defect. An earlier CSS
    // version hid the cell and overlaid absolutely-positioned text, which moves
    // ink and made the pixel components look more capable than they are.
    mutate: `
      const cells = document.querySelectorAll(".official-atc-table .atc-entry td:last-child");
      let changed = 0;
      for (const cell of cells) {
        const text = cell.textContent ?? "";
        if (/\\d/.test(text)) {
          cell.textContent = text.replace(/\\d/, (digit) => (digit === "9" ? "8" : "9"));
          changed += 1;
        }
      }
      if (changed === 0) throw new Error("wrong-static-text mutated nothing");
    `,
    expect: "content-assertion",
    coveredBy: "static-text-exhaustive-v1 (criterion section 3.2)"
  }
];

interface PageMeasurement {
  cell: CellEdgeF1Result;
  structural: StructuralInkResult;
  ink: PageInkBudgetResult;
}

test(`official-fidelity-v1 detects seeded defects`, async ({ page }, testInfo) => {
  const fixture = JSON.parse(
    fs.readFileSync(path.join(REPO_ROOT, FIXTURE), "utf8")
  ) as unknown;
  const blanked = blankComparisonEnvelope(fixture, "2551Q");
  const references = REFERENCES.map((entry) => ({
    page: entry.page,
    image: PNG.sync.read(fs.readFileSync(path.join(REPO_ROOT, entry.chromium)))
  }));

  async function measure(defect: DefectCase | null): Promise<PageMeasurement[]> {
    await renderEnvelope(page, blanked);
    if (defect?.css) await page.addStyleTag({ content: defect.css });
    if (defect?.mutate) {
      await page.evaluate((source) => {
        // eslint-disable-next-line no-new-func
        new Function(source)();
      }, defect.mutate);
    }
    await prepareOfficialBlankComparison(page);
    await page.evaluate(async () => {
      await document.fonts.ready;
      await new Promise((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(resolve))
      );
    });
    const pages = page.locator(".form-page");
    await expect(pages).toHaveCount(references.length);
    const out: PageMeasurement[] = [];
    for (const [index, reference] of references.entries()) {
      const shot = await pages.nth(index).screenshot({
        animations: "disabled",
        caret: "hide"
      });
      const actual = PNG.sync.read(shot);
      out.push({
        cell: computeCellEdgeF1(reference.image, actual, "2551Q", "2018", reference.page),
        structural: computeStructuralInk(reference.image, actual),
        ink: computePageInkBudget(reference.image, actual)
      });
    }
    return out;
  }

  const baseline = await measure(null);
  console.log(
    `baseline: p1 worstF1=${baseline[0].cell.worstScoredF1.toFixed(6)} ` +
      `structRecall=${baseline[0].structural.structuralRecall.toFixed(6)} ` +
      `actualInk=${baseline[0].ink.actualInk}`
  );

  const rows: Array<Record<string, unknown>> = [];
  const failures: string[] = [];

  for (const defect of DEFECTS) {
    const treated = await measure(defect);

    let worstCellDrop = 0;
    let worstStructuralDrop = 0;
    let worstInkRatioShift = 0;
    let paperPixelDrop = 0;

    for (const [index, base] of baseline.entries()) {
      const now = treated[index];

      const baseById = new Map(base.cell.cells.map((cell) => [cell.id, cell]));
      for (const cell of now.cell.cells) {
        const before = baseById.get(cell.id);
        if (!before || !before.scored) continue;
        worstCellDrop = Math.max(worstCellDrop, before.f1 - cell.f1);
      }

      worstStructuralDrop = Math.max(
        worstStructuralDrop,
        base.structural.structuralRecall - now.structural.structuralRecall,
        base.structural.structuralPrecision - now.structural.structuralPrecision
      );

      worstInkRatioShift = Math.max(
        worstInkRatioShift,
        Math.abs(now.ink.actualInk / base.ink.actualInk - 1)
      );
      paperPixelDrop = Math.max(
        paperPixelDrop,
        1 - now.ink.paperPixels / base.ink.paperPixels
      );
    }

    const cellFires = worstCellDrop > M_CELL;
    const structuralFires = worstStructuralDrop > STRUCTURAL_DROP;
    const inkFires = worstInkRatioShift > INK_RATIO_BAND || paperPixelDrop > 0.02;
    const anyFires = cellFires || structuralFires || inkFires;

    rows.push({
      id: defect.id,
      description: defect.description,
      expected_detector: defect.expect,
      worst_cell_f1_drop: worstCellDrop,
      worst_structural_drop: worstStructuralDrop,
      worst_ink_ratio_shift: worstInkRatioShift,
      paper_pixel_drop: paperPixelDrop,
      cell_fires: cellFires,
      structural_fires: structuralFires,
      ink_fires: inkFires,
      covered_by: defect.coveredBy ?? null
    });

    console.log(
      `${defect.id.padEnd(24)} cellDrop=${worstCellDrop.toExponential(3)} ` +
        `structDrop=${worstStructuralDrop.toExponential(3)} ` +
        `inkShift=${worstInkRatioShift.toExponential(3)} ` +
        `paperDrop=${paperPixelDrop.toExponential(3)} -> ` +
        `${anyFires ? "DETECTED" : "not detected"} (expected ${defect.expect})`
    );

    if (defect.expect === "content-assertion") {
      // The binding property: the structural stratum must be font-independent.
      // Cell movement here is incidental and deliberately NOT asserted, so the
      // suite can never be read as promising content coverage from pixels.
      if (structuralFires) {
        failures.push(
          `${defect.id}: the structural stratum must be font-independent by ` +
            `construction, but it moved by ${worstStructuralDrop.toExponential(3)}. ` +
            `Either the stratum is picking up glyphs (check STRUCTURAL_MIN_RUN) ` +
            `or this defect is not text-only.`
        );
      }
      if (!defect.coveredBy) {
        failures.push(
          `${defect.id}: a content-assertion defect must name the assertion that binds it.`
        );
      }
      console.log(
        `${" ".repeat(24)}^ not relied upon: bound by ${defect.coveredBy}; ` +
          `cell movement ${worstCellDrop.toExponential(3)} is incidental`
      );
      continue;
    }
    if (!anyFires) {
      failures.push(
        `${defect.id} (${defect.description}) was NOT detected by any component. ` +
          `The criterion is blind to this defect class; do not promote it until a ` +
          `component or companion assertion covers it.`
      );
      continue;
    }
    const fired =
      defect.expect === "cell"
        ? cellFires
        : defect.expect === "structural"
          ? structuralFires
          : inkFires;
    if (!fired) {
      failures.push(
        `${defect.id}: expected the ${defect.expect} component to fire; it did not ` +
          `(another component did). Either the expectation or the constants are wrong.`
      );
    }
  }

  const reportPath = path.join(testInfo.outputDir, "fidelity-injection-report.json");
  fs.mkdirSync(testInfo.outputDir, { recursive: true });
  fs.writeFileSync(
    reportPath,
    `${JSON.stringify(
      {
        purpose: "criterion_self_regression_test",
        promotion_eligible: false,
        criterion: "official-fidelity-v1",
        m_cell: M_CELL,
        structural_drop: STRUCTURAL_DROP,
        ink_ratio_band: INK_RATIO_BAND,
        defects: rows
      },
      null,
      2
    )}\n`
  );
  console.log(`wrote ${path.relative(REPO_ROOT, reportPath)}`);

  expect(failures, failures.join("\n")).toEqual([]);
});
