// PR 8 (Q8): every TIN row of the 43 built schemas is a shape the phone grid draws — equal columns, the last one
// 1.4 only when it is the 5-digit branch code (tinWeights) — and every digit box of it is one tinSegment recognises
// (digits only, per-box max length). One line per TIN item is printed for the record; the assertions keep the
// supported set honest as the schemas change.
import { expect, test } from "nsh/test";
import { bir } from "../../bir";
import { warmForms } from "../../bir/testing";
import { isTinItem, tinWeights, withoutHidden } from "./schema";
import { tinSegment } from "./controls/model";
import type { FormSchema } from "./contract";

warmForms();

test("every isTinItem across the 43 forms is a supported TIN shape", () => {
  const forms = bir.screen_forms_index();
  const shapes = new Set<string>();
  const typed: string[] = [];
  let count = 0;
  for (const form of forms) {
    const schema = withoutHidden(bir.screen_form_schema(form.c) as FormSchema);
    schema.parts.forEach((p, pi) =>
      p.items.forEach((it, ii) => {
        if (!isTinItem(it)) return;
        count++;
        const fs = it.f || [];
        const where = `${form.c} part=${pi} item=${ii}`;
        // The shape: how many boxes, each one's type and max length, and whether the filer types it.
        const shape = `${fs.length} box${fs.length === 1 ? "" : "es"}: ${fs.map((f) => `${f.t}${f.m ?? ""}`).join(" ")}${it.typed_tin ? " (typed)" : ""}`;
        shapes.add(shape);
        if (it.typed_tin) typed.push(where);
        // The grid's weights: 1 everywhere, 1.4 only on a last box whose max is the branch code's 5 digits.
        const weights = tinWeights(fs);
        expect(weights).toHaveLength(fs.length);
        fs.forEach((f, i) => {
          expect(weights[i]).toBe(i === fs.length - 1 && f.m === 5 ? 1.4 : 1);
        });
        // A TIN item's text/int boxes are either TIN digits (tinSegment) or named otherwise (a count, a name the
        // extractor grouped with the TIN): the digits ones must be recognised so the grid gives them number-pad.
        for (const f of fs) {
          if (!tinSegment(f)) console.log(`  ${where}: non-TIN box in a TIN item: ${f.k} "${f.l}"`);
        }
      }),
    );
  }
  for (const shape of [...shapes].sort()) console.log(`TIN shape: ${shape}`);
  for (const where of typed) console.log(`typed_tin: ${where}`);
  console.log(`TIN items across ${forms.length} forms: ${count}`);
  // The built set is far from empty; a schema regression that loses the TIN rows fails here.
  expect(count).toBeGreaterThan(30);
  expect(forms.length).toBe(43);
});
