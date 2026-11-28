import { expect, open, test } from "nsh/test";

// The page's "Computed from other items" mark sits on exactly the open penalty boxes
// whose component the filer never typed. A shut box carries no mark: its own lock and
// its reason already say the amounts are computed. An ordinary box is never wrapped.
test("no penalty mark on a shut box on 1701", async () => {
  const app = await open("./FormPageAny.tsx", {
    width: 1440, height: 900, platform: "macos", clock: {},
    host: { params: { formCode: "1701", year: 2025, period: 0, openParts: "all" } },
  });
  await app.waitFor("text:BIR Form 1701");
  const box = "id:fp-frm1701:txtPg1I27A";
  for (let i = 0; i < 40 && !app.shows(box); i++) await app.scroll("id:form-scroll", 800);
  expect(app.count(box)).toBe(1);
  // Untouched, the group is locked: the boxes are shut, and no mark and no wrapper show.
  for (const k of ["frm1701:txtPg1I27A", "frm1701:txtPg1I28A", "frm1701:txtPg1I29A"]) {
    expect(app.count(`id:fp-mark-lock-${k}`)).toBe(0);
    expect(app.count(`id:fp-mark-info-${k}`)).toBe(0);
    expect(app.count(`id:fp-penrow-${k}`)).toBe(0);
  }
  // Rows mount as they scroll into view (the list recycles): Item 29 is below Item 27.
  for (let i = 0; i < 12 && !app.shows("id:fp-frm1701:txtPg1I29A"); i++) await app.scroll("id:form-scroll", 300);
  expect(app.count("id:fp-mark-info-frm1701:txtPg1I29A")).toBe(0);
  // A non-penalty box is never wrapped: Item 14's foreign tax number is in the same part.
  expect(app.count("id:fp-penrow-frm1701:txtPg1I14ForeignTaxNumber")).toBe(0);
});

test("no penalty mark on a shut box on 1701 phone", async () => {
  const app = await open("./FormPageAny.tsx", {
    host: { params: { formCode: "1701", year: 2025, period: 0, openParts: "all" } },
  });
  await app.waitFor("text:BIR Form 1701");
  const box = "id:fp-frm1701:txtPg1I27A";
  for (let i = 0; i < 40 && !app.shows(box); i++) await app.scroll("id:form-scroll", 800);
  expect(app.count(box)).toBe(1);
  expect(app.count("id:fp-mark-info-frm1701:txtPg1I27A")).toBe(0);
  expect(app.count("id:fp-penrow-frm1701:txtPg1I27A")).toBe(0);
  // An ordinary phone box keeps the row's full width: no wrapper on Item 14's foreign tax number.
  expect(app.count("id:fp-penrow-frm1701:txtPg1I14ForeignTaxNumber")).toBe(0);
});

// The live flow: Edit penalties opens the group — its untyped boxes carry the mark with
// the info sign, never a lock beside a box the filer can type in. The filer types the
// surcharge (Item 28); its mark lifts once the edit's round trip lands, and the interest
// (27) and compromise (29) boxes keep theirs.
test("penalty mark lifts on the typed component", async () => {
  const app = await open("./FormPageAny.tsx", {
    width: 1440, height: 900, platform: "macos", clock: {},
    host: { params: { formCode: "1701", year: 2025, period: 0, openParts: "all" } },
  });
  await app.waitFor("text:BIR Form 1701");
  const box = "id:fp-frm1701:txtPg1I28A";
  for (let i = 0; i < 40 && !app.shows(box); i++) await app.scroll("id:form-scroll", 800);
  expect(app.count(box)).toBe(1);
  expect(app.count("id:fp-mark-info-frm1701:txtPg1I28A")).toBe(0);
  await app.press("id:fp-penalty-2:6");
  // Open and untyped: the wrapper and the info mark show; never a lock icon.
  for (let i = 0; i < 40 && app.count("id:fp-mark-info-frm1701:txtPg1I28A") !== 1; i++) await app.wait(250);
  expect(app.count("id:fp-mark-info-frm1701:txtPg1I28A")).toBe(1);
  expect(app.count("id:fp-penrow-frm1701:txtPg1I28A")).toBe(1);
  expect(app.count("id:fp-mark-lock-frm1701:txtPg1I28A")).toBe(0);
  // Clear the prefilled 0.00 first: app.type appends.
  await app.clear(box);
  await app.type(box, "1000");
  await app.waitFor("text:✓ Saved", { timeout: 15_000 }).catch(() => {});
  for (let i = 0; i < 40 && app.count("id:fp-mark-info-frm1701:txtPg1I28A") !== 0; i++) await app.wait(250);
  expect(app.count("id:fp-mark-info-frm1701:txtPg1I28A")).toBe(0);
  expect(app.count("id:fp-mark-info-frm1701:txtPg1I27A")).toBe(1);
  expect(app.count("id:fp-mark-info-frm1701:txtPg1I29A")).toBe(1);
  // The mark sits in the box's own trailing slot, so its lifting is only the slot emptying:
  // the box itself is the same mounted input throughout — there is no wrapper to remount.
});

// On a joint 1701 the money rows print in two columns: A. Taxpayer/Filer, then B. Spouse. The
// mark must never move a box: opening the penalty group — its marks showing on the untyped
// boxes — leaves every box of Items 22 to 30 at exactly the x it had while the group was shut
// (the rows report their cells' layout x through the fixture's probe).
test("joint 1701: the penalty rows' boxes keep the columns of the rows above", async () => {
  const app = await open("./FormPageAny.tsx", {
    width: 1440, height: 900, platform: "macos", clock: {},
    host: {
      params: {
        formCode: "1701", year: 2025, period: 0, openParts: "all",
        setup: [
          { pick: "frm1701:rdoPg1I16CivilStatusM" },
          { pick: "frm1701:rdoPg1I17SpouseIncomeYes" },
          { pick: "frm1701:rdoPg1I18FilingStatusJ" },
        ],
      },
    },
  });
  await app.waitFor("text:BIR Form 1701");
  const xs = () => {
    const raw = JSON.parse((app.texts().find((t) => t.startsWith("boxx ")) ?? "boxx {}").slice("boxx ".length)) as Record<string, { x: number; width: number }>;
    return Object.fromEntries(Object.entries(raw).map(([k, v]) => [k.replace(/^box:/, ""), v.x]));
  };
  const keysOf = (at: Record<string, number>) =>
    Object.keys(at).filter((k) => /frm1701:txtPg1I2[2-9][AB]/.test(k) || /frm1701:txtPg1I30[AB]/.test(k));
  for (let i = 0; i < 40 && !app.shows("id:fp-frm1701:txtPg1I28A"); i++) await app.scroll("id:form-scroll", 800).catch(() => {});
  expect(app.count("id:fp-frm1701:txtPg1I28A")).toBe(1);
  for (let i = 0; i < 20 && keysOf(xs()).length < 10; i++) await app.wait(250);
  const shut = xs();
  const keys = keysOf(shut);
  expect(keys.length).toBeGreaterThanOrEqual(10);
  // Open the group: the marks show on its untyped boxes.
  await app.press("id:fp-penalty-2:6");
  for (let i = 0; i < 40 && app.count("id:fp-mark-info-frm1701:txtPg1I28A") !== 1; i++) await app.wait(250);
  expect(app.count("id:fp-mark-info-frm1701:txtPg1I28A")).toBe(1);
  for (let i = 0; i < 20; i++) await app.wait(100);
  const opened = xs();
  for (const k of keys) {
    expect(opened[k]).not.toBeUndefined();
    expect(Math.abs((opened[k] ?? 0) - shut[k])).toBeLessThan(0.5);
  }
  // And the penalty rows share the columns of the money rows above: Item 27's boxes start where
  // Item 25's start, the row right above them whose boxes are the same width.
  for (const col of ["A", "B"]) {
    const penalty = opened[`frm1701:txtPg1I27${col}`];
    const above = opened[`frm1701:txtPg1I25${col}`];
    expect(Math.abs((penalty ?? 0) - (above ?? 0))).toBeLessThan(0.5);
  }
});

// P1: the mark of an untyped computed penalty box sits inside the box's trailing slot, so it can never
// cover a digit of the computed amount it explains: with a nonzero computed interest on the row, the
// input reads the whole amount and the mark's frame starts at or after the input frame's end (the box's
// flex row gives the mark its own room, as the shut box's lock has).
test("the penalty mark never covers the computed amount", async () => {
  const app = await open("./FormPageAny.tsx", {
    width: 1440, height: 900, platform: "macos", clock: {},
    host: {
      params: {
        formCode: "1701", year: 2025, period: 0, openParts: "all",
        setup: [
          { pick: "frm1701:rdoPg1I16CivilStatusM" },
          { pick: "frm1701:rdoPg1I17SpouseIncomeYes" },
          { pick: "frm1701:rdoPg1I18FilingStatusJ" },
          { pick: "frm1701:rdoPg1I6TaxpayerTypeS" },
          { pick: "frm1701:rdoPg1I7ATC_II012" },
          { pick: "frm1701:rdoPg1I21AMethodDeductionO" },
          { edits: { "frm1701:txtPg2IShed3_8A": "1250000.50" } },
        ],
      },
    },
  });
  await app.waitFor("text:BIR Form 1701");
  const frames = () =>
    JSON.parse((app.texts().find((t) => t.startsWith("boxx ")) ?? "boxx {}").slice("boxx ".length)) as Record<string, { x: number; width: number }>;
  for (let i = 0; i < 40 && !app.shows("id:fp-frm1701:txtPg1I28A"); i++) await app.scroll("id:form-scroll", 800).catch(() => {});
  expect(app.count("id:fp-frm1701:txtPg1I28A")).toBe(1);
  await app.press("id:fp-penalty-2:6");
  for (let i = 0; i < 40 && app.count("id:fp-mark-info-frm1701:txtPg1I28A") !== 1; i++) await app.wait(250);
  expect(app.count("id:fp-mark-info-frm1701:txtPg1I28A")).toBe(1);
  // The computed interest is nonzero (the return is months late on a nonzero tax due) and reads whole.
  let value = "";
  for (let i = 0; i < 40; i++) {
    value = app.value("id:fp-frm1701:txtPg1I27A") ?? "";
    if (value && value !== "0.00" && value !== "0") break;
    await app.wait(250);
  }
  expect(value).not.toBe("");
  expect(value).not.toBe("0.00");
  // The frames: the mark inside the box's room, never overlapping the input's frame.
  let at = frames();
  for (let i = 0; i < 40 && (!at["input:fp-frm1701:txtPg1I27A"] || !at["mark:frm1701:txtPg1I27A"]); i++) {
    await app.wait(250);
    at = frames();
  }
  const input = at["input:fp-frm1701:txtPg1I27A"];
  const mark = at["mark:frm1701:txtPg1I27A"];
  expect(input).not.toBeUndefined();
  expect(mark).not.toBeUndefined();
  expect(input.x + input.width).toBeLessThanOrEqual(mark.x + 0.5);
  // The same holds of Item 28's box.
  const input28 = at["input:fp-frm1701:txtPg1I28A"];
  const mark28 = at["mark:frm1701:txtPg1I28A"];
  if (input28 && mark28) expect(input28.x + input28.width).toBeLessThanOrEqual(mark28.x + 0.5);
});
