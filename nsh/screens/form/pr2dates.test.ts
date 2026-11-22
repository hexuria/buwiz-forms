import { expect, test } from "nsh/test";
import { bir } from "../../bir";
import { createDemoTaxpayer, DEMO, warmForms } from "../../bir/testing";
import type { FormSchema, SchemaItem } from "./contract";
import { dateParts, withoutHidden } from "./schema";
import { FormSession } from "./session";

warmForms();

type MarkedItem = SchemaItem & { one_dates?: string[][]; date_ranges?: string[][] };

test("every built three-part date is registered; the 1600WP range ends are one-dates inside their date_ranges mark", () => {
  let registered = 0;
  let ranges = 0;
  for (const form of bir.screen_forms_index()) {
    const schema = withoutHidden(bir.screen_form_schema(form.c) as FormSchema);
    for (const part of schema.parts) for (const item of part.items) {
      // PR 3, R1: a date_ranges item's ends are one-date triples or whole-date boxes; the page draws the pair, not the parts.
      if ((item as MarkedItem).date_ranges?.length) {
        ranges += (item as MarkedItem).date_ranges!.length;
        continue;
      }
      const date = dateParts(item.f || []);
      if (!date?.day) continue;
      const triple = [date.month.k, date.day.k, date.year.k];
      const marked = (item as MarkedItem).one_dates || [];
      expect(`${form.c} ${item.no}: ${marked.some((keys) => keys.join("|") === triple.join("|"))}`).toBe(`${form.c} ${item.no}: true`);
      registered++;
    }
  }
  expect(ranges).toBe(6);
  expect(registered).toBeGreaterThan(19);
});

test("0605 one date accepts typed digits and keeps all three stored parts", async () => {
  createDemoTaxpayer();
  const session = new FormSession({ tin: DEMO.tin, formCode: "0605", year: 2023, period: 1, mode: "return" });
  try {
    expect(await session.open()).toBe(null);
    const month = "frm0605:txtDueDateMonth";
    const key = `${month}#date`;
    for (let i = 1; i <= 8; i++) session.edit(key, "08172023".slice(0, i));
    session.blur(key);
    await session.idle();
    expect(session.shownOf(key)).toBe("August 17, 2023");
    expect([session.valueOf(month), session.valueOf("frm0605:txtDueDateDay"), session.valueOf("frm0605:txtDueDateYear")]).toEqual(["08", "17", "2023"]);
  } finally {
    session.dispose();
  }
});

test("an incomplete existing one-date edit stays unsaved and blocks Queue", async () => {
  createDemoTaxpayer();
  const messages: string[] = [];
  const session = new FormSession(
    { tin: DEMO.tin, formCode: "1706", year: 2025, period: 0, mode: "return" },
    { toast: (text) => { messages.push(text); } },
  );
  try {
    expect(await session.open()).toBe(null);
    const month = "frm1706:txtDateMonthI";
    const key = `${month}#date`;
    session.edit(key, "08/17/19");
    session.blur(key);
    await session.idle();
    expect(session.boxMessageOf(month)).toBe("Item 7: type the four-digit year, for example 08/17/1988.");
    expect(session.shownOf(key)).toBe("08/17/19");
    expect(session.isDirty()).toBe(true);
    expect(session.saveState().state).toBe("unsaved");
    session.queue();
    await session.idle();
    expect(messages[messages.length - 1]).toMatch(/^\d+ items? needs? attention\.$/);
  } finally {
    session.dispose();
  }
});

test("a refused one-date does not double-count the required-date sentence", async () => {
  createDemoTaxpayer();
  const session = new FormSession({ tin: DEMO.tin, formCode: "0605", year: 2023, period: 9, mode: "return" });
  try {
    expect(await session.open()).toBe(null);
    await session.idle();
    session.check();
    await session.idle();
    const before = session.attention();
    session.edit("frm0605:txtDueDateMonth#date", "08/17/1");
    session.blur("frm0605:txtDueDateMonth#date");
    session.check();
    await session.idle();
    expect(session.attention()).toBe(before);
    const item = session.itemIdForKey("frm0605:txtDueDateMonth")!;
    expect(session.row(item)().boxMessages.length).toBe(1);
    expect(session.row(item)().message).toBe("");
  } finally {
    session.dispose();
  }
});

test("0605 typed formats, refusal, correction and reopen keep the three draft keys", async () => {
  createDemoTaxpayer();
  const target = { tin: DEMO.tin, formCode: "0605", year: 2023, period: 2, mode: "return" as const };
  const session = new FormSession(target);
  try {
    expect(await session.open()).toBe(null);
    const month = "frm0605:txtDueDateMonth";
    const key = `${month}#date`;
    for (const text of ["08171988", "08/17/1988", "08-17-1988", "1988-08-17", "Aug 17 1988", " 08 - 17 - 1988 "]) {
      session.focus(key);
      for (let i = 1; i <= text.length; i++) session.edit(key, text.slice(0, i));
      session.blur(key);
      await session.idle();
      expect(session.shownOf(key)).toBe("August 17, 1988");
      expect([session.valueOf(month), session.valueOf("frm0605:txtDueDateDay"), session.valueOf("frm0605:txtDueDateYear")]).toEqual(["08", "17", "1988"]);
    }
    session.edit(key, "08/17/88");
    session.blur(key);
    await session.idle();
    expect(session.shownOf(key)).toBe("08/17/88");
    expect(session.box(key).invalid()).toBe(true);
    expect(session.boxMessageOf(month)).toContain("four-digit year");
    expect(session.valueOf("frm0605:txtDueDateYear")).toBe("1988");
    session.edit(key, "08/17/1988");
    session.blur(key);
    await session.idle();
    expect(session.boxMessageOf(month)).not.toContain("four-digit year");
    session.saveNow();
    await session.idle();
  } finally {
    session.dispose();
  }
  const reopened = new FormSession(target);
  try {
    expect(await reopened.open()).toBe(null);
    expect(reopened.shownOf("frm0605:txtDueDateMonth#date")).toBe("August 17, 1988");
    reopened.edit("frm0605:txtDueDateMonth#date", "");
    reopened.blur("frm0605:txtDueDateMonth#date");
    await reopened.idle();
    expect(reopened.shownOf("frm0605:txtDueDateMonth#date")).toBe("");
  } finally {
    reopened.dispose();
  }
});

test("0619E one-date typing respects its locked month and year", async () => {
  createDemoTaxpayer();
  const session = new FormSession({ tin: DEMO.tin, formCode: "0619E", year: 2025, period: 4, mode: "return" });
  try {
    expect(await session.open()).toBe(null);
    const month = "frm0619E:txtDueMonth";
    const key = `${month}#date`;
    session.edit(key, "06/15/2025");
    session.blur(key);
    await session.idle();
    expect(session.boxMessageOf(month)).toContain("May 1, 2025 to May 31, 2025");
    expect(session.valueOf("frm0619E:txtDueDay")).not.toBe("15");
    session.edit(key, "05/15/2025");
    session.blur(key);
    await session.idle();
    expect(Number(session.valueOf("frm0619E:txtDueDay"))).toBe(15);
    expect(session.shownOf(key)).toBe("May 15, 2025");
  } finally {
    session.dispose();
  }
});
