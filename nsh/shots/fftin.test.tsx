// PR 8 (Q8/P8a) real-window shots of the TIN row: the phone grid (equal columns, the branch code's 1.4, no
// dashes) under 600 pt and the dashed fixed boxes at desktop width, the "From the tax profile" line on a locked
// profile TIN only, typed TINs (1801's executor), a one-box whole TIN (2000's item 9), an
// empty and a filled state, and a refusal sentence (1706's buyer TIN after Check). `just nsh-shots fftin`
// saves ~/BuwizNsh/evidence/fftin/*.png; headless it takes none and still passes.
import { open, test, type App } from "nsh/test";
import { warmForms } from "../bir/testing";
import { shoot } from "./shoot";

warmForms();

const ENTRY = "../FormAnyShot.tsx";

/** Parts are collapsible cards (only part 0 opens with the form) and the rows are a virtualized list: open the
 *  part, then scroll until the row's cell is drawn. */
async function showRow(app: App, part: number, row: string) {
  const id = `id:fp-item-${row}`;
  if (part !== 0) await app.press(`id:fp-part-${part}-toggle`).catch(() => {});
  for (let i = 0; i < 40 && !app.shows(id); i++) await app.scroll("id:form-scroll", 600).catch(() => {});
}

async function openForm(code: string, width: number, height: number, scheme: "light" | "dark", params: Record<string, unknown> = {}) {
  return open(ENTRY, {
    width, height, platform: "macos", colorScheme: scheme,
    host: { params: { formCode: code, year: 0, period: 0, mode: "sample", ...params } },
  });
}

// The locked profile TIN (0605 Item 9) at the three widths the brief names, light and dark.
for (const look of [
  { name: "320-light", width: 320, height: 700, scheme: "light" as const },
  { name: "420-light", width: 420, height: 900, scheme: "light" as const },
  { name: "420-dark", width: 420, height: 900, scheme: "dark" as const },
  { name: "1280-light", width: 1280, height: 900, scheme: "light" as const },
  { name: "1280-dark", width: 1280, height: 900, scheme: "dark" as const },
]) {
  test(`fftin 0605 locked TIN ${look.name}`, async () => {
    const app = await openForm("0605", look.width, look.height, look.scheme, { year: 2025, period: 1, mode: "return" });
    await showRow(app, 1, "1:0");
    await app.wait(400);
    await shoot(app, `tin-0605-${look.name}`);
  }, 120_000);
}

// 1801's typed executor TIN: empty, then filled — the digits-only boxes with no profile line.
for (const look of [
  { name: "390-light", width: 390, height: 844, scheme: "light" as const },
  { name: "1280-light", width: 1280, height: 900, scheme: "light" as const },
]) {
  test(`fftin 1801 typed TIN ${look.name}`, async () => {
    const app = await openForm("1801", look.width, look.height, look.scheme);
    await showRow(app, 1, "1:6");
    await app.wait(300);
    await shoot(app, `tin-1801-empty-${look.name}`);
    await app.type("id:fp-frm1801v2018:txtTINE1", "000");
    await app.type("id:fp-frm1801v2018:txtTINE2", "123");
    await app.type("id:fp-frm1801v2018:txtTINE3", "453");
    await app.type("id:fp-frm1801v2018:txtBranchCodeE", "00001");
    await app.wait(400);
    await shoot(app, `tin-1801-filled-${look.name}`);
  }, 120_000);
}

// A refusal sentence: 1706's buyer TIN is required — Check marks the row and its message shows under the grid.
test("fftin 1706 buyer TIN refusal", async () => {
  const app = await openForm("1706", 390, 844, "light", { year: 2025, period: 0, mode: "return" });
  await showRow(app, 1, "1:6");
  await app.press("id:fp-check");
  await app.wait(800);
  await showRow(app, 1, "1:6");
  await shoot(app, "tin-1706-refusal-390-light");
}, 120_000);

// A non-four-box shape: 2000's one-box whole TIN (row 1:9, the 14-digit box), phone and desktop.
for (const [code, row, name, year, period, mode] of [
  ["2000", "1:9", "onebox", 2025, 0, "return"],
] as const) {
  for (const look of [
    { name: "390-light", width: 390, height: 844, scheme: "light" as const },
    { name: "1280-light", width: 1280, height: 900, scheme: "light" as const },
  ]) {
    test(`fftin ${code} ${name} TIN ${look.name}`, async () => {
      const app = await openForm(code, look.width, look.height, look.scheme, { year, period, mode });
      await showRow(app, 1, row);
      await app.wait(400);
      await shoot(app, `tin-${code}-${name}-${look.name}`);
    }, 120_000);
  }
}
