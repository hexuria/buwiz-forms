// Shared render/blanking helpers for the visual parity suite and its
// diagnostic tools. These must stay behaviorally identical for every caller:
// the parity gate, the browserless replay, and the font sweep all rely on the
// same render sequencing and the same fixture-value blanking rules.

import type { Page } from "@playwright/test";

export async function renderEnvelope(page: Page, envelope: unknown) {
  await page.goto("/");
  await page.waitForFunction(
    () =>
      typeof (
        window as Window & {
          renderEbirForm?: (value: unknown) => void;
        }
      ).renderEbirForm === "function"
  );
  await page.evaluate((value) => {
    const render = (
      window as Window & {
        renderEbirForm?: (input: unknown) => void;
      }
    ).renderEbirForm;
    if (!render) throw new Error("renderEbirForm is not installed");
    render(value);
  }, envelope);
  await page.locator(".form-document").waitFor();
  await page.evaluate(() =>
    new Promise<void>((resolve) => {
      requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
    })
  );
  await page.evaluate(() => document.fonts.ready);
  // Adaptive-fit fields (charbox -> plain-text overflow) mark themselves
  // pending until the post-font fitting ladder settles; capturing earlier
  // races the fit. Forms without such fields pass through immediately.
  await page.waitForFunction(
    () => !document.querySelector('[data-adaptive-fit-state="pending"]')
  );
}

/**
 * The fixture-owned glyph set: values, adaptive plain text, check marks, and
 * inline specification text. Single definition, shared by the pixel gate's
 * blanking below and by `static-text-exhaustive-v1`, so "fixture-owned" can
 * never mean two different things in one criterion.
 */
export const FIXTURE_OWNED_SELECTORS: readonly string[] = [
  ".comb-value > span",
  ".adaptive-plain-value",
  ".check-box",
  ".tax-credit-description"
];

/**
 * Hide fixture-supplied glyphs so the comparison sees the blank official
 * page: values, adaptive plain text, check marks, and inline specification
 * text become transparent while every border, comb cell, label, fill, and
 * artwork stays visible. This is the ONLY masking the gate permits.
 */
export async function prepareOfficialBlankComparison(page: Page) {
  await page.addStyleTag({
    content: `
      ${FIXTURE_OWNED_SELECTORS
        .map((selector) => `.form-page[data-visual-blank-values="true"] ${selector}`)
        .join(",\n      ")} {
        color: transparent !important;
        text-shadow: none !important;
      }
    `
  });
  await page.locator(".form-page").evaluateAll((pages) => {
    for (const formPage of pages) {
      formPage.setAttribute("data-visual-blank-values", "true");
    }
  });
}

// Reviewed per-form fields whose values must be emptied before comparing to
// the blank official reference, because a populated value legitimately claims
// geometry (e.g. a second line) that the unfilled official page does not show.
const BLANK_COMPARISON_FIELDS: Record<string, readonly string[]> = {
  "2551Q": ["other_tax_credit_description"]
};

export function blankComparisonEnvelope(
  envelope: unknown,
  formCode: string
): unknown {
  const blankEnvelope = JSON.parse(JSON.stringify(envelope)) as {
    fields?: Record<string, { value?: unknown }>;
  };
  for (const fieldName of BLANK_COMPARISON_FIELDS[formCode] ?? []) {
    const field = blankEnvelope.fields?.[fieldName];
    if (field) field.value = "";
  }
  return blankEnvelope;
}
