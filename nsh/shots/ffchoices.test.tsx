// Form-fields PR 5 shots: the one-answer groups as a segmented bar (2–4), a dropdown (5–12) or a search list (more),
// the stacked bar for long labels, an "Others (Specify)" box under its group, and several host groups in one item.
// `just nsh-shots ffchoices` saves ~/BuwizNsh/evidence/ffchoices/*.png; BEFORE pictures come from the PR 4 worktree.
// Headless it takes none and still passes.
import { open, test } from "nsh/test";
import { shoot } from "./shoot";

const LOOKS = [
  { name: "420-light", width: 420, colorScheme: "light" as const },
  { name: "640-light", width: 640, colorScheme: "light" as const },
  { name: "1280-light", width: 1280, colorScheme: "light" as const },
];

const ENTRY = "../FormAnyShot.tsx";
type App = Awaited<ReturnType<typeof open>>;
const options = (look: (typeof LOOKS)[number], formCode: string, year: number, period: number, mode: "return" | "sample" = "return") => ({
  width: look.width, height: 900, platform: "macos" as const, colorScheme: look.colorScheme,
  host: { params: { formCode, year, period, mode } },
});

/** Scroll until `sel` shows (the page virtualizes; a real window's press scrolls, headless stays put). */
async function untilShown(app: App, sel: string, steps = 30) {
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
    for (let step = 0; step < 12 && !app.shows(sel); step++) {
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

for (const look of LOOKS) {
  test(`choices ${look.name}`, async () => {
    // 0605: Item 1 Calendar/Fiscal (a pair), Item 3 Quarter (four), Item 17 Manner of Payment (seven, a dropdown)
    // with its "Others (Specify)" box under the group.
    const c = await open(ENTRY, options(look, "0605", 2024, 1));
    await c.waitFor("id:fp-check", { timeout: 30_000 });
    await untilShown(c, "id:fp-frm0605:itemFiscalStartMonth:_1");
    await c.wait(300);
    await shoot(c, `0605-item1-pair-${look.name}`);
    await untilShown(c, "id:fp-itemQuarter_1");
    await c.wait(300);
    await shoot(c, `0605-item3-quarter-${look.name}`);
    // Item 17 sits in Part I (index 1): jump to its dropdown; G1 keeps the specify box hidden until Others is picked.
    await jumpUntil(c, "id:fp-radio-1:8-trigger", 3);
    await c.wait(300);
    await shoot(c, `0605-item17-dropdown-${look.name}`);
    // Open the dropdown and pick Others (Specify): its box appears under the group (the AFTER look; BEFORE's
    // checkboxes have no trigger, the presses are skipped or land on the old labels harmlessly).
    await c.press("id:fp-radio-1:8-trigger").catch(() => {});
    await c.wait(400);
    await shoot(c, `0605-item17-open-${look.name}`);
    await c.press("text:/Others \\(Specify\\)/i").catch(() => {});
    await c.waitFor("id:fp-frm0605:txtOthersName", { timeout: 20_000 }).catch(() => {});
    await c.wait(300);
    await shoot(c, `0605-item17-others-${look.name}`);

    // 1701Q Item 16 Tax Rate: two long labels — the bar stacks full-width.
    const q = await open(ENTRY, options(look, "1701Q", 2024, 1));
    await q.waitFor("id:fp-check", { timeout: 30_000 });
    await jumpUntil(q, "id:fp-frm1701q:optTaxRate_1", 3);
    await q.wait(300);
    await shoot(q, `1701q-item16-stacked-${look.name}`);

    // 0619E Item 4 "Any Taxes Withheld?" — a required Yes/No pair; Yes reveals the dependent rows.
    const w = await open(ENTRY, options(look, "0619E", 2025, 4));
    await w.waitFor("id:fp-check", { timeout: 30_000 });
    await untilShown(w, "id:fp-frm0619E:optWithheld:Y");
    await w.wait(300);
    await shoot(w, `0619e-yesno-pair-${look.name}`);
    await w.press("id:fp-frm0619E:optWithheld:Y").catch(() => {});
    await w.wait(500);
    await shoot(w, `0619e-yesno-dependents-${look.name}`);

    // 1701MS Item 17 Method of Deduction — two host one-answer groups (taxpayer and spouse) in one item.
    const ms = await open(ENTRY, options(look, "1701MS", 2024, 1));
    await ms.waitFor("id:fp-check", { timeout: 30_000 });
    await jumpUntil(ms, "id:fp-frm1701MS:txtTaxpayerNo17a", 3);
    await ms.wait(300);
    await shoot(ms, `1701ms-item17-two-groups-${look.name}`);
  }, 120_000);
}
