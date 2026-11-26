// G1 (form-fields PR 7): a box another answer keeps shut is not drawn — the specify box, the gated row, the
// spouse part — and it comes back in place, in schema order, when the opener is answered. A shut box that still
// holds a value stays, faded, with its "Needs …" line. `just nsh-shots ffgated` saves the PNGs to
// ~/BuwizNsh/evidence/ffgated/; headless it takes none and still passes.
import { open, test } from "nsh/test";
import type { App } from "nsh/test";
import { LOOKS, shoot } from "./shoot";

const ANY = "../FormAnyShot.tsx";

/** Jump to a part the way the page's own navigator does (the wide rail, or the pill's sections sheet when narrow). */
async function jump(app: App, pi: number, wide: boolean): Promise<void> {
  if (wide) {
    await app.press(`id:fp-nav-${pi}`);
  } else {
    await app.press("id:fp-pill-open");
    await app.waitFor(`id:fp-section-${pi}`, { timeout: 10_000 });
    await app.press(`id:fp-section-${pi}`);
  }
  await app.wait(400);
}

/** Scroll down until `sel` is drawn (a part list mounts the rows near the page's viewport). */
async function reach(app: App, sel: string): Promise<void> {
  for (let i = 0; i < 40 && !app.shows(sel); i++) await app.scroll("id:form-scroll", 320).catch(() => {});
  await app.waitFor(sel, { timeout: 20_000 });
}

for (const look of LOOKS) {
  const wide = look.width > 600;

  test(`g1 0619E item 14: the whole row is absent until Item 4 = Yes, then shut again on No (${look.name})`, async () => {
    const app = await open(ANY, { ...look, host: { params: { formCode: "0619E", year: 2025, period: 3, mode: "return" } } });
    await app.waitFor("id:fp-part-0-toggle", { timeout: 30_000 });
    // Part 2 with item 14 shut: no row, no box, the part counts the rows that draw.
    await jump(app, 2, wide);
    await app.wait(300);
    await shoot(app, `g1-0619e-14-closed-${look.name}`);
    // Item 4 = Yes opens it, in place at the top of part 2.
    await jump(app, 0, wide);
    await reach(app, "id:fp-frm0619E:optWithheld:Y");
    await app.press("id:fp-frm0619E:optWithheld:Y");
    await jump(app, 2, wide);
    await reach(app, "id:fp-item-2:0");
    await app.clear("id:fp-frm0619E:txtTax14");
    await app.type("id:fp-frm0619E:txtTax14", "1250.50");
    await app.wait(400);
    await shoot(app, `g1-0619e-14-open-${look.name}`);
    // Answer No again: the host zeroes a gated amount, so the row is gone again.
    await jump(app, 0, wide);
    await reach(app, "id:fp-frm0619E:optWithheld:N");
    await app.press("id:fp-frm0619E:optWithheld:N");
    await jump(app, 2, wide);
    await app.waitFor("id:fp-item-2:0", { state: "gone", timeout: 20_000 });
    await app.wait(300);
    await shoot(app, `g1-0619e-14-shut-${look.name}`);
  }, 180_000);

  test(`g1 0619E item 14: a refused write keeps the shut box drawn with the host's sentence (${look.name})`, async () => {
    // The write a stale page or an agent could still send: the host refuses it (B16), and the sentence
    // keeps the gated box — and its row — drawn, so no message points at nothing.
    const app = await open(ANY, {
      ...look,
      host: { params: { formCode: "0619E", year: 2025, period: 3, mode: "return", edits: { "frm0619E:txtTax14": "500" } } },
    });
    await app.waitFor("id:fp-part-0-toggle", { timeout: 30_000 });
    await jump(app, 2, wide);
    await reach(app, "id:fp-item-2:0");
    await app.waitFor("text:/used when Item 4 is Yes/i", { timeout: 20_000 });
    await app.wait(300);
    await shoot(app, `g1-0619e-14-refused-${look.name}`);
  }, 180_000);

  test(`g1 1700: the spouse chain opens one level at a time — Married, spouse income, joint filing, item 24 (${look.name})`, async () => {
    const app = await open(ANY, { ...look, host: { params: { formCode: "1700", year: 2025, period: 1, mode: "return" } } });
    await app.waitFor("id:fp-part-0-toggle", { timeout: 30_000 });
    // Every spouse item is gated: the part draws its header at 0 items and no rows.
    await jump(app, 2, wide);
    await app.wait(300);
    await shoot(app, `g1-1700-part2-closed-${look.name}`);
    // Married opens item 16 only; spouse income = Yes opens item 17; Joint Filing opens the spouse part.
    await jump(app, 1, wide);
    await reach(app, "id:fp-item-1:12");
    await app.press("id:fp-frm1700:rdoPg1I15CivilStatusM");
    await reach(app, "id:fp-item-1:13");
    await app.press("id:fp-frm1700:rdoPg1I16SpouseHasIncomeY");
    await reach(app, "id:fp-item-1:14");
    await app.press("id:fp-frm1700:rdoPg1I17FilingStatusJ");
    await jump(app, 2, wide);
    await reach(app, "id:fp-item-2:0");
    // Seven of the eight spouse items draw; item 25 still waits on item 24 = Yes.
    await app.wait(300);
    await shoot(app, `g1-1700-part2-open-${look.name}`);
    await reach(app, "id:fp-item-2:6");
    await app.press("id:fp-frm1700:rdoPg1I24SpouseForeignTaxCreditsYes");
    await reach(app, "id:fp-item-2:7");
    await app.wait(300);
    await shoot(app, `g1-1700-part2-full-${look.name}`);
  }, 240_000);

  test(`g1 0605 item 17: the specify box and the approval pair follow their manner picks (${look.name})`, async () => {
    const app = await open(ANY, { ...look, host: { params: { formCode: "0605", year: 2025, period: 1, mode: "return" } } });
    await app.waitFor("id:fp-part-0-toggle", { timeout: 30_000 });
    await jump(app, 1, wide);
    await reach(app, "id:fp-item-1:8");
    await shoot(app, `g1-0605-17-closed-${look.name}`);
    // Others (Specify) opens the specify box inside the same row (seven choices: a dropdown).
    await app.press("id:fp-radio-1:8-trigger");
    await app.press("text:/Others \\(Specify\\)/i");
    await app.waitFor("id:fp-frm0605:txtOthersName", { timeout: 20_000 });
    await app.wait(300);
    await shoot(app, `g1-0605-17-others-${look.name}`);
    // A B-manner opens the Investigating Office approval pair — its own row in the last part.
    await app.press("id:fp-radio-1:8-trigger");
    await app.press("text:/Deficiency Tax/");
    await jump(app, 3, wide);
    await reach(app, "id:fp-item-3:0");
    await app.waitFor("id:fp-frm0605:itemApprovedYN:_1", { timeout: 20_000 });
    await app.wait(300);
    await shoot(app, `g1-0605-17-approved-${look.name}`);
  }, 180_000);

  test(`g1 2551Q item 12: the tax-relief specify box follows the Yes radio (${look.name})`, async () => {
    const app = await open(ANY, { ...look, host: { params: { formCode: "2551Q", year: 2025, period: 1, mode: "return", openParts: [1] } } });
    await app.waitFor("id:fp-part-0-toggle", { timeout: 30_000 });
    await reach(app, "id:fp-item-1:6");
    await shoot(app, `g1-2551q-12-closed-${look.name}`);
    // The choice's own testID sits on a non-pressable leaf here; the label text finds the pressable.
    await app.press("id:fp-item-1:6 >> text:/^Yes$/");
    await app.waitFor("id:fp-frm2551Qv2018:txtTaxReliefSpecify", { timeout: 20_000 });
    await app.wait(300);
    await shoot(app, `g1-2551q-12-open-${look.name}`);
    await app.press("id:fp-item-1:6 >> text:/^No$/");
    await app.waitFor("id:fp-frm2551Qv2018:txtTaxReliefSpecify", { state: "gone", timeout: 20_000 });
    await app.wait(300);
    await shoot(app, `g1-2551q-12-shut-${look.name}`);
  }, 180_000);
}

// The part heights the gates save, measured on the pictures at the two form widths (420 and 640 pt):
// 0619E's part 2 gains item 14, 1700's spouse part goes from empty to eight items.
for (const width of [420, 640]) {
  const size = { width, height: width < 600 ? 2000 : 1400, platform: "macos" as const, colorScheme: "light" as const };

  test(`g1 heights ${width}: 0619E part 2 closed then open`, async () => {
    const app = await open(ANY, { ...size, host: { params: { formCode: "0619E", year: 2025, period: 3, mode: "return", openParts: [0, 2] } } });
    await app.waitFor("id:fp-part-0-toggle", { timeout: 30_000 });
    await jump(app, 2, false);
    await app.wait(300);
    await shoot(app, `g1h-0619e-14-closed-${width}`);
    await jump(app, 0, false);
    await reach(app, "id:fp-frm0619E:optWithheld:Y");
    await app.press("id:fp-frm0619E:optWithheld:Y");
    await jump(app, 2, false);
    await reach(app, "id:fp-item-2:0");
    await app.wait(300);
    await shoot(app, `g1h-0619e-14-open-${width}`);
  }, 120_000);

  test(`g1 heights ${width}: 1700 spouse part empty then full`, async () => {
    const app = await open(ANY, { ...size, host: { params: { formCode: "1700", year: 2025, period: 1, mode: "return" } } });
    await app.waitFor("id:fp-part-0-toggle", { timeout: 30_000 });
    await jump(app, 2, false);
    await app.wait(300);
    await shoot(app, `g1h-1700-part2-closed-${width}`);
    await jump(app, 1, false);
    await reach(app, "id:fp-item-1:12");
    await app.press("id:fp-frm1700:rdoPg1I15CivilStatusM");
    await reach(app, "id:fp-item-1:13");
    await app.press("id:fp-frm1700:rdoPg1I16SpouseHasIncomeY");
    await reach(app, "id:fp-item-1:14");
    await app.press("id:fp-frm1700:rdoPg1I17FilingStatusJ");
    await jump(app, 2, false);
    await reach(app, "id:fp-item-2:0");
    await reach(app, "id:fp-item-2:6");
    await app.press("id:fp-frm1700:rdoPg1I24SpouseForeignTaxCreditsYes");
    await reach(app, "id:fp-item-2:7");
    await app.wait(300);
    await shoot(app, `g1h-1700-part2-full-${width}`);
  }, 180_000);
}
