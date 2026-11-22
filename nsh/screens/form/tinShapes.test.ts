// PR 8 (Q8): every TIN row of the 43 built schemas is a shape the phone grid draws — equal columns, the last one
// 1.4 only when it is the 5-digit branch code (tinWeights) — and every digit box of it is one tinSegment recognises
// (digits only, per-box max length). One line per TIN item is printed for the record; the assertions keep the
// supported set honest as the schemas change.
import { expect, test } from "nsh/test";
import { bir } from "../../bir";
import { warmForms } from "../../bir/testing";
import { isTinItem, tinWeights, withoutHidden } from "./schema";
import { readBoxPad, tinCellRoom, tinSegment } from "./controls/model";
import type { FormSchema, SchemaItem } from "./contract";

warmForms();

test("every isTinItem across the 43 forms is a supported TIN shape", () => {
  const forms = bir.screen_forms_index();
  const shapes = new Set<string>();
  const typed: string[] = [];
  const lost: string[] = [];
  let count = 0;
  // The pre-PR-8 predicate: named, three to five text/int boxes, no per-field TIN-name check. Anything it caught
  // that the new one does not is a TIN row this change would silently stop drawing as one — the sweep says so.
  const oldTinItem = (it: SchemaItem): boolean => {
    const fs = it.f || [];
    const named = /\bTIN\b|Identification/i.test(it.l) || fs.some((x) => /\bTIN\d?\b|txtTIN/i.test(`${x.l} ${x.k}`));
    return named && fs.length >= 3 && fs.length <= 5 && fs.every((x) => x.t === "text" || x.t === "int");
  };
  for (const form of forms) {
    const schema = withoutHidden(bir.screen_form_schema(form.c) as FormSchema);
    schema.parts.forEach((p, pi) =>
      p.items.forEach((it, ii) => {
        if (oldTinItem(it) && !isTinItem(it)) lost.push(`${form.c} part=${pi} item=${ii} "${it.l}"`);
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
  for (const where of lost) console.log(`old-shape item the new predicate drops: ${where}`);
  console.log(`TIN items across ${forms.length} forms: ${count}`);
  // The built set is far from empty; a schema regression that loses the TIN rows fails here. The old predicate also
  // caught two 1702RT signature rows (a TIN-ish field among name/signature boxes is not a TIN row): dropping them is
  // the fix, so they are the whole allowed loss — any other dropped item fails here for review.
  const allowedLoss = new Set(["1702RT part=2 item=9", "1702RT part=2 item=10"]);
  expect(count).toBeGreaterThan(30);
  expect(forms.length).toBe(43);
  expect(lost.filter((w) => !allowedLoss.has(w.split(" \"")[0]))).toEqual([]);
});

test("P8 320: a locked cell of the phone TIN grid has room for its three digits", () => {
  // On a 320 pt phone the grid is 226 pt wide (the real-window shot), so a three-digit column is 47 pt. With the
  // read-only box's usual 10 pt sides that left 25 pt, and "000" and "453" drew as "0…" and "4…": three 13 pt
  // tabular digits of Inter take just over 25 pt. The 5-digit branch code needs five thirds of that.
  const digit = 26 / 3;
  const room = tinCellRoom(226, [1, 1, 1, 1.4], readBoxPad(true));
  expect(room[0]).toBeGreaterThanOrEqual(3 * digit);
  expect(room[3]).toBeGreaterThanOrEqual(5 * digit);
  // Every other read-only box keeps its 10 pt sides.
  expect(readBoxPad(false)).toBe(10);
});
