// The field controls at phone size (390 x 844, iOS: touch sizes) and at a desktop (1440 x 900): each kind shows its value, reports
// an edit in the format the old page sent, reports focus and blur, and respects locked, computed and disabled. The page they run on
// is the gallery (ControlsGallery.tsx, entry nsh/ControlsShot.tsx): under each box it prints what the box reported.
import { expect, open, test } from "nsh/test";
import type { App } from "nsh/test";

const ENTRY = "../../../ControlsShot.tsx";
const SIZES = [
  { name: "phone", options: {} },
  { name: "desktop", options: { width: 1440, height: 900, platform: "macos" as const } },
] as const;

/** The gallery, ready. */
async function gallery(options: (typeof SIZES)[number]["options"]): Promise<App> {
  const app = await open(ENTRY, options);
  await app.waitFor("text:Field controls");
  return app;
}

/** A box's readout line: "money edit: 1250000 | blur: 1 | focus: 1". */
function readoutOf(app: App, key: string): { edit: string; blur: number; focus: number } | null {
  for (const line of app.texts()) {
    if (!line.startsWith(`${key} edit:`)) continue;
    const hit = /^\S+ edit: (.*) \| blur: (\d+) \| focus: (\d+)$/.exec(line);
    if (hit) return { edit: hit[1].trim(), blur: Number(hit[2]), focus: Number(hit[3]) };
  }
  return null;
}

/** What a box reported last ("1250000"; "" before any), or null when the page has no such box. */
function edit(app: App, key: string): string | null {
  const direct = app.texts().find((t) => t.startsWith(`edit ${key}:`));
  if (direct !== undefined) return direct.slice(`edit ${key}:`.length).trim();
  return readoutOf(app, key)?.edit ?? null;
}

/** How many focuses a box reported. */
function focuses(app: App, key: string): number {
  return readoutOf(app, key)?.focus ?? -1;
}

/** How many blurs a box reported. */
function blurs(app: App, key: string): number {
  return readoutOf(app, key)?.blur ?? -1;
}

for (const size of SIZES) {
  test(`${size.name}: text, email, phone and whole-number boxes show, report and honor their length`, async () => {
    const app = await gallery(size.options);
    expect(app.value("id:fp-name")).toBe("Juana Santos");
    expect(app.value("id:fp-email")).toBe("juana@example.test");
    await app.type("id:fp-name", " Jr");
    expect(app.value("id:fp-name")).toBe("Juana Santos Jr");
    expect(edit(app, "name")).toBe("Juana Santos Jr");
    await app.type("id:fp-phone", "0917 555 0100");
    expect(edit(app, "phone")).toBe("0917 555 0100");
    // A whole number takes digits only, at most m of them.
    expect(app.value("id:fp-zip")).toBe("1200");
    await app.clear("id:fp-zip");
    await app.type("id:fp-zip", "12a3456");
    expect(app.value("id:fp-zip")).toBe("1234");
    expect(edit(app, "zip")).toBe("1234");
  });

  test(`${size.name}: a box reports focus once and blur when the keys go elsewhere`, async () => {
    const app = await gallery(size.options);
    expect(blurs(app, "name")).toBe(0);
    expect(focuses(app, "name")).toBe(0);
    await app.type("id:fp-name", "!");
    expect(focuses(app, "name")).toBe(1);
    expect(blurs(app, "name")).toBe(0);
    await app.type("id:fp-email", "x");
    expect(blurs(app, "name")).toBe(1);
    expect(edit(app, "email")).toBe("juana@example.test" + "x");
  });

  test(`${size.name}: money stays plain while typing, reports the plain number, groups with two decimals on leaving`, async () => {
    const app = await gallery(size.options);
    expect(app.value("id:fp-money")).toBe("1,250,000.50");
    await app.clear("id:fp-money");
    await app.type("id:fp-money", "1250000");
    expect(app.value("id:fp-money")).toBe("1250000");
    expect(edit(app, "money")).toBe("1250000");
    await app.type("id:fp-money", ".5");
    expect(app.value("id:fp-money")).toBe("1250000.5");
    expect(edit(app, "money")).toBe("1250000.5");
    await app.type("id:fp-name", " ");
    expect(blurs(app, "money")).toBe(1);
    expect(app.value("id:fp-money")).toBe("1,250,000.50");
    // Whole pesos show no centavos.
    expect(app.value("id:fp-whole")).toBe("48,000");
    // A pasted amount with the peso sign and separators is the plain number.
    await app.clear("id:fp-money");
    await app.paste("id:fp-money", { text: "₱ 2,500.75" });
    expect(edit(app, "money")).toBe("2500.75");
  });

  test(`${size.name}: a percent takes digits and one point and reports as typed`, async () => {
    const app = await gallery(size.options);
    expect(app.value("id:fp-pct")).toBe("8");
    await app.clear("id:fp-pct");
    await app.type("id:fp-pct", "12.5.0a");
    expect(app.value("id:fp-pct")).toBe("12.50");
    expect(edit(app, "pct")).toBe("12.50");
  });

  test(`${size.name}: computed, locked and blank-locked boxes read only and report nothing`, async () => {
    const app = await gallery(size.options);
    const texts = app.texts();
    // Computed: the figure, and 0.00 for a blank one.
    expect(texts).toContain("12,500.00");
    expect(texts).toContain("0.00");
    // Locked from the tax profile: the figure and (as a reason) its tooltip; no input.
    expect(texts).toContain("3,000.00");
    // Locked and blank: what it waits for, in its place.
    expect(texts).toContain("Needs the ATC");
    expect(texts).toContain("Needs the item above");
    expect(texts).toContain("Set by the period");
    for (const key of ["sum", "zero", "lockedAmt", "waits", "atcWait", "ownTin", "fixedDate"]) {
      expect(app.count(`id:fp-${key}`)).toBe(1);
      if (key !== "ownTin") expect(edit(app, key)).toBe("");
    }
    // The locked boxes draw no text input: a type into one is refused.
    let refused = false;
    try {
      await app.type("id:fp-sum", "9");
    } catch {
      refused = true;
    }
    expect(refused).toBe(true);
    expect(edit(app, "sum")).toBe("");
  });

  test(`${size.name}: a disabled box and a box turned off read only and report nothing`, async () => {
    const app = await gallery(size.options);
    expect(app.texts()).toContain("Not now");
    expect(edit(app, "disabled")).toBe("");
    let refused = false;
    try {
      await app.type("id:fp-disabled", "x");
    } catch {
      refused = true;
    }
    expect(refused).toBe(true);
    expect(edit(app, "disabled")).toBe("");
    // A box another answer turns off is drawn (faded) and still shows.
    expect(app.count("id:fp-off")).toBe(1);
  });

  test(`${size.name}: V5: zero TIN parts and branch codes keep their text on focus`, async () => {
    if (size.name !== "phone") return; // the grid is the TIN-part layout under 600 pt
    const app = await gallery(size.options);
    for (const [id, text] of [["fp-gtin1", "000"], ["fp-gbranch", "00000"]] as const) {
      expect(app.value(`id:${id}`)).toBe(text);
      await app.type(`id:${id}`, "");
      await app.wait(150);
      expect(app.value(`id:${id}`)).toBe(text);
    }
  });

  test(`${size.name}: a TIN keeps its dashes while typing and reports its digits`, async () => {
    const app = await gallery(size.options);
    expect(app.value("id:fp-tin")).toBe("000-123-453-00000");
    await app.clear("id:fp-tin");
    await app.type("id:fp-tin", "0001");
    expect(app.value("id:fp-tin")).toBe("000-1");
    await app.type("id:fp-tin", "2345300000999");
    expect(app.value("id:fp-tin")).toBe("000-123-453-00000");
    expect(edit(app, "tin")).toBe("00012345300000");
    // One of the four boxes: digits only, at most m of them, reported as typed.
    await app.clear("id:fp-tin2");
    await app.type("id:fp-tin2", "9x87654");
    expect(app.value("id:fp-tin2")).toBe("987");
    expect(edit(app, "tin2")).toBe("987");
  });

  test(`${size.name}: a TIN grid cell takes digits only, at most its box's max (PR 8)`, async () => {
    if (size.name !== "phone") return; // the grid is the under-600 pt layout; the gallery draws it only there
    const app = await gallery(size.options);
    // The phone grid's cells: same digit rules as the TIN boxes, centred (the look is in the shots).
    expect(app.count("id:fp-gtin1")).toBe(1);
    expect(app.value("id:fp-gtin1")).toBe("000");
    await app.clear("id:fp-gtin1");
    await app.type("id:fp-gtin1", "12a45");
    expect(app.value("id:fp-gtin1")).toBe("124");
    await app.clear("id:fp-gbranch");
    await app.type("id:fp-gbranch", "0000-1x");
    expect(app.value("id:fp-gbranch")).toBe("00001");
    // A locked cell draws read-only (a ReadBox, not an input): it keeps its label and its value as text.
    expect(app.count("id:fp-gtinL")).toBe(1);
    expect(app.value("id:fp-gtinL")).toBe(null);
    expect(app.count("label:/Taxpayer TIN/")).toBeGreaterThan(0);
  });

  test(`${size.name}: a date takes typed text in any old format and reports MM/DD/YYYY`, async () => {
    const app = await gallery(size.options);
    expect(app.value("id:fp-date")).toBe("December 10, 2020");
    await app.clear("id:fp-date");
    await app.type("id:fp-date", "12012020");
    expect(edit(app, "date")).toBe("12/01/2020");
    await app.type("id:fp-name", " ");
    expect(blurs(app, "date")).toBe(1);
    expect(app.value("id:fp-date")).toBe("December 1, 2020");
    await app.clear("id:fp-date");
    await app.type("id:fp-date", "Mar 5 2021");
    expect(edit(app, "date")).toBe("03/05/2021");
    // Text that is no date is reported as typed; the page's own message line says what is wrong (the control prints no sentence).
    await app.clear("id:fp-date");
    await app.type("id:fp-date", "13/45");
    expect(edit(app, "date")).toBe("13/45");
    await app.type("id:fp-name", " ");
    expect(app.texts().some((t) => t.startsWith("Enter a valid date"))).toBe(false);
    // The calendar button is there for the pointer and the finger.
    expect(app.count("id:fp-date-calendar")).toBe(1);
    expect(app.count("id:fp-fixedDate-calendar")).toBe(0);
  });

  test(`${size.name}: a short choice is a dropdown that reports the code once`, async () => {
    const app = await gallery(size.options);
    expect(app.texts()).toContain("03 - March");
    await app.press("id:fp-month-trigger");
    await app.waitFor("text:04 - April");
    await app.press("text:04 - April");
    expect(edit(app, "month")).toBe("04");
    expect(blurs(app, "month")).toBe(1);
    await app.waitFor("text:04 - April");
  });

  test(`${size.name}: a long choice is searched by every typed word, in any order`, async () => {
    const app = await gallery(size.options);
    await app.press("id:fp-atc-trigger");
    await app.waitFor("id:fp-atc-search");
    await app.waitFor("id:fp-atc-opt-WI010");
    // Words in any order and a code both match; nothing else does.
    await app.type("id:fp-atc-search", "juridical fees");
    await app.waitFor("id:fp-atc-opt-WC011");
    expect(app.count("id:fp-atc-opt-WC010")).toBe(1);
    expect(app.count("id:fp-atc-opt-WI010")).toBe(0);
    await app.clear("id:fp-atc-search");
    await app.type("id:fp-atc-search", "wx150");
    await app.waitFor("id:fp-atc-opt-WX150");
    expect(app.count("id:fp-atc-opt-WC010")).toBe(0);
    await app.press("id:fp-atc-opt-WX150");
    expect(edit(app, "atc")).toBe("WX150");
    expect(blurs(app, "atc")).toBe(1);
    // The phone's trigger says the choice; the wide combobox's box holds it as its text.
    if (size.name === "phone") await app.waitFor("text:WX150 — Sample withholding category 51");
    else await app.waitUntil(() => app.value("id:fp-atc-search") === "WX150 — Sample withholding category 51");
  });

  test(`${size.name}: a tick box reports true then false, a locked one does not`, async () => {
    const app = await gallery(size.options);
    await app.press("id:fp-chk");
    expect(edit(app, "chk")).toBe("true");
    await app.press("id:fp-chk");
    expect(edit(app, "chk")).toBe("false");
    expect(blurs(app, "chk")).toBe(2);
    expect(app.enabled("id:fp-lockedChk")).toBe(false);
    expect(edit(app, "lockedChk")).toBe("");
  });

  test(`${size.name}: a radio group picks, a group of three clears, a locked group takes nothing`, async () => {
    const app = await gallery(size.options);
    // A group of three is a segmented bar: a segment picks, Clear returns it to unanswered.
    await app.press("id:fp-regime2");
    expect(edit(app, "regime")).toBe("regime2=true");
    await app.press("id:fp-radio-regime-clear");
    expect(edit(app, "regime")).toBe("regime2=false");
    await app.press("id:fp-regime1");
    expect(edit(app, "regime")).toBe("regime2=false regime1=true");
    // A pair keeps its answer: no Clear, and a checked segment takes no second press.
    await app.press("id:fp-amended1");
    expect(edit(app, "amended")).toBe("amended1=true");
    expect(app.shows("id:fp-radio-amended-clear")).toBe(false);
    // A locked group: disabled segments and the reason it is shut.
    expect(app.enabled("id:fp-taxType1")).toBe(false);
    expect(app.texts()).toContain("Set by the taxpayer type");
  });
}

for (const size of SIZES) {
  test(`${size.name}: a schedule lists its rows as lines; a row opens in the drawer, a cell edits there`, async () => {
    const app = await gallery(size.options);
    expect(app.count("id:fp-table-sch1pay")).toBe(1);
    // Two rows hold something: two lines, no grid; the first empty row waits behind "Add row".
    expect(app.count("id:fp-row-sch1pay-0")).toBe(1);
    expect(app.count("id:fp-row-sch1pay-1")).toBe(1);
    expect(app.count("id:fp-row-sch1pay-2")).toBe(0);
    expect(app.texts()).toContain("Santos Rentals · WI100 · 12,000.00 · 600.00");
    expect(app.count("id:fp-table-sch1pay-add")).toBe(1);
    await app.press("id:fp-row-sch1pay-0");
    await app.waitFor("text:Row 1");
    // The row's boxes stacked, each with its words: a typing box, a choice, an amount, a computed amount (read only), a tick box.
    expect(app.value("id:fp-sch1pay")).toBe("Santos Rentals");
    expect(app.value("id:fp-sch1amt")).toBe("12,000.00");
    expect(app.count("id:fp-sch1tax")).toBe(1);
    expect(app.count("id:fp-sch1res")).toBe(1);
    for (const words of ["Payee", "ATC", "Amount", "Withheld"]) expect(app.texts()).toContain(words);
    await app.clear("id:fp-sch1pay");
    await app.type("id:fp-sch1pay", "Santos Realty");
    await app.clear("id:fp-sch1amt");
    await app.type("id:fp-sch1amt", "5");
    expect(app.texts()).toContain("Santos Realty · WI100 · 5 · 600.00");
    expect(edit(app, "sch1pay")).toBeNull();
    // A tick box in the drawer.
    await app.press("id:fp-sch1res");
    expect(app.texts().some((t) => t.includes("Resident"))).toBe(true);
    // A choice in the drawer.
    await app.press("id:fp-sch1atc-trigger");
    await app.waitFor("text:WC010 - Professional fees, juridical");
    await app.press("text:WC010 - Professional fees, juridical");
    expect(app.texts().some((t) => t.includes("WC010"))).toBe(true);
    await app.press("id:fp-table-sch1pay-done");
    await app.waitFor("text:Row 1", { state: "gone" });
  });

  test(`${size.name}: Delete row removes the row and closes the drawer; Add row opens the first empty row`, async () => {
    const app = await gallery(size.options);
    await app.press("id:fp-row-sch1pay-0");
    await app.waitFor("text:Row 1");
    await app.press("id:fp-table-sch1pay-delete");
    await app.waitFor("text:Row 1", { state: "gone" });
    // Row 1 is gone, not cleared: its line shows nowhere, and the later rows moved up —
    // row 1's place now holds what was row 2.
    expect(app.texts().some((t) => t.includes("Santos Rentals"))).toBe(false);
    expect(app.count("id:fp-row-sch1pay-0")).toBe(1);
    expect(app.texts()).toContain("Cruz Law · WI010 · 30,000.00");
    // The first empty row is row 2 now: Add row opens it.
    await app.press("id:fp-table-sch1pay-add");
    await app.waitFor("text:Row 2");
    await app.type("id:fp-sch2pay", "New payee");
    await app.press("id:fp-table-sch1pay-done");
    await app.waitFor("text:Row 2", { state: "gone" });
    expect(app.count("id:fp-row-sch1pay-1")).toBe(1);
    expect(app.texts()).toContain("New payee");
    // Row 3 is the next empty one.
    await app.press("id:fp-table-sch1pay-add");
    await app.waitFor("text:Row 3");
    await app.back();
    await app.waitFor("text:Row 3", { state: "gone" });
  });
}

test("desktop: a pill step's focus function puts the keys in a long-list combobox's box and in a group's first tick box", async () => {
  const app = await gallery(SIZES[1].options);
  await app.press("id:gallery-focus-atc");
  // The box has the keys: Return opens the whole list, Escape closes it.
  await app.keyboard.press("enter");
  await app.waitFor("id:fp-atc-list");
  await app.keyboard.press("escape");
  await app.waitFor("id:fp-atc-list", { state: "gone" });
  await app.press("id:gallery-focus-regime");
  await app.keyboard.press("space");
  await app.waitFor("text:edit regime: regime1=true");
});
