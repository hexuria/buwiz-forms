// PR 1 handoff-2 rework (R1-R7): a refusal sentence starts with the printed item number (it.no,
// never the row label's digits or the draft key); an impossible complete date beats the date-part
// range sentences; a split date shows every part's sentence; Queue with a refused edit standing
// refuses like a failed Check ("N items need attention."); and a shut/locked refusal puts the
// stored value back without holding Save at "Unsaved" or blocking Queue.
import { expect, test } from "nsh/test";
import { bir } from "../../bir";
import { DEMO, createDemoTaxpayer, warmForms } from "../../bir/testing";
import { FormSession, type FormTarget, type SessionHooks } from "./session";
import { refusalItem, refusalLabel, refusalSentence, type RefusalContext } from "./refusals";
import type { FormSchema, SchemaField, SchemaItem } from "./contract";

// The first form call of a process builds every form's schema; a test's JS call has 2 s on the headless runner, a file's load 10 s.
warmForms();

const at = (formCode: string, year: number, period: number): FormTarget => ({ tin: DEMO.tin, formCode, year, period, mode: "return" });

function hooks() {
  const said = { toasts: [] as string[] };
  const h: SessionHooks = {
    toast: (message) => {
      said.toasts.push(message);
      return `t${said.toasts.length}`;
    },
  };
  return { said, h };
}

async function opened(target: FormTarget, h?: SessionHooks): Promise<FormSession> {
  createDemoTaxpayer();
  const s = new FormSession(target, h);
  expect(await s.open()).toBe(null);
  await s.idle();
  return s;
}

test("R1: a refused 0605 Item 4 month names the printed item, not the row label", async () => {
  const s = await opened(at("0605", 2023, 1));
  const month = "frm0605:txtDueDateMonth";
  s.edit(month, "13");
  await s.idle();
  expect(s.boxMessageOf(month)).toBe("Item 4: the month is 1 to 12.");
  s.dispose();
});

test("R1 sweep: on every built page schema a refusal sentence starts with the printed item or the label, never the draft key", () => {
  const codes = bir.screen_forms_index().map((f) => f.c);
  expect(codes.length).toBe(43);
  let checked = 0;
  for (const code of codes) {
    const schema = bir.screen_form_schema(code) as FormSchema;
    for (const part of schema.parts || []) {
      for (const it of part.items || []) {
        for (const f of it.f || []) {
          const k = String((f as SchemaField).k || "");
          if (!k) continue;
          const item = refusalItem((it as SchemaItem).no);
          const label = refusalLabel((f as SchemaField).l, (it as SchemaItem).l, k);
          const ctx: RefusalContext = { raw: "month must be 01-12", key: k, item, label, kind: String((f as SchemaField).t || ""), form: code };
          const sentence = refusalSentence(ctx);
          const want = `${item ? `Item ${item}` : label}: the month is 1 to 12.`;
          expect([code, k, sentence]).toEqual([code, k, want]);
          expect(sentence.includes(k)).toBe(false);
          checked++;
        }
      }
    }
  }
  expect(checked).toBeGreaterThan(400);
});

test("R5: a split date shows every refused part's sentence in box order", async () => {
  const s = await opened(at("0605", 2023, 1));
  const month = "frm0605:txtDueDateMonth";
  const day = "frm0605:txtDueDateDay";
  s.edit(month, "13");
  await s.idle();
  s.edit(day, "45");
  await s.idle();
  const row = s.itemIdForKey(month)!;
  const shown = s.row(row)().boxMessages.map((b) => b.sentence);
  expect(shown).toEqual(["Item 4: the month is 1 to 12.", "Item 4: the day is 1 to 31."]);
  s.dispose();
});

test("R6: Queue with a refused edit standing refuses like a failed Check and counts it", async () => {
  const { said, h } = hooks();
  const s = await opened(at("0605", 2023, 1), h);
  const month = "frm0605:txtDueDateMonth";
  s.edit(month, "13");
  await s.idle();
  expect(s.boxMessageOf(month)).not.toBe("");
  said.toasts.length = 0;
  s.queue();
  await s.idle();
  const last = said.toasts[said.toasts.length - 1] || "";
  expect(last).toMatch(/^\d+ items? needs? attention\.$/);
  expect(last).not.toContain("month is 1 to 12");
  s.dispose();
});

test("R7: a shut box's refusal puts the stored value back and does not hold Save or Queue", async () => {
  const { said, h } = hooks();
  const s = await opened(at("0619E", 2025, 3), h);
  const k = "frm0619E:txtTax14";
  const stored = s.shownOf(k);
  s.edit(k, "100");
  await s.idle();
  // The sentence stays on the box, but the box shows the stored value again.
  expect(s.boxMessageOf(k)).toContain("used when Item 4 is Yes");
  expect(s.shownOf(k)).toBe(stored);
  expect(s.isDirty()).toBe(false);
  expect(s.saveState().state).not.toBe("unsaved");
  // Queue is not held by this refusal: nothing it says is the box's sentence.
  said.toasts.length = 0;
  s.queue();
  await s.idle();
  expect(said.toasts.join("\n")).not.toContain("used when Item 4 is Yes");
  s.dispose();
});
