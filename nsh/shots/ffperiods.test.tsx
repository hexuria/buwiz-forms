// Form-fields PR 3 shots: the month+year pairs as one MM/YYYY period field and the from/to pairs as
// one row of two date controls. `just nsh-shots ffpperiods` saves ~/BuwizNsh/evidence/ffperiods/*.png;
// BEFORE pictures come from the PR 2 worktree. Headless it takes none and still passes.
import { open, test } from "nsh/test";
import { shoot } from "./shoot";

const LOOKS = [
  { name: "420-light", width: 420, colorScheme: "light" as const },
  { name: "420-dark", width: 420, colorScheme: "dark" as const },
  { name: "1280-light", width: 1280, colorScheme: "light" as const },
  { name: "1280-dark", width: 1280, colorScheme: "dark" as const },
];

const ENTRY = "../FormAnyShot.tsx";
type App = Awaited<ReturnType<typeof open>>;
const options = (look: (typeof LOOKS)[number], formCode: string, year: number, period: number, mode: "return" | "sample" = "return") => ({
  width: look.width, height: 900, platform: "macos" as const, colorScheme: look.colorScheme,
  host: { params: { formCode, year, period, mode } },
});

/** Scroll until `sel` shows (the page virtualizes; a real window's press scrolls, headless stays put). */
async function untilShown(app: App, sel: string, steps = 24) {
  for (let step = 0; step < steps && !app.shows(sel); step++) {
    try {
      await app.scroll("id:form-scroll", 300);
    } catch {
      break;
    }
    await app.wait(200);
  }
  await app.waitFor(sel, { timeout: 15_000 });
}

/** Jump through the sections list part by part until `sel` shows (deep items sit in closed parts). */
async function jumpUntil(app: App, sel: string, parts: number) {
  for (let i = 0; i < parts && !app.shows(sel); i++) {
    await app.press("id:fp-pill-list");
    await app.waitFor("id:fp-sections", { timeout: 10_000 });
    await app.press(`id:fp-section-${i}`);
    await app.wait(400);
    for (let step = 0; step < 10 && !app.shows(sel); step++) {
      try {
        await app.scroll("id:form-scroll", 300);
      } catch {
        break;
      }
      await app.wait(200);
    }
  }
  await app.waitFor(sel, { timeout: 15_000 });
}

/** Jump to a part through the pill's sections list: the row press closes the list and opens the part. */
async function jumpToPart(app: App, part: number) {
  await app.press("id:fp-pill-list");
  await app.waitFor("id:fp-sections", { timeout: 10_000 });
  await app.press(`id:fp-section-${part}`);
  await app.wait(400);
}

for (const look of LOOKS) {
  test(`periods ${look.name}`, async () => {
    const narrow = look.width < 600;

    // 0605 Item 2: the year is the return's own — locked — beside the month picker.
    const c0605 = await open(ENTRY, options(look, "0605", 2024, 11));
    await c0605.waitFor("id:fp-frm0605:itemYearEndMonth#period", { timeout: 30_000 });
    await untilShown(c0605, "id:frm0605:itemYearEndMonth#period-calendar");
    await c0605.wait(400);
    await shoot(c0605, `0605-locked-year-${look.name}`);

    // 0619E sample: month and year both open — the whole MM/YYYY field, and a bad text's sentence.
    const smp = await open(ENTRY, options(look, "0619E", 0, 0, "sample"));
    await smp.waitFor("id:fp-frm0619E:txtMonth#period", { timeout: 30_000 });
    await untilShown(smp, "id:fp-frm0619E:txtMonth#period");
    await smp.wait(400);
    await shoot(smp, `open-pair-${look.name}`);
    const bad = "id:fp-frm0619E:txtMonth#period";
    const badCal = "frm0619E:txtMonth#period-calendar";
    await smp.type(bad, "13/2024");
    // Blur through the picker trigger (it opens the grid/sheet), then close it again for a clean shot.
    await smp.press(`id:${badCal}`);
    await smp.wait(400);
    if (narrow) {
      await smp.press(`id:${badCal}-done`).catch(() => {});
      await smp.waitFor(`id:${badCal}-sheet`, { state: "gone", timeout: 10_000 }).catch(() => {});
    } else {
      await smp.press(`id:${badCal}`).catch(() => {});
      await smp.wait(300);
    }
    await smp.waitFor("text:/1 to 12|MM\\/YYYY/", { timeout: 10_000 });
    await shoot(smp, `period-bad-${look.name}`);

    // 1702EX under Fiscal: the open month with its locked two-digit year, the month grid open.
    const ex = await open(ENTRY, options(look, "1702EX", 2024, 0));
    await ex.waitFor("id:fp-frm1702EX:ddlPg1I2Date#period", { timeout: 30_000 });
    await ex.press("id:fp-frm1702EX:rdoPg1I1Fiscal").catch(() => {});
    await ex.wait(500);
    await shoot(ex, `1702ex-open-${look.name}`);
    const cal = "frm1702EX:ddlPg1I2Date#period-calendar";
    await ex.press(`id:${cal}`);
    await ex.waitFor(`id:${cal}-m6`, { timeout: 10_000 });
    await ex.wait(300);
    await shoot(ex, `period-picker-${look.name}`);

    // 1702EX Item 17: the effectivity range. Desktop typing takes a reversed pair (its sentence on
    // the To end); the touch sheets bound each end by the other, so narrow enters a valid pair.
    const rg = await open(ENTRY, options(look, "1702EX", 2024, 0));
    await rg.waitFor("id:fp-check", { timeout: 30_000 });
    // Item 17 sits in Part I (index 1); the sections row opens just that part.
    await jumpToPart(rg, 1);
    await rg.press("id:fp-frm1702EX:rdoPg1I1Calendar").catch(() => {});
    await rg.wait(400);
    const from = "id:fp-frm1702EX:txtPg1Pt1I17EffectivityFrom";
    const to = "id:fp-frm1702EX:txtPg1Pt1I17EffectivityTo";
    await untilShown(rg, to);
    if (narrow) {
      await rg.press(from);
      await rg.waitFor(`${from}-sheet-input`);
      await rg.type(`${from}-sheet-input`, "11/01/2024");
      await rg.press(`${from}-sheet-done`);
      await rg.waitFor(`${from}-sheet-input`, { state: "gone" });
      // The close's blur flushes the draft; wait for the host's answer (the empty To's sentence)
      // so no in-flight update lands on the next sheet's first frame.
      await rg.waitFor("text:/Please enter a valid date/", { timeout: 15_000 });
      await rg.press(to);
      await rg.waitFor(`${to}-sheet-input`);
      await rg.type(`${to}-sheet-input`, "12/15/2024");
      await rg.press(`${to}-sheet-done`);
      await rg.waitFor(`${to}-sheet-input`, { state: "gone" });
      await rg.wait(400);
      await shoot(rg, `range-row-${look.name}`);
    } else {
      await rg.type(from, "06/01/2024");
      await rg.type(to, "01/15/2024");
      // The inputs take no press: blur the To end through its calendar trigger (open, then shut).
      const toCal = "id:fp-frm1702EX:txtPg1Pt1I17EffectivityTo-calendar";
      await rg.press(toCal);
      await rg.waitFor("text:/end date is before the start date/", { timeout: 15_000 });
      await rg.press(toCal).catch(() => {});
      await rg.wait(300);
      await shoot(rg, `range-reversed-${look.name}`);
    }

    // 1600WP Item 1: the withholding range — two one-date controls with "to" between.
    const wp = await open(ENTRY, options(look, "1600WP", 0, 0, "sample"));
    await wp.waitFor("id:fp-check", { timeout: 30_000 });
    await untilShown(wp, "id:fp-frm1600WP:DateWithholdingToMonth#date");
    await wp.wait(400);
    await shoot(wp, `1600wp-range-${look.name}`);

    // 2550Q Item 4: the return-period range, locked to the quarter.
    const q = await open(ENTRY, options(look, "2550Q", 2025, 4));
    await q.waitFor("id:fp-frm2550qv2024:RtnPeriodFromNo4", { timeout: 30_000 });
    await untilShown(q, "id:fp-frm2550qv2024:RtnPeriodToNo4", 12);
    await q.wait(400);
    await shoot(q, `2550q-range-${look.name}`);

    // 1701MS Item 16: two ranges on one item.
    const ms = await open(ENTRY, options(look, "1701MS", 2024, 0));
    await ms.waitFor("id:fp-check", { timeout: 30_000 });
    await jumpUntil(ms, "id:fp-frm1701MS:txtTaxpayerNo16a", 8);
    await ms.wait(400);
    await shoot(ms, `1701ms-ranges-${look.name}`);
  }, 300_000);
}
