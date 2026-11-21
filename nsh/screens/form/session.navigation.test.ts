import { expect, test } from "nsh/test";
import { bir } from "../../bir";
import { DEMO, createDemoTaxpayer, warmForms } from "../../bir/testing";
import { FormSession } from "./session";

warmForms();

async function opened() {
  createDemoTaxpayer();
  const fake = bir as unknown as { screen_form_open: typeof bir.screen_form_open };
  const real = fake.screen_form_open;
  fake.screen_form_open = async (...args) => {
    const r = await real(...args);
    return { ...r, errors: r.profile.map((key) => ({ field: key, keys: [key], message: "This profile value needs attention." })) };
  };
  const s = new FormSession({ tin: DEMO.tin, formCode: "2551Q", year: 2025, period: 1, mode: "return" });
  try {
    expect(await s.open()).toBe(null);
    await s.idle();
    return s;
  } catch (e) {
    s.dispose();
    throw e;
  } finally {
    fake.screen_form_open = real;
  }
}

test("previous and next to-do open the target part and its folded profile rows without folding other parts", async () => {
  const s = await opened();
  try {
    const pi = 1;
    const fold = s.part(pi)().fold!;
    expect(fold).not.toBeNull();
    const rows = s.todo().filter((row) => row.id.startsWith(`${pi}:`) && fold.items.includes(Number(row.id.split(":")[1])));
    expect(rows.length).toBeGreaterThan(1);
    const first = rows[0].id;
    const second = rows[1].id;
    for (const dir of [1, -1]) {
      s.jumpTo(pi, dir > 0 ? first : second);
      if (s.part(pi)().fold!.open) s.toggleProfileFold(pi);
      s.togglePart(pi);
      s.togglePart(0);
      expect(s.part(pi)().open).toBe(false);
      expect(s.part(pi)().fold!.open).toBe(false);
      const target = dir > 0 ? second : first;
      expect(s.stepTarget(dir)).toEqual({ part: pi, row: target });
      s.step(dir);
      expect(s.part(pi)().open).toBe(true);
      expect(s.part(pi)().fold!.open).toBe(true);
      expect(s.row(target)().tucked).toBe(false);
      expect(s.part(0)().open).toBe(true);
      expect(s.reveal()!.row).toBe(target);
    }
    s.toggleAllParts();
    expect(s.allPartsOpen()).toBe(true);
    s.step(1);
    expect(s.allPartsOpen()).toBe(true);
  } finally {
    s.dispose();
  }
});

test("a direct row jump unfolds its profile group; a part-only jump leaves it folded", async () => {
  const s = await opened();
  try {
    const ii = s.part(1)().fold!.items[0];
    const row = `1:${ii}`;
    s.jumpTo(0);
    const reveal = s.reveal();
    expect(s.stepTarget(-1)).toBeNull();
    s.step(-1);
    expect(s.reveal()).toBe(reveal);
    expect(s.part(1)().open).toBe(false);
    expect(s.part(1)().fold!.open).toBe(false);
    s.jumpTo(1);
    expect(s.part(1)().open).toBe(true);
    expect(s.part(1)().fold!.open).toBe(false);
    s.jumpTo(1, row);
    expect(s.part(1)().fold!.open).toBe(true);
    expect(s.row(row)().tucked).toBe(false);
    expect(s.part(0)().open).toBe(false);
  } finally {
    s.dispose();
  }
});
