// D8, headless: every form in the library opens as a Form library sample for the demo taxpayer, through the form session.
// The list is the door's own (bir.screen_forms_index, expected 43). One test per form, so a failure names its form. Each
// passing test prints one row, `D8 <code> parts=<n> fields=<n> ms=<n>`; the rows are the table kept in ~/BuwizNsh/evidence/forms43/.
import { expect, test } from "nsh/test";
import { bir } from "../../bir";
import { createDemoTaxpayer, DEMO } from "../../bir/testing";
import type { FormSchema } from "./contract";
import { radioKind } from "./controls/model";
import { choiceLabel, radioGroups, withoutHidden } from "./schema";
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

// PR 5: every one-answer group of the 43 forms draws by its size — a segmented bar (2–4), a dropdown (5–12), a search
// list (more) — and every choice keeps its key and its label. No production group reaches the search branch (the largest
// is eight); the branch is covered by the controls gallery's fixture.
test("PR5: every radio group draws by its size and every choice is reachable", () => {
  const seen = { segmented: 0, dropdown: 0, search: 0 };
  let groups = 0;
  for (const form of FORMS) {
    const schema = withoutHidden(bir.screen_form_schema(form.c) as FormSchema);
    for (const part of schema.parts) {
      for (const it of part.items) {
        const radios = (it.f ?? []).filter((f) => f.t === "radio");
        for (const g of radioGroups(radios)) {
          groups += 1;
          seen[radioKind(g.length)] += 1;
          expect(new Set(g.map((f) => f.k)).size).toBe(g.length);
          for (const f of g) {
            expect(f.k.length).toBeGreaterThan(0);
            expect(choiceLabel(f).length).toBeGreaterThan(0);
          }
        }
      }
    }
  }
  expect(groups).toBeGreaterThan(150);
  console.log(`PR5 radio groups=${groups} segmented=${seen.segmented} dropdown=${seen.dropdown} search=${seen.search}`);
});
