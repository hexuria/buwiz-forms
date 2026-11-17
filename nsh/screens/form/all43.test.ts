// D8, headless: every form in the library opens as a Form library sample for the demo taxpayer, through the form session.
// The list is the door's own (bir.screen_forms_index, expected 43). One test per form, so a failure names its form. Each
// passing test prints one row, `D8 <code> parts=<n> fields=<n> ms=<n>`; the rows are the table kept in ~/BuwizNsh/evidence/forms43/.
import { expect, test } from "nsh/test";
import { bir } from "../../bir";
import { createDemoTaxpayer, DEMO } from "../../bir/testing";
import { FormSession } from "./session";

const FORMS = bir.screen_forms_index();

test("the door lists 43 forms, each code once, each with a title", () => {
  expect(FORMS.length).toBe(43);
  expect(new Set(FORMS.map((f) => f.c)).size).toBe(43);
  for (const f of FORMS) expect(f.t.length).toBeGreaterThan(0);
});

FORMS.forEach((form, i) => {
  test(`D8 ${String(i + 1).padStart(2, "0")} ${form.c} opens as a sample`, async () => {
    createDemoTaxpayer();
    const started = Date.now();
    const s = new FormSession({ tin: DEMO.tin, formCode: form.c, year: 0, period: 0, mode: "sample" });
    try {
      expect(await s.open()).toBe(null);
      await s.idle();
      const ms = Date.now() - started;
      expect(s.header().error).toBe("");
      const parts = s.schema?.parts.length ?? 0;
      const fields = s.fieldCount();
      expect(parts).toBeGreaterThan(0);
      expect(fields).toBeGreaterThan(0);
      s.toggleAllParts();
      expect(s.allPartsOpen()).toBe(true);
      console.log(`D8 ${form.c} parts=${parts} fields=${fields} ms=${ms}`);
    } finally {
      s.dispose();
    }
  }, 60_000);
});
