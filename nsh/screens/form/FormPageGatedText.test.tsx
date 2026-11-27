// Gated schedule/specification text on the page: a shut box is a read-only box carrying the official reason;
// the controlling answer opens it, typed content is kept while open, and closing the gate clears it with the
// clearing sentence. 1701's spouse employer name is the dormant-text case; its foreign tax number is the
// discarded-typing case.
import { expect, open, test } from "nsh/test";
import type { App } from "nsh/test";
import { warmForms } from "../../bir/testing";

warmForms();

const ENTRY = "./FormPageAny.tsx";

async function page(options: Record<string, unknown>, params: Record<string, unknown>): Promise<App> {
  const app = await open(ENTRY, { ...options, host: { params } });
  await app.waitFor("text:BIR Form 1701", { timeout: 20000 });
  return app;
}

async function reach(app: App, sel: string): Promise<void> {
  for (let i = 0; i < 50 && !app.shows(sel); i++) await app.scroll("id:form-scroll", 420).catch(() => {});
  await app.waitFor(sel, { timeout: 15000 });
}

async function reachUp(app: App, sel: string): Promise<void> {
  for (let i = 0; i < 50 && !app.shows(sel); i++) await app.scroll("id:form-scroll", -420).catch(() => {});
  await app.waitFor(sel, { timeout: 15000 });
}

test("desktop: 1701 spouse employer name is shut until joint spouse compensation, then clears once", async () => {
  const app = await page(
    { width: 1440, height: 900, platform: "macos" as const },
    { formCode: "1701", year: 2025, period: 0, openParts: "all" },
  );
  const owner = "id:fp-frm1701:chkPg2IShed1a_1Spouse";
  const name = "id:fp-frm1701:txtPg2IShed1a_1TPName";

  await reachUp(app, "id:fp-frm1701:rdoPg1I16CivilStatusM");
  await app.press("id:fp-frm1701:rdoPg1I16CivilStatusM");
  await app.press("id:fp-frm1701:rdoPg1I17SpouseIncomeYes");
  await app.press("id:fp-frm1701:rdoPg1I18FilingStatusJ");
  await reach(app, "id:fp-frm1701:rdoPg2I3SpouseTypeS");
  await app.press("id:fp-frm1701:rdoPg2I3SpouseTypeS");
  await app.press("id:fp-frm1701:rdoPg2I3SpouseTypeC");
  await reach(app, "id:fp-radio-4:3-trigger");
  await app.scroll("id:form-scroll", 180).catch(() => {});
  await app.press("id:fp-radio-4:3-trigger");
  await app.waitFor("text:/II011/", { timeout: 5000 });
  await app.press("text:/^II011/");

  const table = "id:fp-table-frm1701:chkPg2IShed1a_1Taxpayer";
  await reach(app, `${table}-add`);
  await app.press(`${table}-add`);
  await app.waitFor("text:Row 1", { timeout: 5000 });
  await app.press(owner);
  await app.type(name, "SPOUSE EMPLOYER");
  expect(app.value(name)).toBe("SPOUSE EMPLOYER");
  await app.press(`${table}-done`);
  await app.waitFor("text:Row 1", { state: "gone", timeout: 5000 });
  await app.waitFor("text:/SPOUSE EMPLOYER/", { timeout: 10000 });
  await app.waitFor("text:✓ Saved", { timeout: 20000 });

  await reachUp(app, "id:fp-frm1701:rdoPg1I18FilingStatusS");
  await app.press("id:fp-frm1701:rdoPg1I18FilingStatusS");
  await app.waitUntil(() => app.shows("text:Continue") || app.texts().includes("✓ Saved"), { timeout: 5000 });
  if (app.shows("text:Continue")) await app.press("text:Continue");
  await app.waitFor("text:✓ Saved", { timeout: 20000 });
  await reach(app, table);
  await app.waitUntil(() => app.texts().some((text) => text.includes("Employer row employer-line-1 was cleared")), { timeout: 10000 });
  expect(app.texts().some((text) => text.includes("compensation income"))).toBe(true);
}, 60000);

test("phone: 1701 foreign tax number is shut until Item 13 Yes, then clears once", async () => {
  const app = await page({}, { formCode: "1701", year: 2025, period: 0, openParts: "all" });
  const yes = "id:fp-frm1701:rdoPg1I13ForeignTaxCreditsYes";
  const no = "id:fp-frm1701:rdoPg1I13ForeignTaxCreditsNo";
  const foreign = "id:fp-frm1701:txtPg1I14ForeignTaxNumber";

  await reach(app, foreign);
  expect(app.count(foreign)).toBe(1);
  expect(app.texts().some((text) => text.includes("Part I Item 13 Yes"))).toBe(true);
  await app.press(yes);
  await app.waitUntil(() => app.enabled(foreign) === true, { timeout: 10000 });
  await app.type(foreign, "FTN-1");
  expect(app.value(foreign)).toBe("FTN-1");
  await app.waitFor("text:✓ Saved", { timeout: 20000 });
  await app.press(no);
  await app.waitFor("text:Continue", { timeout: 5000 });
  await app.press("text:Continue");
  await app.waitUntil(() => app.texts().some((text) => text.includes("The foreign tax number was cleared")), { timeout: 10000 });
}, 60000);
