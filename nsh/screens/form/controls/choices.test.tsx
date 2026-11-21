// Form fields, PR 5: a one-answer group draws by how many choices it has — two to four a segmented bar (full-width
// segments in a column when the labels cannot hold one line in the room it has), five to twelve a dropdown, more a
// search list — and it clears back to unanswered only where the session unticks (an optional group of three or more).
// The page is the controls gallery (entry nsh/ControlsShot.tsx); demo data only.
import { expect, open, test } from "nsh/test";
import type { App } from "nsh/test";

const ENTRY = "../../../ControlsShot.tsx";

/** The gallery, ready. */
async function gallery(options: Parameters<typeof open>[1] = {}): Promise<App> {
  const app = await open(ENTRY, options);
  await app.waitFor("text:Field controls");
  return app;
}

/** What a radio group reported, as its "edit <name>: k=v ..." line ("" before any pick). */
function picked(app: App, name: string): string {
  const line = app.texts().find((t) => t.startsWith(`edit ${name}:`));
  return line === undefined ? "" : line.slice(`edit ${name}:`.length).trim();
}

test("phone: two to four choices are a segmented bar of radios labelled by the choices", async () => {
  const app = await gallery();
  // regime: three choices — one radiogroup, three radio segments, the choices' own words (not Yes/No).
  expect(app.count("id:fp-radio-regime >> role:radio")).toBe(3);
  expect(app.shows("text:Graduated income tax rate")).toBe(true);
  expect(app.shows("text:8% flat rate")).toBe(true);
  await app.press("id:fp-regime2");
  expect(picked(app, "regime")).toContain("regime2=true");
  // Re-picking another segment moves the answer.
  await app.press("id:fp-regime3");
  expect(picked(app, "regime")).toContain("regime3=true");
});

test("phone: an optional group of three or more clears; a pair has no Clear", async () => {
  const app = await gallery();
  await app.press("id:fp-regime2");
  expect(picked(app, "regime")).toContain("regime2=true");
  await app.press("id:fp-radio-regime-clear");
  expect(picked(app, "regime")).toContain("regime2=false");
  // A pair keeps one answer: no Clear under it.
  expect(app.shows("id:fp-radio-amended-clear")).toBe(false);
  await app.press("id:fp-amended1");
  expect(picked(app, "amended")).toContain("amended1=true");
});

test("phone: five to twelve choices are a dropdown, None clears the optional group", async () => {
  const app = await gallery();
  // No segments are mounted for a dropdown group.
  expect(app.shows("id:fp-payment1")).toBe(false);
  await app.press("id:fp-radio-payment-trigger");
  await app.press("text:Self-Assessment");
  expect(picked(app, "payment")).toContain("payment1=true");
  // The optional group's first entry is None: it clears back to unanswered.
  await app.press("id:fp-radio-payment-trigger");
  await app.press("text:None");
  expect(picked(app, "payment")).toContain("payment1=false");
});

test("phone: more than twelve choices are a search list", async () => {
  const app = await gallery();
  expect(app.shows("id:fp-radio-codes-trigger")).toBe(true);
  await app.press("id:fp-radio-codes-trigger");
  await app.waitFor("id:fp-radio-codes-search");
  // Every typed word matches, in any order.
  await app.type("id:fp-radio-codes-search", "label 7 code");
  await app.waitFor("id:fp-radio-codes-opt-codes7");
  await app.press("id:fp-radio-codes-opt-codes7");
  expect(picked(app, "codes")).toContain("codes7=true");
});

test("phone: a locked group takes nothing and says why", async () => {
  const app = await gallery();
  expect(app.enabled("id:fp-taxType1")).toBe(false);
  expect(app.enabled("id:fp-taxType2")).toBe(false);
  expect(app.texts()).toContain("Set by the taxpayer type");
});

test("phone: long labels stack the bar full-width, and a pick still lands", async () => {
  const app = await gallery();
  // longrate's labels cannot hold one line at phone width: the stacked marker shows.
  expect(app.shows("id:fp-radio-longrate-stacked")).toBe(true);
  await app.press("id:fp-longrate2");
  expect(picked(app, "longrate")).toContain("longrate2=true");
});

test("desktop: short labels hold one line, long ones still stack", async () => {
  const app = await gallery({ width: 1440, height: 900, platform: "macos" });
  // regime's labels fit the wide room on one line; longrate's do not.
  expect(app.shows("id:fp-radio-regime-stacked")).toBe(false);
  expect(app.shows("id:fp-radio-longrate-stacked")).toBe(true);
});

test("phone: two one-answer groups in one item pick on their own", async () => {
  const app = await gallery();
  expect(app.shows("id:fp-radio-taxpayer")).toBe(true);
  expect(app.shows("id:fp-radio-spouse")).toBe(true);
  const line = () => app.texts().find((t) => t.startsWith("edit taxpayer:")) ?? "";
  await app.press("id:fp-taxpayer1");
  expect(line()).toContain("taxpayer1=true");
  expect(line()).not.toContain("spouse1=true");
  expect(line()).not.toContain("spouse2=true");
  await app.press("id:fp-spouse2");
  expect(line()).toContain("spouse2=true");
  expect(line()).toContain("taxpayer1=true");
});
