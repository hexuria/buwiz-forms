// Emit a critical-region table for a form, derived from its own rendered DOM.
//
// WHY. The region-ranked diff report attributes every changed pixel to a named
// region, falling back to uniform tiles where no region is named. 2551Q has 76
// named page-1 regions and the other nine have none, so on those forms every
// diff lands in an anonymous tile and the report can say "3% of the page moved"
// without saying which part of the form moved. That is the difference between a
// number and a lead.
//
// WHAT IT DOES NOT DO. It does not invent semantics. Region names come from the
// form's own heading and label text, and selectors are the real DOM selectors
// that resolve to the measured element. It is CANDIDATE GENERATION: the emitted
// table is a starting point a human edits and reviews, exactly like the comb
// detector's output. Nothing here may be treated as reviewed geometry evidence
// until a person has read it against the pinned PDF.
//
// The rectangles are measured from OUR render, not from the official reference.
// That is correct for attribution (we need to know where our own regions are)
// and wrong for calibration (never use these to argue our geometry matches).
//
// Run one form at a time; the shared dev server means parallel runs collide.

import { expect, test } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { renderEnvelope } from "../support/render-utils";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(HERE, "../../../..");
const DEVICE_SCALE_FACTOR = 1.5;

const FORM_CODE = process.env.REGION_FORM ?? "1601C";
const FIXTURE =
  process.env.REGION_FIXTURE ?? "packages/form-contracts/fixtures/1601c-normal.json";
const OUTPUT =
  process.env.REGION_OUTPUT ?? `packages/form-renderer/visual/regions/${FORM_CODE.toLowerCase()}.ts`;

/** Below this area a region is noise rather than a landmark. */
const MIN_REGION_AREA_PX = 4000;
/** A region covering nearly the whole page attributes nothing useful. */
const MAX_PAGE_FRACTION = 0.85;

interface Emitted {
  page: number;
  name: string;
  selector: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

test(`generate a candidate region table for ${FORM_CODE}`, async ({ page }) => {
  const fixture = JSON.parse(
    fs.readFileSync(path.join(REPO_ROOT, FIXTURE), "utf8")
  ) as unknown;
  await renderEnvelope(page, fixture);

  const pages = page.locator(".form-page");
  const pageCount = await pages.count();
  expect(pageCount).toBeGreaterThan(0);

  const regions: Emitted[] = [];
  for (let index = 0; index < pageCount; index += 1) {
    const formPage = pages.nth(index);
    const pageBox = await formPage.boundingBox();
    if (!pageBox) continue;

    // Structural landmarks: elements that draw a border or fill, i.e. the boxes
    // a reader perceives as the form's sections. Text nodes are deliberately
    // excluded - text is bound by content assertions, not by region geometry.
    const candidates = await formPage.evaluate(
      (scope, { minArea, maxFraction }) => {
        const pageRect = scope.getBoundingClientRect();
        const pageArea = pageRect.width * pageRect.height;

        const describe = (element: Element): string => {
          const heading = element.querySelector("h1,h2,h3,h4,legend,caption");
          const text = (heading?.textContent ?? "").trim().replace(/\s+/g, " ");
          if (text) return text.slice(0, 60);
          const own = (element.textContent ?? "").trim().replace(/\s+/g, " ");
          return own ? own.slice(0, 60) : "";
        };

        const selectorFor = (element: Element): string => {
          const cls = [...element.classList].find((c) => c.length > 2);
          if (!cls) return element.tagName.toLowerCase();
          const same = [...scope.querySelectorAll(`.${CSS.escape(cls)}`)];
          if (same.length === 1) return `.${cls}`;
          return `.${cls}:nth-of-type(${same.indexOf(element) + 1})`;
        };

        const out: Array<Record<string, unknown>> = [];
        for (const element of scope.querySelectorAll("*")) {
          const style = getComputedStyle(element);
          const bordered =
            parseFloat(style.borderTopWidth) > 0 ||
            parseFloat(style.borderBottomWidth) > 0 ||
            parseFloat(style.borderLeftWidth) > 0 ||
            parseFloat(style.borderRightWidth) > 0;
          const filled =
            style.backgroundColor !== "rgba(0, 0, 0, 0)" &&
            style.backgroundColor !== "transparent";
          if (!bordered && !filled) continue;

          const rect = element.getBoundingClientRect();
          const area = rect.width * rect.height;
          if (area < minArea) continue;
          if (area > pageArea * maxFraction) continue;

          out.push({
            name: describe(element),
            selector: selectorFor(element),
            x: rect.x - pageRect.x,
            y: rect.y - pageRect.y,
            width: rect.width,
            height: rect.height
          });
        }
        return out;
      },
      {
        minArea: MIN_REGION_AREA_PX / (DEVICE_SCALE_FACTOR * DEVICE_SCALE_FACTOR),
        maxFraction: MAX_PAGE_FRACTION
      }
    );

    for (const candidate of candidates) {
      const name = String(candidate.name || "").trim();
      regions.push({
        page: index + 1,
        name: name || `region at ${Math.round(Number(candidate.y))}`,
        selector: String(candidate.selector),
        x: Math.round(Number(candidate.x) * DEVICE_SCALE_FACTOR),
        y: Math.round(Number(candidate.y) * DEVICE_SCALE_FACTOR),
        width: Math.round(Number(candidate.width) * DEVICE_SCALE_FACTOR),
        height: Math.round(Number(candidate.height) * DEVICE_SCALE_FACTOR)
      });
    }
  }

  // Drop duplicate rectangles: nested wrappers that share an edge box add rows
  // without adding attribution.
  const seen = new Set<string>();
  const unique = regions.filter((region) => {
    const key = `${region.page}:${region.x},${region.y},${region.width},${region.height}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });

  const byPage = (pageNumber: number) =>
    unique
      .filter((region) => region.page === pageNumber)
      .sort((a, b) => a.y - b.y || a.x - b.x)
      .map(
        (region) =>
          `  { name: ${JSON.stringify(region.name)}, selector: ${JSON.stringify(region.selector)}, ` +
          `x: ${region.x}, y: ${region.y}, width: ${region.width}, height: ${region.height} }`
      )
      .join(",\n");

  const pageConsts = Array.from({ length: pageCount }, (_, index) => {
    const ordinal = ["ONE", "TWO", "THREE", "FOUR", "FIVE", "SIX"][index] ?? `P${index + 1}`;
    return `export const PAGE_${ordinal}_CRITICAL_REGIONS: readonly CriticalRegion[] = [\n${byPage(index + 1)}\n];`;
  }).join("\n\n");

  const source = `// CANDIDATE ${FORM_CODE} critical-region rectangles in 144-DPI reference pixels.
//
// GENERATED by visual/tools/generate-region-table.spec.ts from this form's own
// rendered DOM, then REVIEWED BY HAND. Regeneration is a starting point, not an
// authority: names are lifted from the form's own headings and may be wrong or
// unhelpful, nested wrappers may still produce near-duplicate rows, and the
// rectangles describe OUR render rather than the official page.
//
// Because these are measured from our own output they are valid for ATTRIBUTION
// (telling a reader which part of the form a diff landed in) and invalid as
// calibration evidence (they can never argue that our geometry matches the
// official form - only the pinned references and the geometry contract can).

export interface CriticalRegion {
  name: string;
  selector: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

${pageConsts}

export function criticalRegionsFor(pageNumber: number): readonly CriticalRegion[] {
  const pages = [${Array.from({ length: pageCount }, (_, i) => {
    const ordinal = ["ONE", "TWO", "THREE", "FOUR", "FIVE", "SIX"][i] ?? `P${i + 1}`;
    return `PAGE_${ordinal}_CRITICAL_REGIONS`;
  }).join(", ")}];
  return pages[pageNumber - 1] ?? [];
}
`;

  const outPath = path.join(REPO_ROOT, OUTPUT);
  fs.mkdirSync(path.dirname(outPath), { recursive: true });
  fs.writeFileSync(outPath, source);
  console.log(
    `${FORM_CODE}: ${unique.length} candidate regions across ${pageCount} pages -> ${OUTPUT}`
  );
  expect(unique.length).toBeGreaterThan(0);
});
