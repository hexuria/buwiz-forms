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
 * Values that flow through the envelope but are PRE-PRINTED on the official
 * form, so the blank official page shows them and blanking ours would invent a
 * mismatch we could never close.
 *
 * 1601C item 5 ATC is the reviewed case: the pinned PDF prints `WW010` as a
 * 9.96pt bold text run at (539.6, 112.2) — it is on the paper before a taxpayer
 * touches it — and `bir-core`'s `FORM_1601C_ATC` constrains the field to
 * exactly that string, with validation and XML round-trip both rejecting any
 * other value. It is a form constant carried on the envelope for convenience,
 * not taxpayer input.
 *
 * This list may only ever grow by the same standard: a text run visible in the
 * OFFICIAL raster plus a domain constraint proving the value cannot vary. It
 * makes the comparison stricter, never weaker — the glyphs stay visible on our
 * side and must match ink that is genuinely present on the official page.
 */
export const PRE_PRINTED_CONSTANT_SELECTORS: readonly string[] = [
  ".atc-plain-1601c"
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
      ${PRE_PRINTED_CONSTANT_SELECTORS
        .map((selector) => `.form-page[data-visual-blank-values="true"] ${selector}`)
        .join(",\n      ")} {
        color: inherit !important;
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
