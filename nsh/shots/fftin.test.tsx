// PR 8 (Q8/P8a) real-window shots of the TIN row: the phone grid (equal columns, the branch code's 1.4, no
// dashes) under 600 pt and the dashed fixed boxes at desktop width, the "From the tax profile" line on a locked
// profile TIN only, typed TINs (1801's executor), a one-box whole TIN (2000's item 9), an
// empty and a filled state, and a refusal sentence (1706's buyer TIN after Check). `just nsh-shots fftin`
// saves ~/BuwizNsh/evidence/fftin/*.png; headless it takes none and still passes.
import { open, test, type App } from "nsh/test";
import { shoot } from "./shoot";

// No warmForms() here: a test file's own imports run in the CLI's runner, which on `--target macos` has none of
// the app's native modules — `bir/testing` (ebirforms/native) fails the bundle there. The form schemas build
// lazily inside each real host on the first form call, well inside the tests' timeouts.

const ENTRY = "../FormAnyShot.tsx";

/** Parts are collapsible cards (only part 0 opens with the form) and the rows are a virtualized list: the pill's
 *  sections list jumps to the part (opens it, folds the rest, scrolls its card up — and stays open when it
 *  already is, so calling this twice is safe), then scroll until the row's cell is drawn. */
async function showRow(app: App, part: number, row: string) {
  const id = `id:fp-item-${row}`;
  await app.press("id:fp-pill-open").catch(() => {});
  await app.waitFor(`id:fp-section-${part}`, { timeout: 15_000 }).catch(() => {});
  await app.press(`id:fp-section-${part}`).catch(() => {});
  await app.wait(400);
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
  { name: "320-dark", width: 320, height: 700, scheme: "dark" as const },
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

// Row-height evidence (the brief's 420 and 640 pt): 0605 Item 9 (row 1:0) settled inside the viewport so its
// top and bottom dividers are both in the picture; the heights are measured on the PNGs (shots/rowheights.py).
// The window is tall so the row and its neighbours stay in view, and the extra scroll is best-effort: at the
// page's end there is nothing left to scroll and the row is already as high as it goes.
for (const width of [420, 640]) {
  test(`fftin 0605 TIN row height ${width}`, async () => {
    const app = await openForm("0605", width, 1400, "light", { year: 2025, period: 1, mode: "return" });
    await showRow(app, 1, "1:0");
    // The row enters at the viewport's foot; one more scroll lifts it clear of the floating pill at the bottom.
    await app.scroll("id:form-scroll", 200).catch(() => {});
    await app.wait(400);
    await shoot(app, `tin-0605-rowh-${width}`);
  }, 120_000);
}

// 1801's typed executor TIN: empty, then filled — the digits-only boxes with no profile line. 320 pt is the
// narrowest phone: its 47 pt three-digit column is where a typed box once cut its last digit.
for (const look of [
  { name: "320-light", width: 320, height: 700, scheme: "light" as const },
  { name: "320-dark", width: 320, height: 700, scheme: "dark" as const },
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
  await app.waitFor("text:/items? need attention/", { timeout: 30_000 }).catch(() => {});
  await app.wait(400);
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
