// BEFORE pictures for the form-fields bug fixes (PR 1): Form 0605's typed due date, calendar pick, Check messages and
// Line of Business card; a cleared date part on 0619E; 2200S's folded Line of Business; 1601C item 8; a `date` field.
// `just nsh-shots ffbugs` saves ~/BuwizNsh/evidence/ffbugs/*.png; headless (plain nsh test) it takes none and still passes.
import { expect, open, test } from "nsh/test";
import type { App } from "nsh/test";
import { shoot } from "./shoot";

const LOOKS = [
  { name: "420-light", width: 420, height: 900, platform: "macos" as const, colorScheme: "light" as const },
  { name: "420-dark", width: 420, height: 900, platform: "macos" as const, colorScheme: "dark" as const },
  { name: "1280-light", width: 1280, height: 900, platform: "macos" as const, colorScheme: "light" as const },
  { name: "1280-dark", width: 1280, height: 900, platform: "macos" as const, colorScheme: "dark" as const },
];

const SETTLE = 1500; // the deliberate wait after typing, before the shot

async function eachLook(drive: (app: App, look: (typeof LOOKS)[number]) => Promise<void>): Promise<void> {
  for (const look of LOOKS) {
    const app = await open("../Form0605Shot.tsx", {
      width: look.width,
      height: look.height,
      platform: look.platform,
      colorScheme: look.colorScheme,
    });
    await app.waitFor("id:fp-item-0:3", { timeout: 30_000 });
    await app.wait(800);
    await drive(app, look);
  }
}

async function eachAnyLook(
  params: Record<string, unknown>,
  drive: (app: App, look: (typeof LOOKS)[number]) => Promise<void>,
): Promise<void> {
  for (const look of LOOKS) {
    const app = await open("../FormAnyShot.tsx", {
      width: look.width,
      height: look.height,
      platform: look.platform,
      colorScheme: look.colorScheme,
      host: { params },
    });
    // fp-parts-toggle lives in the desktop side nav only; the Check button
    // is on every layout and means the page is up.
    await app.waitFor("id:fp-check", { timeout: 60_000 });
    await app.waitFor("text:Opening…", { state: "gone", timeout: 60_000 });
    await app.wait(800);
    await drive(app, look);
  }
}

test("open: 0605 top of Part 1, items 1-4", async () => {
  await eachLook(async (app, look) => {
    await shoot(app, `open-${look.name}`);
  });
}, 300_000);

test("chip: the header's fields-count chip", async () => {
  await eachLook(async (app, look) => {
    await shoot(app, `chip-${look.name}`);
  });
}, 300_000);

test("typed-month: 08 typed into Item 4's month box, settled", async () => {
  await eachLook(async (app, look) => {
    const key = "id:fp-frm0605:txtDueDateMonth#date";
    if (look.width < 600) {
      await app.press(key);
      await app.type(`${key}-sheet-input`, "08");
    } else {
      await app.type(key, "08");
    }
    await app.wait(SETTLE);
    await shoot(app, `typed-month-${look.name}`);
  });
}, 300_000);

test("calendar-pick: Item 4's calendar opens, a day is picked, settled", async () => {
  await eachLook(async (app, look) => {
    const key = "id:fp-frm0605:txtDueDateMonth#date";
    await app.press(look.width < 600 ? key : `${key}-calendar`);
    await app.waitFor("text:Su");
    await app.press("role:button >> text:15");
    if (look.width < 600) await app.press(`${key}-sheet-done`);
    await app.wait(SETTLE);
    await shoot(app, `calendar-pick-${look.name}`);
  });
}, 300_000);

test("check-dates: Check pressed, items 4-8 and their messages", async () => {
  await eachLook(async (app, look) => {
    await app.press("id:fp-check");
    // The page checked once Item 4's sentence is on its row (the fp-msg view mounts only with a message now).
    await app.waitFor("text:Item 4 needs a due date.", { timeout: 30_000 });
    await app.wait(800);
    await shoot(app, `check-dates-${look.name}`);
    // The toast gone and the page scrolled: items 4 to 8 with each box's own sentence.
    await app.waitFor("text:/items? need attention/", { state: "gone", timeout: 15_000 });
    await app.scroll("id:form-scroll", look.width < 600 ? 420 : 200);
    await app.wait(600);
    await shoot(app, `check-dates-rows-${look.name}`);
  });
}, 300_000);

test("lob: after Check, Item 12 Line of Business and its card", async () => {
  await eachLook(async (app, look) => {
    await app.press("id:fp-check");
    await app.waitFor("text:Item 4 needs a due date.", { timeout: 30_000 });
    // Item 12 sits in folded Part I, and its unset profile line of business
    // draws the profile card in the row (no pressable box): open the part,
    // the card is the evidence. Wide layouts reach it through the "On this
    // form" navigator (a press on the far toggle can miss while the page
    // settles); narrow through the part's own toggle, then scroll.
    if (app.shows("id:fp-nav-1")) {
      await app.press("id:fp-nav-1");
    } else {
      await app.press("id:fp-part-1-toggle");
      // A real window's press scrolls the part into view; headless the page stays where it was.
      for (let step = 0; step < 12 && !app.shows("text:/not in the tax profile/"); step++) {
        await app.scroll("id:form-scroll", 250);
        await app.wait(250);
      }
    }
    await app.waitFor("text:/not in the tax profile/", { timeout: 15_000 });
    await app.wait(500);
    await shoot(app, `lob-${look.name}`);
  });
}, 300_000);

test("0619e-day: Item 2 due-date day cleared, settled", async () => {
  await eachAnyLook({ formCode: "0619E", year: 2025, period: 6, mode: "return" }, async (app, look) => {
    const key = "id:fp-frm0619E:txtDueMonth#date";
    await app.waitFor(key, { timeout: 30_000 });
    if (look.width < 600) {
      await app.press(key);
      await app.clear(`${key}-sheet-input`);
      await app.type(`${key}-sheet-input`, "07/15/2025");
      await app.press(`${key}-sheet-done`);
      await app.waitFor(`${key}-sheet-input`, { state: "gone" });
    } else {
      await app.clear(key);
      await app.type(key, "07/15/2025");
    }
    await app.wait(600);
    if (look.width < 600) {
      await app.press(key);
      await app.press(`${key}-sheet-clear`);
    } else {
      await app.clear(key);
    }
    await app.wait(SETTLE);
    await shoot(app, `0619e-day-${look.name}`);
  });
}, 300_000);

test("2200s-fold: after Check, the line-of-business row inside folded Part I", async () => {
  await eachAnyLook({ formCode: "2200S", year: 2025, period: 6, mode: "return" }, async (app, look) => {
    await app.press("id:fp-check");
    await app.wait(1200);
    // The schema puts frm2200S:txtLineBus at Part I item 9 ("Main Line of
    // Business"), not in an "Additional details" item: the fold is the part.
    // Its unset profile line of business draws the profile card in the row.
    await app.press("id:fp-part-1-toggle");
    await app.waitFor("text:/not in the tax profile/", { timeout: 15_000 });
    await app.wait(500);
    await shoot(app, `2200s-fold-${look.name}`);
    // The row itself, clear of the toast: the box, its card and the sentence.
    await app.waitFor("text:/items? need attention/", { state: "gone", timeout: 15_000 });
    for (let step = 0; step < 12 && !app.shows("id:fp-frm2200S:txtLineBus"); step++) {
      await app.scroll("id:form-scroll", 250);
      await app.wait(250);
    }
    await app.scroll("id:form-scroll", 200);
    await app.wait(600);
    await shoot(app, `2200s-fold-row-${look.name}`);
  });
}, 300_000);

test("1601c-item8: Part I item 8 withholding agent's name row", async () => {
  await eachAnyLook({ formCode: "1601C", year: 2025, period: 3, mode: "return" }, async (app, look) => {
    // Item 8 is in folded Part I, and the profile-filled items fold into the
    // part's "From tax profile" group: open the part, unfold the group. The
    // name box itself stays profile-locked (its row offers Edit), so the
    // shot is the row as the filer sees it.
    await app.press("id:fp-part-1-toggle");
    await app.waitFor("id:fp-profile-1", { timeout: 15_000 });
    await app.press("id:fp-profile-1");
    await app.waitFor("id:fp-frm1601c:txtTaxpayerName", { timeout: 15_000 });
    await app.wait(500);
    await shoot(app, `1601c-item8-${look.name}`);
  });
}, 300_000);

test("datefield: a kind-date box on 1701, blurred then focused", async () => {
  await eachAnyLook({ formCode: "1701", year: 2025, period: 0, mode: "return" }, async (app, look) => {
    // Item 10's birth date is a whole-date box; the demo profile has none,
    // so the item also carries the "date of birth" fact card whose own date
    // input sits right under the box — the press that blurs it. The field
    // is typed a date first so both shots show one held.
    const box = "id:fp-frm1701:txtPg1I10BirthDate";
    // Q1 P1b: the birth date is a writable box now (no fact card). The pill's sections list jumps to
    // Part I (index 1) and scrolls it into view on both layouts — surer than toggling and scrolling blind.
    await app.press("id:fp-pill-open");
    await app.waitFor("id:fp-section-1", { timeout: 15_000 });
    await app.press("id:fp-section-1");
    await app.wait(800);
    if (app.shows("id:fp-profile-1")) {
      await app.press("id:fp-profile-1");
      await app.wait(400);
    }
    for (let step = 0; step < 14 && !app.shows(box); step++) {
      await app.scroll("id:form-scroll", 250);
      await app.wait(250);
    }
    await app.waitFor(box, { timeout: 15_000 });
    // B14: eight digits rewrite to MM/DD/YYYY with the caret at the end — one more key lands after the year.
    await app.clear(box);
    await app.type(box, "03151990");
    await app.type(box, "1");
    expect(app.value(box)).toBe("03/15/19901");
    await app.clear(box);
    // September 30 is the widest long date a box can hold: the blurred shot
    // must show it unclipped inside the 140 pt width (B14's width check).
    await app.type(box, "09301988");
    await app.wait(600);
    // Tab moves the keyboard off the box: it blurs and shows the held date formatted.
    // Then the box is emptied and typed again — same held date, focused.
    await app.keyboard.press("Tab");
    await app.wait(500);
    await shoot(app, `datefield-blurred-${look.name}`);
    await app.clear(box);
    await app.type(box, "09301988");
    await app.wait(500);
    await shoot(app, `datefield-focused-${look.name}`);
  });
}, 300_000);
