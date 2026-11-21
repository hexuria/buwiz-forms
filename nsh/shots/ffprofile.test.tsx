// AFTER pictures for PR 4 (profile-backed facts): the ask-once "Save to profile?" Dialog (wide) and
// bottom Sheet (narrow) with both answers, a boxed fact answering in its own box with no card, and the
// F2 inline choice cards. `nsh test --target macos` from the evidence folder writes the PNGs there;
// headless (plain nsh test) it takes none and still passes.
import { expect, open, test } from "nsh/test";
import type { App } from "nsh/test";
import { shoot } from "./shoot";

const LOOKS = [
  { name: "420-light", width: 420, height: 900, platform: "macos" as const, colorScheme: "light" as const },
  { name: "420-dark", width: 420, height: 900, platform: "macos" as const, colorScheme: "dark" as const },
  { name: "1280-light", width: 1280, height: 900, platform: "macos" as const, colorScheme: "light" as const },
  { name: "1280-dark", width: 1280, height: 900, platform: "macos" as const, colorScheme: "dark" as const },
];

const ANY = "../FormAnyShot.tsx";
const BIRTH = "id:fp-frm1701:txtPg1I10BirthDate";
const GOV = "id:fp-frm0619E:optCategory:G";

/** 1701 Item 10/11 birth date: pill → Part I → the "From tax profile" group → scroll to the box. */
async function toBirth(app: App) {
  await app.waitFor("text:BIR Form 1701", { timeout: 30_000 });
  await app.press("id:fp-pill-open");
  await app.waitFor("id:fp-section-1", { timeout: 15_000 });
  await app.press("id:fp-section-1");
  await app.wait(600);
  if (app.shows("id:fp-profile-1")) {
    await app.press("id:fp-profile-1");
    await app.wait(400);
  }
  for (let i = 0; i < 16 && !app.shows(BIRTH); i++) await app.scroll("id:form-scroll", 300).catch(() => {});
  await app.waitFor(BIRTH, { timeout: 15_000 });
}

/** 0619E Item 12 category radios: open the collapsed parts, then scroll to the Government radio. */
async function toCategory(app: App) {
  await app.waitFor("text:BIR Form 0619E", { timeout: 30_000 });
  for (const pi of [1, 2, 3]) {
    if (app.shows(GOV)) break;
    await app.press(`id:fp-part-${pi}-toggle`).catch(() => {});
    await app.wait(400);
  }
  for (let i = 0; i < 18 && !app.shows(GOV); i++) await app.scroll("id:form-scroll", 300).catch(() => {});
  await app.waitFor(GOV, { timeout: 15_000 });
}

/** Scroll the page until `sel` is drawn, opening folded parts on the way. */
async function reach(app: App, sel: string, parts = 6) {
  for (let pi = 0; pi < parts && !app.shows(sel); pi++) {
    await app.press(`id:fp-part-${pi}-toggle`).catch(() => {});
    await app.wait(300);
  }
  for (let i = 0; i < 20 && !app.shows(sel); i++) await app.scroll("id:form-scroll", 320).catch(() => {});
  await app.waitFor(sel, { timeout: 15_000 });
}

test("ask: a boxed fact's new value asks once — Sheet narrow, Dialog wide; Save and Only", async () => {
  for (let li = 0; li < LOOKS.length; li++) {
    const look = LOOKS[li];
    const wide = look.width >= 600;
    const answer = li % 2 === 0 ? "save" : "only";
    const app = await open(ANY, {
      width: look.width,
      height: look.height,
      platform: look.platform,
      colorScheme: look.colorScheme,
      host: { params: wide ? { formCode: "1701", year: 2025, period: 0, mode: "return" } : { formCode: "0619E", year: 2025, period: 7, mode: "return" } },
    });
    if (wide) {
      await toBirth(app);
      await app.clear(BIRTH);
      await app.type(BIRTH, "08171988");
      await app.keyboard.press("Tab");
      await app.waitFor("id:profile-ask", { timeout: 15_000 });
    } else {
      await toCategory(app);
      await app.press(GOV);
      await app.waitFor("id:profile-ask-save", { timeout: 15_000 });
    }
    await app.wait(500);
    await shoot(app, `ask-${look.name}`);
    await app.press(answer === "save" ? "id:profile-ask-save" : "id:profile-ask-only");
    await app.wait(700);
    await shoot(app, `${answer}-${look.name}`);
  }
}, 300_000);

test("boxed: the fact answers in its own radios — 0619E Item 12 has no card", async () => {
  for (const look of LOOKS) {
    const app = await open(ANY, {
      width: look.width,
      height: look.height,
      platform: look.platform,
      colorScheme: look.colorScheme,
      host: { params: { formCode: "0619E", year: 2025, period: 7, mode: "return" } },
    });
    await toCategory(app);
    await app.wait(400);
    await shoot(app, `boxed-0619e-${look.name}`);
  }
}, 300_000);

test("f2-eopt: 1601C's EOPT card on the penalties — missing chip, then saved after a pick", async () => {
  for (const look of LOOKS) {
    const app = await open(ANY, {
      width: look.width,
      height: look.height,
      platform: look.platform,
      colorScheme: look.colorScheme,
      host: { params: { formCode: "1601C", year: 2025, period: 3, mode: "return" } },
    });
    await app.waitFor("id:fp-check", { timeout: 30_000 });
    await reach(app, "id:fp-ask-eopt_tier");
    await app.wait(400);
    await shoot(app, `f2-eopt-missing-${look.name}`);
    // Pick the first tier: a choice card has no Save — the pick saves at once and the chip turns saved.
    const seg = "id:fp-ask-eopt_tier";
    if (app.shows(`${seg}-seg-0`) || app.shows("role:button >> text:Micro")) {
      await app.press("role:button >> text:Micro").catch(() => app.press("role:radio >> text:Micro"));
    } else {
      await app.press(`${seg} >> role:button`).catch(() => {});
    }
    await app.wait(600);
    await shoot(app, `f2-eopt-saved-${look.name}`);
  }
}, 300_000);

test("f2-income: 2551Q Item 13's income-type card with its Segmented choices", async () => {
  for (const look of LOOKS) {
    const app = await open(ANY, {
      width: look.width,
      height: look.height,
      platform: look.platform,
      colorScheme: look.colorScheme,
      host: { params: { formCode: "2551Q", year: 2025, period: 1, mode: "return" } },
    });
    await app.waitFor("id:fp-check", { timeout: 30_000 });
    await reach(app, "id:fp-ask-tax_classification");
    await app.wait(400);
    await shoot(app, `f2-income-${look.name}`);
  }
}, 300_000);

// Row heights (shots/rowheights.py reads the divider distances): 0605 Item 12 and 1601C's penalties row,
// at 420 and 640 points, before (card in the row) and after (boxed / the new card).
const ROW_WIDTHS = [420, 640];
test("rows: 0605 Item 12 and 1601C penalties at 420 and 640", async () => {
  for (const w of ROW_WIDTHS) {
    let app = await open(ANY, {
      width: w,
      height: 900,
      platform: "macos",
      colorScheme: "light",
      host: { params: { formCode: "0605", year: 2025, period: 1, mode: "return" } },
    });
    await app.waitFor("id:fp-check", { timeout: 30_000 });
    await app.press("id:fp-check");
    await app.wait(800);
    await reach(app, "id:fp-frm0605:txtLineBus");
    await app.wait(400);
    await shoot(app, `rows-0605-${w}`);
    app = await open(ANY, {
      width: w,
      height: 900,
      platform: "macos",
      colorScheme: "light",
      host: { params: { formCode: "1601C", year: 2025, period: 3, mode: "return" } },
    });
    await app.waitFor("id:fp-check", { timeout: 30_000 });
    await reach(app, "id:fp-ask-eopt_tier");
    await app.wait(400);
    await shoot(app, `rows-1601c-${w}`);
  }
}, 300_000);
