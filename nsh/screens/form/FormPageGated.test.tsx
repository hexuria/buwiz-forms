// G1 on the page: a box another answer keeps shut is not drawn — its testID is absent until the opener is
// answered, the row (number and title too) is absent while every drawn box is shut, and the "N items" counts
// follow the rows that draw. Phone (390 x 844) and desktop (1440 x 900); the demo taxpayer only.
import { expect, open, test } from "nsh/test";
import type { App } from "nsh/test";
import { warmForms } from "../../bir/testing";

warmForms();

const ENTRY = "./FormPageAny.tsx";
const SIZES = [
  { name: "phone", options: {} },
  { name: "desktop", options: { width: 1440, height: 900, platform: "macos" as const } },
] as const;

async function page(options: (typeof SIZES)[number]["options"], params: Record<string, unknown>): Promise<App> {
  const app = await open(ENTRY, { ...options, host: { params } });
  await app.waitFor("id:fp-part-0-toggle", { timeout: 20000 });
  return app;
}

/** Scroll down until `sel` is drawn (rows mount as their part's list reaches them). */
async function reach(app: App, sel: string): Promise<void> {
  for (let i = 0; i < 40 && !app.shows(sel); i++) await app.scroll("id:form-scroll", 320).catch(() => {});
  await app.waitFor(sel, { timeout: 15000 });
}

/** Scroll back up until `sel` is drawn. */
async function reachUp(app: App, sel: string): Promise<void> {
  for (let i = 0; i < 40 && !app.shows(sel); i++) await app.scroll("id:form-scroll", -320).catch(() => {});
  await app.waitFor(sel, { timeout: 15000 });
}

for (const size of SIZES) {
  test(`G1 ${size.name}: 0619E item 14's whole row is absent until Item 4 = Yes; the part count follows`, async () => {
    const app = await page(size.options, { formCode: "0619E", year: 2025, period: 3, openParts: [0, 2] });
    // One money box, shut: the row — its testID, number and title — is not drawn, and not counted.
    expect(app.count("id:fp-item-2:0")).toBe(0);
    expect(app.count("id:fp-frm0619E:txtTax14")).toBe(0);
    expect(app.shows("id:fp-part-2-toggle >> text:7 items")).toBe(true);
    // Item 4 is at the top of the page (part 0): answer Yes there.
    await reach(app, "id:fp-frm0619E:optWithheld:Y");
    await app.press("id:fp-frm0619E:optWithheld:Y");
    // Scroll to where item 14 sits (the top of part 2, row 2:0 — its schema place): the row appears there.
    await reach(app, "id:fp-item-2:0");
    expect(app.count("id:fp-frm0619E:txtTax14")).toBe(1);
    expect(app.shows("id:fp-part-2-toggle >> text:8 items")).toBe(true);
    // Answer No again: it is gone again (it holds no value).
    await reachUp(app, "id:fp-frm0619E:optWithheld:N");
    await app.press("id:fp-frm0619E:optWithheld:N");
    await app.waitFor("id:fp-item-2:0", { state: "gone", timeout: 15000 });
    expect(app.count("id:fp-item-2:0")).toBe(0);
    expect(app.shows("id:fp-part-2-toggle >> text:7 items")).toBe(true);
  });

  test(`G1 ${size.name}: 0605 item 17's gated boxes open one manner pick at a time`, async () => {
    const app = await page(size.options, { formCode: "0605", year: 2025, period: 1, openParts: [1, 3] });
    // The row draws (its manner radios are never gated); the specify box starts absent.
    await reach(app, "id:fp-item-1:8");
    expect(app.count("id:fp-frm0605:txtOthersName")).toBe(0);
    // The approval pair is a row of its own (3:0, in the last part): every box of it is gated, so the
    // whole row — number and title too — is not drawn.
    expect(app.count("id:fp-item-3:0")).toBe(0);
    expect(app.count("id:fp-frm0605:itemApprovedYN:_1")).toBe(0);
    // "Others (Specify)" opens the specify box inside the same row (seven choices: a dropdown).
    await app.press("id:fp-radio-1:8-trigger");
    await app.press("text:/Others \\(Specify\\)/i");
    await app.waitFor("id:fp-frm0605:txtOthersName", { timeout: 15000 });
    expect(app.count("id:fp-frm0605:txtOthersName")).toBe(1);
    // "Preliminary/Final Assessment" opens the approval row in its own part.
    await app.press("id:fp-radio-1:8-trigger");
    await app.press("text:/Deficiency Tax/");
    await reach(app, "id:fp-item-3:0");
    expect(app.count("id:fp-frm0605:itemApprovedYN:_1")).toBe(1);
    expect(app.count("id:fp-frm0605:itemApprovedYN:_2")).toBe(1);
  });

  test(`G1 ${size.name}: 2551Q item 12's specify box follows the Yes radio`, async () => {
    const app = await page(size.options, { formCode: "2551Q", year: 2025, period: 1, openParts: [1] });
    // Item 12's radios draw; the tax-relief specify box is absent until Yes.
    await reach(app, "id:fp-item-1:6");
    expect(app.count("id:fp-frm2551Qv2018:taxTreaty_1")).toBe(1);
    expect(app.count("id:fp-frm2551Qv2018:txtTaxReliefSpecify")).toBe(0);
    await app.press("id:fp-frm2551Qv2018:taxTreaty_1");
    await app.waitFor("id:fp-frm2551Qv2018:txtTaxReliefSpecify", { timeout: 15000 });
    expect(app.count("id:fp-frm2551Qv2018:txtTaxReliefSpecify")).toBe(1);
    await app.press("id:fp-frm2551Qv2018:taxTreaty_2");
    await app.waitFor("id:fp-frm2551Qv2018:txtTaxReliefSpecify", { state: "gone", timeout: 15000 });
    expect(app.count("id:fp-frm2551Qv2018:txtTaxReliefSpecify")).toBe(0);
  });
}
