// PROBE for PR 3: enumerate month+year pairs (no day) and from/to date groups on every built schema.
import { test } from "nsh/test";
import { bir } from "../../bir";
import { warmForms } from "../../bir/testing";
import type { FormSchema, SchemaField } from "./contract";
import { dateParts, withoutHidden } from "./schema";

warmForms();

const box = (f: SchemaField) => f.t !== "radio" && f.t !== "chk" && f.t !== "sel";
const label = (f: SchemaField) => `${f.l} ${String(f.k).split(":").pop()}`;
const namesDay = (f: SchemaField) => /\bday\b/i.test(f.l);

function findMonth(fs: SchemaField[]) {
  return fs.find((f) => box(f) && /month|\bmm\b/i.test(label(f)) && !/months|no\.? of/i.test(label(f)));
}
function findYear(fs: SchemaField[]) {
  return fs.find((f) => box(f) && /year|yyyy|\byr\b/i.test(label(f)) && !/month|years ended|no\.? of/i.test(label(f)) && !namesDay(f));
}
function findDay(fs: SchemaField[]) {
  return fs.find((f) => box(f) && namesDay(f) && !/days|month/i.test(label(f)))
    ?? fs.find((f) => box(f) && /\bday\b|\bdd\b|[a-z]day\b|day\d*$/i.test(label(f)) && !/days|month|year/i.test(label(f)));
}

test("enumerate period pairs and ranges", () => {
  for (const form of bir.screen_forms_index()) {
    const schema = withoutHidden(bir.screen_form_schema(form.c) as FormSchema);
    for (const part of schema.parts) for (const it of part.items) {
      const fs = (it.f || []).filter((f) => f.t !== "radio" && f.t !== "chk");
      if (!fs.length || it.rows) continue;
      const hasDateBox = fs.some((f) => f.t === "date");
      const dp = hasDateBox ? null : dateParts(fs);
      // month+year, no day: heuristic pair OR manual find
      const month = dp ? null : findMonth(fs);
      const year = dp ? null : findYear(fs);
      const day = dp ? null : (month && year ? findDay(fs) : null);
      const pair = dp ? null : (month && year && month !== year ? { month, day, year } : null);
      const selFields = fs.filter((f) => f.t === "sel");
      const fromTo = /from|to\b|start|end|begin|until|thru/i.test(it.l) || fs.some((f) => /from|to\b/i.test(f.l));
      const nums = fs.filter((f) => f.t !== "sel" && box(f));
      if (pair || (dp && !dp.day) || fromTo || nums.length >= 5) {
        console.log(`PROBE ${form.c} p${schema.parts.indexOf(part)} i${it.no ?? "?"} l="${it.l}" fields=${JSON.stringify(fs.map((f) => ({ k: f.k.split(":").pop(), l: f.l, t: f.t, c: f.c, r: f.r, n: (f as { n?: number }).n })))} dp=${dp ? `m:${dp.month.k.split(":").pop()} d:${dp.day?.k.split(":").pop() ?? "-"} y:${dp.year.k.split(":").pop()}` : "-"} pair=${pair ? `m:${pair.month.k.split(":").pop()} d:${pair.day?.k.split(":").pop() ?? "-"} y:${pair.year.k.split(":").pop()}` : "-"}`);
      }
    }
  }
});
