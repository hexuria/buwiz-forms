// Home at phone size (390 x 844) and at desktop size (1440 x 900, macOS): the dashboard, the five tiles and what each does, the
// strip, the picked list, every row's action, the as-needed list, the empty states and the welcome. Against a scratch database
// with the demo taxpayer (HomeDemo.tsx; a test asks for variations with open's host params).
import { type App, expect, open, test } from "nsh/test";
import { MONTHS_LONG } from "../../lib/dates";

const ENTRY = "../../HomeDemo.tsx";
const WIDE = { width: 1440, height: 900, platform: "macos" as const };
const WEEKDAYS = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const SIZES = [
  { name: "phone", size: {} },
  { name: "desktop", size: WIDE },
] as const;
const asking = (params: Record<string, unknown> = {}) => ({ host: { params } });
/** A clock that stands still at the 5th of November of this year: the 1601C of October (due the 10th) is a week away. */
const NOV_5 = () => ({ clock: { time: new Date(new Date().getFullYear(), 10, 5, 9, 0) } });
/** The "when" of a deadline row: a chip or a plain line. Only the wide rows have it. */
const WHEN = /^(In \d+ days?|Due today|Today)$/;

/** The number a tile shows: the text after its label on screen. */
function tileCount(app: App, label: string): number {
  const texts = app.texts();
  return Number(texts[texts.indexOf(label) + 1]);
}

/** Home again: the title bar's Back on desktop; on a phone Filings is a tab, so the Home tab. */
async function backHome(app: App, size: string): Promise<void> {
  await app.press(size === "desktop" ? "id:tb-back" : "id:tab-home");
  await app.waitFor("id:home-dashboard");
}

const dueRows = (app: App) => app.count("id:home-due >> role:button");

for (const { name, size } of SIZES) {
  test(`${name}: the dashboard greets, shows the five tiles, the strip, the open deadlines and each taxpayer`, async () => {
    const app = await open(ENTRY, { ...size, ...asking() });
    await app.waitFor("id:home-dashboard", { timeout: 30000 });
    const now = new Date();
    const h = now.getHours();
    const texts = app.texts();
    expect(texts).toContain(h < 12 ? "Good morning" : h < 18 ? "Good afternoon" : "Good evening");
    expect(texts).toContain(`${WEEKDAYS[now.getDay()]}, ${MONTHS_LONG[now.getMonth()]} ${now.getDate()}, ${now.getFullYear()}.`);
    for (const label of ["Overdue", "Due in 7 days", "Drafts", "Waiting to send", "Filed this month"]) expect(texts).toContain(label);
    for (const caption of ["past the due date", "started, not queued", "queued to send"]) expect(texts).toContain(caption);
    expect(app.count("id:home-tiles >> role:button")).toBe(5);
    for (const heading of ["Next 30 days", "Open deadlines", "Each taxpayer at a glance", "As needed", "Recently filed"]) expect(texts).toContain(heading);
    expect(texts).toContain("Today, " + texts.find((t) => /^Today, [A-Z][a-z]{2} \d+$/.test(t))!.slice(7));
    expect(texts).toContain("Open Calendar");
    expect(texts).toContain("Filed returns show here.");
    expect(texts).toContain("Juana Santos");
    // The TIN under the name is the wide row's; the compact row has the progress lines instead.
    expect(texts.includes("000-123-453")).toBe(name === "desktop");
    expect(app.shows("id:home-welcome")).toBe(false);
    expect(app.count("id:home-strip >> role:button")).toBe(30);
    expect(dueRows(app)).toBeGreaterThan(0);
  }, 60_000);
}

test("phone: the rows are compact, with no when column and no code column", async () => {
  const app = await open(ENTRY, asking());
  await app.waitFor("id:home-dashboard", { timeout: 30000 });
  expect(app.texts().some((t) => WHEN.test(t))).toBe(false);
  expect(app.texts()).not.toContain("000-123-453");
  expect(app.texts().some((t) => /filed this year|Nothing was due yet this year/.test(t))).toBe(true);
  expect(app.texts().some((t) => /^Next: /.test(t) || /^Nothing due in 60 days$/.test(t))).toBe(true);
});

test("desktop at 700 points: the taxpayer and filed rows take their compact forms (so no button is clipped), the deadline rows keep the when column", async () => {
  const app = await open(ENTRY, { width: 700, height: 900, platform: "macos", ...asking({ queued: true }) });
  await app.waitFor("id:home-dashboard", { timeout: 30000 });
  const texts = app.texts();
  expect(texts.includes("000-123-453")).toBe(false); // the wide taxpayer row's TIN line
  expect(texts.some((t) => /^Juana Santos · [A-Z][a-z]{2} \d+, \d{4}$/.test(t))).toBe(true); // the compact filed row names its date under the title
  expect(texts.some((t) => WHEN.test(t))).toBe(true);
  expect(app.shows("id:home-tp-000123453-next")).toBe(true);
});

test("desktop: a deadline row has its when column, and the taxpayer row its bar, progress and next deadline", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking() });
  await app.waitFor("id:home-dashboard", { timeout: 30000 });
  expect(app.texts().some((t) => WHEN.test(t))).toBe(true);
  expect(app.shows("id:home-tp-000123453-bar")).toBe(true);
  expect(app.texts().some((t) => /^\d+ of \d+ filed this year$/.test(t) || t === "Nothing was due yet this year")).toBe(true);
  expect(app.texts().some((t) => /^Next: \w+ · [A-Z][a-z]{2} \d+$/.test(t))).toBe(true);
});

for (const { name, size } of SIZES) {
  test(`${name}: Overdue and Due in 7 days narrow the list, a second press or Show all brings the 30 days back`, async () => {
    const app = await open(ENTRY, { ...size, ...NOV_5(), ...asking() });
    await app.waitFor("id:home-dashboard", { timeout: 30000 });
    expect(app.texts()).toContain("Open deadlines");
    expect(app.shows("id:home-due-show-all")).toBe(false);

    const overdue = tileCount(app, "Overdue");
    expect(overdue).toBeGreaterThan(0);
    await app.press("id:home-tile-overdue");
    await app.waitFor("id:home-due-show-all");
    expect(app.texts().filter((t) => t === "Overdue").length).toBe(2); // the tile and the list's heading
    expect(app.texts()).not.toContain("Open deadlines");
    expect(dueRows(app)).toBe(overdue);

    const week = tileCount(app, "Due in 7 days");
    expect(week).toBeGreaterThan(0);
    await app.press("id:home-tile-week");
    await app.waitFor("text:Due in the next 7 days");
    expect(dueRows(app)).toBe(week);
    await app.press("id:home-tile-week"); // the picked tile again clears the pick
    await app.waitFor("text:Open deadlines");
    expect(app.shows("id:home-due-show-all")).toBe(false);

    await app.press("id:home-tile-overdue");
    await app.waitFor("id:home-due-show-all");
    await app.press("id:home-due-show-all");
    await app.waitFor("text:Open deadlines");
    expect(app.shows("id:home-due-show-all")).toBe(false);
  }, 60_000);

  test(`${name}: a day of the strip picks its deadlines, an empty day says so, the same day again clears it`, async () => {
    const app = await open(ENTRY, { ...size, ...NOV_5(), ...asking() });
    await app.waitFor("id:home-dashboard", { timeout: 30000 });
    const all = dueRows(app);
    expect(all).toBeGreaterThan(0);

    await app.press("label:/: \\d+ deadlines?$/ >> nth=0");
    await app.waitFor("id:home-due-show-all");
    const title = app.texts().find((t) => new RegExp(`^(${WEEKDAYS.join("|")}), (${MONTHS_LONG.join("|")}) \\d+$`).test(t));
    expect(title).toBeDefined();
    expect(dueRows(app)).toBeGreaterThan(0);
    expect(dueRows(app)).toBeLessThanOrEqual(all);
    await app.press("label:/: \\d+ deadlines?$/ >> nth=0");
    await app.waitFor("id:home-due-show-all", { state: "gone" });
    expect(app.texts()).toContain("Open deadlines");

    await app.press("label:/nothing due$/ >> nth=0");
    await app.waitFor("text:Nothing due this day.");
    expect(dueRows(app)).toBe(0);
    await app.press("id:home-due-show-all");
    await app.waitFor("text:Open deadlines");
    expect(dueRows(app)).toBe(all);
  }, 60_000);
}

for (const { name, size } of SIZES) {
  test(`${name}: a deadline row opens its return, and Back finds the pick as it was left`, async () => {
    const app = await open(ENTRY, { ...size, ...NOV_5(), ...asking() });
    await app.waitFor("id:home-dashboard", { timeout: 30000 });
    await app.press("id:home-tile-overdue");
    await app.waitFor("id:home-due-show-all");
    await app.press("id:home-due >> role:button >> nth=0");
    await app.waitFor("id:form-page", { timeout: 30000 });
    expect(app.shows("id:home-dashboard")).toBe(false);
    await app.press(name === "desktop" ? "id:tb-back" : "id:nb-back"); // a return is a pushed page: Back in the header
    await app.waitFor("id:home-dashboard");
    expect(app.shows("id:home-due-show-all")).toBe(true); // the old app kept the pick while it ran
    expect(app.texts().filter((t) => t === "Overdue").length).toBe(2);
  }, 90_000);

  test(`${name}: Drafts, Waiting to send and Filed this month open Filings with their preset`, async () => {
    const app = await open(ENTRY, { ...size, ...asking({ drafts: 2, queued: true, filed: 1, readout: true }) });
    await app.waitFor("id:home-dashboard", { timeout: 60000 });
    expect(tileCount(app, "Drafts")).toBe(2);
    expect(tileCount(app, "Waiting to send")).toBe(1);
    expect(tileCount(app, "Filed this month")).toBe(1);

    await app.press("id:home-tile-drafts");
    await app.waitFor("id:filings");
    expect(app.texts()).toContain('filings {"statuses":["Draft"]}');
    await backHome(app, name);

    await app.press("id:home-tile-waiting");
    await app.waitFor("id:filings");
    expect(app.texts()).toContain('filings {"statuses":["Queued"]}');
    await backHome(app, name);

    const now = new Date();
    const month = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}`;
    await app.press("id:home-tile-filed");
    await app.waitFor("id:filings");
    expect(app.texts()).toContain(`filings {"statuses":["Submitted","Confirmed","Paid"],"updatedMonth":"${month}","filedOnly":true}`);
    expect(app.shows("id:filings-month-chip")).toBe(true);
    expect(app.shows("id:filings-filed-only-chip")).toBe(true);
  }, 120_000);

  test(`${name}: a dry run leaves its deadline open with a Dry run chip and is not filed; a real send closes it and is filed`, async () => {
    const year = new Date().getFullYear();
    const january = `id:due-00012345300000-1601C-${year}-1`;
    // Two saved 1601C returns (January, February: past due) whose deadlines read as sent, and a filed return of last year.
    const dry = await open(ENTRY, { ...size, ...asking({ drafts: 2, dueSent: "dry", filed: 1, sent: "dry" }) });
    await dry.waitFor("id:home-dashboard", { timeout: 60000 });
    await dry.waitUntil(() => dry.texts().includes("Recently filed"), { timeout: 20000 });
    expect(tileCount(dry, "Filed this month")).toBe(0);
    const dryOverdue = tileCount(dry, "Overdue");
    const dryProgress = dry.texts().find((t) => /^\d+ of \d+ filed this year$/.test(t));
    // The Overdue tile's list holds the dry runs: still due, each with its "Dry run" chip.
    await dry.press("id:home-tile-overdue");
    await dry.waitFor(january, { timeout: 20000 });
    expect(dry.shows(`${january} >> text:Dry run`)).toBe(true);
    const real = await open(ENTRY, { ...size, ...asking({ drafts: 2, dueSent: "real", filed: 1, sent: "real" }) });
    await real.waitFor("id:home-dashboard", { timeout: 60000 });
    await real.waitUntil(() => tileCount(real, "Filed this month") === 1, { timeout: 20000 });
    // Three saved returns' deadlines read as sent: January and February, and the filed December of last year (due in January).
    // As dry runs they are still overdue and not filed; sent for real, they are filed and no longer overdue.
    expect(dryOverdue).toBe(tileCount(real, "Overdue") + 3);
    const realProgress = real.texts().find((t) => /^\d+ of \d+ filed this year$/.test(t));
    expect(Number(realProgress?.split(" ")[0])).toBe(Number(dryProgress?.split(" ")[0]) + 3);
    await real.press("id:home-tile-overdue");
    await real.waitFor("text:Overdue");
    await real.waitUntil(() => real.count("text:Overdue") === 2);
    expect(real.shows(january)).toBe(false);
    // The tile's number and the list it opens agree: the dry run is in neither.
    await dry.press("id:home-tile-filed");
    await dry.waitFor("id:filings-filed-only-chip");
    await dry.waitFor("text:No returns match these filters.");
    await real.press("id:home-tile-filed");
    await real.waitFor("id:filings-filed-only-chip");
    await real.waitFor("id:filings-count >> text:1 return");
  }, 120_000);
}

test("desktop: Drafts is read once per visit: a draft saved behind the store's back shows when Home is visited again", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking({ lateDraft: true }) });
  await app.waitFor("id:home-dashboard", { timeout: 30000 });
  expect(tileCount(app, "Drafts")).toBe(0);
  await app.press("id:demo-save-draft");
  await app.wait(300);
  expect(tileCount(app, "Drafts")).toBe(0); // the cached read stands until the next visit (or a write through the store)
  await app.press("id:nav-calendar");
  await app.press("id:nav-home");
  await app.waitFor("id:home-dashboard");
  expect(tileCount(app, "Drafts")).toBe(1);
});

test("desktop: Cancel queue asks the host; a refusal is said as it is and the list is read again", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking({ queued: true }) });
  await app.waitFor("id:home-dashboard", { timeout: 30000 });
  expect(app.texts()).toContain("Queued");
  expect(app.shows("id:return-9101-cancel")).toBe(true);
  expect(app.shows("id:return-9101-open")).toBe(true);
  await app.press("id:return-9101-cancel");
  // The generated queued row is no queue the host holds: it refuses, and every cached read is dropped (the row is gone).
  await app.waitFor("id:return-9101-cancel", { state: "gone" });
  expect(app.texts()).not.toContain("Queue cancelled. The return is an editable draft again.");
  expect(app.texts()).toContain("Filed returns show here.");
  expect(app.texts()).toContain("No queued 2551Q draft for that period"); // the host's own sentence, as the toast says it
});

test("phone: a queued row keeps Cancel queue and Open under its name", async () => {
  const app = await open(ENTRY, asking({ queued: true }));
  await app.waitFor("id:return-9101-cancel", { timeout: 30000 });
  expect(app.shows("id:return-9101-open")).toBe(true);
  expect(app.texts()).toContain("2551Q · Q" + "1 " + new Date().getFullYear());
});

for (const { name, size } of SIZES) {
  test(`${name}: Use as template saves a filed return as its form's template, then the row shows its status`, async () => {
    const app = await open(ENTRY, { ...size, ...asking({ filed: 1 }) });
    await app.waitFor("id:home-dashboard", { timeout: 60000 });
    const year = new Date().getFullYear() - 1;
    expect(app.texts()).toContain(`1601C · December ${year}`);
    expect(app.shows("label:Use as template")).toBe(true);
    await app.press("label:Use as template");
    await app.waitFor(`text:1601C template saved from December ${year}.`);
    await app.waitFor("label:Use as template", { state: "gone" });
    expect(app.texts()).toContain("Filed");
  }, 120_000);
}

for (const { name, size } of SIZES) {
  test(`${name}: a dry-run send's rows say Dry run; a real send's say Filed`, async () => {
    const shown = (texts: string[], word: string) => texts.filter((t) => t === word).length;
    // A filed return with its template (so its row shows its chip) and an undated as-needed return, both sent.
    const dry = await open(ENTRY, { ...size, ...asking({ filed: 1, template: true, undated: true, sent: "dry" }) });
    await dry.waitFor("id:home-dashboard", { timeout: 60000 });
    await dry.waitUntil(() => shown(dry.texts(), "Dry run") === 2, { timeout: 20000 });
    expect(shown(dry.texts(), "Filed")).toBe(0);
    const real = await open(ENTRY, { ...size, ...asking({ filed: 1, template: true, undated: true, sent: "real" }) });
    await real.waitFor("id:home-dashboard", { timeout: 60000 });
    await real.waitUntil(() => shown(real.texts(), "Filed") === 2, { timeout: 20000 });
    expect(shown(real.texts(), "Dry run")).toBe(0);
  }, 120_000);

  test(`${name}: each taxpayer's button opens the next deadline's return`, async () => {
    const app = await open(ENTRY, { ...size, ...asking({ second: true, branch: true }) });
    await app.waitFor("id:home-dashboard", { timeout: 30000 });
    expect(app.shows("id:home-tp-000123453")).toBe(true);
    expect(app.shows("id:home-tp-000111229")).toBe(true);
    expect(app.texts()).toContain("Sample Trading Corp");
    expect(app.texts().some((t) => t === "Juana Santos · Head office")).toBe(true); // a taxpayer with two branches names the branch
    expect(app.texts()).toContain("Start next");
    await app.press("id:home-tp-000111229-next");
    await app.waitFor("id:form-page", { timeout: 30000 });
    expect(app.texts().some((t) => t === "Sample Trading Corp" || t === "SC")).toBe(true); // the context pill names whose return it is (an avatar when narrow)
  }, 90_000);

  test(`${name}: an as-needed form starts a return, and a saved one with no trigger date opens`, async () => {
    const app = await open(ENTRY, { ...size, ...asking({ undated: true }) });
    await app.waitFor("id:home-dashboard", { timeout: 30000 });
    expect(app.texts()).toContain("As needed");
    expect(app.texts()).toContain("2200M · Excise Tax Return for Mineral Products");
    expect(app.texts()).toContain("No removal date yet");
    expect(app.texts().some((t) => /^Saved [A-Z][a-z]{2} \d+, \d{4}$/.test(t))).toBe(true);
    await app.press("id:as-needed-00012345300000-2200M-start");
    await app.waitFor("id:new-return");
    expect(app.texts()).toContain("2200M");
    await app.press("id:new-return-cancel");
    await app.waitFor("id:new-return", { state: "gone" });
    await app.press("id:as-needed-00012345300000-2200M-1-open");
    await app.waitFor("id:form-page", { timeout: 30000 });
  }, 90_000);
}

test("desktop: Open Calendar goes to the Calendar", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking({ readout: true }) });
  await app.waitFor("id:home-dashboard", { timeout: 30000 });
  await app.press("id:home-open-calendar");
  await app.waitFor("text:calendar {}");
});

test("desktop: with no tax year there is nothing to list, and the lines say so", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking({ years: false }) });
  await app.waitFor("id:home-dashboard", { timeout: 30000 });
  const texts = app.texts();
  expect(texts).toContain("Deadlines show here once a taxpayer has forms for a tax year.");
  expect(texts).toContain("Filed returns show here.");
  expect(texts).toContain("Nothing was due yet this year");
  expect(texts).toContain("Nothing due in 60 days");
  expect(texts).not.toContain("As needed");
  expect(app.shows("id:home-tp-000123453-next")).toBe(false);
  expect(dueRows(app)).toBe(0);
  expect(tileCount(app, "Overdue")).toBe(0);
});

test("desktop: an archived taxpayer is left out of Home", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking({ archived: true }) });
  await app.waitFor("id:home-dashboard", { timeout: 30000 });
  expect(app.texts()).toContain("Archived taxpayers are left out of Home.");
  expect(app.shows("id:home-tp-000123453")).toBe(false);
});

for (const { name, size } of SIZES) {
  test(`${name}: a fresh install shows the welcome with three steps, and the button adds the first taxpayer`, async () => {
    const app = await open(ENTRY, { ...size, ...asking({ demo: false, readout: true }) });
    await app.waitFor("id:home-welcome", { timeout: 30000 });
    const texts = app.texts();
    expect(texts).toContain("Welcome to eBIRForms");
    expect(texts).toContain(`Prepare, print and track BIR returns for every taxpayer you handle. Everything stays on this ${name === "phone" ? "device" : "computer"}.`);
    for (const title of ["Add a taxpayer", "Add a tax year", "Choose its forms"]) expect(texts).toContain(title);
    for (const n of ["1", "2", "3"]) expect(app.shows(`id:welcome-step-${n}`)).toBe(true);
    expect(app.shows("id:home-dashboard")).toBe(false);
    await app.press("id:welcome-add-taxpayer");
    await app.waitFor("text:new_taxpayer {}");
  });
}
