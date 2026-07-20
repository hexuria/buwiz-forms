// Capture-only probe for structural diagnosis.
//
// WHY THIS EXISTS. structural-defects.spec.ts answers "which clusters are
// worst" (top 12) but cannot answer "is this page globally misregistered or
// locally defective", because that question needs the FULL offset distribution
// and a whole-page shift sweep, and it needs the DOM geometry that produced
// each stroke. Re-running a browser for every variation of that analysis is
// slow and makes the analysis itself hard to review.
//
// So this tool captures the two inputs ONCE - the blanked page rasters and a
// full DOM geometry/border dump - and writes them to disk. All measurement
// then happens offline against those artifacts. Diagnostic only, never
// promotion evidence; it asserts nothing.

import { expect, test } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  blankComparisonEnvelope,
  prepareOfficialBlankComparison,
  renderEnvelope
} from "../support/render-utils";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(HERE, "../../../..");

const FORM_CODE = process.env.STRUCT_FORM ?? "2551Q";
const FIXTURE =
  process.env.STRUCT_FIXTURE ?? "packages/form-contracts/fixtures/2551q-6-rows.json";
const OUT_DIR = process.env.STRUCT_OUT ?? "";
/** Device px per CSS px; must match the config's deviceScaleFactor. */
const SCALE = Number(process.env.STRUCT_SCALE ?? "1.5");

test(`capture ${FORM_CODE} structural probe`, async ({ page }) => {
  if (!OUT_DIR) throw new Error("STRUCT_OUT is required");
  fs.mkdirSync(OUT_DIR, { recursive: true });

  const fixture = JSON.parse(
    fs.readFileSync(path.join(REPO_ROOT, FIXTURE), "utf8")
  ) as unknown;
  await renderEnvelope(page, blankComparisonEnvelope(fixture, FORM_CODE));
  await prepareOfficialBlankComparison(page);
  await page.evaluate(async () => {
    await document.fonts.ready;
    await new Promise((resolve) =>
      requestAnimationFrame(() => requestAnimationFrame(resolve))
    );
  });

  const pages = page.locator(".form-page");
  const pageCount = await pages.count();
  expect(pageCount).toBeGreaterThan(0);

  for (let index = 0; index < pageCount; index += 1) {
    const shot = await pages.nth(index).screenshot({
      animations: "disabled",
      caret: "hide"
    });
    fs.writeFileSync(path.join(OUT_DIR, `actual-page-${index + 1}.png`), shot);
  }

  // Every element's box in DEVICE px relative to its own .form-page origin,
  // plus the border/outline/background declarations that can emit a stroke.
  // This is what turns "unmatched cluster at y=1830" into "this selector's
  // border-bottom".
  const dom = await page.evaluate((scale) => {
    const cssPath = (element: Element): string => {
      const parts: string[] = [];
      let node: Element | null = element;
      while (node && parts.length < 6 && node.tagName !== "BODY") {
        let part = node.tagName.toLowerCase();
        if (node.id) part += `#${node.id}`;
        const cls = (node.getAttribute("class") ?? "").trim();
        if (cls) part += `.${cls.split(/\s+/).join(".")}`;
        parts.unshift(part);
        node = node.parentElement;
      }
      return parts.join(" > ");
    };

    const formPages = Array.from(document.querySelectorAll(".form-page"));
    return formPages.map((formPage, pageIndex) => {
      const origin = formPage.getBoundingClientRect();
      const elements = Array.from(formPage.querySelectorAll("*"))
        .map((element) => {
          const rect = element.getBoundingClientRect();
          if (rect.width === 0 && rect.height === 0) return null;
          const style = getComputedStyle(element);
          const borders = {
            top: style.borderTopWidth,
            right: style.borderRightWidth,
            bottom: style.borderBottomWidth,
            left: style.borderLeftWidth
          };
          const hasBorder = Object.values(borders).some(
            (value) => parseFloat(value) > 0
          );
          const bg = style.backgroundColor;
          const hasFill = bg !== "rgba(0, 0, 0, 0)" && bg !== "transparent";
          const hasOutline =
            parseFloat(style.outlineWidth) > 0 && style.outlineStyle !== "none";
          if (!hasBorder && !hasFill && !hasOutline) return null;
          return {
            selector: cssPath(element),
            tag: element.tagName.toLowerCase(),
            class: element.getAttribute("class") ?? "",
            text: (element.textContent ?? "").trim().slice(0, 60),
            // device px, page-relative
            x0: (rect.left - origin.left) * scale,
            y0: (rect.top - origin.top) * scale,
            x1: (rect.right - origin.left) * scale,
            y1: (rect.bottom - origin.top) * scale,
            borderTopPx: parseFloat(borders.top),
            borderRightPx: parseFloat(borders.right),
            borderBottomPx: parseFloat(borders.bottom),
            borderLeftPx: parseFloat(borders.left),
            borderTopStyle: style.borderTopStyle,
            borderBottomStyle: style.borderBottomStyle,
            borderLeftStyle: style.borderLeftStyle,
            borderRightStyle: style.borderRightStyle,
            backgroundColor: hasFill ? bg : "",
            boxSizing: style.boxSizing,
            display: style.display
          };
        })
        .filter((entry) => entry !== null);
      return { page: pageIndex + 1, elements };
    });
  }, SCALE);

  fs.writeFileSync(
    path.join(OUT_DIR, "dom-geometry.json"),
    `${JSON.stringify({ form: FORM_CODE, scale: SCALE, pages: dom }, null, 2)}\n`
  );
});
